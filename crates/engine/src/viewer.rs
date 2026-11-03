//! Viewer mode: a session that shows workbooks and never edits them.
//!
//! GridCraft Viewer (the `gridcraft` and `gridcraft-web` apps on the `office-viewer` branch, see
//! `docs/viewer.md`) sets [`crate::Session::viewer`]. In viewer mode [`crate::Session::execute`]
//! refuses every command that records an undo step (every edit) except zoom, and the commands
//! without one that start editing, write the open file in place, make a new workbook to edit, or
//! record and change macros ([`EDITING`]). What is left is what a viewer needs: opening and
//! closing workbooks, exporting a copy ([`crate::cmd`]'s `file.saveCopy`, Export PDF/CSV/HTML),
//! printing, selection, Copy, Find and Go To, sheet switching, recalculation, the window commands
//! and the view adjustments in [`VIEW`] (zoom, widths and heights, hiding, freezing, Wrap Text,
//! filtering), which change no value or formula and never reach a copy. [`refuses`] also greys the commands out ([`crate::CommandSpec::info`]).
//!
//! `tests::viewer` runs every allowed command on a sample workbook and checks the workbook is
//! unchanged, so a command that edits after all can't slip through the lists.

use serde_json::Value as Json;

use crate::cmd::CommandSpec;
use crate::{EngineError, Result, Session};

/// Whether this build has the editing commands (the `editing` feature, on by default). Without it
/// every command [`refuses`] refuses keeps its place in the command list (id, label, menu,
/// shortcut, params) but has no code: [`crate::Session::execute`] refuses it with [`REASON`] as in
/// viewer mode, and the session is always a viewer. GridCraft Viewer's web build leaves it out.
pub const EDITING_BUILD: bool = cfg!(feature = "editing");

/// Why a refused command is unavailable in viewer mode.
pub const REASON: &str = "GridCraft Viewer only views workbooks (editing is turned off)";

/// Undoable commands the viewer allows: they change how the window shows the sheet, never a
/// cell's value or formula. Zoom; gridlines and headings; freezing panes; column widths and row
/// heights (dragging a header border, AutoFit), hiding and unhiding rows and columns; Wrap Text;
/// filtering with an AutoFilter or a table's filter buttons (rows are hidden, none is moved or
/// changed: Sort, which moves cells, stays refused, and so does adding or removing a filter); and
/// expanding or collapsing outline groups.
///
/// They are view-level and last for the session: [`crate::Session::execute`] keeps the workbook
/// clean and drops their undo steps, keeps the workbook as it was before the first of them
/// ([`crate::DocState::as_opened`]), and the commands that write a copy ([`COPIES`]) write that
/// one, so an exported workbook, CSV or web page holds the file's own content. PDF and Print show
/// the sheet as it is on the screen.
pub const VIEW: &[&str] = &[
    "view.zoom",
    "view.zoom100",
    "view.zoomToSelection",
    "view.gridlines",
    "view.headings",
    "view.freezePanes",
    "view.freezeTopRow",
    "view.freezeFirstColumn",
    "view.unfreezePanes",
    "home.columnWidth",
    "home.rowHeight",
    "home.autofitColumnWidth",
    "home.autofitRowHeight",
    "home.hideRows",
    "home.hideColumns",
    "home.unhideRows",
    "home.unhideColumns",
    "home.wrapText",
    "data.filterBy",
    "data.clearFilter",
    "data.reapply",
    "data.showDetail",
    "data.hideDetail",
    "data.showLevels",
];

/// Commands that write a copy of the workbook's content: in the viewer they write the workbook
/// as opened, without the session's view adjustments ([`VIEW`], [`as_opened`]).
pub const COPIES: &[&str] = &["file.saveCopy", "file.exportCsv", "file.exportHtml"];

/// The workbook to write for a copy: `opened` (before the session's view adjustments) where the
/// user is now in `current`: the active sheet, and each sheet's zoom, view mode, active cell and
/// scroll position.
pub fn as_opened(opened: &gridcraft_model::Workbook, current: &gridcraft_model::Workbook) -> gridcraft_model::Workbook {
    let mut wb = opened.clone();
    for sh in &mut wb.sheets {
        if let Some(now) = current.sheets.iter().find(|c| c.uid == sh.uid) {
            let sh = std::sync::Arc::make_mut(sh);
            sh.zoom = now.zoom;
            sh.view = now.view;
            sh.view_active = now.view_active;
            sh.view_top_left = now.view_top_left;
        }
    }
    if let Some(i) = current.active().and_then(|a| wb.sheets.iter().position(|s| s.uid == a.uid)) {
        wb.set_active_sheet(i);
    }
    wb
}

/// Commands without an undo step that the viewer refuses all the same: they edit the workbook on
/// a later step (Undo, Redo, Cut, starting a cell edit, the ink tools), change it without an undo
/// step (custom views, a split's frozen panes, workbook properties), write the open file in place
/// (Save, Save As), make a new workbook to edit (New), or record macros and scripts.
pub const EDITING: &[&str] = &[
    // Undo history: nothing to undo in a viewer.
    "edit.undo",
    "edit.redo",
    // Cut moves cells on the next paste; Copy stays.
    "edit.cut",
    "edit.beginEdit",
    // Writing the open file in place, or a new workbook to edit. `file.saveCopy` and the exports
    // write a copy and stay.
    "file.new",
    "file.save",
    "file.saveAs",
    "file.properties",
    // Change the workbook without an undo step.
    "view.customViews",
    "view.split",
    // Ink.
    "draw.pen",
    "draw.eraser",
    "draw.lasso",
    // Recording and trusting macros and scripts, and the custom dictionary.
    "automate.recordActions",
    "automate.stopAction",
    "automate.trustScripts",
    "macro.record",
    "review.addToDictionary",
];

/// Whether viewer mode refuses `spec`.
pub fn refuses(spec: &CommandSpec) -> bool {
    (spec.undoable && !VIEW.contains(&spec.id)) || EDITING.contains(&spec.id)
}

/// Whether viewer mode refuses the command `id` (unknown ids are refused).
pub fn refuses_id(id: &str) -> bool {
    crate::find_command(id).is_none_or(refuses)
}

/// Whether a build without the editing commands ([`EDITING_BUILD`]) keeps command `id`'s code:
/// [`refuses`], worked out at compile time from the id and whether the command is undoable.
pub const fn kept_without_editing(id: &str, undoable: bool) -> bool {
    if undoable { contains(VIEW, id) } else { !contains(EDITING, id) }
}

/// Whether `list` holds `id`, at compile time.
const fn contains(list: &[&str], id: &str) -> bool {
    let mut i = 0;
    while i < list.len() {
        if same(list[i], id) {
            return true;
        }
        i += 1;
    }
    false
}

/// Whether two strings are equal, at compile time.
const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The code of a command a build without the editing commands leaves out ([`EDITING_BUILD`]).
/// `Session::execute` refuses such a command before it gets here; a nested run gets the same
/// reason.
pub fn left_out(_: &mut Session, _: &Json) -> Result<Json> {
    Err(EngineError::Other(REASON.to_string()))
}
