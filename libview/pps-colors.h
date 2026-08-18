// SPDX-License-Identifier: GPL-3.0-or-later
/* pps-colors.h
 *
 * Copyright (C) 2025 Markus Göllnitz <camelcasenick@bewares.it>
 */

#include <adwaita.h>
#include <gtk/gtk.h>

/* Effective accent: the desktop's, or one inferred from the GTK theme name
 * where the desktop selects its accent by swapping theme variants. */
AdwAccentColor pps_get_accent_color (void);

void get_accent_color (GdkRGBA *color, GdkRGBA *foreground_color);

#ifdef HAVE_TRANSPARENT_SELECTION
void get_selection_color (GtkWidget *widget, GdkRGBA *color);
#endif
