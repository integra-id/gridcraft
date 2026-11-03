# GridCraft Viewer

> **Branch:** `office-viewer` · **Last updated:** 2026-11-03

On this branch the `gridcraft` desktop app and the `gridcraft-web` browser app are **GridCraft
Viewer**. It opens workbooks read-only, and you can print them or export a copy as PDF, CSV or
an Excel workbook. Editing and the ribbon are turned off. The importers, the calculation engine,
the charts and the PDF/print renderers are the same code the editor uses, so workbooks look and
compute exactly as they do in GridCraft.

## Build and run

```sh
cargo run --release -p gridcraft -- book.xlsx      # open a workbook in the viewer
cargo run --release -p gridcraft -- --sample sales # a built-in sample (budget, sales, grades)
cargo run --release -p gridcraft                   # empty window with an Open… button
cd apps/gridcraft-web && trunk build --release     # the browser viewer ([profile.web], see below)
cargo run -p gridcraft-ui-egui --example snapshot -- --viewer --in book.xlsx out.png   # offscreen
```

## What it does

| | |
|---|---|
| **Opens** | everything GridCraft opens (`.xlsx`, `.xlsm`, `.xls`, `.xlsb`, `.ods`, `.csv`, …), password-protected workbooks included. Use the command line, the toolbar's **Open**, File › Open, the recent-files list or drag and drop |
| **Exports** | the toolbar's **Export ▾** or File › Export: PDF, CSV (the active sheet), Excel workbook copy (`file.saveCopy`) and Web Page. Exporting writes a new file. The open workbook keeps its path, title and clean state. In the browser each export is a download |
| **Prints** | the toolbar's **Print** or File › Print |
| **Navigates** | scrolling, arrows, Page Up/Down, Ctrl+arrows, sheet tabs (click, the ◀ ▶ arrows, the sheet list), Go To (Ctrl+G), Find (Ctrl+F, without Replace), and zoom (the toolbar's − / % / + / Fit, the status-bar slider, Ctrl+wheel, the Zoom dialog) |
| **Selects and copies** | mouse and keyboard selection, rows and columns, Ctrl+A, and Ctrl+C or the context menu's Copy. The status bar shows the active cell and what it holds (`G5: =SUM(C5:F5)`), because the viewer has no formula bar |
| **Computes** | formulas recalculate as they do in the editor (`formulas.calculateNow`, volatile functions, charts, sparklines and conditional formats) |
| **Adjusts the view** | column widths and row heights (drag or double-click a header border for AutoFit), Wrap Text, Freeze Panes, Gridlines and Headings, hiding and unhiding rows and columns, AutoFilter on existing filters and tables (without Sort), and the Normal, Page Layout and Page Break Preview views. These last for the session only: see below |

## View adjustments

The viewer can change how a sheet shows without changing what it holds
(`gridcraft_engine::viewer::VIEW`): zoom; Gridlines and Headings; Freeze Panes, Freeze Top Row,
Freeze First Column and Unfreeze; column width and row height (`home.columnWidth`,
`home.rowHeight`, AutoFit); Hide and Unhide rows and columns; Wrap Text; filtering the rows of an
AutoFilter or a table (`data.filterBy`, `data.clearFilter`, `data.reapply`); showing and hiding
outline levels; and the Normal, Page Layout and Page Break Preview views.

- **Where**: the toolbar's **View ▾** menu (Gridlines, Headings, the three views, the freeze
  commands, Wrap Text, AutoFit, Hide/Unhide, Clear Filter). It works on the selection, and at
  narrow widths it keeps its icon like the other buttons. The right-click menu on whole columns
  or rows offers AutoFit, Hide and Unhide; on cells it offers Filter by Selected Cell's Value.
  Both also offer Wrap Text and Freeze or Unfreeze Panes. Drag a header border to resize, and
  double-click it to AutoFit (or to unhide a hidden line). Drag the freeze lines to move them.
  The filter button on an AutoFilter or table header opens the filter list without **Sort**.
- **Session only**: no value, formula or stored setting is written anywhere. The workbook stays
  clean, leaves no undo step and closing never asks to save (`Session::execute` drops the steps
  and keeps `saved` in step). Formula results can read differently while rows are hidden or
  filtered out, because `SUBTOTAL(10x, …)` and `AGGREGATE` leave hidden rows out, as in Excel.
- **Copies keep the original**: the Excel workbook copy, CSV and Web Page exports
  (`viewer::COPIES`) write the workbook as it was opened (`DocState::as_opened`, kept from the
  first adjustment), so widths, hidden rows, filters, wrapping and freezing don't go along. Only
  the zoom, the sheet view and the active sheet carry over (`viewer::as_opened`).
- **PDF and Print show the screen**: they print the sheet as you see it, with hidden rows and
  columns left out, the widths, wrapping and filters you set, and the gridlines and headings as
  the page setup asks for them. To print the original, reopen the workbook.
- **Still refused**: every edit. Sort (it moves cells), adding or removing an AutoFilter
  (`data.filter`), the Column Width and Row Height dialogs, and moving cells or columns by drag.
  There is no full-screen command in GridCraft, so the viewer has none (the browser's F11 works).

## The window

There is one 40 px toolbar (`crates/ui-egui/src/viewer.rs`) in place of the title bar, the
ribbon and the formula bar:

- On the left: **File** (the File page on Open), **Open**, **Export ▾** (PDF, CSV, Excel
  workbook copy) and **Print**.
- In the centre: the workbook's name and a green **Read only** badge.
- On the right: **View ▾** (see *View adjustments*), **Find**, **−**, the zoom percentage (click it for 100%), **+** and **Fit** (fits the
  sheet's used range in the window, 25–200%).
- An embedder can hide or grey out any of these buttons (*Hiding or disabling features*).
- In a narrow window the buttons drop their labels (each keeps its icon and tooltip; **Fit**
  keeps its label), then the zoom steps (− % +), so the two groups never overlap
  (`viewer::bar_fit`). The workbook's name goes before the badge does, and neither spills over
  the buttons (`viewer::title_fit`).

Below the toolbar come the notices, the grid, the sheet tabs and the status bar. With nothing
open the window shows *GridCraft Viewer* and an **Open a workbook…** button. File › Open also
lists the built-in samples (budget, sales, grades). One opens read-only, in place of the empty
window, through `sample::open` (`viewer::open_sample`). The window title is
`<name> - GridCraft Viewer`.

## Hiding or disabling features (embedders)

A host that embeds the viewer can leave features out (**hidden**: not drawn) or grey them out
(**disabled**: drawn, but they don't respond). Either way the feature is off in function too: its
keyboard shortcuts, menu items, command search entries and dialogs do nothing (a disabled one
says *This is turned off here.* in a toast; a hidden one is silent). Everything is shown by
default.

- **Web:** `?hide=open,export,print&disable=pdf` (see `packaging/web/README.md`). A host page in
  `?host=parent` mode can send the same lists later as a `gridcraft:config` message.
- **Desktop:** `gridcraft --hide=open,export,print --disable=pdf book.xlsx` (`--hide open` works
  too; repeated flags add up).

Lists are comma-separated, case-insensitive and may have spaces. A feature in both lists is
hidden. Unknown names are ignored with a warning in the log (the browser console on the web).
The parsing and gating live in `crates/ui-egui/src/features.rs` (`Features`).

| Name | What it covers |
|---|---|
| `file` | The **File** button and the File page (Backstage) with all its pages |
| `open` | **Open**, Ctrl+O, File › Open's Browse and Recent, the empty window's **Open a workbook…** button; also turns off `drop` |
| `drop` | Opening files dropped on the window |
| `samples` | The built-in samples on File › Open |
| `export` | **Export ▾** and File › Export: PDF, CSV, HTML and the Excel copy (also turns off `pdf`) |
| `pdf` | Export as PDF only |
| `print` | **Print**, Ctrl+P and File › Print |
| `find` | **Find**, Ctrl+F/Ctrl+H/Shift+F5, Find Next/Previous |
| `goto` | Go To and Go To Special (Ctrl+G, F5), and the menu item |
| `copy` | Copying cells (Ctrl+C, the cell menu's Copy) |
| `view` | **View ▾**, the view adjustments (gridlines, headings, freeze panes, column widths and row heights, hiding rows and columns, filtering), the status bar's view-mode buttons, resizing and freeze-line drags |
| `zoom` | The toolbar's **Fit**, −, %, +, the status bar's zoom controls, the Zoom dialog and Ctrl+wheel/pinch zoom |
| `options` | File › Options (also turns off `theme` and `language`) |
| `theme` | The display theme picker in Options |
| `language` | The interface language picker in Options |

A File page whose feature is off is left out of (or greyed in) the File page's list, and the
File page opens on the first page still on; with every page off it doesn't open. Programmatic
opens are not features: files named on the command line, `?url=`, `gridcraft:open` from a host
page and the control channel still open workbooks with `open` hidden.

GridCraft has no grammar checking and no background spell checking. Its Spelling dialog (F7) is
part of the editor and is already absent from the viewer, so there is no `spelling` feature
(the name is reported as unknown). On macOS the native menu bar's items stay listed, but hidden
or disabled features don't run from them.

## What is removed or turned off

- **The engine refuses every command that edits** (`Session::viewer`,
  `crates/engine/src/viewer.rs`). It refuses every `undoable` command except the view adjustments (`viewer::VIEW`), and the
  commands that don't touch the undo history but only exist to edit (`viewer::EDITING`): undo,
  redo, cut, begin edit, New, Save, Save As, Properties, custom views, split, the ink tools, macro
  recording and script trust. The gate sits in `Session::execute`, so the window, the control
  channel and MCP clients all get the same answer: `Disabled(id, "GridCraft Viewer only views
  workbooks (editing is turned off)")`. `CommandSpec::info` greys those commands out with that
  reason. Allowed commands that touch view state (zoom and the other view adjustments,
  recalculation) leave no undo step and don't make the workbook dirty, so closing never asks to
  save.
- **Typing, F2, Enter-to-edit, Delete/Backspace, cut, paste and the checkbox Space toggle** do
  nothing on the grid and say why in the "read only" toast, as shortcuts bound to editing
  commands do. Enter only moves.
- **Mouse editing**: the fill handle, moving cells, objects and columns, resizing objects and
  validation pick-lists are off. Drags select, resize rows and columns and move the freeze lines
  (view adjustments).
- **Context menus**: the cell menu offers Copy, Select All, Find…, Go To… and the view
  adjustments above. The AutoFilter menu has no Sort. The sheet-tab menu is gone, and tabs can't
  be renamed, moved or added.
- **The File page** shows only Open, Print, Export, Close and Options (`viewer::BACKSTAGE_PAGES`).
  Home, New, Info, Save, Save As and Account are hidden. Export has an *Excel Workbook copy* row.
- **Dialogs**: only Find, Go To, Zoom, About, command search and the workbook-password prompt open
  (`viewer::DIALOGS`). Save As becomes *save a copy as .xlsx*.
- Interface-only editing is refused too (`viewer::UI_EDITING`: ribbon tabs, formula-bar
  expansion, Python mode, AutoComplete).

## How it is built

- **Engine** (`gridcraft-engine`): `Session::viewer: bool` (default `false`), the `viewer`
  module (`REASON`, `EDITING`, `refuses`) and the new `file.saveCopy` command, which writes a copy
  and keeps the document's file. Viewer mode is off by default, so the CLI, MCP and every existing
  test behave as before. Tests: `crates/engine/src/tests/viewer.rs`.
- **UI** (`gridcraft-ui-egui`): the `viewer` module (toolbar, empty window, allow-lists,
  `SheetApp::is_viewer`), plus small `is_viewer()` checks in the frame layout, key and mouse
  handling, context menus, sheet tabs, Backstage, the status bar, Find and `run_command` /
  `open_dialog`.
- **The `editor` feature** of `gridcraft-ui-egui` is on by default and for the desktop app, and
  off for the web app. Without it `viewer::EDITOR` is false, `SheetApp::new` always puts the
  session in viewer mode and `is_viewer()` is constant, so the editor-only branches are dead code
  and left out of the wasm: the ribbon, the title bar, keytips, the formula bar, every dialog but
  the viewer's (`Dialog::open_viewing`) and the File pages the viewer hides. The macro step
  editor, macro prompts and debugging status, the code editor (Python and formula fields) and
  the in-cell editor are left out the same way (`viewer::EDITOR` checks where they are called).
- **The `macros` feature** of `gridcraft-engine` (on by default, and for the desktop app, CLI, MCP
  and the `editor` feature) is the macro runtime: running, checking, recording and debugging.
  The web viewer builds without it. `macro.run`, `macro.check`, `macro.record` and `macro.debug`
  are still in the command list, but they refuse with `macros::NO_RUNTIME` (`macros::RUNTIME` is
  false), so the runner, recorder and debugger are left out of the wasm. Macros only run when
  someone runs them, never to compute a value, so a workbook shows the same without the
  runtime. Its macros are still read and are kept in an exported `.xlsx` copy. Tests:
  `cargo test -p gridcraft-engine --no-default-features` covers the refusal.
- **The `editing` feature** of `gridcraft-engine` (on by default, and for the desktop app, CLI,
  MCP, xtask and the `editor` feature) is the code of the editing commands. The web viewer builds
  without it. Every command keeps its place in the command list (name, label, shortcut, keytips),
  but each one viewer mode refuses runs `viewer::left_out` instead of its own code
  (`cmd::run_if_kept!`, chosen at compile time from `viewer::EDITING_BUILD`), so its code is left
  out of the wasm. `Session::execute` refuses them with `Disabled(id, viewer::REASON)`, the toast
  viewer mode shows, whether or not `session.viewer` is set; a nested run of one gets the
  same reason. The commands viewer mode allows (navigation, selection, copy, Find, Go To, sheet
  switching, zoom, export, print, `file.saveCopy`/`file.saveBytes`) are kept.
  Samples are built by replaying editing commands, so a build without the feature reads them from
  stored copies (`crates/engine/samples/*.json`); `sample::open` opens one, and the test
  `stored_samples_match_the_replay` keeps the copies equal to the replay
  (`GRIDCRAFT_BLESS_SAMPLES=1 cargo test -p gridcraft-engine stored_samples` rewrites them).
  Tests that edit are gated on the feature; `crates/engine/src/read_only_tests.rs` runs without
  it and covers the refusal, the kept commands, the samples, export, and an `.xlsx` copy keeping
  the workbook's macros. `cargo xtask viewer-engine` (part of `cargo xtask ci`) runs clippy and
  these tests without default features.
- **Desktop app** (`apps/gridcraft`): sets `session.viewer = true` after any `--sample` (made
  with `file.new`) and titles the window *GridCraft Viewer*.
- **Web app** (`apps/gridcraft-web`): sets `session.viewer = true`, then opens any `?sample=`
  with `sample::open` (`file.new` is an editing command), with the page title *GridCraft Viewer*.
  `?url=ADDRESS` (alias `?src=`; `?url=` wins; optional `&name=`) fetches a workbook at start-up
  and queues it in the inbox as a dropped file, so it opens read-only like any other:
  `index.html?url=/files/budget.xlsx`, or `?url=https%3A%2F%2Ffiles.example.org%2Fq3.ods`, whose
  server must then send `Access-Control-Allow-Origin` (CORS). Only http, https and blob
  addresses; relative ones resolve against the page. The name comes from `&name=`,
  `Content-Disposition` or the address (plus an extension from `Content-Type` when it has none);
  the engine tells the formats apart by content. *Opening <name>…* shows while it downloads, and
  a failure (bad address, network/CORS, HTTP error, too large, unreadable) shows a message box.
  `?url=` wins over `?sample` and is ignored with `?host=parent`. The pure logic and its tests are
  in `apps/gridcraft-web/src/url_open.rs`, the browser side in `src/web/open_url.rs`; hosting
  notes in `packaging/web/README.md` (HOSTING.md in the package).

## Download size (web)

`trunk build --release` uses `[profile.web]` (`index.html`'s `data-cargo-profile`). That profile
is `opt-level = "z"`, fat LTO, one codegen unit, `strip` and `panic = "abort"`, followed by
`wasm-opt -Oz`. `packaging/web/package.sh` adds Brotli (quality 11) and gzip (-9) copies. Sizes
are in `packaging/web/README.md`.

No fonts are duplicated in the wasm. The five embedded faces are egui's Ubuntu Light, Hack,
egui-icons, Noto Emoji and emoji-icon-font. GridCraft embeds no craft-fonts in a build without
`CRAFT_FONTS_DIR`. The two emoji faces stay because the UI draws ✕, ◀, ↘ and 📋 as text.
