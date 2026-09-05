// SPDX-License-Identifier: GPL-3.0-or-later
/* pps-colors.h
 *
 * Copyright (C) 2025 Markus Göllnitz <camelcasenick@bewares.it>
 */

#include <adwaita.h>
#include <gtk/gtk.h>

#include "pps-macros.h"

/* Effective accent: the desktop's, or one inferred from the GTK theme name
 * where the desktop selects its accent by swapping theme variants.
 * PPS_PUBLIC because the preferences dialog draws it as a swatch. */
PPS_PUBLIC
AdwAccentColor pps_get_accent_color (void);

/* Re-resolve the accent and reinstall the CSS override. */
void pps_accent_color_refresh (void);

void get_accent_color (GdkRGBA *color, GdkRGBA *foreground_color);

#ifdef HAVE_TRANSPARENT_SELECTION
void get_selection_color (GtkWidget *widget, GdkRGBA *color);
#endif
