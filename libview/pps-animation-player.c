// SPDX-License-Identifier: GPL-2.0-or-later
/* pps-animation-player.c
 *  this file is part of papers, a gnome document viewer
 */

#include <config.h>

#include "pps-animation-player.h"
#include "pps-document-misc.h"

/* How much one page may spend holding decoded frames. Past this, frames are
 * decoded as they are shown and dropped again: slower on every loop, but a
 * zoomed-in animation cannot run away with hundreds of megabytes. */
#define CACHE_MAX_BYTES (48 * 1024 * 1024)

struct _PpsAnimationPlayer {
	/* Owns this player, and outlives it. */
	GtkWidget *widget;

	PpsDocument *document;
	gint page;

	PpsMappingList *animations;

	/* PpsAnimation -> GPtrArray of GdkTexture, one slot per frame, filled
	 * as each frame is first shown. */
	GHashTable *frames;

	/* The scale the cached frames were decoded at; a zoom invalidates the
	 * lot of them. */
	gdouble scale;

	guint index;
	guint tick_id;
	gint64 last_frame_us;
};

/* g_object_unref cannot be used directly: the frame slots start empty, and
 * unreffing NULL is a critical - fatal in a build that treats it so. */
static void
unref_frame (gpointer texture)
{
	if (texture)
		g_object_unref (texture);
}

static void
clear_frames (PpsAnimationPlayer *player)
{
	g_hash_table_remove_all (player->frames);
}

static gdouble
animations_fps (PpsAnimationPlayer *player)
{
	gdouble fps = 1.;

	for (GList *l = pps_mapping_list_get_list (player->animations); l; l = l->next) {
		PpsMapping *mapping = l->data;

		fps = MAX (fps, pps_animation_get_fps (PPS_ANIMATION (mapping->data)));
	}

	return CLAMP (fps, 1., 60.);
}

static gboolean
tick (GtkWidget *widget, GdkFrameClock *clock, gpointer data)
{
	PpsAnimationPlayer *player = data;
	gint64 now = gdk_frame_clock_get_frame_time (clock);
	gint64 interval;

	if (!player->animations) {
		player->tick_id = 0;
		return G_SOURCE_REMOVE;
	}

	interval = (gint64) (G_USEC_PER_SEC / animations_fps (player));

	if (now - player->last_frame_us < interval)
		return G_SOURCE_CONTINUE;

	player->last_frame_us = now;
	player->index++;

	gtk_widget_queue_draw (widget);

	return G_SOURCE_CONTINUE;
}

static void
stop (PpsAnimationPlayer *player)
{
	if (player->tick_id == 0)
		return;

	gtk_widget_remove_tick_callback (player->widget, player->tick_id);
	player->tick_id = 0;
}

static void
start (PpsAnimationPlayer *player)
{
	if (player->tick_id != 0 || !player->animations)
		return;

	/* Tick callbacks only run while the widget is mapped, so a page
	 * scrolled out of view costs nothing. */
	player->tick_id = gtk_widget_add_tick_callback (player->widget, tick, player, NULL);
}

PpsAnimationPlayer *
pps_animation_player_new (GtkWidget *widget)
{
	PpsAnimationPlayer *player = g_new0 (PpsAnimationPlayer, 1);

	player->widget = widget;
	player->page = -1;
	player->frames = g_hash_table_new_full (g_direct_hash, g_direct_equal, NULL,
	                                        (GDestroyNotify) g_ptr_array_unref);

	return player;
}

void
pps_animation_player_free (PpsAnimationPlayer *player)
{
	if (!player)
		return;

	stop (player);

	g_clear_pointer (&player->animations, pps_mapping_list_unref);
	g_clear_pointer (&player->frames, g_hash_table_unref);
	g_free (player);
}

void
pps_animation_player_set_page (PpsAnimationPlayer *player,
                               PpsDocument *document,
                               gint page)
{
	g_return_if_fail (player != NULL);

	stop (player);
	clear_frames (player);
	g_clear_pointer (&player->animations, pps_mapping_list_unref);

	player->document = document;
	player->page = page;
	player->index = 0;

	if (!document || page < 0 || !PPS_IS_DOCUMENT_ANIMATIONS (document))
		return;

	player->animations =
	    pps_document_animations_get_animation_mapping (PPS_DOCUMENT_ANIMATIONS (document),
	                                                   page);

	start (player);
}

gboolean
pps_animation_player_is_playing (PpsAnimationPlayer *player)
{
	return player && player->animations != NULL;
}

/* The texture for an animation's current frame, decoded if it is not already
 * in hand. This runs on the main thread rather than through a job: a frame is
 * one scaled image, and waiting for a job would drop frames every loop. */
static GdkTexture *
frame_texture (PpsAnimationPlayer *player, PpsAnimation *animation, gdouble scale)
{
	guint n_frames = pps_animation_get_n_frames (animation);
	GPtrArray *frames;
	guint frame;
	cairo_surface_t *surface;
	GdkTexture *texture;

	if (n_frames == 0)
		return NULL;

	frame = player->index % n_frames;

	frames = g_hash_table_lookup (player->frames, animation);

	if (!frames) {
		frames = g_ptr_array_new_full (n_frames, unref_frame);
		g_ptr_array_set_size (frames, n_frames);
		g_hash_table_insert (player->frames, animation, frames);
	}

	texture = g_ptr_array_index (frames, frame);
	if (texture)
		return texture;

	surface = pps_document_animations_render_frame (PPS_DOCUMENT_ANIMATIONS (player->document),
	                                                animation, frame, scale);

	if (!surface)
		return NULL;

	texture = pps_document_misc_texture_from_surface (surface);
	cairo_surface_destroy (surface);

	if (!texture)
		return NULL;

	/* Kept only while the whole loop fits the budget. Past that the
	 * texture is handed to the caller and forgotten, so the next pass
	 * decodes it again - the animation still plays, it just costs more. */
	if ((gsize) gdk_texture_get_width (texture) * gdk_texture_get_height (texture) * 4 * n_frames <=
	    CACHE_MAX_BYTES) {
		g_ptr_array_index (frames, frame) = texture;
	} else {
		g_object_set_data_full (G_OBJECT (player->widget), "pps-animation-transient",
		                        texture, g_object_unref);
	}

	return texture;
}

/* Where a rectangle given in PDF points, top-left origin, lands inside the
 * page as it is drawn. */
static void
map_rect (const PpsRectangle *doc_rect,
          const GdkRectangle *page_area,
          gdouble scale,
          guint rotation,
          GdkRectangle *out)
{
	gdouble x, y, width, height;

	switch (rotation) {
	case 90:
		x = page_area->width - doc_rect->y2 * scale;
		y = doc_rect->x1 * scale;
		width = (doc_rect->y2 - doc_rect->y1) * scale;
		height = (doc_rect->x2 - doc_rect->x1) * scale;
		break;
	case 180:
		x = page_area->width - doc_rect->x2 * scale;
		y = page_area->height - doc_rect->y2 * scale;
		width = (doc_rect->x2 - doc_rect->x1) * scale;
		height = (doc_rect->y2 - doc_rect->y1) * scale;
		break;
	case 270:
		x = doc_rect->y1 * scale;
		y = page_area->height - doc_rect->x2 * scale;
		width = (doc_rect->y2 - doc_rect->y1) * scale;
		height = (doc_rect->x2 - doc_rect->x1) * scale;
		break;
	default:
		x = doc_rect->x1 * scale;
		y = doc_rect->y1 * scale;
		width = (doc_rect->x2 - doc_rect->x1) * scale;
		height = (doc_rect->y2 - doc_rect->y1) * scale;
		break;
	}

	out->x = (gint) (page_area->x + x);
	out->y = (gint) (page_area->y + y);
	out->width = (gint) ceil (width);
	out->height = (gint) ceil (height);
}

void
pps_animation_player_snapshot (PpsAnimationPlayer *player,
                               GtkSnapshot *snapshot,
                               const GdkRectangle *page_area,
                               gdouble page_width,
                               gdouble page_height,
                               guint rotation)
{
	gdouble scale;

	if (!player || !player->animations || page_width <= 0. || page_height <= 0.)
		return;

	/* The page is drawn to fit its area, so that ratio is the scale the
	 * frames have to be decoded at - which is also how presentation mode,
	 * with no zoom level of its own, gets the right size. */
	if (rotation == 90 || rotation == 270)
		scale = page_area->height / page_width;
	else
		scale = page_area->width / page_width;

	if (scale <= 0.)
		return;

	if (scale != player->scale) {
		player->scale = scale;
		clear_frames (player);
	}

	for (GList *l = pps_mapping_list_get_list (player->animations); l; l = l->next) {
		PpsMapping *mapping = l->data;
		PpsAnimation *animation = PPS_ANIMATION (mapping->data);
		GdkRectangle view_rect;
		GdkTexture *texture;

		texture = frame_texture (player, animation, scale);
		if (!texture)
			continue;

		map_rect (&mapping->area, page_area, scale, rotation, &view_rect);

		gtk_snapshot_append_texture (snapshot, texture,
		                             &GRAPHENE_RECT_INIT (view_rect.x, view_rect.y,
		                                                  view_rect.width, view_rect.height));
	}
}
