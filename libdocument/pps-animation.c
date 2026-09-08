// SPDX-License-Identifier: GPL-2.0-or-later
/* pps-animation.c
 *  this file is part of papers, a gnome document viewer
 */

#include <config.h>

#include "pps-animation.h"

struct _PpsAnimation {
	GObject base_instance;

	guint page;
	guint n_frames;
	gdouble fps;

	/* Owned by the document, which outlives every animation it hands out. */
	gpointer backend_data;
};

G_DEFINE_TYPE (PpsAnimation, pps_animation, G_TYPE_OBJECT)

static void
pps_animation_class_init (PpsAnimationClass *klass)
{
}

static void
pps_animation_init (PpsAnimation *animation)
{
}

PpsAnimation *
pps_animation_new (guint page,
                   guint n_frames,
                   gdouble fps,
                   gpointer backend_data)
{
	PpsAnimation *animation = PPS_ANIMATION (g_object_new (PPS_TYPE_ANIMATION, NULL));

	animation->page = page;
	animation->n_frames = n_frames;
	animation->fps = fps;
	animation->backend_data = backend_data;

	return animation;
}

guint
pps_animation_get_page_index (PpsAnimation *animation)
{
	g_return_val_if_fail (PPS_IS_ANIMATION (animation), 0);

	return animation->page;
}

guint
pps_animation_get_n_frames (PpsAnimation *animation)
{
	g_return_val_if_fail (PPS_IS_ANIMATION (animation), 0);

	return animation->n_frames;
}

gdouble
pps_animation_get_fps (PpsAnimation *animation)
{
	g_return_val_if_fail (PPS_IS_ANIMATION (animation), 0.);

	return animation->fps;
}

gpointer
pps_animation_get_backend_data (PpsAnimation *animation)
{
	g_return_val_if_fail (PPS_IS_ANIMATION (animation), NULL);

	return animation->backend_data;
}
