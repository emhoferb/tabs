// SPDX-License-Identifier: GPL-3.0-or-later

//! The curated set of customizable shortcuts.
//!
//! Most bindings stay where upstream put them, in the widget templates. The
//! ones the fork added - tabs, sidebar, outline, floating controls,
//! preferences - are the ones a user is likely to want on different keys, so
//! they resolve through the `custom-shortcuts` GSettings map (id -> accel)
//! with the defaults below.
//!
//! Window-scope entries are installed as application accels, which makes
//! GTK route them to the focused window and keeps `action-name` items in the
//! shortcuts dialog truthful. Document-scope entries target the "doc" action
//! group, which only resolves from inside a document view, so those are a
//! `GtkShortcutController` each view installs on itself.

use crate::deps::*;
use std::collections::HashMap;

pub const SETTINGS_KEY: &str = "custom-shortcuts";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Window,
    Document,
}

pub struct Entry {
    pub id: &'static str,
    pub action: &'static str,
    pub scope: Scope,
    pub default: &'static str,
    /// Shown in Preferences and the shortcuts dialog; wrap in gettext() there.
    pub title: &'static str,
}

pub const ENTRIES: &[Entry] = &[
    Entry {
        id: "new-tab",
        action: "win.new-tab",
        scope: Scope::Window,
        default: "<Control>t",
        title: "Open Document in New Tab",
    },
    Entry {
        id: "close-tab",
        action: "win.close-tab",
        scope: Scope::Window,
        default: "<Control>w",
        title: "Close Tab",
    },
    Entry {
        id: "duplicate-tab",
        action: "doc.open-copy",
        scope: Scope::Document,
        default: "<Control><Shift>d",
        title: "Open Copy in New Tab",
    },
    Entry {
        id: "new-window",
        action: "win.new-window",
        scope: Scope::Window,
        default: "<Control><Shift>n",
        title: "Open Document in New Window",
    },
    Entry {
        id: "toggle-sidebar",
        action: "doc.show-sidebar",
        scope: Scope::Document,
        default: "F9",
        title: "Toggle Sidebar",
    },
    Entry {
        id: "outline-toggle",
        action: "doc.outline-toggle",
        scope: Scope::Document,
        default: "F8",
        title: "Expand or Collapse Outline",
    },
    Entry {
        id: "toggle-overlay",
        action: "win.show-zoom-overlay",
        scope: Scope::Window,
        default: "<Control>h",
        title: "Toggle Floating Controls",
    },
    Entry {
        id: "preferences",
        action: "win.preferences",
        scope: Scope::Window,
        default: "<Control>comma",
        title: "Preferences",
    },
];

fn overrides(settings: &gio::Settings) -> HashMap<String, String> {
    settings.value(SETTINGS_KEY).get().unwrap_or_default()
}

/// The effective accel for an entry: the user's override, or the default.
pub fn accel(settings: &gio::Settings, entry: &Entry) -> String {
    overrides(settings)
        .remove(entry.id)
        .unwrap_or_else(|| entry.default.to_string())
}

/// Store an override, or clear it to fall back to the default.
pub fn set_accel(settings: &gio::Settings, id: &str, accel: Option<&str>) {
    let mut map = overrides(settings);

    match accel {
        Some(a) => map.insert(id.to_string(), a.to_string()),
        None => map.remove(id),
    };

    let _ = settings.set_value(SETTINGS_KEY, &map.to_variant());
}

/// The entry, if any, whose effective accel collides with `keyval`+`mask`,
/// other than `except`. Compared parsed, not as strings, since the same combo
/// has several spellings ("<Ctrl>w" and "<Control>w").
pub fn conflicting_entry(
    settings: &gio::Settings,
    keyval: gdk::Key,
    mask: gdk::ModifierType,
    except: &str,
) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| {
        e.id != except
            && gtk::accelerator_parse(&accel(settings, e))
                .is_some_and(|(k, m)| k == keyval && m == mask)
    })
}

/// Install (or refresh) the window-scope entries as application accels.
pub fn apply_app_accels(app: &impl IsA<gtk::Application>, settings: &gio::Settings) {
    for e in ENTRIES.iter().filter(|e| e.scope == Scope::Window) {
        app.as_ref()
            .set_accels_for_action(e.action, &[&accel(settings, e)]);
    }
}

/// A controller holding the document-scope entries, to be attached to a
/// document view. Rebuilt from scratch whenever the map changes.
pub fn document_controller(settings: &gio::Settings) -> gtk::ShortcutController {
    let controller = gtk::ShortcutController::new();

    controller.set_propagation_phase(gtk::PropagationPhase::Bubble);

    for e in ENTRIES.iter().filter(|e| e.scope == Scope::Document) {
        if let Some(trigger) = gtk::ShortcutTrigger::parse_string(&accel(settings, e)) {
            controller.add_shortcut(gtk::Shortcut::new(
                Some(trigger),
                Some(gtk::NamedAction::new(e.action)),
            ));
        }
    }

    controller
}
