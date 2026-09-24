// SPDX-License-Identifier: GPL-2.0-or-later
/* pps-pdf-animation.cc
 *
 * Animations produced by the LaTeX "animate" package.
 *
 * The package does not embed a video or a GIF. It lays every frame down as a
 * hidden pushbutton form field - named "<n>.<i>" for animation n, frame i -
 * whose icon is a form XObject drawing that frame, and puts one visible
 * pushbutton ("anm<n>") where the animation belongs. A viewer with a
 * JavaScript engine plays it by copying frame icons onto the visible button;
 * Acrobat and Okular do that, which is why the animations move there and sit
 * still everywhere else.
 *
 * Rather than bring up a JavaScript engine, this reads the structure directly
 * and hands the frames back as surfaces for the view to cycle.
 *
 * None of it is reachable through poppler-glib - form field icons are not
 * part of that API - so this uses poppler's core classes and opens its own
 * handle on the file. Nothing else touches that handle, and the PDF backend
 * takes a lock of its own around every call into it.
 */

#include "pps-pdf-animation.h"

#include <algorithm>
#include <cmath>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include <poppler/Annot.h>
#include <poppler/Array.h>
#include <poppler/Catalog.h>
#include <poppler/Dict.h>
#include <poppler/Form.h>
#include <poppler/GlobalParams.h>
#include <poppler/Object.h>
#include <poppler/PDFDoc.h>
#include <poppler/Page.h>
#include <poppler/Stream.h>
#include <goo/GooString.h>

#include <glib.h>

/* The animate package's own default, used when the frame rate cannot be read
 * back out of the document's JavaScript. */
#define DEFAULT_FPS 12.

/* The longest side a frame is decoded at. Past it the view stretches the
 * frame instead, which loses nothing: a frame is a raster image, and at that
 * size it is already being magnified. */
#define MAX_FRAME_SIZE 4096.

struct _PpsPdfAnimations {
	std::unique_ptr<PDFDoc> doc;

	/* Per animation, the frame icons in order. Kept as Objects rather than
	 * re-looked-up per frame: the lookup walks every form field. */
	std::vector<std::vector<Object>> frames;

	/* Parallel to `frames`, and what callers see. */
	std::vector<PpsPdfAnimation> animations;

	/* Per page, indices into `animations`. */
	GHashTable *by_page; /* guint -> GPtrArray of PpsPdfAnimation* */
};

/* Every terminal field in the form, flattened. The animate package puts its
 * frame fields at the top level, but a document is free to nest them. */
static void
collect_fields (FormField *field, std::vector<FormField *> &out)
{
	if (!field)
		return;

	if (field->getNumWidgets () > 0) {
		out.push_back (field);
		return;
	}

	for (int i = 0; i < field->getNumChildren (); i++)
		collect_fields (field->getChildren (i), out);
}

static std::string
field_name (FormField *field)
{
	const GooString *name = field->getFullyQualifiedName ();

	return name ? std::string (name->c_str ()) : std::string ();
}

/* A field name's number. Digits only - no sign, no spaces - and it has to fit
 * an int: names are whatever the file says, and atoi() on a long run of
 * digits is undefined. */
static bool
parse_index (const std::string &digits, int *out)
{
	guint64 value;

	if (!g_ascii_string_to_unsigned (digits.c_str (), 10, 0, G_MAXINT, &value, nullptr))
		return false;

	*out = (int) value;

	return true;
}

/* "anm4" -> 4, or -1. */
static int
animation_index_of (const std::string &name)
{
	int index;

	if (name.compare (0, 3, "anm") != 0 || !parse_index (name.substr (3), &index))
		return -1;

	return index;
}

/* "4.7" -> (4, 7), or false. */
static bool
frame_index_of (const std::string &name, int *animation, int *frame)
{
	size_t dot = name.find ('.');

	if (dot == std::string::npos)
		return false;

	return parse_index (name.substr (0, dot), animation) &&
	       parse_index (name.substr (dot + 1), frame);
}

/* A field's icon: the /MK /I entry of its widget's annotation dictionary,
 * which for these frames is the form XObject that draws it. */
static Object
field_icon (FormField *field)
{
	if (field->getNumWidgets () < 1)
		return Object ();

	FormWidget *widget = field->getWidget (0);
	if (!widget)
		return Object ();

	std::shared_ptr<AnnotWidget> annot = widget->getWidgetAnnotation ();
	if (!annot)
		return Object ();

	Object mk = annot->getAnnotObj ().dictLookup ("MK");
	if (!mk.isDict ())
		return Object ();

	Object icon = mk.dictLookup ("I");
	if (!icon.isStream ())
		return Object ();

	return icon;
}

/* Which page a widget sits on.
 *
 * AnnotWidget::getPageNum() is no use here: it is filled in when a page's
 * annotations are loaded, and nothing has loaded any. Comparing references
 * against each page's /Annots array answers the same question without
 * building a single annotation object.
 *
 * Returns a 1-based page number, or 0.
 */
static int
widget_page (PDFDoc *doc, FormWidget *widget)
{
	Ref ref = widget->getRef ();

	for (int page_num = 1; page_num <= doc->getNumPages (); page_num++) {
		Page *page = doc->getPage (page_num);

		if (!page)
			continue;

		Object annots = page->getAnnotsObject ();
		if (!annots.isArray ())
			continue;

		Array *array = annots.getArray ();

		for (int i = 0; i < array->getLength (); i++) {
			const Object &entry = array->getNF (i);

			if (entry.isRef () && entry.getRef () == ref)
				return page_num;
		}
	}

	return 0;
}

/* The frame rate the document asks for.
 *
 * animate writes "a<n>_fps=<value>" into the JavaScript it runs when the page
 * opens. Reading it back beats guessing: these decks are timed.
 */
static double
animation_fps (PDFDoc *doc, int page_num, int index)
{
	Page *page = doc->getPage (page_num);
	if (!page)
		return DEFAULT_FPS;

	Object annots = page->getAnnotsObject ();
	if (!annots.isArray ())
		return DEFAULT_FPS;

	std::string needle = "a" + std::to_string (index) + "_fps=";

	for (int i = 0; i < annots.arrayGetLength (); i++) {
		Object annot = annots.arrayGet (i, 0);
		if (!annot.isDict ())
			continue;

		Object aa = annot.dictLookup ("AA");
		if (!aa.isDict ())
			continue;

		/* PO and PV are "page open" and "page visible"; animate puts
		 * the definitions in one and the start in the other. */
		for (const char *key : { "PO", "PV" }) {
			Object action = aa.dictLookup (key);
			if (!action.isDict ())
				continue;

			Object js = action.dictLookup ("JS");
			std::string text;

			if (js.isString ()) {
				text = js.getString ();
			} else if (js.isStream ()) {
				js.getStream ()->fillString (text);
			} else {
				continue;
			}

			size_t at = text.find (needle);
			if (at == std::string::npos)
				continue;

			double fps = g_ascii_strtod (text.c_str () + at + needle.size (), nullptr);

			return fps > 0. ? fps : DEFAULT_FPS;
		}
	}

	return DEFAULT_FPS;
}

PpsPdfAnimations *
pps_pdf_animations_new (const gchar *filename, const gchar *password)
{
	if (!filename)
		return nullptr;

	if (!globalParams)
		globalParams = std::make_unique<GlobalParams> ();

	std::optional<GooString> pw;
	if (password)
		pw = GooString (password);

	auto doc = std::make_unique<PDFDoc> (std::make_unique<GooString> (filename), pw, pw);

	if (!doc->isOk ())
		return nullptr;

	Form *form = doc->getCatalog () ? doc->getCatalog ()->getForm () : nullptr;
	if (!form)
		return nullptr;

	std::vector<FormField *> fields;
	for (int i = 0; i < form->getNumFields (); i++)
		collect_fields (form->getRootField (i), fields);

	/* First pass: the visible buttons, which give position and page. */
	std::vector<int> ids;
	std::vector<FormField *> buttons;

	for (FormField *field : fields) {
		int index = animation_index_of (field_name (field));

		if (index >= 0) {
			ids.push_back (index);
			buttons.push_back (field);
		}
	}

	if (ids.empty ())
		return nullptr;

	/* Second pass: the frames, bucketed by the animation they belong to. */
	std::vector<std::vector<std::pair<int, FormField *>>> frame_fields (ids.size ());

	for (FormField *field : fields) {
		int animation, frame;

		if (!frame_index_of (field_name (field), &animation, &frame))
			continue;

		for (size_t i = 0; i < ids.size (); i++) {
			if (ids[i] == animation) {
				frame_fields[i].push_back ({ frame, field });
				break;
			}
		}
	}

	PpsPdfAnimations *self = new PpsPdfAnimations ();
	self->by_page = g_hash_table_new_full (g_direct_hash, g_direct_equal, nullptr,
	                                       (GDestroyNotify) g_ptr_array_unref);

	for (size_t i = 0; i < ids.size (); i++) {
		if (frame_fields[i].size () < 2)
			continue;

		std::sort (frame_fields[i].begin (), frame_fields[i].end (),
		           [] (const std::pair<int, FormField *> &a,
		               const std::pair<int, FormField *> &b) { return a.first < b.first; });

		FormWidget *widget = buttons[i]->getWidget (0);
		if (!widget)
			continue;

		int page_num = widget_page (doc.get (), widget);
		if (page_num < 1)
			continue;

		std::vector<Object> icons;
		for (auto &entry : frame_fields[i]) {
			Object icon = field_icon (entry.second);

			if (icon.isStream ())
				icons.push_back (std::move (icon));
		}

		if (icons.size () < 2)
			continue;

		double x1, y1, x2, y2;
		widget->getRect (&x1, &y1, &x2, &y2);

		Page *page = doc->getPage (page_num);
		double page_height = page ? page->getCropHeight () : 0.;

		PpsPdfAnimation animation;
		animation.page = (guint) (page_num - 1);
		animation.x1 = MIN (x1, x2);
		animation.x2 = MAX (x1, x2);
		/* PDF puts the origin at the bottom left; everything above this
		 * layer counts from the top. */
		animation.y1 = page_height - MAX (y1, y2);
		animation.y2 = page_height - MIN (y1, y2);
		animation.n_frames = (guint) icons.size ();
		animation.fps = animation_fps (doc.get (), page_num, ids[i]);

		self->animations.push_back (animation);
		self->frames.push_back (std::move (icons));
	}

	if (self->animations.empty ()) {
		g_hash_table_unref (self->by_page);
		delete self;
		return nullptr;
	}

	self->doc = std::move (doc);

	for (PpsPdfAnimation &animation : self->animations) {
		gpointer key = GUINT_TO_POINTER (animation.page);
		GPtrArray *list = (GPtrArray *) g_hash_table_lookup (self->by_page, key);

		if (!list) {
			list = g_ptr_array_new ();
			g_hash_table_insert (self->by_page, key, list);
		}

		g_ptr_array_add (list, &animation);
	}

	return self;
}

void
pps_pdf_animations_free (PpsPdfAnimations *self)
{
	if (!self)
		return;

	g_clear_pointer (&self->by_page, g_hash_table_unref);
	delete self;
}

const GPtrArray *
pps_pdf_animations_for_page (PpsPdfAnimations *self, guint page)
{
	if (!self)
		return nullptr;

	return (GPtrArray *) g_hash_table_lookup (self->by_page, GUINT_TO_POINTER (page));
}

/* The single image a frame draws.
 *
 * An animate frame is one rasterised picture scaled into the button's box -
 * "q <scale> cm q <size> cm /Im1 Do Q Q" - so the image in the appearance
 * stream's resources is the frame. Anything more elaborate than that (vector
 * drawing, several images) is left alone: better a still page than a wrong
 * one, and when this returns nothing the page shows as it would unanimated.
 */
static Object
frame_image (const Object &icon)
{
	Object resources = icon.getStream ()->getDict ()->lookup ("Resources");
	if (!resources.isDict ())
		return Object ();

	Object xobjects = resources.dictLookup ("XObject");
	if (!xobjects.isDict ())
		return Object ();

	Object found;
	int images = 0;

	for (int i = 0; i < xobjects.dictGetLength (); i++) {
		Object entry = xobjects.dictGetVal (i);

		if (!entry.isStream ())
			continue;

		Object subtype = entry.getStream ()->getDict ()->lookup ("Subtype");
		if (!subtype.isName ("Image"))
			continue;

		images++;
		found = std::move (entry);
	}

	return images == 1 ? std::move (found) : Object ();
}

/* The colour spaces a frame can be in, resolved by hand.
 *
 * GfxColorSpace::parse() wants the resources and graphics state it would have
 * during a page render, and faults when handed none - and we have none here,
 * since nothing is being rendered. Frames written by animate are plain 8-bit
 * images, so the few spaces that actually occur are recognised directly and
 * anything else is declined.
 */
enum class FrameSpace {
	Gray,
	RGB,
	CMYK,
	Unsupported
};

static FrameSpace
resolve_space (const Object &cs)
{
	if (cs.isName ()) {
		const char *name = cs.getName ();

		if (!g_strcmp0 (name, "DeviceGray") || !g_strcmp0 (name, "CalGray") || !g_strcmp0 (name, "G"))
			return FrameSpace::Gray;
		if (!g_strcmp0 (name, "DeviceRGB") || !g_strcmp0 (name, "CalRGB") || !g_strcmp0 (name, "RGB"))
			return FrameSpace::RGB;
		if (!g_strcmp0 (name, "DeviceCMYK") || !g_strcmp0 (name, "CMYK"))
			return FrameSpace::CMYK;

		return FrameSpace::Unsupported;
	}

	if (!cs.isArray () || cs.arrayGetLength () < 1)
		return FrameSpace::Unsupported;

	Object head = cs.arrayGet (0, 0);
	if (!head.isName ())
		return FrameSpace::Unsupported;

	if (head.isName ("CalGray"))
		return FrameSpace::Gray;
	if (head.isName ("CalRGB"))
		return FrameSpace::RGB;

	/* ICCBased carries the component count, and its alternate is one of
	 * the device spaces; treating it as that alternate is what poppler
	 * does without colour management. */
	if (head.isName ("ICCBased") && cs.arrayGetLength () >= 2) {
		Object stream = cs.arrayGet (1, 0);

		if (!stream.isStream ())
			return FrameSpace::Unsupported;

		Object n = stream.getStream ()->getDict ()->lookup ("N");
		if (!n.isInt ())
			return FrameSpace::Unsupported;

		switch (n.getInt ()) {
		case 1:
			return FrameSpace::Gray;
		case 3:
			return FrameSpace::RGB;
		case 4:
			return FrameSpace::CMYK;
		default:
			return FrameSpace::Unsupported;
		}
	}

	return FrameSpace::Unsupported;
}

static int
space_components (FrameSpace space)
{
	switch (space) {
	case FrameSpace::Gray:
		return 1;
	case FrameSpace::RGB:
		return 3;
	case FrameSpace::CMYK:
		return 4;
	default:
		return 0;
	}
}

/* Decode an image XObject to 8-bit RGB rows. Returns false for anything it
 * cannot make sense of, leaving @out untouched. */
static bool
decode_image (const Object &image, int *width, int *height, std::vector<unsigned char> &out)
{
	Dict *dict = image.getStream ()->getDict ();
	Object w = dict->lookup ("Width");
	Object h = dict->lookup ("Height");
	Object bpc = dict->lookup ("BitsPerComponent");
	Object cs = dict->lookup ("ColorSpace");

	if (!w.isInt () || !h.isInt () || !bpc.isInt ())
		return false;

	/* 8 bits per component only: that is what a rasterised frame is, and
	 * unpacking the rest would be work spent on files that do not exist. */
	if (bpc.getInt () != 8)
		return false;

	*width = w.getInt ();
	*height = h.getInt ();

	/* 32767 a side is as large as a cairo surface goes. Refusing past it
	 * here, before decoding, matters: a frame that fails is asked for
	 * again on every tick of the animation. */
	if (*width <= 0 || *height <= 0 || *width > 32767 || *height > 32767 ||
	    (gint64) *width * *height > 64 * 1024 * 1024)
		return false;

	FrameSpace space = resolve_space (cs);
	int comps = space_components (space);

	if (comps == 0)
		return false;

	ImageStream stream (image.getStream (), *width, comps, 8);

	if (!stream.rewind ())
		return false;

	out.resize ((size_t) *width * *height * 3);

	for (int y = 0; y < *height; y++) {
		unsigned char *row = stream.getLine ();

		if (!row) {
			out.clear ();
			stream.close ();
			return false;
		}

		unsigned char *dest = out.data () + (size_t) y * *width * 3;

		for (int x = 0; x < *width; x++) {
			const unsigned char *in = row + (size_t) x * comps;

			switch (space) {
			case FrameSpace::Gray:
				dest[x * 3] = dest[x * 3 + 1] = dest[x * 3 + 2] = in[0];
				break;
			case FrameSpace::RGB:
				dest[x * 3] = in[0];
				dest[x * 3 + 1] = in[1];
				dest[x * 3 + 2] = in[2];
				break;
			case FrameSpace::CMYK: {
				int k = in[3];

				dest[x * 3] = (unsigned char) MAX (0, 255 - in[0] - k);
				dest[x * 3 + 1] = (unsigned char) MAX (0, 255 - in[1] - k);
				dest[x * 3 + 2] = (unsigned char) MAX (0, 255 - in[2] - k);
			} break;
			default:
				break;
			}
		}
	}

	stream.close ();

	return true;
}

/* The image's soft mask as an alpha plane at the image's own size, or empty
 * when it has none. */
static std::vector<unsigned char>
decode_smask (const Object &image, int width, int height)
{
	std::vector<unsigned char> alpha;
	Object smask = image.getStream ()->getDict ()->lookup ("SMask");

	if (!smask.isStream ())
		return alpha;

	int mask_width, mask_height;
	std::vector<unsigned char> rgb;

	if (!decode_image (smask, &mask_width, &mask_height, rgb))
		return alpha;

	alpha.resize ((size_t) width * height, 0xff);

	/* A mask is allowed its own resolution, so sample it. */
	for (int y = 0; y < height; y++) {
		int my = mask_height == height ? y : (int) ((gint64) y * mask_height / height);

		for (int x = 0; x < width; x++) {
			int mx = mask_width == width ? x : (int) ((gint64) x * mask_width / width);

			alpha[(size_t) y * width + x] = rgb[((size_t) my * mask_width + mx) * 3];
		}
	}

	return alpha;
}

cairo_surface_t *
pps_pdf_animations_render_frame (PpsPdfAnimations *self,
                                 const PpsPdfAnimation *animation,
                                 guint frame,
                                 gdouble scale)
{
	if (!self || !animation || scale <= 0.)
		return nullptr;

	/* Which animation this is: the caller holds pointers into our own
	 * vector, so the index comes straight from the address. */
	size_t index = (size_t) (animation - self->animations.data ());

	if (index >= self->animations.size () || frame >= self->frames[index].size ())
		return nullptr;

	/* The rectangle is as the file gave it, so check it before it becomes
	 * a size in pixels. */
	double target_width = (animation->x2 - animation->x1) * scale;
	double target_height = (animation->y2 - animation->y1) * scale;

	if (!std::isfinite (target_width) || !std::isfinite (target_height) ||
	    target_width <= 0. || target_height <= 0.)
		return nullptr;

	double fit = MIN (1., MAX_FRAME_SIZE / MAX (target_width, target_height));
	int out_width = (int) ceil (target_width * fit);
	int out_height = (int) ceil (target_height * fit);

	int width, height;
	std::vector<unsigned char> rgb;
	Object image = frame_image (self->frames[index][frame]);

	if (!image.isStream () || !decode_image (image, &width, &height, rgb))
		return nullptr;

	std::vector<unsigned char> alpha = decode_smask (image, width, height);

	/* cairo wants premultiplied BGRA in native byte order. A surface that
	 * failed - out of memory, say - has no pixels to write to. */
	cairo_surface_t *decoded = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, width, height);

	if (cairo_surface_status (decoded) != CAIRO_STATUS_SUCCESS) {
		cairo_surface_destroy (decoded);
		return nullptr;
	}

	unsigned char *data = cairo_image_surface_get_data (decoded);
	int stride = cairo_image_surface_get_stride (decoded);

	cairo_surface_flush (decoded);

	for (int y = 0; y < height; y++) {
		guint32 *out = (guint32 *) (data + (size_t) y * stride);

		for (int x = 0; x < width; x++) {
			const unsigned char *in = rgb.data () + ((size_t) y * width + x) * 3;
			guint a = alpha.empty () ? 0xff : alpha[(size_t) y * width + x];

			out[x] = (a << 24) |
			         ((in[0] * a / 255) << 16) |
			         ((in[1] * a / 255) << 8) |
			         (in[2] * a / 255);
		}
	}

	cairo_surface_mark_dirty (decoded);

	cairo_surface_t *surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
	                                                       out_width, out_height);

	if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
		cairo_surface_destroy (surface);
		cairo_surface_destroy (decoded);
		return nullptr;
	}

	cairo_t *cr = cairo_create (surface);

	cairo_scale (cr, (double) out_width / width, (double) out_height / height);
	cairo_set_source_surface (cr, decoded, 0, 0);
	cairo_pattern_set_filter (cairo_get_source (cr), CAIRO_FILTER_GOOD);
	cairo_paint (cr);

	cairo_destroy (cr);
	cairo_surface_destroy (decoded);

	return surface;
}
