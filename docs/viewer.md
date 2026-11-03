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

## The window

There is one 40 px toolbar (`crates/ui-egui/src/viewer.rs`) in place of the title bar, the
ribbon and the formula bar:

- On the left: **File** (the File page on Open), **Open**, **Export ▾** (PDF, CSV, Excel
  workbook copy) and **Print**.
- In the centre: the workbook's name and a green **Read only** badge.
- On the right: **Find**, **−**, the zoom percentage (click it for 100%), **+** and **Fit** (fits the
  sheet's used range in the window, 25–200%).

Below the toolbar come the notices, the grid, the sheet tabs and the status bar. With nothing
open the window shows *GridCraft Viewer* and an **Open a workbook…** button. The window title is
`<name> - GridCraft Viewer`.

## What is removed or turned off

- **The engine refuses every command that edits** (`Session::viewer`,
  `crates/engine/src/viewer.rs`). It refuses every `undoable` command except view zoom, and the
  commands that don't touch the undo history but only exist to edit (`viewer::EDITING`): undo,
  redo, cut, begin edit, New, Save, Save As, Properties, custom views, split, the ink tools, macro
  recording and script trust. The gate sits in `Session::execute`, so the window, the control
  channel and MCP clients all get the same answer: `Disabled(id, "GridCraft Viewer only views
  workbooks (editing is turned off)")`. `CommandSpec::info` greys those commands out with that
  reason. Allowed commands that touch view state (zoom, recalculation) leave no undo step and
  don't make the workbook dirty, so closing never asks to save.
- **Typing, F2, Enter-to-edit, Delete/Backspace, cut, paste and the checkbox Space toggle** do
  nothing on the grid. Enter only moves. Shortcuts bound to editing commands say why in a toast.
- **Mouse editing**: the fill handle, moving or resizing cells, objects, rows and columns, header
  autofit and validation pick-lists are off. Drags only select.
- **Context menus**: the cell menu offers Copy, Select All, Find… and Go To…. The AutoFilter menu
  and the sheet-tab menu are gone, and tabs can't be renamed, moved or added.
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

## Download size (web)

`trunk build --release` uses `[profile.web]` (`index.html`'s `data-cargo-profile`). That profile
is `opt-level = "z"`, fat LTO, one codegen unit, `strip` and `panic = "abort"`, followed by
`wasm-opt -Oz`. `packaging/web/package.sh` adds Brotli (quality 11) and gzip (-9) copies. Sizes
are in `packaging/web/README.md`.

No fonts are duplicated in the wasm. The five embedded faces are egui's Ubuntu Light, Hack,
egui-icons, Noto Emoji and emoji-icon-font. GridCraft embeds no craft-fonts in a build without
`CRAFT_FONTS_DIR`. The two emoji faces stay because the UI draws ✕, ◀, ↘ and 📋 as text.
