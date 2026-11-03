//! The viewer's view adjustments through real egui input: resizing and AutoFit on the headers,
//! the View menu and the header menu change how the sheet shows, never a cell, and leave the
//! workbook clean; editing is still refused.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::kittest::Queryable as _;
use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::{SheetApp, grid::Geo};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

/// A clean workbook (as if just opened) with some text, in the viewer, `selection` selected.
fn harness(selection: &str) -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Short", "This column has considerably longer content", "Third"], [1, 2, 3]]}))
        .unwrap();
    s.execute("home.columnWidth", json!({"cols": "A:D", "width": 100})).unwrap();
    s.execute("selection.set", json!({"range": selection})).unwrap();
    let d = s.active_mut().unwrap();
    d.saved = d.wb.clone();
    d.undo.clear();
    s.viewer = true;
    let builder = crate::common::builder().with_step_dt(1.0 / 60.0).with_size(vec2(1200.0, 800.0));
    crate::common::harness_with(builder, crate::common::app(s), 4)
}

fn geo(h: &Harness) -> Geo {
    let app = h.state();
    Geo::new(app.session.active().unwrap().wb.active().unwrap(), app.grid.rect.unwrap(), app.view().scroll)
}

/// The border after column `col` in the column header.
fn col_edge(h: &Harness, col: u32) -> Pos2 {
    let g = geo(h);
    pos2(g.x(h.state().session.active().unwrap().wb.active().unwrap(), col + 1), g.rect.top() + g.header_h / 2.0)
}

fn button(h: &mut Harness, pos: Pos2, which: PointerButton, pressed: bool) {
    h.input_mut().events.extend([Event::PointerMoved(pos), Event::PointerButton { pos, button: which, pressed, modifiers: Modifiers::NONE }]);
    h.step();
}

fn click(h: &mut Harness, pos: Pos2) {
    button(h, pos, PointerButton::Primary, true);
    button(h, pos, PointerButton::Primary, false);
    h.run_steps(2);
}

fn width(h: &Harness, col: u32) -> f32 {
    h.state().session.active().unwrap().wb.active().unwrap().col_width(col)
}

/// The cells' text, to check no adjustment changed one.
fn cells(h: &Harness) -> Vec<String> {
    let d = h.state().session.active().unwrap();
    let sh = d.wb.active().unwrap();
    ["A1", "B1", "C1", "A2", "B2", "C2"].iter().map(|a| gridcraft_engine::display::edit_text(&d.wb, sh, CellRef::parse(a).unwrap())).collect()
}

fn assert_clean(h: &Harness, what: &str) {
    let d = h.state().session.active().unwrap();
    assert!(!d.is_dirty() && d.undo.is_empty(), "{what}: dirty {} undo {}", d.is_dirty(), d.undo.len());
    assert_ne!(h.state().toast.as_ref().map(|t| t.0.as_str()), Some(gridcraft_engine::viewer::REASON), "{what}: no read-only toast");
}

#[test]
fn dragging_a_header_border_resizes_and_a_double_click_autofits() {
    let mut h = harness("A1");
    let before = cells(&h);
    let start = col_edge(&h, 0);
    let w = width(&h, 0);
    button(&mut h, start, PointerButton::Primary, true);
    for dx in [15.0, 30.0] {
        h.input_mut().events.push(Event::PointerMoved(start + vec2(dx, 0.0)));
        h.step();
    }
    button(&mut h, start + vec2(30.0, 0.0), PointerButton::Primary, false);
    h.run_steps(2);
    assert!(width(&h, 0) > w + 10.0, "dragged {w} → {}", width(&h, 0));
    assert_clean(&h, "resize");

    let edge = col_edge(&h, 1);
    let w = width(&h, 1);
    click(&mut h, edge);
    h.step();
    click(&mut h, edge);
    assert!(width(&h, 1) > w, "AutoFit widened B for its long text: {w} → {}", width(&h, 1));
    assert_clean(&h, "AutoFit");
    assert_eq!(cells(&h), before);
}

#[test]
fn the_view_menu_hides_gridlines_and_freezes_panes() {
    let mut h = harness("B2");
    let menu = h.get_by_label("View").rect().center();
    click(&mut h, menu);
    let item = h.get_by_label("Gridlines").rect().center();
    click(&mut h, item);
    assert!(!h.state().session.active().unwrap().wb.active().unwrap().show_gridlines);
    assert_clean(&h, "Gridlines");

    let menu = h.get_by_label("View").rect().center();
    if h.query_by_label("Freeze Panes").is_none() {
        click(&mut h, menu);
    }
    let item = h.get_by_label("Freeze Panes").rect().center();
    click(&mut h, item);
    assert_eq!(h.state().session.active().unwrap().wb.active().unwrap().freeze, Some((1, 1)));
    assert_clean(&h, "Freeze Panes");
}

#[test]
fn the_header_menu_hides_columns_and_editing_is_still_refused() {
    let mut h = harness("B:B");
    let before = cells(&h);
    let g = geo(&h);
    let sh = h.state().session.active().unwrap().wb.active().unwrap().clone();
    let header = pos2(g.x(&sh, 1) + sh.col_width(1) * g.z / 2.0, g.rect.top() + g.header_h / 2.0);
    button(&mut h, header, PointerButton::Secondary, true);
    button(&mut h, header, PointerButton::Secondary, false);
    h.run_steps(2);
    assert!(h.query_by_label("Delete…").is_none(), "the viewer's menu has no editing");
    let hide = h.get_by_label("Hide").rect().center();
    click(&mut h, hide);
    assert!(h.state().session.active().unwrap().wb.active().unwrap().is_col_hidden(1));
    assert_clean(&h, "Hide");

    // Typing into a cell is still refused, with the read-only toast.
    h.state_mut().session.execute("selection.set", json!({"range": "A1"})).unwrap();
    h.input_mut().events.push(Event::Key {
        key: Key::Delete,
        physical_key: Some(Key::Delete),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    h.run_steps(2);
    assert_eq!(h.state().toast.as_ref().map(|t| t.0.as_str()), Some(gridcraft_engine::viewer::REASON));
    assert_eq!(cells(&h), before);
    assert!(!h.state().session.active().unwrap().is_dirty());
}
