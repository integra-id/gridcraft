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
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::widgets::small_button;

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

/// Whether the File page shows `page` in this app.
pub fn shows_backstage_page(app: &SheetApp, page: crate::backstage::Page) -> bool {
    !app.is_viewer() || BACKSTAGE_PAGES.contains(&page)
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

/// The viewer's toolbar across the top of the window: File, Open, Export and Print on the left;
/// Find and zoom on the right; the workbook's name with a "Read only" badge between.
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
            let mut left_end = full.min.x;
            let mut right_start = full.max.x;
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                if app.chrome.file_tab && small_button(ui, Icon::Book, "File", "File: open, print and export", false).clicked() {
                    app.open_backstage(BACKSTAGE_START);
                }
                separator(ui);
                if small_button(ui, Icon::OpenFolder, "Open", "Open a workbook (Ctrl+O)", false).clicked() {
                    act(app, "file.open", json!({}));
                }
                ui.add_enabled_ui(has, |ui| {
                    let menu = small_button(ui, Icon::Share, "Export", "Export a copy as PDF, CSV or Excel workbook", true);
                    egui::Popup::menu(&menu).show(|ui| {
                        ui.set_min_width(200.0);
                        for (fmt, label) in [("pdf", "PDF (.pdf)"), ("csv", "CSV (.csv), the active sheet"), ("xlsx", "Excel Workbook copy (.xlsx)")]
                        {
                            if ui.button(lang.tr(label)).clicked() {
                                export(app, fmt);
                                ui.close();
                            }
                        }
                    });
                    if small_button(ui, Icon::Print, "Print", "Print (Ctrl+P)", false).clicked() {
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
                    let zoom = app.session.active().and_then(|d| d.wb.active().map(|s| s.zoom)).unwrap_or(100);
                    if small_button(ui, Icon::Zoom, "Fit", "Fit the sheet's data in the window", false).clicked()
                        && let Some(z) = fit_zoom(app)
                    {
                        act(app, "view.zoom", json!({"percent": z}));
                    }
                    if small_button(ui, Icon::Plus, "", "Zoom In", false).clicked() {
                        act(app, "view.zoom", json!({"percent": zoom_step(zoom, true)}));
                    }
                    let pct = ui.add(
                        egui::Label::new(egui::RichText::new(format!("{zoom}%")).font(theme::ui_font(12.5)).color(t.text))
                            .selectable(false)
                            .sense(Sense::click()),
                    );
                    if pct.on_hover_text(lang.tr("Zoom to 100%")).clicked() {
                        act(app, "view.zoom100", json!({}));
                    }
                    if small_button(ui, Icon::Minus, "", "Zoom Out", false).clicked() {
                        act(app, "view.zoom", json!({"percent": zoom_step(zoom, false)}));
                    }
                    separator(ui);
                    if small_button(ui, Icon::Find, "Find", "Find (Ctrl+F)", false).clicked() {
                        app.open_dialog("find", json!({}));
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
            if room > 60.0 {
                let total = (name_w + if has { badge_w + 8.0 } else { 0.0 }).min(room);
                let x = ((left_end + right_start) / 2.0 - total / 2.0).max(left_end + 12.0);
                let cy = full.center().y;
                let name_room = (room - if has { badge_w + 8.0 } else { 0.0 }).max(20.0);
                let clip = Rect::from_min_size(pos2(x, full.min.y), vec2(name_room, full.height()));
                painter.with_clip_rect(clip).text(pos2(x, cy), Align2::LEFT_CENTER, &name, font, t.text);
                if has {
                    let bx = x + name_w.min(name_room) + 8.0;
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
            let open = ui.add(
                egui::Button::new(egui::RichText::new(lang.tr("Open a workbook…")).font(theme::ui_font(14.0)).color(t.on_accent))
                    .fill(t.accent)
                    .min_size(vec2(180.0, 34.0)),
            );
            if open.clicked() {
                act(app, "file.open", json!({}));
            }
            ui.add_space(8.0);
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
