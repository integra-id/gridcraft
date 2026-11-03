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

#[test]
fn samples_open_from_their_stored_copies() {
    let mut s = Session::new();
    for (name, title) in crate::sample::SAMPLES {
        let i = crate::sample::open(&mut s, name).unwrap_or_else(|| panic!("{name} opens"));
        let d = s.doc().unwrap();
        assert_eq!(s.active_index(), i);
        assert_eq!(d.display_title(), *title);
        assert!(!d.is_dirty() && d.undo.is_empty(), "{name}");
    }
    assert!(crate::sample::open(&mut s, "nope").is_none());
    // The sales sample's totals are formulas with their values.
    crate::sample::open(&mut s, "sales").unwrap();
    let d = s.doc().unwrap();
    let sh = d.wb.active().unwrap();
    assert_eq!(sh.name, "Sales");
    assert!(matches!(sh.value(CellRef::parse("G5").unwrap()), Value::Number(n) if n > 0.0), "{:?}", sh.value(CellRef::parse("G5").unwrap()));
}

#[test]
fn a_sample_can_be_searched_navigated_and_exported() {
    let mut s = Session::new();
    crate::sample::open(&mut s, "sales").unwrap();
    let found = s.execute("edit.find", json!({"what": "North", "all": true})).unwrap();
    assert!(found["count"].as_u64().is_some_and(|n| n >= 1), "{found}");
    s.execute("edit.goTo", json!({"reference": "G5"})).unwrap();
    s.execute("sheet.next", json!({})).unwrap();
    s.execute("sheet.activate", json!({"sheet": 0})).unwrap();
    let (bytes, pages) = {
        let r = s.execute("file.exportPdf", json!({})).unwrap();
        (crate::io::base64_decode(r["base64"].as_str().unwrap()).unwrap(), r["pages"].as_u64().unwrap())
    };
    assert!(bytes.starts_with(b"%PDF") && pages >= 1);
    s.execute("file.printPreview", json!({})).unwrap();
    for format in ["xlsx", "csv", "html", "json"] {
        let r = s.execute("file.saveBytes", json!({"format": format})).unwrap();
        assert!(r["bytes"].as_u64().is_some_and(|n| n > 0), "{format}: {r}");
    }
    let d = s.doc().unwrap();
    assert!(!d.is_dirty() && d.undo.is_empty(), "viewing leaves the workbook clean");
}

/// The web viewer leaves out both editing and the macro runtime, yet a copy it saves keeps the
/// workbook's macros.
#[test]
fn a_saved_copy_keeps_the_workbooks_macros() {
    let mut s = Session::new();
    let mut wb = Workbook::new();
    let m = crate::macros::parse_macro(
        &json!({"name": "Fill", "description": "Puts 1 in A1", "steps": [{"command": "cell.set", "params": {"cell": "A1", "input": "1"}}]}),
    )
    .unwrap();
    wb.macros.push(m);
    s.add_document(DocState::new(wb, None, "Book".into(), Trust::Local));
    let r = s.execute("file.saveBytes", json!({"format": "xlsx"})).unwrap();
    s.execute("file.open", json!({"name": "Copy.xlsx", "base64": r["base64"]})).unwrap();
    let d = s.doc().unwrap();
    assert_eq!(d.display_title(), "Copy.xlsx");
    let names: Vec<&str> = d.wb.macros.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["Fill"], "the reopened copy has the macro");
    assert_eq!(d.wb.macros[0].description, "Puts 1 in A1");
}
