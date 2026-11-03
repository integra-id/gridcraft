//! GridCraft Viewer's window (`docs/viewer.md`): one thin toolbar in place of the title bar,
//! ribbon and formula bar, over read-only sheets.
//!
//! Viewer mode is the engine's [`gridcraft_engine::Session::viewer`]: the engine refuses every
//! command that edits ([`gridcraft_engine::viewer`]), and this module refuses the interface
//! commands and dialogs that only make sense for editing ([`UI_EDITING`], [`DIALOGS`]). What the
//! window offers is opening, exporting a copy (PDF, CSV, xlsx), printing, Find, Go To, zoom and the
//! sheet tabs; the grid itself still scrolls, selects and copies, and formulas still compute.

use egui::{Align2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;

use crate::SheetApp;
use crate::features::{Feature, Show};
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::widgets::{small_button, small_button_width};

/// Whether this build has the editing interface (the `editor` feature, on for the desktop app).
/// Without it the app is always the viewer ([`SheetApp::is_viewer`]).
pub const EDITOR: bool = cfg!(feature = "editor");

impl SheetApp {
    /// Whether the window is GridCraft Viewer: the session is in viewer mode, or this build has no
    /// editing interface ([`EDITOR`]).
    pub fn is_viewer(&self) -> bool {
        !EDITOR || self.session.viewer
    }
}

/// The app's name in the viewer's toolbar and window title.
pub const NAME: &str = "GridCraft Viewer";

/// Height of the viewer's toolbar.
pub const BAR_HEIGHT: f32 = 40.0;

/// The badge beside the workbook's name.
pub const BADGE: &str = "Read only";

/// Interface commands the viewer refuses: the ribbon, Python cells and AutoComplete only matter
/// while editing.
pub const UI_EDITING: &[&str] = &["ui.ribbonTab", "view.collapseRibbon", "view.expandFormulaBar", "ui.pythonMode", "options.autoComplete"];

/// Dialogs the viewer opens: finding, going to a cell, zoom, the command search and About. The
/// File page's Open, Print, Export and Options open through `open_dialog` as well.
pub const DIALOGS: &[&str] = &["find", "goTo", "zoom", "about", "commandSearch", "openPassword", "open", "options", "backstage", "saveAs"];

/// The File page's (Backstage's) pages the viewer shows: no New, Info, Save As or Account.
pub const BACKSTAGE_PAGES: &[crate::backstage::Page] =
    &[crate::backstage::Page::Open, crate::backstage::Page::Print, crate::backstage::Page::Export, crate::backstage::Page::Options];

/// The File page the viewer shows in place of one it doesn't have.
pub const BACKSTAGE_START: crate::backstage::Page = crate::backstage::Page::Open;

/// Why the viewer refuses command `id` (engine or interface), or `None` when it runs it or the
/// app isn't the viewer.
pub fn refuses(app: &SheetApp, id: &str) -> Option<&'static str> {
    if !app.is_viewer() {
        return None;
    }
    if UI_EDITING.contains(&id) {
        return Some(gridcraft_engine::viewer::REASON);
    }
    // Interface commands that aren't the engine's (dialogs, theme, language, print settings) stay.
    let spec = gridcraft_engine::find_command(id)?;
    gridcraft_engine::viewer::refuses(spec).then_some(gridcraft_engine::viewer::REASON)
}

/// Whether the viewer opens dialog `name` (every dialog opens outside the viewer).
pub fn opens_dialog(app: &SheetApp, name: &str) -> bool {
    !app.is_viewer() || DIALOGS.contains(&name)
}

/// How the embedder's features (`features`) show File page `page`: Open while opening or the
/// samples are on, Print, Export and Options by their own features.
fn page_show(app: &SheetApp, page: crate::backstage::Page) -> Show {
    use crate::backstage::Page;
    let f = app.features;
    match page {
        Page::Open => f.show(Feature::Open).min(f.show(Feature::Samples)),
        Page::Print => f.show(Feature::Print),
        Page::Export => f.show(Feature::Export),
        Page::Options => f.show(Feature::Options),
        _ => Show::Shown,
    }
}

/// Whether the File page lists `page` in this app (greyed out when [`opens_backstage_page`] is
/// false).
pub fn shows_backstage_page(app: &SheetApp, page: crate::backstage::Page) -> bool {
    (!app.is_viewer() || BACKSTAGE_PAGES.contains(&page)) && page_show(app, page) != Show::Hidden
}

/// Whether the File page can show `page` (it is listed and not disabled).
pub fn opens_backstage_page(app: &SheetApp, page: crate::backstage::Page) -> bool {
    shows_backstage_page(app, page) && page_show(app, page) == Show::Shown
}

/// `page` if the File page can show it, else the first viewer page it can; `None` when the
/// embedder turned them all off.
pub fn backstage_page(app: &SheetApp, page: crate::backstage::Page) -> Option<crate::backstage::Page> {
    if opens_backstage_page(app, page) {
        return Some(page);
    }
    BACKSTAGE_PAGES.iter().copied().find(|p| opens_backstage_page(app, *p))
}

/// Whether the active workbook is the blank one the window keeps when nothing is open: the
/// viewer shows a prompt to open a workbook in its place ([`empty`]).
pub fn placeholder(app: &SheetApp) -> bool {
    let Some(d) = app.session.active() else { return true };
    d.path.is_none()
        && !d.is_dirty()
        && d.title.starts_with("Book")
        && d.wb.sheets.iter().all(|sh| sh.cells.is_empty() && sh.charts.is_empty() && sh.images.is_empty())
}

/// Opens built-in sample `name`: `file.new {sample}` in the editor. The viewer, which refuses
/// `file.new`, opens the engine's copy of it as it opens a file: read-only, in place of the blank
/// workbook it keeps when nothing is open.
pub fn open_sample(app: &mut SheetApp, name: &str) {
    if !app.is_viewer() {
        app.run_or_alert("file.new", json!({"sample": name}));
        return;
    }
    let blank = (placeholder(app) && app.session.documents().len() == 1).then(|| app.session.active_index());
    if gridcraft_engine::sample::open(&mut app.session, name).is_none() {
        app.toast = Some((format!("Unknown sample: {name}"), crate::now_ms()));
    } else if let Some(i) = blank {
        app.session.close_document(i);
    }
}

/// The window title: the workbook's name, then the app's.
pub fn window_title(app: &SheetApp) -> String {
    match app.session.active() {
        Some(d) if !placeholder(app) => format!("{} - {NAME}", d.display_title()),
        _ => NAME.to_string(),
    }
}

/// Zoom steps for the toolbar's − and + (Excel's View › Zoom presets and the ones between).
const ZOOM_STEPS: &[u16] = &[10, 25, 50, 75, 90, 100, 110, 125, 150, 175, 200, 250, 300, 400];

/// The zoom one step in or out from `zoom`.
pub fn zoom_step(zoom: u16, inwards: bool) -> u16 {
    if inwards {
        ZOOM_STEPS.iter().copied().find(|z| *z > zoom).unwrap_or(400)
    } else {
        ZOOM_STEPS.iter().rev().copied().find(|z| *z < zoom).unwrap_or(10)
    }
}

/// The zoom that fits the active sheet's used range into the grid's width (and height, when the
/// range is short enough), between 25% and 200%.
pub fn fit_zoom(app: &SheetApp) -> Option<u16> {
    let d = app.session.active()?;
    let sh = d.wb.active()?;
    let used = sh.used_range()?;
    let view = app.grid.rect?;
    let (vw, vh) = ((view.width() - 50.0).max(50.0) as f64, (view.height() - 24.0).max(50.0) as f64);
    let w = (sh.col_left(used.end.col + 1) - sh.col_left(used.start.col)).max(1.0);
    let h = (sh.row_top(used.end.row + 1) - sh.row_top(used.start.row)).max(1.0);
    let by_width = vw / w;
    let fit = if h * by_width <= vh { by_width } else { by_width.min((vh / h).max(0.25)) };
    Some((fit * 100.0).clamp(25.0, 200.0).floor() as u16)
}

/// Runs a viewer action, showing what went wrong in a message.
fn act(app: &mut SheetApp, id: &str, params: serde_json::Value) {
    app.run_or_alert(id, params);
}

/// Exports a copy of the workbook as `fmt` (`pdf`, `csv` or `xlsx`), through the save picker or
/// as a browser download.
pub fn export(app: &mut SheetApp, fmt: &str) {
    let _ = crate::backstage::export_as(app, fmt);
}

fn separator(ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, t.separator));
}

/// What the viewer's toolbar has room for: its buttons' labels, and the zoom steps (−, the
/// percentage, +). A narrow window drops the labels first (each button keeps its icon and
/// tooltip; Fit keeps its label), then the zoom steps, so the two groups of buttons never overlap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarFit {
    pub labels: bool,
    pub zoom_steps: bool,
}

/// Which toolbar buttons are there: an embedder may hide any of them (`features`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarButtons {
    pub file: bool,
    pub open: bool,
    pub export: bool,
    pub print: bool,
    pub zoom: bool,
    pub find: bool,
    pub view: bool,
}

impl BarButtons {
    /// Every button.
    pub const ALL: BarButtons = BarButtons { file: true, open: true, export: true, print: true, zoom: true, find: true, view: true };

    /// The buttons `app` shows: File only with its file tab, none an embedder hid.
    pub fn of(app: &SheetApp) -> BarButtons {
        let f = app.features;
        BarButtons {
            file: app.chrome.file_tab && f.visible(Feature::File),
            open: f.visible(Feature::Open),
            export: f.visible(Feature::Export),
            print: f.visible(Feature::Print),
            zoom: f.visible(Feature::Zoom),
            find: f.visible(Feature::Find),
            view: f.visible(Feature::View),
        }
    }

    /// Whether the File button has buttons after it, to keep a separator from.
    fn file_separator(self) -> bool {
        self.file && (self.open || self.export || self.print)
    }

    /// Whether the zoom group has Find or View after it, to keep a separator from.
    fn zoom_separator(self) -> bool {
        self.zoom && (self.find || self.view)
    }
}

/// The fullest toolbar that fits `width` (inside its margins). `button(label, dropdown)` is a button's width (`""` for
/// icon only), `pct` the zoom percentage's; `shows` and `has` (a workbook is open) say which
/// buttons are there.
pub fn bar_fit(width: f32, shows: BarButtons, has: bool, pct: f32, button: impl Fn(&str, bool) -> f32) -> BarFit {
    let need = |fit: BarFit| {
        let b = |on: bool, label: &str, dropdown: bool| if on { button(if fit.labels { label } else { "" }, dropdown) + 2.0 } else { 0.0 };
        let sep = |on: bool| if on { SEPARATOR } else { 0.0 };
        let left = b(shows.file, "File", false)
            + sep(shows.file_separator())
            + b(shows.open, "Open", false)
            + b(shows.export, "Export", true)
            + b(shows.print, "Print", false);
        // Fit keeps its label: its icon alone would look like Find's.
        let zoom = match (shows.zoom, fit.zoom_steps) {
            (false, _) => 0.0,
            (true, steps) => button("Fit", false) + 2.0 + if steps { 2.0 * b(true, "", false) + pct + 2.0 } else { 0.0 },
        };
        let right = if has { b(shows.view, "View", true) + b(shows.find, "Find", false) + sep(shows.zoom_separator()) + zoom } else { 0.0 };
        left + right + BAR_GAP
    };
    let fits = [BarFit { labels: true, zoom_steps: true }, BarFit { labels: false, zoom_steps: true }];
    fits.into_iter().find(|fit| need(*fit) <= width).unwrap_or(BarFit { labels: false, zoom_steps: false })
}

/// How the workbook's name and the badge (`badge`, its width, when a workbook is open) share
/// `room` between the toolbar's groups: the room the name gets (0 for none, below 30 px) and
/// whether the badge shows. The name goes first, then the badge; neither spills past `room`.
pub fn title_fit(room: f32, badge: Option<f32>) -> (f32, bool) {
    let badge_shown = badge.is_some_and(|w| w <= room);
    if badge.is_some() && !badge_shown {
        // An open workbook's name alone, without the badge, would hide that it's read-only.
        return (0.0, false);
    }
    let left = room - badge.filter(|_| badge_shown).map_or(0.0, |w| w + 8.0);
    (if left >= 30.0 { left } else { 0.0 }, badge_shown)
}

/// A separator's width with the spacing after it.
const SEPARATOR: f32 = 11.0;
/// The least room kept between the toolbar's two groups.
const BAR_GAP: f32 = 12.0;

/// The View menu: how the sheet shows, for this session only. Each item is a view adjustment
/// ([`gridcraft_engine::viewer::VIEW`]): it changes no value or formula, leaves the workbook
/// clean, and copies and exports leave it out (PDF and Print show the sheet as on screen).
fn view_menu(app: &mut SheetApp, ui: &mut Ui) {
    let lang = app.ui.language;
    ui.set_min_width(220.0);
    let Some(sh) = app.session.active().and_then(|d| d.wb.active().cloned()) else { return };
    let mut gridlines = sh.show_gridlines;
    if ui.checkbox(&mut gridlines, lang.tr("Gridlines")).changed() {
        act(app, "view.gridlines", json!({"on": gridlines}));
    }
    let mut headings = sh.show_headings;
    if ui.checkbox(&mut headings, lang.tr("Headings")).changed() {
        act(app, "view.headings", json!({"on": headings}));
    }
    ui.separator();
    use gridcraft_engine::model::SheetViewMode as Mode;
    for (mode, label, id) in [
        (Mode::Normal, "Normal", "view.normal"),
        (Mode::PageLayout, "Page Layout", "view.pageLayout"),
        (Mode::PageBreakPreview, "Page Break Preview", "view.pageBreakPreview"),
    ] {
        if ui.radio(sh.view == mode, lang.tr(label)).clicked() {
            act(app, id, json!({}));
            ui.close();
        }
    }
    ui.separator();
    let frozen = sh.freeze.is_some();
    let items: &[(&str, &str)] = &[
        ("Freeze Panes", "view.freezePanes"),
        ("Freeze Top Row", "view.freezeTopRow"),
        ("Freeze First Column", "view.freezeFirstColumn"),
        ("Unfreeze Panes", "view.unfreezePanes"),
        ("-", ""),
        ("Wrap Text", "home.wrapText"),
        ("AutoFit Column Width", "home.autofitColumnWidth"),
        ("AutoFit Row Height", "home.autofitRowHeight"),
        ("-", ""),
        ("Hide Rows", "home.hideRows"),
        ("Hide Columns", "home.hideColumns"),
        ("Unhide Rows", "home.unhideRows"),
        ("Unhide Columns", "home.unhideColumns"),
        ("-", ""),
        ("Clear Filter", "data.clearFilter"),
    ];
    for &(label, id) in items {
        if label == "-" {
            ui.separator();
            continue;
        }
        let enabled = match id {
            "view.unfreezePanes" => frozen,
            "data.clearFilter" => sh.autofilter.as_ref().is_some_and(|a| !a.criteria.is_empty()),
            _ => true,
        };
        if ui.add_enabled(enabled, egui::Button::new(lang.tr(label))).clicked() {
            act(app, id, json!({}));
            ui.close();
        }
    }
}

/// The viewer's toolbar across the top of the window: File, Open, Export and Print on the left;
/// View, Find and zoom on the right; the workbook's name with a "Read only" badge between.
pub fn bar(app: &mut SheetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language;
    egui::Panel::top("viewer_bar")
        .exact_size(BAR_HEIGHT)
        .frame(
            egui::Frame::NONE
                .fill(t.window)
                .inner_margin(egui::Margin { left: 8, right: 10, top: 0, bottom: 0 })
                .stroke(Stroke::new(1.0, t.ribbon_border)),
        )
        .show(ui, |ui| {
            let full = ui.max_rect();
            // The bar drags the window, as the title bar it replaces did.
            let drag = ui.interact(full, ui.scope_id().with("viewer_drag"), Sense::click_and_drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            let has = !placeholder(app);
            let zoom = app.session.active().and_then(|d| d.wb.active().map(|s| s.zoom)).unwrap_or(100);
            let pct_text = format!("{zoom}%");
            let pct_w = ui.painter().layout_no_wrap(pct_text.clone(), theme::ui_font(12.5), t.text).size().x;
            let shows = BarButtons::of(app);
            let fit = bar_fit(full.width(), shows, has, pct_w, |label, dropdown| small_button_width(ui, label, dropdown));
            // Without room for labels a button shows its icon, and its tooltip names it.
            let shown = |label: &'static str| if fit.labels { label } else { "" };
            // A button the embedder disabled (`features`) stays, greyed out.
            let on = |app: &SheetApp, f: Feature| app.features.allows(f);
            let mut left_end = full.min.x;
            let mut right_start = full.max.x;
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                if shows.file
                    && ui
                        .add_enabled_ui(on(app, Feature::File), |ui| {
                            small_button(ui, Icon::Book, shown("File"), "File: open, print and export", false)
                        })
                        .inner
                        .clicked()
                {
                    app.open_backstage(BACKSTAGE_START);
                }
                if shows.file_separator() {
                    separator(ui);
                }
                if shows.open
                    && ui
                        .add_enabled_ui(on(app, Feature::Open), |ui| {
                            small_button(ui, Icon::OpenFolder, shown("Open"), "Open a workbook (Ctrl+O)", false)
                        })
                        .inner
                        .clicked()
                {
                    act(app, "file.open", json!({}));
                }
                ui.add_enabled_ui(has, |ui| {
                    if shows.export {
                        ui.add_enabled_ui(on(app, Feature::Export), |ui| {
                            let menu = small_button(ui, Icon::Share, shown("Export"), "Export a copy as PDF, CSV or Excel workbook", true);
                            egui::Popup::menu(&menu).show(|ui| {
                                ui.set_min_width(200.0);
                                for (fmt, label) in
                                    [("pdf", "PDF (.pdf)"), ("csv", "CSV (.csv), the active sheet"), ("xlsx", "Excel Workbook copy (.xlsx)")]
                                {
                                    let feature = if fmt == "pdf" { Feature::Pdf } else { Feature::Export };
                                    if !app.features.visible(feature) {
                                        continue;
                                    }
                                    if ui.add_enabled(app.features.allows(feature), egui::Button::new(lang.tr(label))).clicked() {
                                        export(app, fmt);
                                        ui.close();
                                    }
                                }
                            });
                        });
                    }
                    if shows.print
                        && ui
                            .add_enabled_ui(on(app, Feature::Print), |ui| small_button(ui, Icon::Print, shown("Print"), "Print (Ctrl+P)", false))
                            .inner
                            .clicked()
                    {
                        act(app, "file.print", json!({}));
                    }
                });
                left_end = ui.min_rect().max.x;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                    if !has {
                        right_start = ui.min_rect().min.x;
                        return;
                    }
                    if shows.zoom {
                        ui.add_enabled_ui(on(app, Feature::Zoom), |ui| {
                            if small_button(ui, Icon::Zoom, "Fit", "Fit the sheet's data in the window", false).clicked()
                                && let Some(z) = fit_zoom(app)
                            {
                                act(app, "view.zoom", json!({"percent": z}));
                            }
                            if fit.zoom_steps {
                                if small_button(ui, Icon::Plus, "", "Zoom In", false).clicked() {
                                    act(app, "view.zoom", json!({"percent": zoom_step(zoom, true)}));
                                }
                                let pct = ui.add(
                                    egui::Label::new(egui::RichText::new(&pct_text).font(theme::ui_font(12.5)).color(t.text))
                                        .selectable(false)
                                        .sense(Sense::click()),
                                );
                                if pct.on_hover_text(lang.tr("Zoom to 100%")).clicked() {
                                    act(app, "view.zoom100", json!({}));
                                }
                                if small_button(ui, Icon::Minus, "", "Zoom Out", false).clicked() {
                                    act(app, "view.zoom", json!({"percent": zoom_step(zoom, false)}));
                                }
                            }
                        });
                    }
                    if shows.zoom_separator() {
                        separator(ui);
                    }
                    if shows.find
                        && ui
                            .add_enabled_ui(on(app, Feature::Find), |ui| small_button(ui, Icon::Find, shown("Find"), "Find (Ctrl+F)", false))
                            .inner
                            .clicked()
                    {
                        app.open_dialog("find", json!({}));
                    }
                    if shows.view {
                        ui.add_enabled_ui(on(app, Feature::View), |ui| {
                            let menu =
                                small_button(ui, Icon::Gridlines, shown("View"), "View: gridlines, freeze panes, column widths, hidden rows", true);
                            egui::Popup::menu(&menu).show(|ui| view_menu(app, ui));
                        });
                    }
                    right_start = ui.min_rect().min.x;
                });
            });
            // The workbook's name and the badge, centred in the room between the two groups.
            let name = if has { app.session.active().map(|d| d.display_title()).unwrap_or_default() } else { NAME.to_string() };
            let font = theme::ui_bold(13.0);
            let badge_font = theme::ui_font(11.5);
            let badge = lang.tr(BADGE).to_string();
            let painter = ui.painter();
            let name_w = painter.layout_no_wrap(name.clone(), font.clone(), t.text).size().x;
            let badge_w = if has { painter.layout_no_wrap(badge.clone(), badge_font.clone(), t.text).size().x + 14.0 } else { 0.0 };
            let room = right_start - left_end - 24.0;
            let (name_room, badge_shown) = title_fit(room, if has { Some(badge_w) } else { None });
            let shown_w = name_w.min(name_room) + if badge_shown { badge_w + if name_room > 0.0 { 8.0 } else { 0.0 } } else { 0.0 };
            if shown_w > 0.0 {
                let x = ((left_end + right_start) / 2.0 - shown_w / 2.0).max(left_end + 12.0);
                let cy = full.center().y;
                if name_room > 0.0 {
                    let clip = Rect::from_min_size(pos2(x, full.min.y), vec2(name_room, full.height()));
                    painter.with_clip_rect(clip).text(pos2(x, cy), Align2::LEFT_CENTER, &name, font, t.text);
                }
                if badge_shown {
                    let bx = x + if name_room > 0.0 { name_w.min(name_room) + 8.0 } else { 0.0 };
                    let r = Rect::from_min_size(pos2(bx, cy - 10.0), vec2(badge_w, 20.0));
                    painter.rect_filled(r, 10.0, t.accent_soft);
                    painter.rect_stroke(r, 10.0, Stroke::new(1.0, t.accent), StrokeKind::Inside);
                    painter.text(r.center(), Align2::CENTER_CENTER, &badge, badge_font, t.accent);
                }
            }
            let _ = drag;
        });
}

/// The window with no workbook open: a prompt to open one.
pub fn empty(app: &mut SheetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language;
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.grid_bg)).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() / 2.0 - 70.0).max(20.0));
            ui.label(egui::RichText::new(NAME).font(theme::ui_bold(22.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(lang.tr("Open a workbook to view it. Editing is turned off.")).font(theme::ui_font(13.5)).color(t.text_dim));
            ui.add_space(16.0);
            // Without opening (`features`) the prompt has no button; the host opens the file.
            if app.features.visible(Feature::Open) {
                let open = ui.add_enabled(
                    app.features.allows(Feature::Open),
                    egui::Button::new(egui::RichText::new(lang.tr("Open a workbook…")).font(theme::ui_font(14.0)).color(t.on_accent))
                        .fill(t.accent)
                        .min_size(vec2(180.0, 34.0)),
                );
                if open.clicked() {
                    act(app, "file.open", json!({}));
                }
                ui.add_space(8.0);
            }
            ui.label(
                egui::RichText::new(lang.tr("Excel workbooks (.xlsx, .xlsm, .xls), OpenDocument (.ods), CSV and text files"))
                    .font(theme::ui_font(12.0))
                    .color(t.text_dim),
            );
        });
    });
}

/// The status bar's left end in the viewer: the active cell's address and what it holds (its
/// formula, or its text), in place of the formula bar.
pub fn cell_summary(app: &SheetApp) -> Option<String> {
    let d = app.session.active()?;
    let sh = d.wb.active()?;
    let at = d.selection.active;
    let text = gridcraft_engine::display::edit_text(&d.wb, sh, at);
    Some(if text.is_empty() { at.a1() } else { format!("{}: {}", at.a1(), text) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Buttons 8 px per letter wide, as `small_button` lays them out (24 px of icon, 6 of gap).
    fn width(label: &str, dropdown: bool) -> f32 {
        24.0 + if label.is_empty() { 0.0 } else { label.len() as f32 * 8.0 + 6.0 } + if dropdown { 12.0 } else { 0.0 }
    }

    #[test]
    fn a_sample_opens_in_place_of_the_blank_window() {
        let mut app = viewer_app();
        assert!(placeholder(&app));
        open_sample(&mut app, "sales");
        assert_eq!(app.session.documents().len(), 1, "the sample takes the blank workbook's place");
        assert!(!placeholder(&app) && app.toast.is_none());
        // A second sample opens beside it.
        open_sample(&mut app, "budget");
        assert_eq!(app.session.documents().len(), 2);
        open_sample(&mut app, "nope");
        assert_eq!(app.session.documents().len(), 2);
        assert!(app.toast.as_ref().is_some_and(|(t, _)| t.contains("nope")));
    }

    #[test]
    fn the_title_never_spills_past_its_room() {
        assert_eq!(title_fit(300.0, Some(70.0)), (222.0, true));
        // Too little for the name: the badge alone.
        assert_eq!(title_fit(90.0, Some(70.0)), (0.0, true));
        // Too little for the badge: nothing.
        assert_eq!(title_fit(50.0, Some(70.0)), (0.0, false));
        assert_eq!(title_fit(20.0, Some(70.0)), (0.0, false));
        assert_eq!(title_fit(200.0, None), (200.0, false));
        for room in (-50..400).map(|r| r as f32) {
            let (name, badge) = title_fit(room, Some(70.0));
            assert!(name + if badge { 70.0 + if name > 0.0 { 8.0 } else { 0.0 } } else { 0.0 } <= room.max(0.0), "{room}");
        }
    }

    #[test]
    fn the_toolbar_drops_labels_then_zoom_steps_as_it_narrows() {
        let fit = |w| bar_fit(w, BarButtons::ALL, true, 30.0, width);
        assert_eq!(fit(1000.0), BarFit { labels: true, zoom_steps: true });
        assert_eq!(fit(400.0), BarFit { labels: false, zoom_steps: true });
        assert_eq!(fit(250.0), BarFit { labels: false, zoom_steps: false });
        // Without a workbook the right group is gone, and labels fit sooner.
        assert_eq!(bar_fit(400.0, BarButtons::ALL, false, 30.0, width), BarFit { labels: true, zoom_steps: true });
        // Buttons an embedder hid take no room.
        let few = BarButtons { open: false, print: false, find: false, view: false, ..BarButtons::ALL };
        assert_eq!(bar_fit(330.0, few, true, 30.0, width), BarFit { labels: true, zoom_steps: true });
        let none = BarButtons { file: false, open: false, export: false, print: false, zoom: false, find: false, view: false };
        assert_eq!(bar_fit(13.0, none, true, 30.0, width), BarFit { labels: true, zoom_steps: true });
        // Each step is the widest that fits.
        for w in (200..1200).step_by(10).map(|w| w as f32) {
            let got = fit(w);
            assert!(got.labels <= got.zoom_steps, "{w}: labels go before the zoom steps");
        }
    }

    fn viewer_app() -> SheetApp {
        let mut session = gridcraft_engine::Session::new();
        session.new_workbook();
        session.viewer = true;
        SheetApp::new(session, crate::Services::default())
    }

    #[test]
    fn the_viewer_refuses_editing_commands_and_dialogs() {
        let mut app = viewer_app();
        assert!(app.is_viewer());
        for id in ["cell.set", "home.bold", "edit.paste", "edit.cut", "file.save", "edit.undo", "ui.ribbonTab", "home.insertSheet"] {
            assert!(refuses(&app, id).is_some(), "{id}");
        }
        for id in
            ["edit.copy", "selection.set", "edit.find", "view.zoom", "file.open", "file.exportPdf", "file.saveCopy", "ui.backstage", "view.darkMode"]
        {
            assert!(refuses(&app, id).is_none(), "{id}");
        }
        assert!(app.run("cell.set", json!({"cell": "A1", "input": "1"})).is_err());
        assert!(opens_dialog(&app, "find") && !opens_dialog(&app, "formatCells") && !opens_dialog(&app, "insertCells"));
        app.open_dialog("formatCells", json!({}));
        assert!(app.dialog.is_none(), "an editing dialog opened in the viewer");
        assert!(shows_backstage_page(&app, crate::backstage::Page::Open) && !shows_backstage_page(&app, crate::backstage::Page::New));
    }

    #[test]
    fn the_viewer_doesnt_start_a_cell_edit() {
        let mut app = viewer_app();
        app.begin_edit(Some("5".into()), false);
        assert!(app.editor.is_none());
        let wb = app.session.active().map(|d| d.wb.clone());
        assert!(app.session.active().map(|d| d.wb.clone()).zip(wb).is_some_and(|(a, b)| std::sync::Arc::ptr_eq(&a, &b)));
    }

    #[test]
    fn the_blank_window_is_a_placeholder_and_titles_name_the_viewer() {
        let mut app = viewer_app();
        assert!(placeholder(&app));
        assert_eq!(window_title(&app), NAME);
        app.session.viewer = false;
        app.run("cell.set", json!({"cell": "A1", "input": "1"})).unwrap();
        app.session.viewer = true;
        assert!(!placeholder(&app));
        assert!(window_title(&app).ends_with(" - GridCraft Viewer"), "{}", window_title(&app));
    }

    #[test]
    fn zoom_steps_stay_in_range() {
        assert_eq!(zoom_step(100, true), 110);
        assert_eq!(zoom_step(100, false), 90);
        assert_eq!(zoom_step(400, true), 400);
        assert_eq!(zoom_step(10, false), 10);
        assert_eq!(zoom_step(133, true), 150);
    }
}
