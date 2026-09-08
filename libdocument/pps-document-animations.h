// SPDX-License-Identifier: GPL-2.0-or-later
/* pps-document-animations.h
 *  this file is part of papers, a gnome document viewer
 */

#pragma once

#if !defined(__PPS_PAPERS_DOCUMENT_H_INSIDE__) && !defined(PAPERS_COMPILATION)
#error "Only <papers-document.h> can be included directly."
#endif

#include <cairo.h>
#include <glib-object.h>
#include <glib.h>

#include "pps-animation.h"
#include "pps-document.h"
#include "pps-macros.h"
#include "pps-mapping-list.h"

G_BEGIN_DECLS

#define PPS_TYPE_DOCUMENT_ANIMATIONS (pps_document_animations_get_type ())

PPS_PUBLIC
G_DECLARE_INTERFACE (PpsDocumentAnimations, pps_document_animations, PPS, DOCUMENT_ANIMATIONS, GObject)

struct _PpsDocumentAnimationsInterface {
	GTypeInterface base_iface;

	/* Methods */
	PpsMappingList *(*get_animation_mapping) (PpsDocumentAnimations *document_animations,
	                                          PpsPage *page);
	cairo_surface_t *(*render_frame) (PpsDocumentAnimations *document_animations,
	                                  PpsAnimation *animation,
	                                  guint frame,
	                                  gdouble scale);
};

PPS_PUBLIC
PpsMappingList *pps_document_animations_get_animation_mapping (PpsDocumentAnimations *document_animations,
                                                               PpsPage *page);

/* Renders one frame at @scale, sized to the animation's own rectangle. */
PPS_PUBLIC
cairo_surface_t *pps_document_animations_render_frame (PpsDocumentAnimations *document_animations,
                                                       PpsAnimation *animation,
                                                       guint frame,
                                                       gdouble scale);

G_END_DECLS
