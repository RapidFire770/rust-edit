# File Browser Popup — Design Spec

**Date:** 2026-06-30
**Status:** Approved
**Scope:** Phase 1 enhancement to existing Open/Save As dialogs.

## Goal

Add an optional file-system browser to the Open File / Save As flows so users
can pick a file (or save target) by navigating directories instead of typing a
full path. Also keep a small recent-files list for quick reopening.

## Non-goals

- No bookmarks, no search-within-browser, no extension filter dropdown.
- No markdown preview, no LSP — out of scope (AGENTS §8).
- No replacement of the typed-path dialog — that flow stays intact.

## User flow

1. User opens the existing Open/Save As popup (Ctrl+O / Ctrl+Alt+S / menu).
2. The popup now has three buttons: `[ Open/Save ]  [ Browse… ]  [ Cancel ]`.
3. Clicking `[ Browse… ]` (or arrow-keying to it and pressing Enter) opens a
   larger directory-listing popup centered on screen.
4. The user navigates the listing (keyboard or mouse). From Open they may
   switch to the Recent view and pick a file. From Save As they pick a file
   to overwrite, or `[ Use folder ]` to confirm the current directory.
5. The browser closes; the selected absolute path populates the path field of
   the original dialog. The user can still edit the path or press Open/Save.

## Decisions

- **Integration model:** Add `[ Browse… ]` button to the existing path dialog
  rather than replacing it. Keeps typed-path flow and minimizes disruption.
- **Hidden files:** Default-hide dotfiles; `Ctrl+H` toggles visibility.
- **Recent files:** Persisted list (max 10, MRU) stored in `config.toml` under
  `recent_files = [...]`. Surfaced as a separate view inside the browser via a
  `[ Recent ]` / `[ Browse ]` toggle button.

## New module: `src/ui/popups/file_browser.rs`

Pure presentation + value types. State lives on `App`.

```rust
pub struct BrowserEntry { pub name: String, pub is_dir: bool, pub path: PathBuf }
pub enum BrowserMode { Dir, Recent }
```

Pure functions (unit-tested):
- `sort_entries(Vec<BrowserEntry>) -> Vec<BrowserEntry>` — directories first,
  alpha case-insensitive within each group.
- `filter_hidden(Vec<BrowserEntry>, show_hidden: bool) -> Vec<BrowserEntry>`
- `read_directory(cwd: &Path, show_hidden: bool) -> std::io::Result<Vec<BrowserEntry>>`
  — wraps `fs::read_dir`, follows symlinks via `metadata()`, inserts the pinned
  `..` entry at index 0 when the cwd has a parent.
- `compose_save_path(dir, current_input, fallback_filename) -> PathBuf` —
  Save As folder pick: `dir` + basename of `current_input`, else fallback.

## New `App` state

- `show_file_browser: bool`
- `browser_cwd: PathBuf`
- `browser_entries: Vec<BrowserEntry>` (cached, refreshed on dir change / hidden toggle)
- `browser_selected: usize`
- `browser_scroll: u16`
- `browser_show_hidden: bool` (default `false`)
- `browser_error: Option<String>` (permission/IO error; never crashes)
- `browser_mode: BrowserMode` (default `Dir`)
- `browser_origin_is_save: bool`

Methods: `open_file_browser()`, `close_file_browser(confirmed: bool)`,
`browser_use_current_folder()`, `browser_activate_footer(idx)`,
`browser_enter_selected()`, `browser_go_up()`, `refresh_browser()`,
`browser_toggle_hidden()`, `browser_toggle_mode()`, `browser_move_cursor(i64)`,
`browser_page(up: bool)`, `browser_select_index(usize)`.

`open_file_browser` seeds `browser_cwd` from the active buffer's path parent
(or the OS home dir via `directories::BaseDirs` as fallback).

## Config additions (`config.rs`)

```rust
pub recent_files: Vec<String>,  // default empty, max 10 enforced by App
```

New method: `record_recent_file(&mut self, path: &Path)` — moves `path` to
front, dedupes, truncates to 10, calls `self.save()`.

Called on successful open **and** save inside `App::close_file_dialog`.

## Existing dialog changes

- `file_dialog_selected` widens to `0..=2`.
- Labels: `[ Open/Save ]  [ Browse… ]  [ Cancel ]`.
- A new general-purpose `ButtonRow` component (N buttons) is added in
  `src/ui/button.rs`; the existing `ButtonGroup` (2 buttons) is left untouched
  so `confirm.rs` and its tests are unaffected. `file_dialog.rs` switches to
  `ButtonRow`.
- Activating index 1 in the dialog calls `open_file_browser()` instead of
  confirming; indices 0 and 2 behave as today.

## Browser popup layout

Centered rect ~80% × 70% (clamped to min 50 × 16). Title shows the mode and
(for Dir mode) the current absolute path.

Open (unchanged):

```
┌─ Browse — /home/josh/Documents  ───────────────┐
│ ../                                            │
│ Cargo.toml                                     │
│ src/                                           │
│ README.md                                      │
│ ...                                            │
│                                                │
│ [ Up ]  [ Recent ]  [ Hidden: off ]  [ Cancel ]│
└────────────────────────────────────────────────┘
```

Save As — title `Save to — {cwd}`, Recent omitted, extra folder action:

```
┌─ Save to — /home/josh/Documents  ───────────────┐
│ ../                                             │
│ src/                                            │
│ README.md                                       │
│ ...                                             │
│                                                 │
│ [ Use folder ]  [ Up ]  [ Hidden: off ]  [ Cancel ]│
└─────────────────────────────────────────────────┘
```

Directories styled in accent + bold; the selected row gets an inverted/accent
background. Errors replace the list body with the error message and a hint.

## Input parity (keyboard + mouse — AGENTS §1.2)

Keyboard:
- `↑/k` `↓/j` move selection
- `Enter` `l` `→` activate (enter dir, or confirm file)
- `Backspace` `h` `←` `-` go up one directory (Dir mode only)
- `g` top, `G` bottom
- `PgUp` / `PgDn` page
- `Ctrl+H` toggle hidden files (Dir mode only)
- `Tab` cycles button focus (Open: Up/Recent/Hidden/Cancel; Save As: Use
  folder/Up/Hidden/Cancel) — mirrors the existing arrow-key navigation
  pattern used elsewhere
- `Esc` `q` cancel

Mouse:
- Click on a list row = select + activate (dir → enter; file → populate path
  and close browser). Single-click activates because ratatui has no robust
  double-click and picker UX favors immediacy.
- Wheel scrolls the list.
- Click footer buttons. Open: `[ Up ]`, `[ Recent/Browse ]`, `[ Hidden ]`,
  `[ Cancel ]`. Save As: `[ Use folder ]`, `[ Up ]`, `[ Hidden ]`, `[ Cancel ]`.

## Confirm action

**Open:** In Dir mode the selected entry must be a file (directories are
entered, not confirmed). The file's absolute path is written to
`app.file_path_input`, `show_file_browser` is set false, and focus returns to
the path dialog (action button selected). In Recent mode: selecting any entry
confirms immediately.

**Save As:** `[ Use folder ]` confirms `browser_cwd` joined with the current
filename via `compose_save_path` (basename of the typed path, else the buffer
name, else `untitled`) and returns to the Save As dialog. File rows still
confirm a full path (overwrite). Directory rows still enter. Recent view is
Open-only.

## Tests (pure logic)

- `sort_entries` ordering (dirs first, alpha, case-insensitive)
- `filter_hidden` respects flag, keeps `..`
- `read_directory` correctness against a tempdir with mixed entries
- `read_directory` returns Err on a non-directory / unreadable path
- `Config::record_recent_file` dedupe + MRU + truncate-to-10 (via serializing
  to a temp file to verify persistence shape)

## Manual verification (AGENTS §7)

Two terminals (kitty + alacritry):
- Browse opens from both Open and Save As dialogs.
- Keyboard navigation: enter dir, go up, page, g/G, hidden toggle.
- Mouse: click dirs to enter, click file to confirm, wheel scroll, buttons.
- Permission-denied directory shows error, does not crash.
- Recent view: opens a file via Open, reopens browser, file appears at top of
  Recent; selecting it populates the path; restarting the editor keeps the
  recent list (config persistence).

## Tradeoffs

- `std::fs` rather than `walkdir`/`ignore` — no new dependency, sufficient for
  single-directory browsing (AGENTS §5).
- New `ButtonRow` instead of generalizing `ButtonGroup` — keeps existing
  2-button popups and their tests untouched.
- Single-click activates rather than double-click — ratatui has no robust
  double-click; cheap to revisit later.

## Files touched

- New: `src/ui/popups/file_browser.rs`
- Edited: `src/ui/button.rs` (add `ButtonRow`), `src/ui/popups/mod.rs`,
  `src/config.rs`, `src/app.rs`, `src/main.rs`,
  `src/ui/popups/file_dialog.rs`, `src/ui/popups/help.rs`
