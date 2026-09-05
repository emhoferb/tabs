// SPDX-License-Identifier: GPL-3.0-or-later

//! Appearance and behaviour preferences.

use crate::deps::*;
use crate::shortcuts;
use crate::simple_color_swatch::SimpleColorSwatch;
use glib::translate::FromGlib;

extern "C" {
    /// libview's resolved accent: the desktop's, or the one inferred from the
    /// theme name. What the swatch for "Follow system" should show.
    fn pps_get_accent_color() -> i32;
}

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

/// The colour each ACCENTS entry applies, for the swatches in the combo row.
/// Mirrors named_accents in pps-colors.c: the Yaru names resolve to the
/// nearest libadwaita accent, so neighbours like sage and olive genuinely
/// produce the same colour - the swatches show what is actually applied.
fn accent_rgba(name: &str) -> gdk::RGBA {
    use adw::AccentColor::*;

    let accent = match name {
        "system" => unsafe { adw::AccentColor::from_glib(pps_get_accent_color()) },
        "orange" => Orange,
        "bark" => Yellow,
        "sage" | "olive" => Green,
        "viridian" | "prussiangreen" => Teal,
        "purple" => Purple,
        "magenta" => Pink,
        "red" => Red,
        _ => Blue, // "upstream", "blue"
    };

    accent.to_rgba()
}

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
        pub(super) collapse_sidebar_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) zoom_overlay_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) keep_expanded_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) new_tab_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) shortcuts_group: TemplateChild<adw::PreferencesGroup>,
        pub(super) settings: gio::Settings,
    }

    impl Default for PpsPreferencesDialog {
        fn default() -> Self {
            Self {
                highlight_row: Default::default(),
                accent_row: Default::default(),
                collapse_sidebar_row: Default::default(),
                zoom_overlay_row: Default::default(),
                keep_expanded_row: Default::default(),
                new_tab_row: Default::default(),
                shortcuts_group: Default::default(),
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

            s.bind(
                "collapse-sidebar-when-narrow",
                &*self.collapse_sidebar_row,
                "active",
            )
            .build();
            s.bind("show-zoom-overlay", &*self.zoom_overlay_row, "active")
                .build();
            s.bind("keep-outline-expanded", &*self.keep_expanded_row, "active")
                .build();
            s.bind("open-in-new-tab", &*self.new_tab_row, "active")
                .build();

            self.setup_accent_swatches();
            self.build_shortcut_rows();
        }
    }

    impl PpsPreferencesDialog {
        /// A coloured circle in front of every accent choice, in the popup
        /// and in the collapsed row alike.
        fn setup_accent_swatches(&self) {
            let factory = gtk::SignalListItemFactory::new();

            factory.connect_setup(|_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };

                let swatch = SimpleColorSwatch::new();
                swatch.set_size_request(16, 16);
                swatch.set_valign(gtk::Align::Center);

                let label = gtk::Label::new(None);
                label.set_xalign(0.);

                let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                hbox.append(&swatch);
                hbox.append(&label);

                item.set_child(Some(&hbox));
            });

            factory.connect_bind(|_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };
                let Some(hbox) = item.child() else { return };
                let Some(swatch) = hbox.first_child().and_downcast::<SimpleColorSwatch>() else {
                    return;
                };
                let Some(label) = swatch.next_sibling().and_downcast::<gtk::Label>() else {
                    return;
                };
                let Some(text) = item.item().and_downcast::<gtk::StringObject>() else {
                    return;
                };

                label.set_text(&text.string());

                let name = ACCENTS
                    .get(item.position() as usize)
                    .copied()
                    .unwrap_or("system");
                swatch.set_color(accent_rgba(name));
            });

            self.accent_row.set_factory(Some(&factory));
        }

        fn build_shortcut_rows(&self) {
            for entry in shortcuts::ENTRIES {
                let row = adw::ActionRow::builder()
                    .title(gettext(entry.title))
                    .activatable(true)
                    .build();

                let label = gtk::ShortcutLabel::new(&shortcuts::accel(&self.settings, entry));
                label.set_valign(gtk::Align::Center);
                row.add_suffix(&label);
                row.add_suffix(&gtk::Image::from_icon_name("document-edit-symbolic"));

                row.connect_activated(glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    #[weak]
                    label,
                    move |_| obj.capture_shortcut(entry, label.clone())
                ));

                self.shortcuts_group.add(&row);
            }
        }

        /// A small dialog that waits for one key combination and stores it.
        fn capture_shortcut(&self, entry: &'static shortcuts::Entry, label: gtk::ShortcutLabel) {
            let dialog = adw::AlertDialog::builder()
                .heading(gettext(entry.title))
                .body(gettext(
                    "Press the new shortcut. Backspace restores the default, Esc cancels.",
                ))
                .build();

            dialog.add_response("cancel", &gettext("Cancel"));
            dialog.set_close_response("cancel");

            let key = gtk::EventControllerKey::new();
            key.set_propagation_phase(gtk::PropagationPhase::Capture);

            key.connect_key_pressed(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[weak]
                dialog,
                #[weak]
                label,
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_, keyval, _, state| {
                    let mask = state & gtk::accelerator_get_default_mod_mask();

                    if mask.is_empty() && keyval == gdk::Key::Escape {
                        dialog.close();
                        return glib::Propagation::Stop;
                    }

                    if mask.is_empty() && keyval == gdk::Key::BackSpace {
                        shortcuts::set_accel(&obj.settings, entry.id, None);
                        label.set_accelerator(entry.default);
                        dialog.close();
                        return glib::Propagation::Stop;
                    }

                    // Lone modifiers and un-bindable plain keys fall through
                    // here until a full chord arrives.
                    if !gtk::accelerator_valid(keyval, mask) {
                        return glib::Propagation::Stop;
                    }

                    if let Some(other) =
                        shortcuts::conflicting_entry(&obj.settings, keyval, mask, entry.id)
                    {
                        dialog.set_body(&format!(
                            "{} \u{201c}{}\u{201d}",
                            gettext("Already taken by"),
                            gettext(other.title)
                        ));
                        return glib::Propagation::Stop;
                    }

                    let accel = gtk::accelerator_name(keyval, mask);
                    shortcuts::set_accel(&obj.settings, entry.id, Some(&accel));
                    label.set_accelerator(&accel);
                    dialog.close();

                    glib::Propagation::Stop
                }
            ));

            dialog.add_controller(key);
            dialog.present(Some(self.obj().as_ref()));
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
