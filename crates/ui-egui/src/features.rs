//! Features an embedder hides or disables in GridCraft Viewer (`docs/viewer.md`): the web app's
//! `?hide=` / `?disable=` (and the `gridcraft:config` message under `?host=parent`), the desktop
//! app's `--hide=` / `--disable=`.
//!
//! Each [`Feature`] is shown (the default), hidden (not drawn) or disabled (drawn greyed out). A
//! hidden or disabled feature is turned off in function as well: its keyboard shortcut, its
//! context-menu item and its File page do nothing, and dropping a file doesn't open it when
//! `open` or `drop` is off. The gates are [`SheetApp::run_or_alert`], [`SheetApp::open_dialog`],
//! [`SheetApp::open_backstage`], [`SheetApp::print_now`], [`SheetApp::copy_to_clipboard`] and
//! [`crate::backstage::export_as`], which every button, menu and shortcut goes through. Opening a
//! workbook programmatically (the command line, `?url=`, a host page's `gridcraft:open`, the
//! control channel) is not a user action and still works.

use crate::SheetApp;

/// A feature of the viewer an embedder can hide or disable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    /// The toolbar's File button and the File page (Backstage).
    File,
    /// Opening a file: the toolbar's Open, the empty window's button, File › Open's Browse and
    /// Recent list, Ctrl+O, and drag and drop.
    Open,
    /// Opening a file by dropping it on the window (also off when `open` is).
    Drop,
    /// The built-in samples on File › Open.
    Samples,
    /// Export ▾ and File › Export: PDF, CSV, Web Page and the Excel workbook copy.
    Export,
    /// Export as PDF (also off when `export` is). Print is separate.
    Pdf,
    /// The toolbar's Print, File › Print and Ctrl+P.
    Print,
    /// Find: the toolbar's Find, Ctrl+F, Shift+F5 and the context menu's Find….
    Find,
    /// Go To: Ctrl+G, F5 and the context menu's Go To….
    Goto,
    /// Copying cells: Ctrl+C and the context menu's Copy.
    Copy,
    /// View ▾ and the view adjustments: gridlines, headings, freezing, widths and heights (and
    /// their drags), hiding rows and columns, Wrap Text, filtering, the view modes.
    View,
    /// Zoom: the toolbar's − % + and Fit, the status bar's zoom, Ctrl+wheel and the Zoom dialog.
    Zoom,
    /// File › Options (and with it the theme and language pickers).
    Options,
    /// The display theme picker in File › Options (also off when `options` is).
    Theme,
    /// The interface language picker in File › Options (also off when `options` is).
    Language,
}

impl Feature {
    /// Every feature, in the order the docs list them.
    pub const ALL: [Feature; 15] = [
        Feature::File,
        Feature::Open,
        Feature::Drop,
        Feature::Samples,
        Feature::Export,
        Feature::Pdf,
        Feature::Print,
        Feature::Find,
        Feature::Goto,
        Feature::Copy,
        Feature::View,
        Feature::Zoom,
        Feature::Options,
        Feature::Theme,
        Feature::Language,
    ];

    /// The name in `?hide=` / `--hide=`.
    pub fn name(self) -> &'static str {
        match self {
            Feature::File => "file",
            Feature::Open => "open",
            Feature::Drop => "drop",
            Feature::Samples => "samples",
            Feature::Export => "export",
            Feature::Pdf => "pdf",
            Feature::Print => "print",
            Feature::Find => "find",
            Feature::Goto => "goto",
            Feature::Copy => "copy",
            Feature::View => "view",
            Feature::Zoom => "zoom",
            Feature::Options => "options",
            Feature::Theme => "theme",
            Feature::Language => "language",
        }
    }

    /// The feature named `name`, ignoring case and surrounding spaces.
    pub fn parse(name: &str) -> Option<Feature> {
        let name = name.trim();
        Feature::ALL.into_iter().find(|f| f.name().eq_ignore_ascii_case(name))
    }

    /// The feature this one is part of: turning that off turns this off too.
    fn parent(self) -> Option<Feature> {
        match self {
            Feature::Pdf => Some(Feature::Export),
            Feature::Drop => Some(Feature::Open),
            Feature::Theme | Feature::Language => Some(Feature::Options),
            _ => None,
        }
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }

    /// The feature a command run from a button, menu or shortcut belongs to.
    pub fn of_command(id: &str) -> Option<Feature> {
        Some(match id {
            "file.open" => Feature::Open,
            "file.print" => Feature::Print,
            "file.exportPdf" => Feature::Pdf,
            "file.exportCsv" | "file.exportHtml" | "file.saveCopy" | "file.saveAs" => Feature::Export,
            "edit.find" | "edit.findNext" | "edit.findPrevious" | "edit.replace" => Feature::Find,
            "edit.goTo" | "edit.goToSpecial" => Feature::Goto,
            "edit.copy" => Feature::Copy,
            "view.zoom" | "view.zoom100" | "view.zoomToSelection" => Feature::Zoom,
            "view.theme" | "view.darkMode" => Feature::Theme,
            "app.language.set" => Feature::Language,
            "file.options" => Feature::Options,
            "ui.backstage" => Feature::File,
            "view.normal" | "view.pageLayout" | "view.pageBreakPreview" => Feature::View,
            id if gridcraft_engine::viewer::VIEW.contains(&id) => Feature::View,
            _ => return None,
        })
    }

    /// The feature a dialog belongs to.
    pub fn of_dialog(name: &str) -> Option<Feature> {
        Some(match name {
            "open" => Feature::Open,
            "find" => Feature::Find,
            "goTo" | "goToSpecial" => Feature::Goto,
            "zoom" => Feature::Zoom,
            "options" => Feature::Options,
            "backstage" => Feature::File,
            "saveAs" => Feature::Export,
            _ => return None,
        })
    }

    /// The feature of a context-menu item (`ui:` items open dialogs).
    pub fn of_menu_item(id: &str) -> Option<Feature> {
        match id {
            "ui:find" => Some(Feature::Find),
            "ui:goTo" => Some(Feature::Goto),
            "ui:filterValue" => Some(Feature::View),
            id => Feature::of_command(id),
        }
    }
}

/// How a feature shows. Hidden outranks disabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Show {
    #[default]
    Shown,
    Disabled,
    Hidden,
}

/// Which features are hidden or disabled. The default shows everything.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Features {
    hidden: u16,
    disabled: u16,
}

/// The warning for a feature name `name` that [`Features::parse`] doesn't know.
pub fn unknown_warning(name: &str, source: &str) -> String {
    let known: Vec<&str> = Feature::ALL.iter().map(|f| f.name()).collect();
    format!("{source}: unknown feature {name:?} ignored; known features: {}", known.join(", "))
}

/// What the viewer says when a shortcut reaches a disabled feature.
pub const REASON: &str = "This is turned off here.";

impl Features {
    /// Features from a `hide` and a `disable` list (comma-separated names, any case). A feature in
    /// both is hidden. Returns the names it doesn't know, which the caller warns about.
    pub fn parse(hide: Option<&str>, disable: Option<&str>) -> (Features, Vec<String>) {
        let mut f = Features::default();
        let mut unknown = f.add(disable.unwrap_or_default(), Show::Disabled);
        unknown.extend(f.add(hide.unwrap_or_default(), Show::Hidden));
        (f, unknown)
    }

    /// [`Features::parse`], warning in the log (the browser console on the web) about each name it
    /// doesn't know; `source` says where the lists came from (`?hide=`, `--hide`, …).
    pub fn parse_logged(hide: Option<&str>, disable: Option<&str>, source: &str) -> Features {
        let (features, unknown) = Features::parse(hide, disable);
        for name in unknown {
            log::warn!("{}", unknown_warning(&name, source));
        }
        features
    }

    /// Hides or disables (`show`) the features named in `list`; returns the unknown names. Blank
    /// entries (`open,,print`) are skipped. Hidden is never downgraded to disabled.
    pub fn add(&mut self, list: &str, show: Show) -> Vec<String> {
        let mut unknown = Vec::new();
        for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            match Feature::parse(name) {
                Some(feature) => self.set(feature, show.max(self.own(feature))),
                None => unknown.push(name.to_string()),
            }
        }
        unknown
    }

    pub fn set(&mut self, feature: Feature, show: Show) {
        let bit = feature.bit();
        self.hidden &= !bit;
        self.disabled &= !bit;
        match show {
            Show::Shown => {}
            Show::Disabled => self.disabled |= bit,
            Show::Hidden => self.hidden |= bit,
        }
    }

    /// How `feature` itself was set, without the feature it is part of.
    fn own(self, feature: Feature) -> Show {
        let bit = feature.bit();
        if self.hidden & bit != 0 {
            Show::Hidden
        } else if self.disabled & bit != 0 {
            Show::Disabled
        } else {
            Show::Shown
        }
    }

    /// How `feature` shows: its own setting, or the feature it is part of when that is further off.
    pub fn show(self, feature: Feature) -> Show {
        let own = self.own(feature);
        feature.parent().map_or(own, |p| own.max(self.own(p)))
    }

    /// Whether `feature` works (shown, neither hidden nor disabled).
    pub fn allows(self, feature: Feature) -> bool {
        self.show(feature) == Show::Shown
    }

    /// Whether `feature` is drawn (shown or disabled).
    pub fn visible(self, feature: Feature) -> bool {
        self.show(feature) != Show::Hidden
    }

    /// Whether everything is shown.
    pub fn is_default(self) -> bool {
        self == Features::default()
    }

    /// The hidden and the disabled features' names, comma-separated, as `?hide=` and `?disable=`
    /// take them.
    pub fn lists(self) -> (String, String) {
        let names = |show: Show| Feature::ALL.into_iter().filter(|f| self.own(*f) == show).map(Feature::name).collect::<Vec<_>>().join(",");
        (names(Show::Hidden), names(Show::Disabled))
    }
}

impl SheetApp {
    /// Whether a button, menu item or shortcut for `feature` may run. A disabled one says why in
    /// a toast; a hidden one does nothing, as if it weren't there.
    pub fn feature_allowed(&mut self, feature: Feature) -> bool {
        match self.features.show(feature) {
            Show::Shown => true,
            Show::Disabled => {
                self.toast = Some((self.ui.language.tr(REASON).to_string(), crate::now_ms()));
                false
            }
            Show::Hidden => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn names_round_trip_and_ignore_case_and_spaces() {
        for f in Feature::ALL {
            assert_eq!(Feature::parse(f.name()), Some(f));
            assert_eq!(Feature::parse(&format!("  {}  ", f.name().to_uppercase())), Some(f));
        }
        assert_eq!(Feature::parse("spelling"), None, "GridCraft has no spelling or grammar checker");
        assert_eq!(Feature::parse(""), None);
        assert!(Feature::ALL.len() <= 16, "the bit sets are u16");
    }

    #[test]
    fn lists_parse_with_hide_winning_and_unknown_names_reported() {
        let (f, unknown) = Features::parse(Some("open,Export, PRINT,,bogus"), Some("pdf,print,zoom,nope"));
        assert_eq!(unknown, vec!["nope".to_string(), "bogus".to_string()]);
        assert_eq!(f.show(Feature::Open), Show::Hidden);
        assert_eq!(f.show(Feature::Export), Show::Hidden);
        assert_eq!(f.show(Feature::Print), Show::Hidden, "hide wins over disable");
        assert_eq!(f.show(Feature::Zoom), Show::Disabled);
        assert_eq!(f.show(Feature::Find), Show::Shown);
        assert!(!f.allows(Feature::Zoom) && f.visible(Feature::Zoom));
        assert!(!f.visible(Feature::Open));
        // Order doesn't matter: disabling after hiding keeps it hidden.
        let mut g = Features::default();
        let _ = g.add("print", Show::Hidden);
        let _ = g.add("print", Show::Disabled);
        assert_eq!(g.show(Feature::Print), Show::Hidden);
        assert_eq!(Features::parse(None, None), (Features::default(), vec![]));
        assert!(Features::parse(Some(" , "), Some("")).0.is_default());
    }

    #[test]
    fn a_part_is_off_when_its_whole_is() {
        let (f, _) = Features::parse(Some("export"), Some("options"));
        assert_eq!(f.show(Feature::Pdf), Show::Hidden);
        assert_eq!(f.show(Feature::Theme), Show::Disabled);
        assert_eq!(f.show(Feature::Language), Show::Disabled);
        let (f, _) = Features::parse(Some("pdf"), None);
        assert_eq!(f.show(Feature::Export), Show::Shown, "the whole stays when a part goes");
        let (f, _) = Features::parse(None, Some("open"));
        assert!(!f.allows(Feature::Drop), "no dropping to open while open is off");
        let (f, _) = Features::parse(Some("drop"), None);
        assert!(f.allows(Feature::Open));
    }

    #[test]
    fn lists_write_back_what_was_parsed() {
        let (f, _) = Features::parse(Some("print,open"), Some("pdf"));
        assert_eq!(f.lists(), ("open,print".to_string(), "pdf".to_string()));
        assert_eq!(Features::parse(Some(&f.lists().0), Some(&f.lists().1)).0, f);
    }

    #[test]
    fn commands_dialogs_and_menu_items_map_to_features() {
        for (id, f) in [
            ("file.open", Feature::Open),
            ("file.print", Feature::Print),
            ("file.exportPdf", Feature::Pdf),
            ("file.exportCsv", Feature::Export),
            ("file.saveCopy", Feature::Export),
            ("edit.find", Feature::Find),
            ("edit.goTo", Feature::Goto),
            ("edit.copy", Feature::Copy),
            ("view.zoom", Feature::Zoom),
            ("view.freezePanes", Feature::View),
            ("home.hideRows", Feature::View),
            ("data.filterBy", Feature::View),
            ("view.theme", Feature::Theme),
            ("app.language.set", Feature::Language),
        ] {
            assert_eq!(Feature::of_command(id), Some(f), "{id}");
        }
        for id in ["selection.set", "edit.selectAll", "sheet.next", "file.close"] {
            assert_eq!(Feature::of_command(id), None, "{id}");
        }
        assert_eq!(Feature::of_dialog("find"), Some(Feature::Find));
        assert_eq!(Feature::of_dialog("about"), None);
        assert_eq!(Feature::of_menu_item("ui:goTo"), Some(Feature::Goto));
        assert_eq!(Feature::of_menu_item("edit.copy"), Some(Feature::Copy));
    }

    fn viewer_app(hide: &str, disable: &str) -> SheetApp {
        let mut session = gridcraft_engine::Session::new();
        session.new_workbook();
        session.viewer = true;
        let mut app = SheetApp::new(session, crate::Services::default());
        let _ = gridcraft_engine::sample::open(&mut app.session, "sales");
        app.features = Features::parse(Some(hide), Some(disable)).0;
        app
    }

    #[test]
    fn hidden_and_disabled_features_are_blocked_in_function() {
        let mut app = viewer_app("find,file", "zoom,goto");
        let zoom = |app: &SheetApp| app.session.active().and_then(|d| d.wb.active().map(|s| s.zoom));
        let before = zoom(&app);
        app.run_or_alert("view.zoom", json!({"percent": 200}));
        assert_eq!(zoom(&app), before, "a disabled zoom doesn't zoom");
        assert!(app.toast.as_ref().is_some_and(|(t, _)| t == REASON), "a disabled feature says why");
        app.toast = None;
        app.open_dialog("find", json!({}));
        assert!(app.dialog.is_none() && app.toast.is_none(), "a hidden feature does nothing, silently");
        app.open_dialog("goTo", json!({}));
        assert!(app.dialog.is_none());
        app.open_backstage(crate::backstage::Page::Open);
        assert!(app.backstage.is_none(), "no File page while file is hidden");
        app.run_or_alert("ui.backstage", json!({}));
        assert!(app.backstage.is_none());
        // What stays on still works.
        app.features = Features::default();
        app.run_or_alert("view.zoom", json!({"percent": 200}));
        assert_eq!(zoom(&app), Some(200));
        app.open_dialog("find", json!({}));
        assert!(app.dialog.is_some());
    }

    #[test]
    fn file_pages_follow_their_features() {
        use crate::backstage::Page;
        let mut app = viewer_app("print,options", "export");
        assert!(!crate::viewer::shows_backstage_page(&app, Page::Print));
        assert!(!crate::viewer::shows_backstage_page(&app, Page::Options));
        assert!(crate::viewer::shows_backstage_page(&app, Page::Export), "a disabled page is listed, greyed out");
        assert!(!crate::viewer::opens_backstage_page(&app, Page::Export));
        app.open_backstage(Page::Print);
        assert_eq!(app.backstage.as_ref().map(|b| b.page), Some(Page::Open), "a page that is off opens File › Open instead");
        app.backstage = None;
        // Open and samples both off: the File page starts on the first page left.
        app.features = Features::parse(Some("open,samples,print"), None).0;
        assert!(!crate::viewer::shows_backstage_page(&app, Page::Open));
        app.open_backstage(Page::Open);
        assert_eq!(app.backstage.as_ref().map(|b| b.page), Some(Page::Export));
        app.backstage = None;
        app.features = Features::parse(Some("open,samples,print,export,options"), None).0;
        app.open_backstage(Page::Open);
        assert!(app.backstage.is_none(), "no page left: the File page doesn't open");
    }

    #[test]
    fn exports_follow_export_and_pdf() {
        let mut app = viewer_app("pdf", "");
        app.services.download = Some(Box::new(|_, _| {}));
        assert!(!crate::backstage::export_as(&mut app, "pdf"));
        assert!(crate::backstage::export_as(&mut app, "csv"));
        app.features = Features::parse(None, Some("export")).0;
        assert!(!crate::backstage::export_as(&mut app, "csv"));
        assert!(!crate::backstage::export_as(&mut app, "xlsx"));
    }

    #[test]
    fn an_unknown_name_warns_with_the_known_ones() {
        let w = unknown_warning("spelling", "?hide=");
        assert!(w.starts_with("?hide=: unknown feature \"spelling\""), "{w}");
        assert!(w.contains("open, drop, samples") && w.ends_with("language"), "{w}");
        let f = Features::parse_logged(Some("print,spelling"), None, "?hide=");
        assert!(!f.visible(Feature::Print) && f.visible(Feature::Open));
    }
}
