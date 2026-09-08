// SPDX-License-Identifier: GPL-2.0-or-later
/* pps-document-animations.c
 *  this file is part of papers, a gnome document viewer
 */

#include <config.h>

#include "pps-document-animations.h"

G_DEFINE_INTERFACE (PpsDocumentAnimations, pps_document_animations, 0)

static void
pps_document_animations_default_init (PpsDocumentAnimationsInterface *klass)
{
}

PpsMappingList *
pps_document_animations_get_animation_mapping (PpsDocumentAnimations *document_animations,
                                               PpsPage *page)
{
	PpsDocumentAnimationsInterface *iface = PPS_DOCUMENT_ANIMATIONS_GET_IFACE (document_animations);

	if (!iface->get_animation_mapping)
		return NULL;

	return iface->get_animation_mapping (document_animations, page);
}

cairo_surface_t *
pps_document_animations_render_frame (PpsDocumentAnimations *document_animations,
                                      PpsAnimation *animation,
                                      guint frame,
                                      gdouble scale)
{
	PpsDocumentAnimationsInterface *iface = PPS_DOCUMENT_ANIMATIONS_GET_IFACE (document_animations);

	if (!iface->render_frame)
		return NULL;

	return iface->render_frame (document_animations, animation, frame, scale);
}
