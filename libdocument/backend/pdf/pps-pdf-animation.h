// SPDX-License-Identifier: GPL-2.0-or-later
/* pps-pdf-animation.h
 *
 * Animations produced by the LaTeX "animate" package.
 */

#pragma once

#include <cairo.h>
#include <glib.h>

G_BEGIN_DECLS

typedef struct _PpsPdfAnimations PpsPdfAnimations;

/* One animation: where it sits on its page, how many frames it has, and how
 * fast the document wants them played. Coordinates are PDF points, with the
 * origin at the top left of the page, matching PpsMapping. */
typedef struct {
	guint page;
	gdouble x1, y1, x2, y2;
	guint n_frames;
	gdouble fps;
} PpsPdfAnimation;

/* Opens its own handle on the file: poppler-glib keeps its PDFDoc private,
 * and none of what this needs - form field icons - is reachable through the
 * glib API. Returns NULL for a document without animations, so the caller
 * pays nothing for the common case. */
PpsPdfAnimations *pps_pdf_animations_new (const gchar *filename,
                                          const gchar *password);

void pps_pdf_animations_free (PpsPdfAnimations *self);

/* The animations on one page, as PpsPdfAnimation, or NULL if there are none.
 * The array belongs to @self. */
const GPtrArray *pps_pdf_animations_for_page (PpsPdfAnimations *self,
                                              guint page);

/* Renders one frame at @scale, sized to the animation's rectangle. */
cairo_surface_t *pps_pdf_animations_render_frame (PpsPdfAnimations *self,
                                                  const PpsPdfAnimation *animation,
                                                  guint frame,
                                                  gdouble scale);

G_END_DECLS
