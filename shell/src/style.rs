// SPDX-License-Identifier: GPL-3.0-or-later

//! Appearance preferences that several widgets share.
//!
//! The sidebar lists and the thumbnail grid all mark their selection the same
//! way, and the accent they use is configurable, so the plumbing lives here
//! rather than being repeated in each widget.

use crate::deps::*;

pub const HL_OUTLINE: &str = "pps-hl-outline";
pub const HL_FILL: &str = "pps-hl-fill";

fn settings() -> gio::Settings {
    gio::Settings::new("org.gnome.Papers")
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

fn apply_highlight_style(widget: &gtk::Widget, style: &str) {
    widget.remove_css_class(HL_OUTLINE);
    widget.remove_css_class(HL_FILL);
    widget.add_css_class(if style == "fill" { HL_FILL } else { HL_OUTLINE });
}
