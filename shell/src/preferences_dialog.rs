// SPDX-License-Identifier: GPL-3.0-or-later

//! Appearance and behaviour preferences.

use crate::deps::*;

/// Accent choices, in the order they appear in the combo row. The first two are
/// the special sources; the rest name a fixed colour.
const ACCENTS: [&str; 12] = [
    "system",
    "upstream",
    "orange",
    "bark",
    "sage",
    "olive",
    "viridian",
    "prussiangreen",
    "blue",
    "purple",
    "magenta",
    "red",
];

const HIGHLIGHTS: [&str; 2] = ["outline", "fill"];

mod imp {
    use super::*;

    #[derive(Debug, gtk::CompositeTemplate)]
    #[template(resource = "/org/gnome/papers/ui/preferences-dialog.ui")]
    pub struct PpsPreferencesDialog {
        #[template_child]
        pub(super) highlight_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) accent_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) zoom_overlay_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) keep_expanded_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) new_tab_row: TemplateChild<adw::SwitchRow>,
        pub(super) settings: gio::Settings,
    }

    impl Default for PpsPreferencesDialog {
        fn default() -> Self {
            Self {
                highlight_row: Default::default(),
                accent_row: Default::default(),
                zoom_overlay_row: Default::default(),
                keep_expanded_row: Default::default(),
                new_tab_row: Default::default(),
                settings: gio::Settings::new("org.gnome.Papers"),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PpsPreferencesDialog {
        const NAME: &'static str = "PpsPreferencesDialog";
        type Type = super::PpsPreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PpsPreferencesDialog {
        fn constructed(&self) {
            self.parent_constructed();

            let s = &self.settings;

            // Combo rows carry an index, so the string settings are mapped to
            // and from a position rather than bound directly.
            let style = s.string("highlight-style");
            self.highlight_row.set_selected(
                HIGHLIGHTS.iter().position(|h| *h == style).unwrap_or(0) as u32,
            );
            self.highlight_row.connect_selected_notify(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |row| {
                    let idx = row.selected() as usize;
                    let _ = obj
                        .settings
                        .set_string("highlight-style", HIGHLIGHTS.get(idx).copied().unwrap_or("outline"));
                }
            ));

            let accent = s.string("accent-source");
            self.accent_row
                .set_selected(ACCENTS.iter().position(|a| *a == accent).unwrap_or(0) as u32);
            self.accent_row.connect_selected_notify(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |row| {
                    let idx = row.selected() as usize;
                    let _ = obj
                        .settings
                        .set_string("accent-source", ACCENTS.get(idx).copied().unwrap_or("system"));
                }
            ));

            s.bind("show-zoom-overlay", &*self.zoom_overlay_row, "active")
                .build();
            s.bind("keep-outline-expanded", &*self.keep_expanded_row, "active")
                .build();
            s.bind("open-in-new-tab", &*self.new_tab_row, "active")
                .build();
        }
    }

    impl WidgetImpl for PpsPreferencesDialog {}
    impl AdwDialogImpl for PpsPreferencesDialog {}
    impl PreferencesDialogImpl for PpsPreferencesDialog {}
}

glib::wrapper! {
    pub struct PpsPreferencesDialog(ObjectSubclass<imp::PpsPreferencesDialog>)
        @extends adw::PreferencesDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PpsPreferencesDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for PpsPreferencesDialog {
    fn default() -> Self {
        Self::new()
    }
}
