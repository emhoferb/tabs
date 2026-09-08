// SPDX-License-Identifier: GPL-2.0-or-later
/* pps-animation.h
 *  this file is part of papers, a gnome document viewer
 */

#pragma once

#if !defined(__PPS_PAPERS_DOCUMENT_H_INSIDE__) && !defined(PAPERS_COMPILATION)
#error "Only <papers-document.h> can be included directly."
#endif

#include <glib-object.h>
#include <glib.h>

#include "pps-macros.h"

G_BEGIN_DECLS

#define PPS_TYPE_ANIMATION (pps_animation_get_type ())

PPS_PUBLIC
G_DECLARE_FINAL_TYPE (PpsAnimation, pps_animation, PPS, ANIMATION, GObject)

/* A frame sequence living at one place on a page.
 *
 * The document decides how many frames there are and how fast they run; the
 * view asks the document to render frame n when it needs it. @backend_data
 * is the backend's own handle on the animation and is not interpreted here.
 */
PPS_PUBLIC
PpsAnimation *pps_animation_new (guint page,
                                 guint n_frames,
                                 gdouble fps,
                                 gpointer backend_data);

PPS_PUBLIC
guint pps_animation_get_page_index (PpsAnimation *animation);

PPS_PUBLIC
guint pps_animation_get_n_frames (PpsAnimation *animation);

PPS_PUBLIC
gdouble pps_animation_get_fps (PpsAnimation *animation);

PPS_PUBLIC
gpointer pps_animation_get_backend_data (PpsAnimation *animation);

G_END_DECLS
