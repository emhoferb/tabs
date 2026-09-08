// SPDX-License-Identifier: GPL-3.0-or-later

//! Appearance preferences that several widgets share.
//!
//! The sidebar lists and the thumbnail grid all mark their selection the same
//! way, and the accent they use is configurable, so the plumbing lives here
//! rather than being repeated in each widget.

use crate::deps::*;

pub const HL_OUTLINE: &str = "pps-hl-outline";
pub const HL_FILL: &str = "pps-hl-fill";

const TAB_UNDERLINE: &str = "pps-tab-underline";
const TAB_FILLED: &str = "pps-tab-filled";

fn settings() -> gio::Settings {
    // One shared instance for the whole process. A fresh instance per call
    // used to be dropped as soon as the caller returned, killing its
    // "changed" subscription - which is why a new highlight style only
    // showed up after a restart.
    thread_local! {
        static SETTINGS: gio::Settings = gio::Settings::new("org.gnome.Papers");
    }

    SETTINGS.with(|s| s.clone())
}

/// Apply the configured selection style to `widget` and keep it in step.
///
/// Called from each list's constructor; the widget holds a reference to the
/// settings object through the closure, so it stops updating when destroyed.
pub fn track_highlight_style(widget: &impl IsA<gtk::Widget>) {
    let widget = widget.as_ref().clone();
    let settings = settings();

    apply_highlight_style(&widget, &settings.string("highlight-style"));

    settings.connect_changed(
        Some("highlight-style"),
        glib::clone!(
            #[weak]
            widget,
            move |settings, _| {
                apply_highlight_style(&widget, &settings.string("highlight-style"));
            }
        ),
    );
}

/// Apply the configured style for the selected tab, and keep it in step.
///
/// Separate from the list highlight above, and deliberately so: these are two
/// preferences about two different things.
pub fn track_tab_highlight_style(widget: &impl IsA<gtk::Widget>) {
    let widget = widget.as_ref().clone();
    let settings = settings();

    apply_tab_highlight_style(&widget, &settings.string("tab-highlight-style"));

    settings.connect_changed(
        Some("tab-highlight-style"),
        glib::clone!(
            #[weak]
            widget,
            move |settings, _| {
                apply_tab_highlight_style(&widget, &settings.string("tab-highlight-style"));
            }
        ),
    );
}

fn apply_tab_highlight_style(widget: &gtk::Widget, style: &str) {
    widget.remove_css_class(TAB_UNDERLINE);
    widget.remove_css_class(TAB_FILLED);

    // "default" adds nothing, leaving the grey libadwaita draws itself.
    match style {
        "underline" => widget.add_css_class(TAB_UNDERLINE),
        "filled" => widget.add_css_class(TAB_FILLED),
        _ => {}
    }
}

fn apply_highlight_style(widget: &gtk::Widget, style: &str) {
    widget.remove_css_class(HL_OUTLINE);
    widget.remove_css_class(HL_FILL);
    widget.add_css_class(if style == "fill" { HL_FILL } else { HL_OUTLINE });
}
