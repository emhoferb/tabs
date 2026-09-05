use crate::deps::*;
use crate::window::WindowRunMode;

use std::cell::Cell;
use std::collections::VecDeque;
use std::path::PathBuf;

use glib::translate::{FromGlib, IntoGlib};
use glib::{UserDirectory, Variant, VariantTy};
use gtk::{PrintOperationResult, TextDirection};

use papers_document::{
    Annotation, AnnotationMarkup, AnnotationText, AnnotationTextMarkup, Attachment,
    DocumentAnnotations, DocumentForms, Link, LinkAction, LinkActionType, LinkDest, Outlines,
};
use papers_view::{History, PageLayout, SizingMode};

use crate::application;

/// Action handling of this widget
mod actions;
mod enums;
mod io;
mod print;
/// Signal handling callbacks of this widget
mod signals;

const GS_LOCKDOWN_PRINT_SETUP: &str = "disable-print-setup";
const GS_LOCKDOWN_PRINT: &str = "disable-printing";
const GS_LOCKDOWN_SAVE: &str = "disable-save-to-disk";

const GS_OVERRIDE_RESTRICTIONS: &str = "override-restrictions";
const GS_PAGE_CACHE_SIZE: &str = "page-cache-size";
const GS_LAST_DOCUMENT_DIRECTORY: &str = "document-directory";
const GS_LAST_PICTURES_DIRECTORY: &str = "pictures-directory";
const GS_ALLOW_LINKS_CHANGE_ZOOM: &str = "allow-links-change-zoom";

/// Bounds a dragged sidebar is held within, in pixels. These match the values
/// the template gives the split view.
const MIN_SIDEBAR: f64 = 150.;
const MAX_SIDEBAR: f64 = 650.;

/// The rate at which the sidebar narrows once the window is too small to hold
/// it at its stored width — libadwaita's default, so the slope is upstream's.
const BASE_FRACTION: f64 = 0.25;

mod imp {
    use super::*;

    #[derive(Properties, Default, Debug, CompositeTemplate)]
    #[properties(wrapper_type = super::PpsDocumentView)]
    #[template(resource = "/org/gnome/papers/ui/document-view.ui")]
    pub struct PpsDocumentView {
        #[template_child]
        pub(super) model: TemplateChild<DocumentModel>,
        #[template_child]
        pub(super) view: TemplateChild<papers_view::View>,
        #[template_child]
        pub(super) view_popup: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub(super) split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub(super) page_selector: TemplateChild<PpsPageSelector>,
        #[template_child]
        pub(super) zoom_fit_best_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(super) zoom_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) zoom_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) tab_bar: TemplateChild<adw::TabBar>,
        #[template_child]
        pub(super) zoom_fit_menu: TemplateChild<gio::Menu>,
        #[template_child]
        pub(super) sidebar_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) narrow_breakpoint: TemplateChild<adw::Breakpoint>,
        #[template_child]
        pub(super) very_narrow_breakpoint: TemplateChild<adw::Breakpoint>,
        /// Whether the narrow-window breakpoints are currently installed.
        pub(super) breakpoints_installed: Cell<bool>,
        /// Whether a sidebar resize drag is in progress.
        pub(super) sidebar_dragging: Cell<bool>,
        /// The customizable document-scope shortcuts, replaced on rebind.
        pub(super) custom_shortcuts: RefCell<Option<gtk::ShortcutController>>,
        #[template_child]
        pub(super) banner: TemplateChild<adw::Banner>,

        #[template_child]
        pub(super) annot_menu: TemplateChild<gio::Menu>,
        #[template_child]
        pub(super) annot_menu_child: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) annot_toolbar: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) zoom_toolbar: TemplateChild<gtk::Box>,

        // Ink and text tools
        #[template_child]
        pub(super) eraser_settings_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) eraser_radius_popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub(super) pencil_stroke_popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub(super) pencil_color_popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub(super) highlight_stroke_popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub(super) highlight_color_popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub(super) text_font_popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub(super) text_color_popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub(super) draw_revealer: TemplateChild<gtk::Revealer>,

        // sidebar
        #[template_child]
        pub(super) sidebar: TemplateChild<PpsSidebar>,
        #[template_child]
        pub(super) sidebar_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) find_sidebar: TemplateChild<PpsFindSidebar>,
        #[template_child]
        pub(super) sidebar_layers: TemplateChild<PpsSidebarLayers>,
        #[template_child]
        pub(super) sidebar_annots: TemplateChild<PpsSidebarAnnotations>,
        #[template_child]
        pub(super) sidebar_links: TemplateChild<PpsSidebarLinks>,
        #[template_child]
        pub(super) sidebar_attachments: TemplateChild<PpsSidebarAttachments>,
        #[template_child]
        pub(super) attachment_context: TemplateChild<papers_view::AttachmentContext>,

        #[template_child]
        pub(super) header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) action_menu_button: TemplateChild<gtk::MenuButton>,

        #[template_child]
        pub(super) error_alert: TemplateChild<adw::AlertDialog>,
        #[template_child]
        pub(super) print_cancel_alert: TemplateChild<adw::AlertDialog>,
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,

        // Settings
        #[template_child]
        pub(super) settings: TemplateChild<gio::Settings>,
        #[template_child]
        pub(super) default_settings: TemplateChild<gio::Settings>,
        pub(super) lockdown_settings: RefCell<Option<gio::Settings>>,

        #[template_child]
        pub(super) history: TemplateChild<History>,

        #[template_child]
        pub(super) document_action_group: TemplateChild<gio::SimpleActionGroup>,

        #[template_child]
        pub(super) document_toolbar_view: TemplateChild<adw::ToolbarView>,

        pub(super) message_area: RefCell<Option<PpsProgressMessageArea>>,

        #[template_child]
        pub(super) signature_banner: TemplateChild<adw::Banner>,

        #[property(get, set)]
        pub(super) stub: std::cell::Cell<bool>,
        pub(super) mode: Cell<WindowRunMode>,

        pub(super) display_name: RefCell<String>,
        pub(super) edit_name: RefCell<String>,
        pub(super) metadata: RefCell<Option<papers_view::Metadata>>,

        pub(super) link: RefCell<Option<papers_document::Link>>,
        pub(super) image: RefCell<Option<papers_document::Image>>,
        pub(super) annot: RefCell<Option<papers_document::Annotation>>,
        pub(super) attachment: RefCell<Option<papers_document::Attachment>>,

        // Misc Runtime State
        pub(super) sidebar_was_open_before_find: Cell<bool>,
        pub(super) sidebar_was_open_before_collapsed: Cell<bool>,
        pub(super) close_after_save: Cell<bool>,
        pub(super) close_after_print: Cell<bool>,
        pub(super) modified: Cell<bool>,
        pub(super) progress_cancellable: RefCell<Option<gio::Cancellable>>,

        // Signal handlers
        pub(super) modified_handler_id: RefCell<Option<SignalHandlerId>>,

        // Loaders
        pub(super) file: RefCell<Option<gio::File>>,

        // Print queue
        pub(super) print_queue: RefCell<VecDeque<papers_view::PrintOperation>>,

        // Search
        pub(super) search_context: RefCell<Option<papers_view::SearchContext>>,

        // Undo
        #[template_child]
        pub(super) undo_context: TemplateChild<papers_view::UndoContext>,

        // Annotations
        #[template_child]
        pub(super) annots_context: TemplateChild<papers_view::AnnotationsContext>,
        pub(super) deletion_toast: RefCell<Option<adw::Toast>>,
        #[template_child]
        pub(super) draw_button: TemplateChild<gtk::Button>,

        // Signature
        #[template_child]
        pub(super) rect_small_alert: TemplateChild<adw::AlertDialog>,
        pub(super) signature: RefCell<Option<papers_document::Signature>>,
        pub(super) certificate_info: RefCell<Option<papers_document::CertificateInfo>>,
        pub(super) signature_page: Cell<u32>,
        pub(super) signature_bounding_box: RefCell<Option<papers_document::Rectangle>>,

        // Job
        pub(super) save_job: RefCell<Option<papers_view::JobSave>>,
        pub(super) save_job_handler: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PpsDocumentView {
        const NAME: &'static str = "PpsDocumentView";
        type Type = super::PpsDocumentView;
        type ParentType = adw::BreakpointBin;

        fn class_init(klass: &mut Self::Class) {
            // for drop target support
            gdk::FileList::ensure_type();

            gio::Settings::ensure_type();

            papers_view::View::ensure_type();
            papers_view::ViewPresentation::ensure_type();

            PpsFindSidebar::ensure_type();
            PpsPageSelector::ensure_type();
            PpsSearchBox::ensure_type();
            PpsSidebar::ensure_type();
            PpsSidebarAnnotations::ensure_type();
            PpsSidebarAttachments::ensure_type();
            PpsSidebarLayers::ensure_type();
            PpsSidebarLinks::ensure_type();
            PpsSidebarThumbnails::ensure_type();

            crate::simple_color_swatch::SimpleColorSwatch::ensure_type();

            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
            unsafe {
                obj.as_ref().imp().setup_actions();

                // These are only enabled once the user takes action
                obj.as_ref().imp().set_action_enabled("find-next", false);
                obj.as_ref()
                    .imp()
                    .set_action_enabled("find-previous", false);
                obj.as_ref().imp().set_action_enabled("copy", false);
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PpsDocumentView {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("visibility-changed").run_last().build(),
                    Signal::builder("update-visibility").run_last().build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.setup_zoom_menu();
            self.setup_sidebar_resizer();
            self.setup_custom_shortcuts();
            self.sidebar_was_open_before_find.set(true);

            let page_cache_mb = self.settings.uint(GS_PAGE_CACHE_SIZE) as usize;
            self.view.set_page_cache_size(page_cache_mb * 1024 * 1024);

            let allow_links_change_zoom = self.settings.boolean(GS_ALLOW_LINKS_CHANGE_ZOOM);
            self.view
                .set_allow_links_change_zoom(allow_links_change_zoom);

            self.view.set_model(&self.model);
            self.view
                .set_annotations_context(&self.annots_context.get());

            self.default_settings.delay();
            self.setup_default();

            // setup the settings
            self.allow_links_change_zoom_changed("", &self.settings);
            self.page_cache_size_changed("", &self.settings);

            self.model.notify("sizing-mode");

            self.document_action_group
                .lookup_action("find")
                .unwrap()
                .bind_property("enabled", &self.find_sidebar.get(), "visible")
                .sync_create()
                .build();

            let search_context = papers_view::SearchContext::new(&self.model);

            search_context.connect_result_activated(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.sidebar_navigate_to_view();
                }
            ));

            self.setup_lockdown();

            self.find_sidebar.set_search_context(Some(&search_context));
            self.view.set_search_context(&search_context);
            self.search_context.replace(Some(search_context));

            // Auto-dismiss deletion toast when any undoable action is added
            self.undo_context.connect_stacks_changed(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |_context, _handler| {
                    // Dismiss any existing deletion toast when a new action is added
                    let toast = obj.deletion_toast.borrow_mut().take();
                    if let Some(toast) = toast {
                        toast.dismiss();
                    }
                }
            ));

            let eraser_settings_popover = self.eraser_settings_button.popover().unwrap();
            eraser_settings_popover.set_has_arrow(false);
            eraser_settings_popover.set_offset(0, -8);
            self.eraser_radius_popover.set_offset(0, -8);
            self.pencil_stroke_popover.set_offset(0, -8);
            self.pencil_color_popover.set_offset(0, -8);
            self.highlight_stroke_popover.set_offset(0, -8);
            self.highlight_color_popover.set_offset(0, -8);
            self.text_font_popover.set_offset(0, -8);
            self.text_color_popover.set_offset(0, -8);
        }
    }

    impl BreakpointBinImpl for PpsDocumentView {}

    impl WidgetImpl for PpsDocumentView {}

    impl PpsDocumentView {
        /// Sidebar resizing.
        ///
        /// The controllers go on the split view, not on a strip inside the
        /// sidebar, for two reasons.
        ///
        /// A widget inside the sidebar moves as the sidebar is resized, so the
        /// pointer's position *relative to it* changes even while the pointer
        /// is still. Feeding that back in as a drag offset makes the width
        /// oscillate: the wobble seen while dragging. The split view does not
        /// move, so a width taken from the pointer's position within it is
        /// stable — which is how GtkPaned, and therefore Evince, stays smooth.
        ///
        /// It also frees the grab zone from the sidebar's bounds, so it can
        /// reach well into the document side where there is room, while barely
        /// intruding on the sidebar's own last column.
        /// The customizable doc-scope shortcuts (sidebar, outline,
        /// duplicate-tab). Window-scope ones are application accels; these
        /// need a widget with the "doc" group in scope, which is this view.
        fn setup_custom_shortcuts(&self) {
            let install = glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move || {
                    if let Some(old) = obj.custom_shortcuts.take() {
                        obj.obj().remove_controller(&old);
                    }

                    let controller = crate::shortcuts::document_controller(&obj.settings);
                    obj.obj().add_controller(controller.clone());
                    obj.custom_shortcuts.replace(Some(controller));
                }
            );

            install();

            self.settings.connect_changed(
                Some(crate::shortcuts::SETTINGS_KEY),
                move |_, _| install(),
            );
        }

        fn setup_sidebar_resizer(&self) {
            // Asymmetric on purpose: a pointer arrow is drawn down and to the
            // right of its hotspot, so coming from the document side the cursor
            // visually overlaps the edge long before the hotspot reaches it.
            const GRAB_INSIDE: f64 = 2.0;
            const GRAB_OUTSIDE: f64 = 11.0;

            let in_zone = glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[upgrade_or]
                false,
                move |x: f64| {
                    let edge = obj.sidebar_box.width() as f64;

                    !obj.split_view.is_collapsed()
                        && obj.split_view.shows_sidebar()
                        && x > edge - GRAB_INSIDE
                        && x < edge + GRAB_OUTSIDE
                }
            );

            // Cursor feedback.
            let motion = gtk::EventControllerMotion::new();
            motion.connect_motion(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[strong]
                in_zone,
                move |_, x, _| {
                    obj.split_view.set_cursor_from_name(if in_zone(x) {
                        Some("col-resize")
                    } else {
                        None
                    });
                }
            ));
            self.split_view.add_controller(motion);

            let drag = gtk::GestureDrag::new();
            drag.set_propagation_phase(gtk::PropagationPhase::Capture);

            drag.connect_drag_begin(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[strong]
                in_zone,
                move |gesture, x, _| {
                    if in_zone(x) {
                        obj.sidebar_dragging.set(true);
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                    } else {
                        obj.sidebar_dragging.set(false);
                        gesture.set_state(gtk::EventSequenceState::Denied);
                    }
                }
            ));

            drag.connect_drag_update(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |gesture, offset_x, _| {
                    if !obj.sidebar_dragging.get() {
                        return;
                    }

                    if let Some((start_x, _)) = gesture.start_point() {
                        // Absolute pointer position inside the split view: it
                        // does not move, so there is no feedback loop.
                        obj.set_sidebar_width_px(start_x + offset_x);
                    }
                }
            ));

            drag.connect_drag_end(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |gesture, offset_x, _| {
                    if !obj.sidebar_dragging.replace(false) {
                        return;
                    }

                    let Some((start_x, _)) = gesture.start_point() else {
                        return;
                    };

                    // Saved from the pointer rather than from the sidebar's
                    // allocation, which may not have caught up with this last
                    // update yet.
                    let width = obj.set_sidebar_width_px(start_x + offset_x);

                    // Records the new ceiling and slope, which every open tab
                    // picks up through the settings; then lift the pin this
                    // tab is still holding from the drag.
                    obj.save_sidebar_width(width);
                    obj.apply_sidebar_width();
                }
            ));
            self.split_view.add_controller(drag);

            // Applied straight away, not from a tick callback: both values
            // are plain properties, so nothing here needs the split view to
            // have been allocated. That matters most for a tab opened into an
            // already-running window, which used to measure a layout still in
            // flux and settle on its own width.
            self.apply_sidebar_width();

            // Follow later changes so every open tab tracks the same width.
            for key in ["sidebar-width", "sidebar-width-fraction"] {
                self.settings.connect_changed(
                    Some(key),
                    glib::clone!(
                        #[weak(rename_to = obj)]
                        self,
                        move |_, _| obj.apply_sidebar_width()
                    ),
                );
            }

            self.breakpoints_installed.set(true);
            self.apply_collapse_setting();
            self.settings.connect_changed(
                Some("collapse-sidebar-when-narrow"),
                glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |_, _| obj.apply_collapse_setting()
                ),
            );
        }

        /// Install or remove the narrow-window breakpoints. AdwBreakpointBin has
        /// no way to disable a breakpoint, so honouring the preference means
        /// taking them out of the bin and putting them back.
        fn apply_collapse_setting(&self) {
            let want = self.settings.boolean("collapse-sidebar-when-narrow");

            if want == self.breakpoints_installed.get() {
                return;
            }

            let obj = self.obj();

            if want {
                obj.add_breakpoint(self.narrow_breakpoint.clone());
                obj.add_breakpoint(self.very_narrow_breakpoint.clone());
            } else {
                obj.remove_breakpoint(&*self.narrow_breakpoint);
                obj.remove_breakpoint(&*self.very_narrow_breakpoint);
                self.split_view.set_collapsed(false);
            }

            self.breakpoints_installed.set(want);
        }

        /// Give the split view the stored width, as a ceiling rather than a
        /// target.
        ///
        /// The split view allocates `clamp(fraction * total, min, max)`. Upstream
        /// leaves the fraction at its default, so the sidebar is purely
        /// proportional and a wide window gets an absurdly wide sidebar. Feeding
        /// the dragged width in as `max` instead pins it: past
        /// `width / fraction` the window can grow without the sidebar following,
        /// and below that threshold the fraction takes over and the sidebar
        /// gives ground as the window narrows — the behaviour Evince had.
        pub(super) fn apply_sidebar_width(&self) {
            let width = (self.settings.int("sidebar-width") as f64).clamp(MIN_SIDEBAR, MAX_SIDEBAR);
            let fraction = self
                .settings
                .double("sidebar-width-fraction")
                .clamp(BASE_FRACTION, 1.);

            self.split_view.set_min_sidebar_width(MIN_SIDEBAR);
            self.split_view.set_max_sidebar_width(width);
            self.split_view.set_sidebar_width_fraction(fraction);
        }

        /// Pin the sidebar to an exact pixel width.
        ///
        /// Setting only the fraction leaves the final width to rounding inside
        /// the split view, which is another source of jitter while dragging.
        /// Clamping min and max to the same value makes it exact.
        fn set_sidebar_width_px(&self, width: f64) -> f64 {
            let width = width.clamp(MIN_SIDEBAR, MAX_SIDEBAR).round();
            let total = self.split_view.width();
            if total <= 0 {
                return width;
            }

            self.split_view.set_min_sidebar_width(width);
            self.split_view.set_max_sidebar_width(width);
            self.split_view
                .set_sidebar_width_fraction(width / total as f64);

            width
        }

        /// Store the dragged width, plus the slope to shrink along.
        ///
        /// The fraction is normally the default, so the sidebar narrows at the
        /// same rate as upstream's; it is only raised when the window is too
        /// narrow for the default fraction to have reached the requested width,
        /// since a ceiling the fraction can never reach would make the drag
        /// itself impossible.
        ///
        /// Keeping the fraction here rather than deriving it per tab is what
        /// makes a new tab match: it never has to measure anything.
        pub(super) fn save_sidebar_width(&self, width: f64) {
            let total = self.split_view.width();

            let fraction = if total > 0 {
                (width / total as f64).clamp(BASE_FRACTION, 1.)
            } else {
                BASE_FRACTION
            };

            // Fraction first: the width is the value both handlers key off, so
            // writing it last means the pair is never briefly mismatched in a
            // way that outlives the frame.
            let _ = self.settings.set_double("sidebar-width-fraction", fraction);
            let _ = self.settings.set_int("sidebar-width", width as i32);
        }

        /// Assemble the zoom menu: the fit modes come from the template, the
        /// percentage presets are built here because Blueprint cannot express
        /// a double-typed action target.
        fn setup_zoom_menu(&self) {
            const PRESETS: [(&str, f64); 10] = [
                ("50%", 50.),
                ("70%", 70.),
                ("85%", 85.),
                ("100%", 100.),
                ("125%", 125.),
                ("150%", 150.),
                ("175%", 175.),
                ("200%", 200.),
                ("300%", 300.),
                ("400%", 400.),
            ];

            let menu = gio::Menu::new();
            menu.append_section(None, &*self.zoom_fit_menu);

            let presets = gio::Menu::new();
            for (label, percent) in PRESETS {
                let item = gio::MenuItem::new(Some(label), None);
                item.set_action_and_target_value(
                    Some("doc.zoom-percent"),
                    Some(&percent.to_variant()),
                );
                presets.append_item(&item);
            }
            menu.append_section(None, &presets);

            self.zoom_button.set_menu_model(Some(&menu));
        }

        pub(super) fn file(&self) -> Option<gio::File> {
            self.file.borrow().clone()
        }

        pub(super) fn lockdown_settings(&self) -> Option<gio::Settings> {
            self.lockdown_settings.borrow().clone()
        }

        pub(super) fn warning_message(&self, msg: &str) {
            let toast = adw::Toast::builder().timeout(20).title(msg).build();

            self.toast_overlay.add_toast(toast);
        }

        pub(super) fn error_message(&self, error: Option<&glib::Error>, msg: &str) {
            let toast = adw::Toast::builder().timeout(20).title(msg).build();

            if let Some(error) = error {
                toast.set_button_label(Some(&gettext("View Details")));

                toast.connect_button_clicked(glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |_| obj.error_alert.present(Some(obj.obj().as_ref()))
                ));

                self.error_alert.set_heading(Some(msg));
                self.error_alert.set_body(error.message());
            }

            self.toast_overlay.add_toast(toast);
        }

        pub(super) fn set_caret_navigation_enabled(&self, enabled: bool) {
            self.metadata_and_then(|m| {
                m.set_boolean("caret-navigation", enabled);
            });

            self.view.set_caret_navigation_enabled(enabled);
        }

        pub(super) fn handle_link(&self, dest: &papers_document::LinkDest) {
            let action = LinkAction::new_dest(dest);
            let link = Link::new(None, &action);

            self.view.handle_link(&link);
        }

        pub(super) fn is_empty(&self) -> bool {
            self.document().is_none()
        }

        fn save_settings(&self) {
            use glib::translate::IntoGlib;

            let _ = self
                .default_settings
                .set_boolean("continuous", self.model.is_continuous());
            let _ = self
                .default_settings
                .set_boolean("dual-page", self.model.page_layout() == PageLayout::Dual);
            let _ = self.default_settings.set_boolean(
                "dual-page-odd-left",
                self.model.is_dual_page_odd_pages_left(),
            );

            let sizing_mode = self.model.sizing_mode();
            let _ = self
                .default_settings
                .set_enum("sizing-mode", sizing_mode.into_glib());
            if sizing_mode == SizingMode::Free {
                let zoom = self.model.scale();
                let dpi = Document::misc_get_widget_dpi(self.obj().as_ref());
                let _ = self.default_settings.set_double("zoom", zoom * 72. / dpi);
            }

            let _ = self
                .default_settings
                .set_boolean("show-sidebar", self.split_view.shows_sidebar());

            let annot_keys = vec![
                "annot-color",
                "pen-color",
                "pen-stroke",
                "highlight-stroke",
                "highlight-color",
                "text-color",
                "eraser-mode-objects",
            ];

            for key in annot_keys {
                let action_value = self.document_action_group.action_state(key).unwrap();

                let _ = self.default_settings.set_value(key, &action_value);
            }

            if cfg!(feature = "spell-check") {
                let _ = self
                    .default_settings
                    .set_boolean("enable-spellchecking", self.view.enables_spellchecking());
            }

            self.default_settings.apply();
        }

        // metadata
        pub(super) fn metadata(&self) -> Option<papers_view::Metadata> {
            self.metadata.borrow().clone()
        }

        pub(super) fn metadata_and_then<F>(&self, f: F)
        where
            F: FnOnce(papers_view::Metadata),
        {
            if let Some(metadata) = self.metadata() {
                if !self.is_empty() {
                    f(metadata);
                }
            }
        }

        pub(super) fn setup_sidebar(&self) {
            // Use BIND_GET_NO_CHANGES so that when we have several DocumentViews
            // One instance sidebar does not get updated when the other one applies
            // the settings

            self.default_settings
                .bind("sidebar-page", &self.sidebar.get(), "visible-child-name")
                .get_no_changes()
                .build();
        }

        pub(super) fn setup_document(&self) {
            // Make sure to not open a document on the last page,
            // since closing it on the last page most likely means the
            // user was finished reading the document. In that case, reopening should
            // show the first page.
            let page = self.model.page();
            let n_pages = self.document().unwrap().n_pages();
            if page == n_pages - 1 {
                self.model.set_page(0);
            }

            if n_pages == 1 {
                self.model.set_page_layout(PageLayout::Single);
            } else if n_pages == 2 {
                self.model.set_dual_page_odd_pages_left(true);
            }

            if self
                .document()
                .unwrap()
                .dynamic_cast_ref::<DocumentAnnotations>()
                .is_some_and(|d| d.can_add_annotation())
            {
                // Not unconditionally visible: the floating controls may be
                // switched off, and this runs after that setting was applied,
                // so it used to resurrect the annotation button on restart.
                self.annot_toolbar
                    .set_visible(self.settings.boolean("show-zoom-overlay"));
                self.zoom_toolbar.set_margin_bottom(70);
            } else {
                self.annot_toolbar.set_visible(false);
                self.zoom_toolbar
                    .set_margin_bottom(self.annot_toolbar.margin_bottom());
            }
        }

        pub(super) fn setup_view_from_metadata(&self) {
            let Some(metadata) = self.metadata() else {
                return;
            };

            // caret navigation mode
            if self.view.supports_caret_navigation() {
                if let Some(caret_position) = metadata.string("caret-position") {
                    if let Some((page, offset)) =
                        Variant::parse(VariantTy::new("(uu)").ok(), &caret_position)
                            .ok()
                            .and_then(|v| v.get::<(u32, u32)>())
                    {
                        self.view.set_caret_cursor_position(page, offset);
                    }
                }

                if let Some(caret_navigation) = metadata.boolean("caret-navigation") {
                    self.view.set_caret_navigation_enabled(caret_navigation);
                }
            }
        }

        pub(super) fn setup_model_from_metadata(&self) {
            let Some(metadata) = self.metadata() else {
                return;
            };

            // Current page
            if let Some(page) = metadata.int("page") {
                self.model.set_page(page);
            }

            // Sizing mode
            if let Some(sizing_mode) = metadata.string("sizing-mode") {
                let enum_class = glib::EnumClass::new::<SizingMode>();
                let mode = enum_class.value_by_nick(&sizing_mode).unwrap().value();
                let value = unsafe { SizingMode::from_glib(mode) };

                self.model.set_sizing_mode(value);
            }

            // Zoom
            if self.model.sizing_mode() == SizingMode::Free {
                if let Some(zoom) = metadata.double("zoom") {
                    let dpi = Document::misc_get_widget_dpi(self.obj().as_ref());
                    let zoom = zoom * dpi / 72.;
                    self.model.set_scale(zoom);
                }
            }

            // Rotation
            if let Some(rotation) = metadata.int("rotation") {
                let rotation = match rotation {
                    90 | 180 | 270 => rotation,
                    _ => 0,
                };

                self.model.set_rotation(rotation);
            }

            // Continuous
            if let Some(continuous) = metadata.boolean("continuous") {
                self.model.set_continuous(continuous);
            }

            // Fullscreen
            if let Some(fullscreen) = metadata.boolean("fullscreen") {
                self.parent_window()
                    .dynamic_cast::<gio::ActionGroup>()
                    .unwrap()
                    .change_action_state("fullscreen", &fullscreen.into());
            }

            // Dual page
            if let Some(dual_page) = metadata.boolean("dual-page") {
                let page_layout = if dual_page {
                    PageLayout::Dual
                } else {
                    PageLayout::Single
                };

                self.model.set_page_layout(page_layout);
            }

            // Dual page odd pages left
            if let Some(dual_page_odd_left) = metadata.boolean("dual-page-odd-left") {
                self.model.set_dual_page_odd_pages_left(dual_page_odd_left);
            }

            // Right to left document
            if let Some(rtl) = metadata.boolean("rtl") {
                self.model.set_rtl(rtl);
            }
        }

        pub(super) fn document(&self) -> Option<Document> {
            self.model.document()
        }

        pub(super) fn show_find_bar(&self) {
            if self
                .split_view
                .sidebar()
                .is_some_and(|bar| bar == self.find_sidebar.clone().upcast::<gtk::Widget>())
            {
                self.find_sidebar.grab_focus();
                return;
            }

            if !self
                .document()
                .map(|d| d.is::<papers_document::DocumentFind>())
                .unwrap_or_default()
            {
                glib::g_error!(
                    "",
                    "Find action should be insensitive since document doesn't support find"
                );
                return;
            }

            if !self.split_view.is_collapsed() {
                let show_sidebar = self
                    .document_action_group
                    .action_state("show-sidebar")
                    .unwrap()
                    .get::<bool>()
                    .unwrap();

                self.sidebar_was_open_before_find.set(show_sidebar);
            } else {
                self.sidebar_was_open_before_find
                    .set(self.sidebar_was_open_before_collapsed.get());
            }

            self.history.freeze();

            self.sidebar_stack
                .set_visible_child(&self.find_sidebar.get());
            self.document_action_group
                .change_action_state("show-sidebar", &true.into());
            self.find_sidebar.grab_focus();
            self.set_action_enabled("find-next", true);
            self.set_action_enabled("find-previous", true);
        }

        pub(super) fn find_restart(&self) {
            self.find_sidebar.restart();
        }

        pub(super) fn close_find_bar(&self) {
            if self
                .sidebar_stack
                .visible_child()
                .is_some_and(|bar| bar != self.find_sidebar.clone().upcast::<gtk::Widget>())
            {
                return;
            }

            if !self.split_view.is_collapsed() {
                self.split_view
                    .set_show_sidebar(self.sidebar_was_open_before_find.get());
            }

            self.sidebar_was_open_before_find.set(true);

            self.sidebar_stack.set_visible_child(&self.sidebar.get());

            self.set_action_enabled("find-next", false);
            self.set_action_enabled("find-previous", false);
            self.history.thaw();
        }

        fn setup_default(&self) {
            let settings = self.default_settings.get();

            // sidebar
            let show_sidebar = settings.boolean("show-sidebar");

            if self.split_view.is_collapsed() {
                self.sidebar_was_open_before_collapsed.set(show_sidebar);
                self.document_action_group
                    .change_action_state("show-sidebar", &false.into());
            } else {
                self.document_action_group
                    .change_action_state("show-sidebar", &show_sidebar.into());
            }
            let annot_keys = vec![
                "annot-color",
                "pen-color",
                "pen-stroke",
                "highlight-stroke",
                "highlight-color",
                "text-color",
                "eraser-mode-objects",
            ];

            for key in annot_keys {
                let setting_value = settings.value(key);
                self.document_action_group
                    .change_action_state(key, &setting_value);
            }

            // document model
            self.model.set_continuous(settings.boolean("continuous"));
            self.model
                .set_page_layout(if settings.boolean("dual-page") {
                    PageLayout::Dual
                } else {
                    PageLayout::Single
                });
            self.model
                .set_dual_page_odd_pages_left(settings.boolean("dual-page-odd-left"));
            self.model
                .set_rtl(gtk::Widget::default_direction() == TextDirection::Rtl);

            let sizing_mode = unsafe { SizingMode::from_glib(settings.enum_("sizing-mode")) };

            self.model.set_sizing_mode(sizing_mode);

            if sizing_mode == SizingMode::Free {
                self.model.set_scale(settings.double("zoom"));
            }

            let spellchecking = if cfg!(feature = "spell-check") {
                settings.boolean("enable-spellchecking")
            } else {
                false
            };

            self.document_action_group
                .change_action_state("enable-spellchecking", &spellchecking.into());
        }

        fn settings_key_for_directory(dir: UserDirectory) -> String {
            match dir {
                UserDirectory::Pictures => GS_LAST_PICTURES_DIRECTORY,
                _ => GS_LAST_DOCUMENT_DIRECTORY,
            }
            .into()
        }

        pub(super) fn file_dialog_restore_folder(
            &self,
            dialog: &gtk::FileDialog,
            dir: UserDirectory,
        ) {
            let settings = self.settings.get();
            let key = Self::settings_key_for_directory(dir);

            let folder = settings.get::<Option<String>>(&key).map(PathBuf::from);
            let folder = folder
                .or_else(|| glib::user_special_dir(dir))
                .unwrap_or_else(glib::home_dir);

            dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
        }

        pub(super) fn file_dialog_save_folder(&self, file: Option<&gio::File>, dir: UserDirectory) {
            let folder = file.and_then(|f| f.parent());

            // store 'nothing' if the folder is the default one
            let path = folder
                .filter(|f| f.path() != glib::user_special_dir(dir))
                .and_then(|f| f.path())
                .and_then(|path| path.into_os_string().into_string().ok());

            let settings = self.settings.get();
            let key = Self::settings_key_for_directory(dir);

            settings
                .set(&key, path)
                .expect("Failed to save folder path");
        }

        pub(super) fn check_document_modified(&self) -> bool {
            let Some(document) = self.document() else {
                return false;
            };

            let forms_modified = document
                .dynamic_cast_ref::<papers_document::DocumentForms>()
                .map(|d| d.document_is_modified())
                .unwrap_or_default();
            let annots_modified = document
                .dynamic_cast_ref::<DocumentAnnotations>()
                .map(|d| d.document_is_modified())
                .unwrap_or_default();

            let secondary_text = match (forms_modified, annots_modified) {
                (true, true) => Some(gettext("Document contains new or modified annotations and form fields that have been filled out.")),
                (true, false) => Some(gettext("Document contains form fields that have been filled out.")),
                (false, true) => Some(gettext("Document contains new or modified annotations.")),
                (false, false) => None,
            };

            let Some(secondary_text) = secondary_text else {
                return false;
            };
            let secondary_text_command =
                gettext("If you don’t save a copy, changes will be permanently lost.");
            let text = gettext("Save Changes to a Copy?");

            let dialog = adw::AlertDialog::builder()
                .body(format!("{secondary_text} {secondary_text_command}"))
                .heading(text)
                .default_response("yes")
                .build();

            dialog.add_responses(&[
                ("no", &gettext("Close _Without Saving")),
                ("cancel", &gettext("_Cancel")),
                ("yes", &gettext("_Save a Copy")),
            ]);

            dialog.set_response_appearance("no", adw::ResponseAppearance::Destructive);
            dialog.set_response_appearance("yes", adw::ResponseAppearance::Suggested);

            dialog.connect_response(
                None,
                glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |_, response| {
                        match response {
                            "yes" => {
                                obj.close_after_save.set(true);
                                obj.save_as();
                            }
                            "no" => obj.parent_window().destroy(),
                            "cancel" => obj.close_after_save.set(false),
                            _ => (),
                        };
                    }
                ),
            );

            dialog.present(Some(self.obj().as_ref()));

            true
        }

        pub(super) fn close_handled(&self) -> glib::Propagation {
            self.save_settings();

            if let Some(id) = self.modified_handler_id.take() {
                self.document().unwrap().disconnect(id);
            }

            if self.check_document_modified() {
                return glib::Propagation::Stop;
            }

            if self.check_print_queue() {
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        }
    }
}

glib::wrapper! {
    pub struct PpsDocumentView(ObjectSubclass<imp::PpsDocumentView>)
        @extends gtk::Widget, adw::BreakpointBin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PpsDocumentView {
    /// Point this view's tab bar at the window's tab view.
    pub fn set_tab_view(&self, tab_view: &adw::TabView) {
        self.imp().tab_bar.set_view(Some(tab_view));
    }

    /// Show or hide the floating controls over the page: the zoom buttons and
    /// the annotation button alike, so the page can be read unobstructed.
    pub fn set_zoom_overlay_visible(&self, visible: bool) {
        let imp = self.imp();

        imp.zoom_toolbar.set_visible(visible);
        imp.annot_toolbar.set_visible(visible);
    }

    pub fn new() -> Self {
        glib::Object::builder()
            .property("application", gio::Application::default())
            .property("show-menubar", false)
            .build()
    }

    pub fn uri(&self) -> Option<String> {
        self.imp().file.borrow().as_ref().map(|f| f.uri().into())
    }

    pub fn is_empty(&self) -> bool {
        self.imp().is_empty()
    }

    pub fn print_range(&self, start: i32, end: i32) {
        self.imp().print_range(start, end)
    }

    pub fn metadata(&self) -> Option<papers_view::Metadata> {
        self.imp().metadata()
    }

    pub fn handle_annot_popup(&self, annot: &impl IsA<Annotation>) {
        self.imp().view_menu_annot_popup(Some(annot.as_ref()));
    }

    pub fn model(&self) -> DocumentModel {
        self.imp().model.clone()
    }

    pub fn set_fullscreen_mode(&self, fullscreened: bool) {
        self.imp()
            .header_bar
            .set_show_end_title_buttons(!fullscreened);
    }

    pub fn set_inverted_colors(&self, inverted: bool) {
        self.imp().model.set_inverted_colors(inverted);
    }

    pub fn close_handled(&self) -> glib::Propagation {
        self.imp().close_handled()
    }

    pub fn set_filenames(&self, display_name: &str, edit_name: &str) {
        self.imp().set_filenames(display_name, edit_name);
    }

    pub fn open_document(
        &self,
        document: &Document,
        metadata: Option<&papers_view::Metadata>,
        dest: Option<&LinkDest>,
        mode: WindowRunMode,
    ) {
        self.imp().open_document(document, metadata, dest, mode);
    }

    pub fn reload_document(&self, document: Option<&Document>) {
        self.imp().reload_document(document);
    }

    pub fn error_message(&self, error: Option<&glib::Error>, msg: &str) {
        self.imp().error_message(error, msg);
    }

    pub fn focus_view(&self) {
        self.imp().view.grab_focus();
    }

    pub fn open_annotation_editor(&self, annot: &papers_document::Annotation) {
        self.imp().view.open_annotation_editor(annot);
    }
}

impl Default for PpsDocumentView {
    fn default() -> Self {
        Self::new()
    }
}
