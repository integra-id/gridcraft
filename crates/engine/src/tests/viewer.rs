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
    for spec in allowed {
        for params in [json!({}), json!({"range": "B2:C4", "cell": "B2", "what": "a"})] {
            let _ = s.execute(spec.id, params);
            s.ui_requests.clear();
            assert_eq!(s.documents().len(), 1, "{} opened or closed a workbook", spec.id);
            let d = s.doc().unwrap();
            assert!(content(&d.wb) == before, "{} changed the workbook", spec.id);
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
