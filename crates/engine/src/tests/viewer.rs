//! Viewer mode ([`crate::viewer`]): every editing command is refused and leaves the workbook as it
//! was; everything else runs and doesn't change the workbook either.

use std::sync::Arc;

use gridcraft_core::{CellRef, Value};
use gridcraft_model::Workbook;
use serde_json::json;

use crate::{EngineError, Session, command_specs, viewer};

/// The sample workbook in a viewer session, opened as a file would be: clean, with no history.
fn viewer_session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"sample": "sales"})).unwrap();
    s.execute("cell.set", json!({"cell": "Z1", "input": "2"})).unwrap();
    s.execute("cell.set", json!({"cell": "Z2", "input": "=Z1*3"})).unwrap();
    s.execute("selection.set", json!({"range": "B2:C4"})).unwrap();
    s.viewer = true;
    let d = s.doc_mut().unwrap();
    d.undo.clear();
    d.redo.clear();
    d.saved = d.wb.clone();
    s
}

/// The workbook without what only changes how a window shows it: zoom, view mode, the active
/// sheet and cell, and the scroll position.
fn content(wb: &Workbook) -> Workbook {
    let mut wb = wb.clone();
    wb.set_active_sheet(0);
    for sh in &mut wb.sheets {
        let sh = Arc::make_mut(sh);
        sh.zoom = 100;
        sh.view = Default::default();
        sh.view_active = CellRef::default();
        sh.view_top_left = CellRef::default();
    }
    wb
}

/// Every cell's stored content, by sheet: its formula, or its value when it has none. No viewer
/// command may change it. Styles, widths, hidden rows and other view adjustments
/// ([`viewer::VIEW`]) are left out, and so are formula results: `SUBTOTAL(109, …)` leaves out
/// hidden rows, so it reads differently while rows are hidden or filtered out, as in Excel.
fn values(wb: &Workbook) -> Vec<(String, CellRef, Value, Option<String>)> {
    let mut all = wb
        .sheets
        .iter()
        .flat_map(|sh| {
            sh.cells.iter().filter(|(_, c)| c.value != Value::Empty || c.formula.is_some()).map(|(at, c)| match &c.formula {
                Some(f) => (sh.name.clone(), at, Value::Empty, Some(f.text.clone())),
                None => (sh.name.clone(), at, c.value.clone(), None),
            })
        })
        .collect::<Vec<_>>();
    all.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    all
}

/// Commands the allowed-command sweep doesn't run: they open, close or switch workbooks and
/// windows, write files, or need the interface.
const SWEEP_SKIPS: &[&str] = &[
    "file.open",
    "file.close",
    "file.saveCopy",
    "file.exportCsv",
    "file.exportHtml",
    "window.close",
    "window.activate",
    "view.newWindow",
    "view.hideWindow",
    "insert.picture",
    "pageLayout.background",
];

#[test]
fn refused_commands_leave_the_workbook_alone() {
    let refused: Vec<_> = command_specs().iter().filter(|c| viewer::refuses(c)).collect();
    assert!(refused.len() > 250, "only {} commands refused", refused.len());
    let mut s = viewer_session();
    let before = s.doc().unwrap().wb.clone();
    for spec in refused {
        for params in [json!({}), json!({"range": "B2:C4", "cell": "B2", "input": "1", "path": "x.xlsx"})] {
            match s.execute(spec.id, params) {
                Err(EngineError::Disabled(id, why)) => assert!(id == spec.id && why == viewer::REASON, "{}: {why}", spec.id),
                other => panic!("{} ran in the viewer: {other:?}", spec.id),
            }
        }
        let info = spec.info(&s);
        assert!(!info.enabled && info.disabled_reason.as_deref() == Some(viewer::REASON), "{} not greyed out", spec.id);
        let d = s.doc().unwrap();
        assert!(Arc::ptr_eq(&d.wb, &before), "{} changed the workbook", spec.id);
        assert!(d.undo.is_empty() && d.redo.is_empty() && !d.is_dirty(), "{} left history", spec.id);
    }
}

#[test]
fn allowed_commands_dont_edit() {
    let mut s = viewer_session();
    let before = content(&s.doc().unwrap().wb);
    let allowed: Vec<_> = command_specs().iter().filter(|c| !viewer::refuses(c) && !SWEEP_SKIPS.contains(&c.id)).collect();
    assert!(allowed.len() > 50, "only {} commands allowed", allowed.len());
    let cells = values(&before);
    for spec in allowed {
        let adjusts = viewer::VIEW.contains(&spec.id);
        for params in [json!({}), json!({"range": "B2:C4", "cell": "B2", "what": "a"})] {
            let _ = s.execute(spec.id, params);
            s.ui_requests.clear();
            assert_eq!(s.documents().len(), 1, "{} opened or closed a workbook", spec.id);
            let d = s.doc().unwrap();
            if adjusts {
                // A view adjustment changes how the sheet shows, never a value or formula, and
                // the workbook as opened stays as it was.
                assert!(values(&d.wb) == cells, "{} changed a cell", spec.id);
                assert!(d.as_opened.as_deref().is_none_or(|w| content(w) == before), "{} changed the workbook as opened", spec.id);
            } else {
                assert!(content(&d.wb) == before || d.as_opened.is_some(), "{} changed the workbook", spec.id);
                assert!(values(&d.wb) == cells, "{} changed a cell", spec.id);
            }
            assert!(d.undo.is_empty() && !d.is_dirty(), "{} edited (undo {}, dirty {})", spec.id, d.undo.len(), d.is_dirty());
        }
    }
}

#[test]
fn viewer_selects_copies_finds_zooms_and_computes() {
    let mut s = viewer_session();
    s.execute("selection.set", json!({"range": "A1:B3"})).unwrap();
    let copied = s.execute("edit.copy", json!({})).unwrap();
    assert!(!copied["text"].as_str().unwrap_or("").is_empty(), "{copied}");
    let found = s.execute("edit.find", json!({"what": "Z1*3", "all": true})).unwrap();
    assert!(found.to_string().contains("Z2"), "{found}");
    s.execute("view.zoom", json!({"percent": 150})).unwrap();
    s.execute("view.zoom100", json!({})).unwrap();
    s.execute("formulas.calculateNow", json!({})).unwrap();
    let d = s.doc().unwrap();
    assert!(!d.is_dirty() && d.undo.is_empty());
    // Formulas compute.
    assert_eq!(d.wb.active().unwrap().value(CellRef::parse("Z2").unwrap()), Value::Number(6.0));
    // Typing a formula is refused, so the viewer can't change what they compute.
    assert!(s.execute("cell.set", json!({"cell": "Z1", "input": "=1+1"})).is_err());
}

#[test]
fn viewer_saves_a_copy_and_keeps_its_file() {
    let dir = std::env::temp_dir().join(format!("gridcraft-viewer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let own = dir.join("own.xlsx");
    let mut editor = Session::new();
    editor.execute("file.new", json!({"sample": "sales"})).unwrap();
    editor.execute("file.saveAs", json!({"path": own})).unwrap();

    let mut s = Session::new();
    s.viewer = true;
    s.execute("file.open", json!({"path": own})).unwrap();
    let own_path = s.doc().unwrap().path.clone();
    let title = s.doc().unwrap().display_title();
    assert!(s.execute("file.save", json!({})).is_err());
    assert!(s.execute("file.saveAs", json!({"path": dir.join("as.xlsx")})).is_err());
    assert!(s.execute("file.saveCopy", json!({"path": own})).is_err(), "a copy replaced the open file");
    assert!(s.execute("file.saveCopy", json!({})).is_err());
    for name in ["copy.xlsx", "copy.csv", "copy.json"] {
        s.execute("file.saveCopy", json!({"path": dir.join(name)})).unwrap();
        assert!(dir.join(name).is_file(), "{name}");
    }
    s.execute("file.exportCsv", json!({"path": dir.join("export.csv")})).unwrap();
    let d = s.doc().unwrap();
    assert_eq!(d.path, own_path);
    assert_eq!(d.display_title(), title);
    assert!(!d.is_dirty() && d.undo.is_empty());

    let mut check = Session::new();
    check.execute("file.open", json!({"path": dir.join("copy.xlsx")})).unwrap();
    let a = &s.doc().unwrap().wb;
    let b = &check.doc().unwrap().wb;
    assert_eq!(a.sheets.len(), b.sheets.len());
    let at = CellRef::parse("B2").unwrap();
    assert_eq!(a.active().unwrap().value(at), b.active().unwrap().value(at));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The view adjustments a viewer makes: they last for the session.
fn adjust(s: &mut Session) {
    for (id, params) in [
        ("home.columnWidth", json!({"cols": "B:B", "width": 300})),
        ("home.rowHeight", json!({"rows": "2:2", "height": 60})),
        ("home.autofitColumnWidth", json!({"cols": "C:C"})),
        ("home.hideRows", json!({"rows": "5:6"})),
        ("home.hideColumns", json!({"cols": "E:E"})),
        ("home.wrapText", json!({"range": "B2:C4", "on": true})),
        ("view.gridlines", json!({"on": false})),
        ("view.headings", json!({"on": false})),
        ("view.freezePanes", json!({"cell": "B5"})),
    ] {
        s.execute(id, params).unwrap_or_else(|e| panic!("{id}: {e}"));
    }
}

#[test]
fn view_adjustments_show_the_sheet_differently_and_keep_it_clean() {
    let mut s = viewer_session();
    let opened = s.doc().unwrap().wb.clone();
    adjust(&mut s);
    let d = s.doc().unwrap();
    let sh = d.wb.active().unwrap();
    assert!((sh.col_width(1) - 300.0).abs() < 1.0, "{}", sh.col_width(1));
    assert!(sh.is_row_hidden(4) && sh.is_row_hidden(5) && sh.is_col_hidden(4));
    assert!(!sh.show_gridlines && !sh.show_headings && sh.freeze.is_some());
    let at = CellRef::parse("B2").unwrap();
    assert!(d.wb.styles.get(sh.cell(at).map(|c| c.style).unwrap_or_default()).align.wrap);
    // No value or formula changed, nothing to save or undo, and the workbook as opened is kept.
    assert!(values(&d.wb) == values(&opened));
    assert!(!d.is_dirty() && d.undo.is_empty() && d.redo.is_empty());
    assert!(d.as_opened.as_ref().is_some_and(|w| Arc::ptr_eq(w, &opened)));
    // Unhiding and unfreezing are view adjustments too.
    s.execute("home.unhideRows", json!({"rows": "4:7"})).unwrap();
    s.execute("view.unfreezePanes", json!({})).unwrap();
    let sh = s.doc().unwrap().wb.active().unwrap().clone();
    assert!(!sh.is_row_hidden(4) && sh.freeze.is_none());
    // Editing is still refused, Sort and adding a filter too.
    for (id, params) in [
        ("cell.set", json!({"cell": "B2", "input": "x"})),
        ("data.sortAscending", json!({"range": "B4:H10"})),
        ("data.filter", json!({"range": "B4:H10"})),
        ("home.bold", json!({"range": "B2"})),
        ("home.insertRows", json!({"rows": "3:3"})),
        ("edit.undo", json!({})),
    ] {
        assert!(matches!(s.execute(id, params), Err(EngineError::Disabled(..))), "{id} ran");
    }
    assert!(values(&s.doc().unwrap().wb) == values(&opened));
}

#[test]
fn viewer_filters_rows_without_changing_them() {
    let mut s = viewer_session();
    let opened = s.doc().unwrap().wb.clone();
    let Some(region) =
        opened.active().unwrap().tables.first().map(|t| t.range).or_else(|| opened.active().unwrap().autofilter.as_ref().map(|a| a.range))
    else {
        panic!("the sales sample has a table or a filter")
    };
    s.execute("selection.set", json!({"range": CellRef::new(region.start.row, region.start.col).a1()})).unwrap();
    let col = gridcraft_core::col_to_letters(region.start.col);
    let first = opened.active().unwrap().value(CellRef::new(region.start.row + 1, region.start.col));
    let Value::Text(first) = first else { panic!("{first:?}") };
    s.execute("data.filterBy", json!({"column": col, "values": [first.to_string()]})).unwrap();
    let sh = s.doc().unwrap().wb.active().unwrap().clone();
    let hidden = (region.start.row + 1..=region.end.row).filter(|r| sh.is_row_hidden(*r)).count();
    assert!(hidden > 0, "nothing filtered out");
    assert!(values(&s.doc().unwrap().wb) == values(&opened));
    assert!(!s.doc().unwrap().is_dirty() && s.doc().unwrap().undo.is_empty());
    s.execute("data.clearFilter", json!({})).unwrap();
    let sh = s.doc().unwrap().wb.active().unwrap().clone();
    assert!((region.start.row + 1..=region.end.row).all(|r| !sh.is_row_hidden(r)));
}

#[test]
fn copies_hold_the_workbook_as_opened() {
    let dir = std::env::temp_dir().join(format!("gridcraft-viewer-view-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = viewer_session();
    let opened = s.doc().unwrap().wb.clone();
    adjust(&mut s);
    s.execute("view.zoom", json!({"percent": 150})).unwrap();
    s.execute("file.saveCopy", json!({"path": dir.join("copy.xlsx")})).unwrap();
    s.execute("file.exportCsv", json!({"path": dir.join("copy.csv")})).unwrap();
    // The session still shows its adjustments, and stays clean.
    let d = s.doc().unwrap();
    assert!(d.wb.active().unwrap().is_row_hidden(4) && !d.is_dirty() && d.undo.is_empty());

    let mut check = Session::new();
    check.execute("file.open", json!({"path": dir.join("copy.xlsx")})).unwrap();
    let (a, b) = (opened.active().unwrap(), check.doc().unwrap().wb.active().unwrap().clone());
    assert!((b.col_width(1) - a.col_width(1)).abs() < 1.0, "{} vs {}", b.col_width(1), a.col_width(1));
    assert!(!b.is_row_hidden(4) && !b.is_col_hidden(4) && b.show_gridlines == a.show_gridlines && b.show_headings == a.show_headings);
    assert_eq!(b.freeze, a.freeze);
    let at = CellRef::parse("B2").unwrap();
    let wrap = |wb: &Workbook, sh: &gridcraft_model::Sheet| wb.styles.get(sh.cell(at).map(|c| c.style).unwrap_or_default()).align.wrap;
    assert_eq!(wrap(&check.doc().unwrap().wb, &b), wrap(&opened, a));
    // Where the user is goes along: the zoom.
    assert_eq!(b.zoom, 150);
    let csv = std::fs::read_to_string(dir.join("copy.csv")).unwrap();
    assert!(csv.lines().count() >= 10, "{csv}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_editor_session_is_not_a_viewer() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "5"})).unwrap();
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Number(5.0));
    assert!(viewer::refuses_id("cell.set") && viewer::refuses_id("no.such") && !viewer::refuses_id("edit.copy"));
}

/// Without the macro runtime (the web viewer's build) the macro commands are there and say why
/// they don't run, while a workbook's macros are still read and listed.
#[cfg(not(feature = "macros"))]
#[test]
fn a_build_without_the_macro_runtime_refuses_to_run_macros() {
    let mut s = Session::new();
    s.new_workbook();
    let steps = json!([{"type": "command", "id": "cell.set", "params": {"cell": "A1", "input": "1"}}]);
    s.execute("macro.save", json!({"macro": {"name": "M", "steps": steps}})).unwrap();
    assert_eq!(s.execute("macro.list", json!({})).unwrap().as_array().map(Vec::len), Some(1));
    for (id, params) in [
        ("macro.run", json!({"name": "M"})),
        ("macro.check", json!({"name": "M"})),
        ("macro.record", json!({})),
        ("macro.debug", json!({"action": "start", "name": "M"})),
    ] {
        let err = s.execute(id, params).unwrap_err();
        assert!(matches!(&err, EngineError::Disabled(_, why) if why == crate::macros::NO_RUNTIME), "{id}: {err}");
    }
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Empty);
}
