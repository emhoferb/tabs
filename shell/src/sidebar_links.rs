use crate::deps::*;
use papers_document::{Link, Outlines};
use papers_view::JobPriority;
use papers_view::Metadata;

use glib::SignalHandlerId;
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

mod imp {

    use super::*;

    /// A Data structure to represent the expand status of outlines tree
    ///
    /// A [TreePath] can be in two state:
    /// * Expand
    /// * Collapse
    ///
    /// The final goal of this data structure is allow user to set the expand
    /// status and serialize/deserialize the state into/from two string in special
    /// format:
    ///
    ///     |<path1>|<path2>|<path3>|
    ///
    /// Document producer may mark a path to be expanded explicitly so we accept
    /// this data through `expand_by_producer` interface. And we will produce
    /// collapse string to mark a path explicitly collapse by user.
    #[derive(Debug, Clone, Default)]
    struct PathData {
        collapse: HashSet<TreePath>,
        expand: HashSet<TreePath>,
        expand_by_producer: HashSet<TreePath>,
        dirty_collapse: bool,
        dirty_expand: bool,
    }

    impl PathData {
        const KEY_EXPAND: &'static str = "index-expand";
        const KEY_COLLAPSE: &'static str = "index-collapse";

        fn clear(&mut self) {
            self.collapse.clear();
            self.expand.clear();
            self.expand_by_producer.clear();
        }

        fn load_metadata(&mut self, metadata: &Metadata) {
            if let Some(collapse) = metadata.string(Self::KEY_COLLAPSE) {
                debug!("load collapse string: {collapse}");
                for path in PathData::parse_tree_paths(&collapse) {
                    self.collapse.insert(path);
                }
            }

            if let Some(expand) = metadata.string(Self::KEY_EXPAND) {
                debug!("load expand string: {expand}");
                for path in PathData::parse_tree_paths(&expand) {
                    self.expand.insert(path);
                }
            }
        }

        fn parse_tree_paths(data: &str) -> impl IntoIterator<Item = TreePath> + '_ {
            data.split('|').filter(|p| !p.is_empty()).map(|p| {
                TreePath(
                    p.split(':')
                        .map(|n| n.parse::<u32>().unwrap())
                        .collect::<Vec<u32>>(),
                )
            })
        }

        fn set_to_string(&self, set: &HashSet<TreePath>) -> String {
            let mut out = String::new();

            if !set.is_empty() {
                out.push('|');

                for p in set {
                    out.push_str(&format!("{p}|"));
                }
            }

            out
        }

        fn expand_string(&self) -> String {
            self.set_to_string(&self.expand)
        }

        fn collapse_string(&self) -> String {
            self.set_to_string(&self.collapse)
        }

        fn store_to(&mut self, metadata: &Metadata) {
            if self.dirty_collapse {
                debug!("store collapse string: {}", self.collapse_string());
                metadata.set_string(Self::KEY_COLLAPSE, &self.collapse_string());
            }

            if self.dirty_expand {
                debug!("store expand string: {}", self.expand_string());
                metadata.set_string(Self::KEY_EXPAND, &self.expand_string());
            }

            self.dirty_collapse = false;
            self.dirty_expand = false;
        }

        fn expanded_paths(&self) -> impl IntoIterator<Item = &TreePath> {
            self.expand_by_producer
                .difference(&self.collapse)
                .chain(self.expand.iter())
        }

        fn is_expand(&self, path: &TreePath) -> bool {
            (self.expand_by_producer.contains(path) && !self.collapse.contains(path))
                || self.expand.contains(path)
        }

        fn expand(&mut self, path: &TreePath) {
            debug!("expand: {path}");
            self.collapse_remove(path);

            if !self.expand_by_producer.contains(path) {
                self.expand_insert(path);
            }
        }

        fn expand_by_producer(&mut self, path: &TreePath) {
            debug!("expand by producer: {path}");
            self.collapse_remove(path);
            self.expand_remove(path);
            self.expand_by_producer.insert(path.clone());
        }

        fn collapse(&mut self, path: &TreePath) {
            debug!("collapse: {path}");
            if self.expand_by_producer.contains(path) {
                self.collapse_insert(path);
            } else {
                self.expand_remove(path);
            }
        }

        fn collapse_all(&mut self) {
            if !self.expand.is_empty() {
                self.expand.clear();
                self.dirty_expand = true;
            }

            if !self.collapse.is_empty() {
                self.collapse.clear();
                self.dirty_collapse = true;
            }
        }

        fn set_expanded(&mut self, path: &TreePath, expanded: bool) {
            if expanded {
                self.expand(path);
            } else {
                self.collapse(path);
            }
        }

        // We must use the following functions to operate collapse and expand
        // sets to maintain the dirty states.
        fn expand_insert(&mut self, path: &TreePath) {
            self.dirty_expand |= self.expand.insert(path.clone());
        }

        fn expand_remove(&mut self, path: &TreePath) {
            self.dirty_expand |= self.expand.remove(path);
        }

        fn collapse_insert(&mut self, path: &TreePath) {
            self.dirty_collapse |= self.collapse.insert(path.clone());
        }

        fn collapse_remove(&mut self, path: &TreePath) {
            self.dirty_collapse |= self.collapse.remove(path);
        }
    }

    /// An internal type to replace [gtk::TreePath]
    ///
    /// It's basically a series of index to represent a path to a tree item.
    #[derive(Debug, Clone, Eq, Hash, PartialEq)]
    struct TreePath(Vec<u32>);

    impl TreePath {
        /// Expand the item of tree pointed by the path. Returns the final position
        /// of the row in [gtk::TreeListModel]
        fn expand(&self, tree_model: &gtk::TreeListModel) -> Option<u32> {
            let path = &self.0;

            debug_assert_ne!(path.len(), 0);

            let mut row = tree_model.child_row(path[0])?;
            if !row.is_expanded() {
                row.set_expanded(true);
            }

            for pos in path.iter().skip(1) {
                row = row.child_row(*pos)?;
                if !row.is_expanded() {
                    row.set_expanded(true);
                }
            }

            Some(row.position())
        }

        fn collapse(&self, tree_model: &gtk::TreeListModel) {
            let path = &self.0;

            let mut row = tree_model.child_row(path[0]).unwrap();

            for pos in path.iter().skip(1) {
                if let Some(child) = row.child_row(*pos) {
                    row = child;
                } else {
                    return;
                }
            }

            if row.is_expanded() {
                debug!("collapse tree list model: {self}");
                row.set_expanded(false);
            }
        }

        fn is_anccestor_of(&self, path: &TreePath) -> bool {
            path.0.starts_with(&self.0)
        }

        fn _iterate_outlines<F>(&mut self, f: &F, model: &gio::ListModel)
        where
            F: Fn(&Outlines, &TreePath) -> bool,
        {
            let n_items = model.n_items();

            for n in 0..n_items {
                let outlines = model.item(n).unwrap().dynamic_cast::<Outlines>().unwrap();

                self.0.push(n);

                if f(&outlines, self) {
                    if let Some(ref children) = outlines.children() {
                        self._iterate_outlines(f, children);
                    }
                }

                self.0.pop();
            }
        }

        fn iterate_outlines<F>(f: &F, model: &gio::ListModel)
        where
            F: Fn(&Outlines, &TreePath) -> bool,
        {
            TreePath(vec![])._iterate_outlines(f, model);
        }

        fn item(&self, model: &gio::ListModel) -> Option<Outlines> {
            let mut outlines = model.item(self.0[0]).and_downcast::<Outlines>()?;

            for n in self.0.iter().skip(1) {
                let children = outlines.children()?;
                outlines = children.item(*n).and_downcast::<Outlines>()?;
            }

            Some(outlines)
        }

        fn next(&self) -> TreePath {
            let mut path = self.0.clone();
            let last_index = path.len() - 1;

            path[last_index] += 1;

            TreePath(path)
        }

        fn from_row(row: &gtk::TreeListRow) -> TreePath {
            let mut path = vec![];
            let mut row = row.clone();
            let mut parent_row = row.parent();

            while let Some(ref parent) = parent_row {
                path.push(parent.position() - row.position());

                row = parent.clone();
                parent_row = row.parent();
            }

            path.push(row.position());

            path.reverse();

            TreePath(path)
        }
    }

    impl std::fmt::Display for TreePath {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(
                f,
                "{}",
                self.0
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<String>>()
                    .join(":")
            )
        }
    }

    #[derive(Properties, Default, Debug, CompositeTemplate)]
    #[properties(wrapper_type = super::PpsSidebarLinks)]
    #[template(resource = "/org/gnome/papers/ui/sidebar-links.ui")]
    pub struct PpsSidebarLinks {
        #[template_child]
        pub(super) list_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) selection_model: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) popup: TemplateChild<gtk::PopoverMenu>,
        /// Whether any top-level section is open. The sidebar header's
        /// toggle button points its arrow at whatever the next press does.
        #[property(get, set)]
        pub(super) outline_expanded: std::cell::Cell<bool>,
        #[template_child]
        pub(super) action_group: TemplateChild<gio::SimpleActionGroup>,
        #[property(name = "model", nullable, get)]
        pub(super) outlines: RefCell<Option<gio::ListModel>>,
        /// A cache mapping a page number to the outline entries that start on
        /// it, in document order. A page routinely holds several sections, so
        /// this cannot be one path per page: keeping only the last meant that
        /// arriving at a page selected its *final* subsection, skipping the
        /// ones above it.
        page_map: RefCell<BTreeMap<usize, Vec<TreePath>>>,
        /// The entry the user last activated, and the page it went to. Held so
        /// that the page-changed which follows navigation does not immediately
        /// move the selection to a different section on the same page.
        activated: RefCell<Option<(usize, TreePath)>>,
        path_data: RefCell<PathData>,
        sig_handlers: RefCell<HashMap<gtk::TreeListRow, SignalHandlerId>>,
        timeout_id: RefCell<Option<glib::SourceId>>,
        update_page_to: Cell<i32>,
        selected_row: RefCell<Option<gtk::TreeListRow>>,
        pub(super) block_activate: Cell<bool>,
        pub(super) block_page_changed: Cell<bool>,
        pub(super) block_row_expand: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PpsSidebarLinks {
        const NAME: &'static str = "PpsSidebarLinks";
        type Type = super::PpsSidebarLinks;
        type ParentType = PpsSidebarPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PpsSidebarLinks {
        fn constructed(&self) {
            let obj = self.obj();
            crate::style::track_highlight_style(&*self.list_view);

            let action_entries = [
                gio::ActionEntry::builder("collapse-all")
                    .activate(glib::clone!(
                        #[weak(rename_to = obj)]
                        self,
                        move |_: &gio::SimpleActionGroup, _, _| {
                            obj.block_row_expand.set(true);
                            obj.collapse_all();
                            obj.block_row_expand.set(false);

                            obj.path_data.borrow_mut().collapse_all();
                            obj.store_metadata();
                        }
                    ))
                    .build(),
                gio::ActionEntry::builder("expand-all")
                    .activate(glib::clone!(
                        #[weak(rename_to = obj)]
                        self,
                        move |_: &gio::SimpleActionGroup, _, _| {
                            obj.block_row_expand.set(true);
                            obj.expand_all();
                            obj.block_row_expand.set(false);
                        }
                    ))
                    .build(),
                gio::ActionEntry::builder("expand-all-under")
                    .activate(glib::clone!(
                        #[weak(rename_to = obj)]
                        self,
                        move |_: &gio::SimpleActionGroup, _, _| {
                            obj.block_row_expand.set(true);
                            obj.expand_all_under();
                            obj.block_row_expand.set(false);
                        }
                    ))
                    .build(),
                gio::ActionEntry::builder("print-section")
                    .activate(glib::clone!(
                        #[weak(rename_to = obj)]
                        self,
                        move |_: &gio::SimpleActionGroup, _, _| {
                            obj.print_section();
                        }
                    ))
                    .build(),
            ];

            self.action_group.add_action_entries(action_entries);
            obj.insert_action_group("links", Some(&self.action_group.get()));

            if let Some(model) = self.obj().document_model() {
                model.connect_document_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        obj.imp().document_changed();
                    }
                ));

                model.connect_page_changed(glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |_, _, new| {
                        if obj.block_page_changed.get() {
                            return;
                        }

                        obj.update_page_to.set(new);

                        if let Some(source) = obj.timeout_id.take() {
                            source.remove();
                        }

                        let id = glib::timeout_add_local_once(
                            Duration::from_millis(200),
                            glib::clone!(
                                #[weak]
                                obj,
                                move || {
                                    let page = obj.update_page_to.get();

                                    obj.set_current_page(page);

                                    obj.timeout_id.take();
                                }
                            ),
                        );

                        obj.timeout_id.replace(Some(id));
                    }
                ));
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![Signal::builder("link-activated")
                    .run_last()
                    .param_types([Link::static_type()])
                    .build()]
            })
        }
    }

    impl WidgetImpl for PpsSidebarLinks {}

    impl BinImpl for PpsSidebarLinks {}

    impl PpsSidebarPageImpl for PpsSidebarLinks {
        fn support_document(&self, document: &Document) -> bool {
            document
                .dynamic_cast_ref::<DocumentLinks>()
                .map(|d| d.has_document_links())
                .unwrap_or(false)
        }
    }

    #[gtk::template_callbacks]
    impl PpsSidebarLinks {
        fn set_action_enabled(&self, name: &str, enabled: bool) {
            if let Some(action) = self
                .action_group
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_enabled(enabled);
            }
        }

        fn document_changed(&self) {
            if let Some(document) = self.document() {
                if !self.support_document(&document) {
                    return;
                }

                let job = JobLinks::new(&document);

                job.connect_finished(glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |job| {
                        let model = job.model().unwrap();

                        obj.clear_path_cache();
                        obj.outlines.replace(Some(model.clone()));

                        let tree_model = gtk::TreeListModel::new(model.clone(), false, false, |o| {
                            o.dynamic_cast_ref::<papers_document::Outlines>()
                                .unwrap()
                                .children()
                        });

                        // Rows first: the outline renders this frame. The
                        // page cache walks every entry through poppler, so it
                        // runs after the widget has had a chance to draw.
                        obj.selection_model.set_model(Some(&tree_model));

                        // Any expansion or collapse inserts or removes rows,
                        // so this one signal keeps the toggle button's
                        // direction truthful however the change came about.
                        tree_model.connect_items_changed(glib::clone!(
                            #[weak]
                            obj,
                            move |_, _, _, _| obj.update_expand_toggle()
                        ));
                        obj.update_expand_toggle();

                        glib::idle_add_local_once(glib::clone!(
                            #[weak]
                            obj,
                            move || {
                                obj.build_path_cache(&model);

                                // Update action sensitivity
                                let is_flat = model
                                    .iter::<Outlines>()
                                    .map(|o| o.unwrap())
                                    .all(|o| o.children().is_none());
                                obj.set_action_enabled("collapse-all", !is_flat);
                                obj.set_action_enabled("expand-all", !is_flat);
                                obj.set_action_enabled("expand-all-under", !is_flat);

                                obj.expand_open_links();

                                if let Some(model) = obj.obj().document_model() {
                                    obj.set_current_page(model.page());
                                }
                            }
                        ));
                    }
                ));

                // High, like the thumbnails: at PriorityNone the one cheap
                // links job queued behind a fresh tab's stack of thumbnail
                // renders, which is why the outline appeared seconds late.
                job.scheduler_push_job(JobPriority::PriorityHigh);
            }
        }

        fn outlines_to_page(&self, outlines: &Outlines) -> usize {
            let document = self.document_links().unwrap();
            let link = outlines.link().unwrap();
            document.link_page(&link) as usize
        }

        fn outlines_to_path(&self, outlines: &Outlines) -> Option<TreePath> {
            let page = self.outlines_to_page(outlines);
            self.page_map
                .borrow()
                .get(&page)
                .and_then(|paths| paths.first())
                .cloned()
        }

        /// The path of a specific outline entry, matched by identity rather
        /// than by page, so that sections sharing a page stay distinguishable.
        fn selected_path_for(&self, outlines: &Outlines) -> Option<TreePath> {
            self.selection_model
                .selected_item()
                .and_downcast::<gtk::TreeListRow>()
                .and_then(|row| row.item())
                .and_downcast::<Outlines>()
                .filter(|selected| selected == outlines)
                .and_then(|_| self.current_path())
                .or_else(|| self.outlines_to_path(outlines))
        }

        fn current_path(&self) -> Option<TreePath> {
            self.selection_model
                .selected_item()
                .and_downcast::<gtk::TreeListRow>()
                .and_then(|row| row.item())
                .and_downcast::<Outlines>()
                .and_then(|outlines| self.outlines_to_path(&outlines))
        }

        fn set_current_page(&self, page: i32) {
            debug_assert!(page >= 0);

            let upper = (page + 1) as usize;

            // An entry the user just clicked wins over anything derived from
            // the page number, and does so exactly once. Several sections
            // commonly share a page, and the destination often sits near a page
            // boundary, so the page reported after navigating is frequently not
            // the clicked entry's own page - matching on it would let the
            // selection slide onto a neighbour. Consume the pin instead, so the
            // click stands and the next genuine scroll takes over.
            let pinned = self
                .activated
                .replace(None)
                .map(|(_, path)| path);

            // Otherwise the first section starting on this page, since the page
            // change fires as its top comes into view; failing that, the last
            // section before it, which is still the enclosing one.
            let derived = pinned.or_else(|| {
                let map = self.page_map.borrow();

                map.get(&(page as usize))
                    .and_then(|paths| paths.first())
                    .cloned()
                    .or_else(|| {
                        map.range(..upper)
                            .next_back()
                            .and_then(|(_, paths)| paths.last())
                            .cloned()
                    })
            });

            if let Some(path) = derived.as_ref() {
                debug!("set current selected page to {page}, row position {path}");

                let current_path = self.current_path();

                if Some(path) != current_path.as_ref() {
                    if let Some(tree_model) = self.tree_model() {
                        self.block_activate.set(true);
                        self.block_row_expand.set(true);

                        self.sidebar_collapse(Some(path));

                        if let Some(pos) = path.expand(&tree_model) {
                            glib::idle_add_local_once(glib::clone!(
                                #[weak(rename_to = obj)]
                                self,
                                move || {
                                    obj.block_row_expand.set(true);
                                    obj.block_activate.set(true);

                                    obj.list_view.scroll_to(
                                        pos,
                                        gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT,
                                        None,
                                    );

                                    obj.block_row_expand.set(false);
                                    obj.block_activate.set(false);
                                }
                            ));
                        }

                        self.block_activate.set(false);
                        self.block_row_expand.set(false);
                    }
                }
            }
        }

        fn tree_model(&self) -> Option<gtk::TreeListModel> {
            self.selection_model
                .model()
                .and_then(|m| m.downcast::<gtk::TreeListModel>().ok())
        }

        /// Whether any top-level section is currently open - the state the
        /// toggle button and the F8 toggle both key off.
        fn any_top_level_expanded(&self) -> bool {
            let Some(tree_model) = self.tree_model() else {
                return false;
            };

            let mut index = 0;
            while let Some(row) = tree_model.child_row(index) {
                if row.is_expanded() {
                    return true;
                }
                index += 1;
            }

            false
        }

        /// Republish the expansion state: rows just appeared or vanished,
        /// so re-read it and let the header button follow.
        pub(super) fn update_expand_toggle(&self) {
            let expanded = self.any_top_level_expanded();

            if expanded != self.outline_expanded.get() {
                self.obj().set_outline_expanded(expanded);
            }
        }

        /// One-key toggle: any top-level section open means "collapse to
        /// the chapters", none open means "expand everything".
        pub(super) fn toggle_expansion(&self) {
            if self.tree_model().is_none() {
                return;
            }

            if self.any_top_level_expanded() {
                self.collapse_all();
            } else {
                self.expand_all();
            }
        }

        fn collapse_all(&self) {
            if let Some(tree_model) = self.tree_model() {
                let mut index = 0;

                while let Some(row) = tree_model.child_row(index) {
                    row.set_expanded(false);
                    index += 1;
                }
            }
        }

        fn expand_all_under(&self) {
            if let Some(row) = self.selected_row.take() {
                let mut path = TreePath::from_row(&row);
                let tree_model = self.tree_model().unwrap();

                row.set_expanded(true);

                path._iterate_outlines(
                    &|_, path| {
                        path.expand(&tree_model);
                        self.path_data.borrow_mut().expand(path);
                        true
                    },
                    &row.children().unwrap(),
                );

                self.store_metadata();
            }
        }

        fn expand_all(&self) {
            if let Some(tree_model) = self.tree_model() {
                let model = tree_model.model();

                self.for_each_outlines(
                    |outlines, path| {
                        path.expand(&tree_model);

                        if outlines.children().is_some() {
                            self.path_data.borrow_mut().expand(path);
                        }

                        true
                    },
                    &model,
                );

                self.store_metadata();
            }
        }

        fn print_section(&self) {
            if let Some(row) = self.selected_row.take() {
                if let Some(outlines) = row.item().and_downcast::<Outlines>() {
                    let Some(link) = outlines.link() else { return };
                    let Some(document_links) = self.document_links() else {
                        return;
                    };
                    let Some(document) = self.document() else {
                        return;
                    };
                    let Some(model) = self.tree_model().map(|tree_model| tree_model.model()) else {
                        return;
                    };

                    let first_page = document_links.link_page(&link) + 1;

                    let last_page = TreePath::from_row(&row)
                        .next()
                        .item(&model)
                        .and_then(|outlines| outlines.link())
                        .map_or_else(
                            || document.n_pages(),
                            |link| document_links.link_page(&link),
                        );

                    let last_page = if last_page == -1 {
                        document.n_pages()
                    } else {
                        last_page
                    };

                    if let Some(window) = self.obj().native().and_downcast::<PpsWindow>() {
                        window.print_range(first_page, last_page);
                    }
                }
            }
        }

        fn clear_path_cache(&self) {
            self.page_map.borrow_mut().clear();
            self.path_data.borrow_mut().clear();
        }

        /// Iterate all outlines.
        ///
        /// The accepted closure returns a boolean to stop iterate an outlines' children.
        fn for_each_outlines<F>(&self, f: F, model: &gio::ListModel)
        where
            F: Fn(&Outlines, &TreePath) -> bool,
        {
            TreePath::iterate_outlines(&f, model);
        }

        fn document(&self) -> Option<Document> {
            self.obj()
                .document_model()
                .and_then(|model| model.document())
        }

        fn document_links(&self) -> Option<DocumentLinks> {
            self.document()
                .and_then(|d| d.dynamic_cast::<DocumentLinks>().ok())
        }

        /// Build the path cache
        ///
        /// The path cache is composed by two parts: Path Data and Page Map
        fn build_path_cache(&self, model: &gio::ListModel) {
            self.for_each_outlines(
                |outlines, path| {
                    let page = self.outlines_to_page(outlines);

                    // build expand by producer
                    if outlines.expands() {
                        self.path_data.borrow_mut().expand_by_producer(path);
                    }

                    self.page_map
                        .borrow_mut()
                        .entry(page)
                        .or_default()
                        .push(path.clone());

                    true
                },
                model,
            );

            if let Some(metadata) = self.metadata() {
                self.path_data.borrow_mut().load_metadata(&metadata);
            }
        }

        #[template_callback]
        fn list_view_selection_changed(&self) {
            if self.block_activate.get() {
                return;
            }

            if let Some(outlines) = self
                .selection_model
                .selected_item()
                .and_then(|o| o.downcast::<gtk::TreeListRow>().ok())
                .and_then(|row| row.item())
                .and_then(|item| item.downcast::<Outlines>().ok())
            {
                if let Some(link) = outlines.link() {
                    debug!("link activated: `{}`", link.title().unwrap_or_default());

                    // Remember what was clicked. Navigation emits page-changed
                    // asynchronously, well after the guard below has lifted, and
                    // without this the selection would jump to whichever section
                    // sits last on the destination page.
                    let page = self.outlines_to_page(&outlines);
                    if let Some(path) = self.selected_path_for(&outlines) {
                        self.activated.replace(Some((page, path)));
                    }

                    self.block_page_changed.set(true);
                    self.obj().emit_by_name::<()>("link-activated", &[&link]);
                    self.obj().navigate_to_view();
                    self.block_page_changed.set(false);
                }
            }
        }

        fn metadata(&self) -> Option<Metadata> {
            self.obj()
                .native()
                .and_downcast::<PpsWindow>()
                .and_then(|w| w.metadata())
        }

        fn store_metadata(&self) {
            if let Some(metadata) = self.metadata() {
                self.path_data.borrow_mut().store_to(&metadata);
            }
        }

        fn sidebar_collapse(&self, current_path: Option<&TreePath>) {
            // Sections auto-expand as the document is scrolled into them. By
            // default they now stay open: having them snap shut again as soon
            // as the reader moves past makes the outline appear to rearrange
            // itself underneath the cursor.
            if gio::Settings::new("io.github.emhoferb.Tabs").boolean("keep-outline-expanded") {
                return;
            }

            if let Some(tree_model) = self.tree_model() {
                let model = tree_model.model();

                self.for_each_outlines(
                    |_, path| {
                        if !self.path_data.borrow().is_expand(path)
                            && !current_path
                                .map(|cp| path.is_anccestor_of(cp))
                                .unwrap_or(false)
                        {
                            path.collapse(&tree_model);
                            false
                        } else {
                            true
                        }
                    },
                    &model,
                );
            }
        }

        fn expand_open_links(&self) {
            if let Some(tree_model) = self.tree_model() {
                self.block_row_expand.set(true);

                for path in self.path_data.borrow().expanded_paths() {
                    path.expand(&tree_model);
                }

                self.block_row_expand.set(false);
            }
        }

        #[template_callback]
        fn list_view_factory_setup(
            &self,
            item: &gtk::ListItem,
            _factory: &gtk::SignalListItemFactory,
        ) {
            // One line per entry, ellipsized, as Evince 46 had it: an outline
            // is a scanning aid, so fitting more of it on screen matters more
            // than showing every title in full. The full text stays reachable
            // through the tooltip.
            let outline = gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .hexpand(true)
                .single_line_mode(true)
                .has_tooltip(true)
                .build();

            let index = gtk::Label::builder()
                .xalign(1.0)
                .max_width_chars(7)
                .css_classes(["dim-label"])
                .build();

            let box_ = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(6)
                .build();

            box_.append(&outline);
            box_.append(&index);

            let expander = gtk::TreeExpander::builder().child(&box_).build();

            item.set_focusable(false);
            item.set_child(Some(&expander));

            let gesture_click = gtk::GestureClick::builder()
                .button(gdk::BUTTON_SECONDARY)
                .build();

            gesture_click.connect_pressed(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[weak]
                box_,
                move |gesture_click, _, x, y| {
                    // FIXME: check menu availability first
                    let row = gesture_click
                        .widget()
                        .and_then(|w| w.parent())
                        .and_downcast::<gtk::TreeExpander>()
                        .unwrap()
                        .list_row()
                        .unwrap();
                    let has_children = row.is_expandable();

                    obj.selected_row.replace(Some(row));

                    let point = box_
                        .compute_point(
                            obj.obj().as_ref(),
                            &gtk::graphene::Point::new(x as f32, y as f32),
                        )
                        .unwrap();
                    let rect = gdk::Rectangle::new(point.x() as i32, point.y() as i32, 0, 0);

                    obj.set_action_enabled("expand-all-under", has_children);

                    obj.popup.set_pointing_to(Some(&rect));
                    obj.popup.popup();
                }
            ));

            box_.add_controller(gesture_click);
        }

        #[template_callback]
        fn list_view_factory_bind(
            &self,
            list_item: &gtk::ListItem,
            _factory: &gtk::SignalListItemFactory,
        ) {
            let tree_item = list_item.item().and_downcast::<gtk::TreeListRow>().unwrap();
            let item = tree_item.item().and_downcast::<Outlines>().unwrap();
            let expander = list_item
                .child()
                .and_downcast::<gtk::TreeExpander>()
                .unwrap();
            let box_ = expander.child().unwrap();
            let outlines = box_.first_child().and_downcast::<gtk::Label>().unwrap();
            let page_label = box_.last_child().and_downcast::<gtk::Label>().unwrap();

            expander.set_list_row(Some(&tree_item));

            if tree_item.is_expandable() {
                self.sig_handlers.borrow_mut().insert(
                    tree_item.clone(),
                    tree_item.connect_expanded_notify(glib::clone!(
                        #[weak(rename_to = obj)]
                        self,
                        move |row| {
                            if obj.block_row_expand.get() {
                                return;
                            }

                            let outlines = row.item().and_downcast::<Outlines>().unwrap();

                            if let Some(path) = obj.outlines_to_path(&outlines) {
                                debug!(
                                    "click expander: `{}` {}",
                                    outlines.markup().unwrap_or_default(),
                                    row.is_expanded()
                                );
                                obj.path_data
                                    .borrow_mut()
                                    .set_expanded(&path, row.is_expanded());
                            }

                            obj.store_metadata();
                        }
                    )),
                );
            }

            let markup = item.markup();

            outlines.set_tooltip_markup(markup.as_ref().map(|m| m.as_str().trim_end()));
            outlines.set_markup(markup.unwrap_or_default().trim_end());
            page_label.set_label(&item.label().unwrap_or_default());
        }

        #[template_callback]
        fn list_view_factory_unbind(
            &self,
            list_item: &gtk::ListItem,
            _factory: &gtk::SignalListItemFactory,
        ) {
            let expander = list_item
                .child()
                .and_downcast::<gtk::TreeExpander>()
                .unwrap();

            let tree_item = expander.list_row().unwrap();

            expander.set_list_row(None);

            if let Some(id) = self.sig_handlers.borrow_mut().remove(&tree_item) {
                tree_item.disconnect(id);
            }
        }
    }
}

glib::wrapper! {
    pub struct PpsSidebarLinks(ObjectSubclass<imp::PpsSidebarLinks>)
        @extends PpsSidebarPage, adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PpsSidebarLinks {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Expand the whole outline, or collapse it back to the top-level
    /// chapters - whichever the current state calls for.
    pub fn toggle_expansion(&self) {
        self.imp().toggle_expansion();
    }
}

impl Default for PpsSidebarLinks {
    fn default() -> Self {
        Self::new()
    }
}
