//! A build without the editing commands (`viewer::EDITING_BUILD` false, as in the web viewer): the
//! editing commands are still listed and refuse as in viewer mode, and the session views
//! workbooks without the `viewer` flag.

use gridcraft_core::{CellRef, Value};
use gridcraft_model::Workbook;
use serde_json::json;

use crate::{DocState, EngineError, Session, Trust, command_specs, find_command, viewer};

/// A session with one workbook holding a number, opened as a file would be.
fn session() -> Session {
    let mut s = Session::new();
    let mut wb = Workbook::new();
    if let Some(sh) = wb.sheet_mut(0) {
        sh.set_value(CellRef::parse("A1").unwrap(), Value::Number(5.0));
    }
    s.add_document(DocState::new(wb, None, "Book".into(), Trust::Local));
    s
}

#[test]
fn editing_commands_are_listed_and_refused_with_the_viewers_reason() {
    let mut s = session();
    assert!(!s.viewer, "the build refuses without the flag");
    for id in ["cell.set", "edit.cut", "edit.undo", "file.new", "file.save", "home.bold", "sheet.hide", "macro.record"] {
        let spec = find_command(id).unwrap_or_else(|| panic!("{id} is listed"));
        assert!(viewer::refuses(spec), "{id}");
        let err = s.execute(id, json!({"cell": "A1", "input": "7"})).unwrap_err();
        assert!(matches!(&err, EngineError::Disabled(got, why) if got == id && why == viewer::REASON), "{id}: {err}");
    }
    // A nested run of one gets the same reason instead of running.
    let spec = find_command("cell.set").unwrap();
    let err = (spec.run)(&mut s, &json!({"cell": "A1", "input": "7"})).unwrap_err();
    assert!(err.to_string().contains(viewer::REASON), "{err}");
    let d = s.doc().unwrap();
    assert_eq!(d.wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Number(5.0));
    assert!(!d.is_dirty() && d.undo.is_empty());
}

#[test]
fn every_command_the_viewer_allows_is_kept() {
    for spec in command_specs() {
        assert_eq!(viewer::kept_without_editing(spec.id, spec.undoable), !viewer::refuses(spec), "{}", spec.id);
    }
}

#[test]
fn reading_commands_still_run() {
    let mut s = session();
    s.execute("selection.set", json!({"range": "A1:B2"})).unwrap();
    s.execute("edit.copy", json!({})).unwrap();
    s.execute("view.zoom", json!({"percent": 150})).unwrap();
    s.execute("formulas.calculateNow", json!({})).unwrap();
    let d = s.doc().unwrap();
    assert!(!d.is_dirty() && d.undo.is_empty(), "viewing leaves the workbook clean");
}
