//! The viewer through real egui input: the toolbar at narrow widths.

use egui::{Event, Modifiers};
use egui_kittest::kittest::Queryable as _;
use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

/// A workbook with A1 = 5 opened in the viewer.
fn viewer_session() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "5"})).unwrap();
    s.execute("selection.set", json!({"range": "A1"})).unwrap();
    s.viewer = true;
    s
}

fn viewer_harness(width: f32) -> Harness {
    let builder = crate::common::builder().with_size(egui::vec2(width, 700.0));
    crate::common::harness_with(builder, crate::common::app(viewer_session()), 4)
}

fn click(h: &mut Harness, at: egui::Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }
    h.run_steps(2);
}

/// The toolbar's buttons, by their accessible names (a label, or the tooltip when icon only).
const BUTTONS: [(&str, &str); 6] = [
    ("File", "File: open, print and export"),
    ("Open", "Open a workbook (Ctrl+O)"),
    ("Export", "Export a copy as PDF, CSV or Excel workbook"),
    ("Print", "Print (Ctrl+P)"),
    ("Find", "Find (Ctrl+F)"),
    ("Fit", "Fit the sheet's data in the window"),
];

fn button_rect(h: &Harness, label: &str, tip: &str) -> egui::Rect {
    let mut found = h.query_all_by_label(label).chain(h.query_all_by_label(tip));
    found.next().unwrap_or_else(|| panic!("no toolbar button {label}")).rect()
}

#[test]
fn the_toolbar_fits_and_find_works_at_every_width() {
    for width in [320.0, 380.0, 460.0, 560.0, 880.0, 1200.0] {
        let mut h = viewer_harness(width);
        let rects: Vec<_> = BUTTONS.iter().map(|(label, tip)| (*label, button_rect(&h, label, tip))).collect();
        for (i, (a, ra)) in rects.iter().enumerate() {
            assert!(ra.right() <= width + 0.5, "{width}: {a} is inside the window");
            for (b, rb) in &rects[i + 1..] {
                assert!(!ra.intersects(rb.shrink(0.5)), "{width}: {a} {ra:?} overlaps {b} {rb:?}");
            }
        }
        let (label, tip) = BUTTONS[4];
        let at = button_rect(&h, label, tip).center();
        // A real pointer click at the button's middle: nothing else is on top of it.
        click(&mut h, at);
        assert_eq!(h.state().dialog.as_ref().map(|d| d.name.as_str()), Some("find"), "{width}: Find opens");
    }
}
