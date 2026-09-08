// SPDX-License-Identifier: GPL-2.0-or-later
/* pps-animation-player.h
 *  this file is part of papers, a gnome document viewer
 */

#pragma once

#include <gtk/gtk.h>
#include <papers-document.h>

G_BEGIN_DECLS

/* Plays the frame sequences a page carries (see PpsDocumentAnimations).
 *
 * Both the scrolled view and presentation mode show pages, so both need this;
 * they differ only in where the page lands on screen, which is why the caller
 * passes that in at snapshot time rather than the player working it out.
 */
typedef struct _PpsAnimationPlayer PpsAnimationPlayer;

/* @widget drives the timing and is what gets redrawn. It is not referenced:
 * the player belongs to it and never outlives it. */
PpsAnimationPlayer *pps_animation_player_new (GtkWidget *widget);

void pps_animation_player_free (PpsAnimationPlayer *player);

/* Point the player at a page. Pass a NULL document or a negative page to stop
 * it; a page with no animations costs nothing. */
void pps_animation_player_set_page (PpsAnimationPlayer *player,
                                    PpsDocument *document,
                                    gint page);

gboolean pps_animation_player_is_playing (PpsAnimationPlayer *player);

/* Draw the current frame of each animation. @page_area is where the page is
 * on screen, @page_width and @page_height its unrotated size in PDF points.
 */
void pps_animation_player_snapshot (PpsAnimationPlayer *player,
                                    GtkSnapshot *snapshot,
                                    const GdkRectangle *page_area,
                                    gdouble page_width,
                                    gdouble page_height,
                                    guint rotation);

G_END_DECLS
