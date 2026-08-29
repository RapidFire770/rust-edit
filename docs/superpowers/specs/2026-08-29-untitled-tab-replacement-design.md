# Design: Auto-close Untitled Placeholder Tab on File Open + Duplicate Tab Prevention

**Date:** 2026-08-29
**Status:** Approved (user confirmed Approach A)
**Scope:** Tab/buffer management in `src/app.rs` and `src/buffer.rs`

## Problem

Launching `rust-edit` with no file arguments creates one empty buffer rendered as
`[ untitled ]` in the tab bar. When the user then opens a file (File > Open, file
dialog, or file browser), the new file is appended as a second tab and the empty
untitled tab remains behind — a stray tab the user never asked for.

Additionally, opening a file that is already open in another tab creates a
duplicate tab instead of focusing the existing one.

## Current Behavior (verified in code)

- `App::new` (`src/app.rs:142-164`): with no initial files, pushes one empty
  `Buffer::new()` (no path, `is_modified: false`). With initial files, no
  untitled buffer is created.
- `close_file_dialog` open branch (`src/app.rs:905-917`): the single runtime
  entry point for opening files (the file browser only fills `file_path_input`
  and returns to the dialog). Always does `buffers.push(new_buf)` + activate.
- `Buffer` (`src/buffer.rs:52-92`): `path: Option<PathBuf>` (`None` = untitled),
  `is_modified: bool` (set true on edits; recomputed on undo as
  `!undo_stack.is_empty()` at `src/buffer.rs:299`).
- Placeholder auto-creation sites (editor creates an empty buffer as the sole
  tab):
  - Startup with no args: `src/app.rs:152`
  - Closing the last tab: `src/app.rs:305` (`close_current_tab`)
  - Confirming close of last modified tab: `src/app.rs:837` (`close_confirm`)
- User-created empty tabs: `new_tab` (`src/app.rs:315-318`, the `[+]` button) —
  these are intentional and must never be auto-closed.

## User Decisions

1. **Scope:** Only the editor's initial/placeholder buffer is eligible for
   replacement — including the buffer auto-created when the last tab is closed
   (same empty-placeholder UX situation). Tabs the user explicitly created via
   `[+]` are never auto-closed.
2. **Duplicates:** Opening a file already open in a tab must focus that tab
   instead of adding a duplicate.

## Behavior Specification

When opening file `F` via the Open dialog, in order:

1. **Read failure** — if `F` cannot be read, show the existing
   `Open error: ...` dialog message; change nothing.
2. **Duplicate** — if a buffer already holds `F` (paths equal after
   canonicalization), activate that buffer. No new tab; placeholder cleanup is
   skipped.
3. **Placeholder replacement** — else, if a placeholder buffer exists
   (`is_initial && path.is_none() && !is_modified`), replace it **in place**
   with the new buffer; the new file is active at that position. Other tabs'
   relative order is preserved.
4. **Append** — else, append the new buffer and activate it (current behavior).

After a successful open (rules 2-4), record the file in recents (existing behavior).

### Edge cases

- **Typed then fully undone** in the placeholder: `is_modified` is false and
  content is empty (`src/buffer.rs:299`), so the buffer is replaced. Nothing is
  lost; accepted.
- **Placeholder is the active tab:** replacement keeps the same index, so the
  cursor/viewport simply shows the new file.
- **Placeholder is not active:** it is still replaced in place; the new file's
  tab becomes active.
- **Multiple placeholders** cannot occur (only one sole-tab placeholder exists
  at a time), but the implementation searches for the first match defensively.
- **Buffer whose file was deleted on disk:** canonicalization fails; fall back
  to raw path comparison for duplicate detection.

## Implementation

### `src/buffer.rs`

- Add field `is_initial: bool` to `Buffer` (default `false` in `new()` and
  `from_str()`).
- Set `is_initial = true` at the three auto-creation sites.

### `src/app.rs`

- New method `open_file_in_tab(&mut self, path: &Path) -> Result<(), String>`:
  1. `std::fs::read_to_string(path)` → on error return `Err(e.to_string())`.
  2. Duplicate check: canonicalize `path` (fallback to raw if it fails); scan
     `buffers` for a buffer whose canonicalized `path` matches → set
     `self.active` to that index, record recent, return `Ok(())`.
  3. Build `Buffer::from_str(contents, Some(path))`.
  4. Find first placeholder index (`is_initial && path.is_none() &&
     !is_modified`) → replace in place, `self.active = idx`; else push and
     activate last.
  5. `config.record_recent_file(path)`.
- `close_file_dialog` open branch: call `open_file_in_tab`; on `Err`, set
  `file_dialog_message = format!("Open error: {e}")` and return (dialog stays
  open, matching current UX).
- Set `is_initial = true` in `App::new` (no-args branch), `close_current_tab`,
  and `close_confirm` ("close_tab" last-tab branch).

### Not changed

- Header/tab rendering (`src/ui/header.rs`) — renders from `app.buffers`.
- Highlight cache — keyed by generation/language/scroll; buffer replacement
  invalidates naturally.
- CLI startup path — already creates no untitled buffer when files are passed.

## Tradeoffs

- **Flag vs heuristic:** a boolean on `Buffer` travels with the buffer through
  `Vec` operations (no index bookkeeping); a heuristic ("only tab, untitled,
  unmodified") would wrongly kill `[+]`-created empty tabs.
- **Canonicalize for duplicates:** resolves relative/absolute and symlink
  differences; precedent already exists in `open_file_browser`
  (`src/app.rs:981`). Cost is negligible for typical tab counts.
- **In-place replacement:** the opened file takes the placeholder's tab
  position, so the tab the user is looking at becomes the file; other tabs do
  not shift unexpectedly.

## Testing

### Unit tests (`src/app.rs` test module)

1. Open file with only the initial placeholder → placeholder replaced; 1 tab;
   correct path, active index 0.
2. Placeholder modified (has text) → file appended; 2 tabs; placeholder kept.
3. No placeholder (app started with a file arg) → file appended.
4. Open same file twice → no duplicate; existing tab focused.
5. Same file via relative vs absolute path → no duplicate.
6. `[+]`-created empty tab is never replaced (file appended instead).
7. Reset buffer after closing the last tab is replaceable.

### Manual checklist (per AGENTS.md §7, 2 terminals)

- Launch with no args → open a file via dialog, via file browser, and via
  Recent files → single tab showing the file.
- Type in the untitled tab → open a file → both tabs remain.
- Open an already-open file → focuses existing tab, no duplicate.
- Close all tabs → open a file → single tab (no stray empty tab).
- `[+]` empty tab + open file → both tabs remain.

## Out of Scope

- Session restore, tab persistence.
- Changing `is_modified` semantics (the "rough" undo-based heuristic stays).
- Any UI/styling changes.
