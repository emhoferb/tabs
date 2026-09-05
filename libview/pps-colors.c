// SPDX-License-Identifier: GPL-3.0-or-later
/* pps-colors.c
 *
 * Copyright (C) 2025 Markus Göllnitz <camelcasenick@bewares.it>
 */

#include "pps-colors.h"

#include <adwaita.h>

/* Some desktops select an accent colour by switching GTK theme variants rather
 * than by publishing org.freedesktop.appearance accent-color. Ubuntu is the
 * common case: choosing an accent in Settings swaps the theme between
 * Yaru, Yaru-red, Yaru-magenta and so on, and nothing appears on the portal.
 * libadwaita then reports its default, so selections and highlights come out
 * blue while every other application on the desktop is using the chosen
 * accent.
 *
 * Where the theme name encodes an accent and libadwaita has not been told
 * otherwise, follow the theme. On a desktop that does publish an accent,
 * libadwaita reports it and that value wins.
 */
static const struct {
	const char *prefix;
	AdwAccentColor accent;
} theme_accents[] = {
	/* Longest first: every variant also starts with "Yaru". */
	{ "Yaru-prussiangreen", ADW_ACCENT_COLOR_TEAL },
	{ "Yaru-viridian", ADW_ACCENT_COLOR_TEAL },
	{ "Yaru-magenta", ADW_ACCENT_COLOR_PINK },
	{ "Yaru-purple", ADW_ACCENT_COLOR_PURPLE },
	{ "Yaru-olive", ADW_ACCENT_COLOR_GREEN },
	{ "Yaru-sage", ADW_ACCENT_COLOR_GREEN },
	{ "Yaru-bark", ADW_ACCENT_COLOR_YELLOW },
	{ "Yaru-blue", ADW_ACCENT_COLOR_BLUE },
	{ "Yaru-red", ADW_ACCENT_COLOR_RED },
	{ "Yaru", ADW_ACCENT_COLOR_ORANGE },
};

static gboolean
accent_from_theme_string (const char *theme, AdwAccentColor *out)
{
	if (!theme)
		return FALSE;

	for (guint i = 0; i < G_N_ELEMENTS (theme_accents); i++) {
		if (g_str_has_prefix (theme, theme_accents[i].prefix)) {
			*out = theme_accents[i].accent;
			return TRUE;
		}
	}

	return FALSE;
}

/* Read the desktop's GTK theme name.
 *
 * GtkSettings is no use inside a sandbox: Flatpak reports "Adwaita-empty" so
 * that host themes cannot interfere with libadwaita. The settings portal still
 * carries the real value, so ask it directly, and fall back to GtkSettings for
 * unsandboxed builds.
 */
static char *
read_desktop_gtk_theme (void)
{
	g_autoptr (GDBusConnection) bus = NULL;
	g_autoptr (GVariant) reply = NULL;
	g_autoptr (GVariant) value = NULL;
	GtkSettings *settings;
	char *theme = NULL;

	bus = g_bus_get_sync (G_BUS_TYPE_SESSION, NULL, NULL);
	if (bus) {
		reply = g_dbus_connection_call_sync (
		    bus,
		    "org.freedesktop.portal.Desktop",
		    "/org/freedesktop/portal/desktop",
		    "org.freedesktop.portal.Settings",
		    "ReadOne",
		    g_variant_new ("(ss)", "org.gnome.desktop.interface", "gtk-theme"),
		    G_VARIANT_TYPE ("(v)"),
		    G_DBUS_CALL_FLAGS_NONE, 1000, NULL, NULL);

		if (reply) {
			g_variant_get (reply, "(v)", &value);
			if (value && g_variant_is_of_type (value, G_VARIANT_TYPE_STRING))
				return g_variant_dup_string (value, NULL);
		}
	}

	settings = gtk_settings_get_default ();
	if (settings)
		g_object_get (settings, "gtk-theme-name", &theme, NULL);

	return theme;
}

static gboolean
accent_from_theme_name (AdwAccentColor *out)
{
	g_autofree char *theme = read_desktop_gtk_theme ();

	return accent_from_theme_string (theme, out);
}

/* Named accents the user can pick explicitly, matching the Ubuntu palette. */
static const struct {
	const char *name;
	AdwAccentColor accent;
} named_accents[] = {
	{ "orange", ADW_ACCENT_COLOR_ORANGE },
	{ "bark", ADW_ACCENT_COLOR_YELLOW },
	{ "sage", ADW_ACCENT_COLOR_GREEN },
	{ "olive", ADW_ACCENT_COLOR_GREEN },
	{ "viridian", ADW_ACCENT_COLOR_TEAL },
	{ "prussiangreen", ADW_ACCENT_COLOR_TEAL },
	{ "blue", ADW_ACCENT_COLOR_BLUE },
	{ "purple", ADW_ACCENT_COLOR_PURPLE },
	{ "magenta", ADW_ACCENT_COLOR_PINK },
	{ "red", ADW_ACCENT_COLOR_RED },
	{ "teal", ADW_ACCENT_COLOR_TEAL },
	{ "slate", ADW_ACCENT_COLOR_SLATE },
};

static gboolean
resolve_accent (AdwAccentColor *out, gboolean *overridden)
{
	AdwStyleManager *style_manager = adw_style_manager_get_default ();
	AdwAccentColor accent = adw_style_manager_get_accent_color (style_manager);
	AdwAccentColor from_theme;
	g_autoptr (GSettings) settings = g_settings_new ("org.gnome.Papers");
	g_autofree char *source = g_settings_get_string (settings, "accent-source");

	/* An explicit choice wins over everything. */
	if (source && g_strcmp0 (source, "system") != 0) {
		if (g_strcmp0 (source, "upstream") == 0) {
			*out = ADW_ACCENT_COLOR_BLUE;
			*overridden = TRUE;
			return TRUE;
		}

		for (guint i = 0; i < G_N_ELEMENTS (named_accents); i++) {
			if (g_strcmp0 (source, named_accents[i].name) == 0) {
				*out = named_accents[i].accent;
				*overridden = TRUE;
				return TRUE;
			}
		}
	}

	/* Blue is also libadwaita's value when nothing has been published, so it
	 * is the only case where the theme name is worth consulting. Anything
	 * else means the desktop stated a preference, which takes precedence. */
	if (accent == ADW_ACCENT_COLOR_BLUE && accent_from_theme_name (&from_theme)) {
		*out = from_theme;
		*overridden = TRUE;
		return TRUE;
	}

	*out = accent;
	*overridden = FALSE;
	return TRUE;
}

/* The accent can change at runtime now that it is a preference, so the CSS
 * override has to be replaceable rather than installed once. */
static GtkCssProvider *accent_provider = NULL;

void
pps_accent_color_refresh (void)
{
	GdkDisplay *display = gdk_display_get_default ();
	AdwAccentColor accent;
	gboolean overridden = FALSE;
	GdkRGBA bg;
	g_autofree char *bg_str = NULL;
	g_autofree char *css = NULL;

	if (!display)
		return;

	if (accent_provider) {
		gtk_style_context_remove_provider_for_display (
		    display, GTK_STYLE_PROVIDER (accent_provider));
		g_clear_object (&accent_provider);
	}

	resolve_accent (&accent, &overridden);
	if (!overridden)
		return;

	adw_accent_color_to_rgba (accent, &bg);
	bg_str = gdk_rgba_to_string (&bg);
	css = g_strdup_printf (":root {"
			       "  --accent-bg-color: %s;"
			       "  --accent-color: %s;"
			       "  --accent-fg-color: #ffffff;"
			       "}",
			       bg_str, bg_str);

	accent_provider = gtk_css_provider_new ();
	gtk_css_provider_load_from_string (accent_provider, css);
	gtk_style_context_add_provider_for_display (
	    display, GTK_STYLE_PROVIDER (accent_provider),
	    GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
}

/* Widgets styled by libadwaita - the sidebar selection above all - take their
 * colour from the --accent-* CSS variables rather than from this code, and
 * libadwaita has no setter for the accent. When the accent has been inferred
 * or chosen, redefine those variables so CSS and rendering agree.
 */
static void
ensure_accent_css (void)
{
	static gsize done = 0;

	if (g_once_init_enter (&done)) {
		g_autoptr (GSettings) settings = g_settings_new ("org.gnome.Papers");

		pps_accent_color_refresh ();

		/* Deliberately leaked: it must outlive every view. */
		g_signal_connect (g_object_ref (settings), "changed::accent-source",
				  G_CALLBACK (pps_accent_color_refresh), NULL);

		g_once_init_leave (&done, 1);
	}
}

/* The desktop's own accent - portal or theme-name inference - ignoring the
 * accent-source preference. What a "follow the system" swatch should show
 * even while a fixed colour is applied. */
AdwAccentColor
pps_get_system_accent_color (void)
{
	AdwStyleManager *style_manager = adw_style_manager_get_default ();
	AdwAccentColor accent = adw_style_manager_get_accent_color (style_manager);
	AdwAccentColor from_theme;

	if (accent == ADW_ACCENT_COLOR_BLUE && accent_from_theme_name (&from_theme))
		return from_theme;

	return accent;
}

AdwAccentColor
pps_get_accent_color (void)
{
	AdwAccentColor accent;
	gboolean overridden;

	ensure_accent_css ();
	resolve_accent (&accent, &overridden);

	return accent;
}

void
get_accent_color (GdkRGBA *background_color, GdkRGBA *foreground_color)
{
	if (background_color)
		adw_accent_color_to_rgba (pps_get_accent_color (), background_color);

	if (foreground_color) {
		foreground_color->red = 1.0;
		foreground_color->green = 1.0;
		foreground_color->blue = 1.0;
	}
}

#ifdef HAVE_TRANSPARENT_SELECTION
void
get_selection_color (GtkWidget *widget, GdkRGBA *color)
{
	if (gtk_widget_get_state_flags (widget) & GTK_STATE_FLAG_FOCUS_WITHIN) {
		adw_accent_color_to_rgba (pps_get_accent_color (), color);
		color->alpha *= 0.3;
	} else {
		gtk_widget_get_color (widget, color);
		color->alpha *= 0.1;
	}
}
#endif
