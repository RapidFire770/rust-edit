# Untitled Tab Replacement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> Before coding: use superpowers:using-git-worktrees to isolate this work off `main`. Also copy this plan into the repo at `docs/superpowers/plans/2026-08-29-untitled-tab-replacement.md` (the previous session wrote the spec but never produced a plan file).

**Goal:** When opening a file, replace the editor's auto-created untitled placeholder tab (startup or last-tab close) in place, and focus an already-open file instead of creating a duplicate tab. User-created `[+]` empty tabs are never auto-closed.

**Architecture:** Add `Buffer.is_initial` (default `false`) so placeholder tabs are distinct from `[+]` empty tabs. Route all runtime file-open through a new `App::open_file_in_tab` that, in order: reads the file, focuses an existing tab on canonical-path match, replaces the first placeholder in place, or appends. The Open dialog becomes a thin caller of that method. CLI startup is unchanged.

**Tech Stack:** Existing `Buffer` + `App` (ropey, no new crates). Tests with `pretty_assertions` and unique temp files. Toolchain: `/usr/bin/cargo`, or `$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo` if the system proxy is broken (AGENTS.md §2).

**Spec:** `docs/superpowers/specs/2026-08-29-untitled-tab-replacement-design.md` (Approved, Approach A). Design Spec §4.2 / §5.1 (tabbed multi-buffer; `Ctrl+O` open). Todo-List.md line 4 is this bug.

---

## Investigation (why this plan exists)

The spec is still accurate against current `main`. Nothing from it has been implemented:

| Spec claim | Current code |
|---|---|
| Empty launch creates one untitled buffer | `App::new` (`src/app.rs:151-152`) pushes `Buffer::new()` |
| Open always appends a new tab | `close_file_dialog` open branch (`src/app.rs:905-917`) always `buffers.push` |
| File browser / Recents do not open directly | Browser writes `file_path_input` and returns to the dialog (`src/app.rs:936-942`, `close_file_browser`) — so `close_file_dialog` is the single runtime open entry point |
| `[+]` uses the same constructor as the placeholder | `new_tab` (`src/app.rs:315-318`) also calls `Buffer::new()` — a heuristic of “untitled + unmodified” would wrongly kill user tabs; the `is_initial` flag is required |
| Last-tab close recreates a placeholder | `close_current_tab` (`src/app.rs:304-307`) and `close_confirm` `"close_tab"` (`src/app.rs:837-838`) assign `Buffer::new()` |
| No duplicate detection | No path scan anywhere on open |
| CLI with file args creates no untitled | `App::new` else-branch (`src/app.rs:153-163`) — leave this path alone |

`Buffer` (`src/buffer.rs:53-92`) has `path: Option<PathBuf>` and `is_modified: bool` (recomputed on undo as `!undo_stack.is_empty()` at line 299). There is no `is_initial` field.

The previous session stopped after the spec: `docs/superpowers/plans/` has no untitled-tab plan.

### Spec details this plan makes explicit

1. **`Config::save` must not run during unit tests.** `App::new` loads the user's real XDG config. `record_recent_file` persists the whole file. Tests that open temp files would rewrite `~/.config/rust-edit/config.toml`. Gate the persist in `record_recent_file` with `#[cfg(not(test))]`. In-memory recents still update, so tests can assert them.
2. **`Buffer::placeholder()`** — thin wrapper around `Buffer::new()` + `is_initial = true`. The three auto-creation sites call it; `new()` / `from_str()` stay `is_initial = false`. This is the spec's three-site rule, without copy-paste.
3. **Extra tests** beyond the spec's seven: read-failure leaves buffers unchanged; placeholder that is not the active tab is still replaced; fully-undone typing is still replaced; `close_file_dialog` error keeps the dialog open.

### Out of scope (do not implement)

- CLI startup duplicate handling / canonicalizing argv paths
- Session restore, tab persistence
- Changing `is_modified` semantics
- Any UI / header / help / styling changes
- Markdown preview, LSP, split panes

---

## File Structure

**Modified:**

- `src/buffer.rs` — add `is_initial: bool`; default `false` in `new()` and `from_str()`; add `Buffer::placeholder()`.
- `src/app.rs` — set placeholder at the three auto-creation sites; add `open_file_in_tab` + two private helpers; `close_file_dialog` open branch calls it; unit tests in the existing `tests` module.
- `src/config.rs` — skip disk persist inside `record_recent_file` under `cfg(test)` only.
- `AGENTS.md` — document `is_initial` on the Buffer fields bullet.
- `docs/Todo-List.md` — check off the untitled-tab bug.

**Not modified:** `src/ui/header.rs`, highlight cache, CLI argv loop, file browser, save-as branch of `close_file_dialog`.

---

## Global Constraints

- TDD: no production logic without a failing test first. Watch it fail for the expected reason, then write the minimum code to pass.
- `cargo fmt && cargo clippy -- -D warnings && cargo test` must pass after every task.
- Toolchain: `/usr/bin/cargo` (fallback: `$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo`).
- Do not `unwrap()` / `expect()` in production paths.
- `open_file_in_tab` signature: `pub fn open_file_in_tab(&mut self, path: &Path) -> Result<(), String>`.
- Placeholder predicate (search first match): `is_initial && path.is_none() && !is_modified`.
- Duplicate predicate: canonicalize both sides; if canonicalize fails, compare the raw `PathBuf`.
- After a successful open (duplicate / replace / append), call `self.config.record_recent_file(path)`.
- `[+]` (`new_tab`) must keep calling `Buffer::new()`, never `placeholder()`.

---

### Task 1: Add `is_initial` and mark placeholder creation sites

**Files:**
- Modify: `src/buffer.rs` (`Buffer` struct ~line 53, `new()` ~101, `from_str()` ~121, tests ~900)
- Modify: `src/app.rs` (`App::new` ~151, `close_current_tab` ~304, `close_confirm` ~837)

- [ ] **Step 1: Write failing Buffer tests for the new flag**

In `src/buffer.rs` `tests` module, extend `new_buffer_is_empty` and add two tests. They will fail to compile (`no field is_initial` / `no method placeholder`).

```rust
    #[test]
    fn new_buffer_is_empty() {
        let b = Buffer::new();
        assert_eq!(b.line_count(), 1);
        assert_eq!(b.text(), "");
        assert_eq!(b.cursor_position(), (0, 0));
        assert!(!b.is_initial, "Buffer::new is a user tab, not a placeholder");
        assert!(!b.is_modified);
        assert!(b.path.is_none());
    }

    #[test]
    fn from_str_is_not_initial() {
        let b = Buffer::from_str("hi", None);
        assert!(!b.is_initial);
    }

    #[test]
    fn placeholder_is_initial_untitled_unmodified() {
        let b = Buffer::placeholder();
        assert!(b.is_initial);
        assert!(b.path.is_none());
        assert!(!b.is_modified);
        assert_eq!(b.text(), "");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `/usr/bin/cargo test --lib` is wrong (this crate is a bin). Use:

```bash
CARGO="${CARGO:-/usr/bin/cargo}"
"$CARGO" test --bin rust-edit new_buffer_is_empty from_str_is_not_initial placeholder_is_initial -- --nocapture
```

Expected: compile error — `no field 'is_initial' on type 'Buffer'` and/or `no function or associated item named 'placeholder'`.

- [ ] **Step 3: Add the field, default it false, add `placeholder()`**

In the `Buffer` struct, after `is_modified`:

```rust
    pub is_modified: bool,
    /// True only for the editor-created empty untitled buffer (startup with no
    /// files, or the reset buffer after closing the last tab). `[+]` / `Ctrl+N`
    /// tabs stay false so they are never auto-replaced on File > Open.
    pub is_initial: bool,
```

In `Buffer::new()` and `Buffer::from_str()`, set `is_initial: false`.

Add, immediately after `new()`:

```rust
    /// Auto-created untitled placeholder (startup / last-tab close).
    pub fn placeholder() -> Self {
        let mut b = Self::new();
        b.is_initial = true;
        b
    }
```

Do **not** yet change `App`. The two new tests should now pass; we next write App tests that fail.

- [ ] **Step 4: Write failing App tests for the three creation sites**

Append to `src/app.rs` `tests` module (after the browser tests):

```rust
    #[test]
    fn empty_launch_creates_placeholder() {
        let app = App::new(&[]);
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        assert!(app.buffers[0].path.is_none());
        assert!(!app.buffers[0].is_modified);
    }

    #[test]
    fn new_tab_is_not_a_placeholder() {
        let mut app = App::new(&[]);
        app.new_tab();
        assert_eq!(app.buffers.len(), 2);
        assert!(app.buffers[0].is_initial, "original startup tab stays initial");
        assert!(
            !app.buffers[1].is_initial,
            "[+] tab must never be marked initial"
        );
    }

    #[test]
    fn closing_last_tab_creates_a_fresh_placeholder() {
        let mut app = App::new(&[]);
        app.buffers[0].is_initial = false; // pretend it was a user tab
        app.close_current_tab();
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        assert!(app.buffers[0].path.is_none());
        assert!(!app.buffers[0].is_modified);
    }
```

- [ ] **Step 5: Run App tests to verify they fail**

```bash
"$CARGO" test --bin rust-edit empty_launch_creates_placeholder new_tab_is_not_a_placeholder closing_last_tab_creates_a_fresh_placeholder -- --nocapture
```

Expected: FAIL — `empty_launch` and `closing_last_tab` see `is_initial == false`. `new_tab_is_not_a_placeholder` should already PASS (both `new()` calls are false). If `new_tab` fails, stop and inspect.

- [ ] **Step 6: Switch the three auto-creation sites to `Buffer::placeholder()`**

`App::new` no-args branch (`src/app.rs:151-152`):

```rust
        if initial_files.is_empty() {
            buffers.push(Buffer::placeholder());
```

`close_current_tab` last-tab branch (`src/app.rs:304-307`):

```rust
        if self.buffers.len() <= 1 {
            self.buffers[0] = Buffer::placeholder();
            self.active = 0;
            return;
        }
```

`close_confirm` `"close_tab"` last-tab branch (`src/app.rs:836-838`):

```rust
                        } else {
                            self.buffers[0] = Buffer::placeholder();
                            self.active = 0;
                        }
```

Leave `new_tab` as `Buffer::new()`. Leave the CLI failed-open path (`Buffer::new()` + `set_path`) as `is_initial = false`.

Add a confirm-dialog test next to the close test:

```rust
    #[test]
    fn confirming_close_of_last_modified_tab_creates_placeholder() {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("x");
        app.show_confirm_dialog("Close tab with unsaved changes?", "close_tab");
        app.close_confirm(true);
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        assert_eq!(app.buffers[0].text(), "");
    }
```

- [ ] **Step 7: Run tests and make sure they pass**

```bash
"$CARGO" test --bin rust-edit empty_launch_creates_placeholder new_tab_is_not_a_placeholder closing_last_tab_creates_a_fresh_placeholder confirming_close_of_last_modified_tab_creates_placeholder new_buffer_is_empty from_str_is_not_initial placeholder_is_initial -- --nocapture
"$CARGO" test --bin rust-edit
```

Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add src/buffer.rs src/app.rs
git commit -m "$(cat <<'EOF'
feat(buffer): mark auto-created untitled tabs as placeholders

Add Buffer.is_initial (false by default) and Buffer::placeholder()
so startup / last-tab-close empty buffers can be distinguished from
user-created [+] tabs. No open-file behavior change yet.

EOF
)"
```

---

### Task 2: `open_file_in_tab` — read failure, placeholder replace, recents test-guard

**Files:**
- Modify: `src/config.rs:112-116` (`record_recent_file`)
- Modify: `src/app.rs` — add `use std::path::{Path, PathBuf};`, add `open_file_in_tab` + helpers, tests

- [ ] **Step 1: Gate recents persist in unit tests**

This is test infrastructure, not feature logic. Change `Config::record_recent_file` to:

```rust
    pub fn record_recent_file(&mut self, path: &Path) {
        if self.record_recent_file_inmemory(path) {
            // App unit tests load the user's real XDG config via App::new.
            // Skip persist so opening temp files cannot rewrite config.toml.
            #[cfg(not(test))]
            self.save();
        }
    }
```

Existing `record_recent_file_inmemory` tests are unaffected.

- [ ] **Step 2: Write failing tests for replace / keep-modified / read-error**

Add this helper (and `use std::fs;`) at the top of `src/app.rs` `tests`:

```rust
    fn unique_temp_file(contents: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-open-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&p, contents).unwrap();
        p
    }
```

Add tests (they fail to compile: `no method named open_file_in_tab`):

```rust
    #[test]
    fn open_replaces_initial_placeholder() {
        let mut app = App::new(&[]);
        let path = unique_temp_file("hello from disk");
        app.open_file_in_tab(&path).expect("open");
        assert_eq!(app.buffers.len(), 1, "placeholder must be replaced, not appended");
        assert_eq!(app.active, 0);
        assert_eq!(app.buffers[0].text(), "hello from disk");
        assert_eq!(app.buffers[0].path.as_deref(), Some(path.as_path()));
        assert!(!app.buffers[0].is_initial);
        assert!(!app.buffers[0].is_modified);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_keeps_modified_placeholder_and_appends() {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("typed");
        let path = unique_temp_file("file");
        app.open_file_in_tab(&path).expect("open");
        assert_eq!(app.buffers.len(), 2);
        assert_eq!(app.buffers[0].text(), "typed");
        assert!(app.buffers[0].is_initial, "modified placeholder is kept");
        assert_eq!(app.buffers[1].path.as_deref(), Some(path.as_path()));
        assert_eq!(app.active, 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_read_error_changes_nothing() {
        let mut app = App::new(&[]);
        let err = app
            .open_file_in_tab(Path::new("/this/does/not/exist/rust-edit-nope.txt"))
            .unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        assert_eq!(app.active, 0);
    }

    #[test]
    fn undone_typing_in_placeholder_is_still_replaced() {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("hello");
        app.active_buffer_mut().undo();
        assert!(!app.buffers[0].is_modified);
        let path = unique_temp_file("replaced");
        app.open_file_in_tab(&path).expect("open");
        assert_eq!(app.buffers.len(), 1);
        assert_eq!(app.buffers[0].text(), "replaced");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn inactive_placeholder_is_still_replaced_and_activated() {
        let mut app = App::new(&[]);
        app.new_tab(); // active = 1, placeholder remains at 0
        assert_eq!(app.active, 1);
        let path = unique_temp_file("now-active");
        app.open_file_in_tab(&path).expect("open");
        assert_eq!(app.buffers.len(), 2, "placeholder replaced in place; [+] kept");
        assert_eq!(app.active, 0, "opened file becomes active");
        assert_eq!(app.buffers[0].path.as_deref(), Some(path.as_path()));
        assert!(app.buffers[1].path.is_none());
        assert!(!app.buffers[1].is_initial);
        let _ = fs::remove_file(&path);
    }
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
"$CARGO" test --bin rust-edit open_replaces_initial_placeholder open_keeps_modified_placeholder_and_appends open_read_error_changes_nothing undone_typing_in_placeholder_is_still_replaced inactive_placeholder_is_still_replaced_and_activated -- --nocapture
```

Expected: compile error `no method named 'open_file_in_tab'`.

- [ ] **Step 4: Implement `open_file_in_tab` (replace / append / error only; duplicates in Task 3)**

Change the import at `src/app.rs:13`:

```rust
use std::path::{Path, PathBuf};
```

Add these methods on `impl App` immediately before `close_file_dialog` (so the dialog can call them in Task 5). For Task 2, a stub that always appends is **wrong** — write the real replace/append logic. Duplicate focusing may still append in this task; tests 4–5 are Task 3.

```rust
    /// Open `path` into a tab.
    ///
    /// Order: read the file (error → `Err`, nothing changes); if a buffer
    /// already holds this file, focus it; else replace the first placeholder
    /// (`is_initial && path.is_none() && !is_modified`) in place; else append.
    /// On success, records the path in recents.
    pub fn open_file_in_tab(&mut self, path: &Path) -> Result<(), String> {
        let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

        if let Some(idx) = self.index_of_open_path(path) {
            self.active = idx;
            self.config.record_recent_file(path);
            return Ok(());
        }

        let new_buf = Buffer::from_str(&contents, Some(path.to_path_buf()));
        if let Some(idx) = self.placeholder_index() {
            self.buffers[idx] = new_buf;
            self.active = idx;
        } else {
            self.buffers.push(new_buf);
            self.active = self.buffers.len() - 1;
        }
        self.config.record_recent_file(path);
        Ok(())
    }

    fn placeholder_index(&self) -> Option<usize> {
        self.buffers
            .iter()
            .position(|b| b.is_initial && b.path.is_none() && !b.is_modified)
    }

    fn index_of_open_path(&self, path: &Path) -> Option<usize> {
        let want = Self::canonical_or_raw(path);
        self.buffers.iter().position(|b| {
            b.path
                .as_ref()
                .is_some_and(|p| Self::canonical_or_raw(p) == want)
        })
    }

    fn canonical_or_raw(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }
```

Including the duplicate branch now is fine: Task 3 tests lock it down. Do not leave it as a later TODO.

- [ ] **Step 5: Run Task 2 tests**

```bash
"$CARGO" test --bin rust-edit open_replaces_initial_placeholder open_keeps_modified_placeholder_and_appends open_read_error_changes_nothing undone_typing_in_placeholder_is_still_replaced inactive_placeholder_is_still_replaced_and_activated -- --nocapture
"$CARGO" test --bin rust-edit
```

Expected: all PASS. Confirm the user's `~/.config/rust-edit/config.toml` was **not** rewritten with temp-file recents (the `cfg(not(test))` guard).

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/config.rs
git commit -m "$(cat <<'EOF'
feat(app): replace untitled placeholder when opening a file

Add App::open_file_in_tab: read the file, replace the auto-created
untitled tab in place when it is unmodified, otherwise append.
Skip Config persist while unit tests run so temp files cannot
clobber the user's config.toml.

EOF
)"
```

---

### Task 3: Duplicate-tab prevention (canonical paths)

**Files:**
- Modify: `src/app.rs` tests only, unless Task 2's `index_of_open_path` is missing (then fix production)

- [ ] **Step 1: Write failing duplicate tests**

```rust
    #[test]
    fn open_same_file_twice_focuses_existing_tab() {
        let path = unique_temp_file("once");
        let mut app = App::new(&[]);
        app.open_file_in_tab(&path).unwrap();
        app.new_tab();
        assert_eq!(app.buffers.len(), 2);
        assert_eq!(app.active, 1);
        app.open_file_in_tab(&path).unwrap();
        assert_eq!(app.buffers.len(), 2, "must not duplicate");
        assert_eq!(app.active, 0, "existing tab focused");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_relative_and_absolute_path_are_the_same_tab() {
        // Cargo.toml at the crate root is a real file both path forms can name.
        let rel = PathBuf::from("Cargo.toml");
        let abs = std::fs::canonicalize(&rel).expect("Cargo.toml must exist");
        let mut app = App::new(&[]);
        app.open_file_in_tab(&rel).unwrap();
        assert_eq!(app.buffers.len(), 1);
        app.open_file_in_tab(&abs).unwrap();
        assert_eq!(app.buffers.len(), 1, "relative vs absolute must not duplicate");
        assert_eq!(app.active, 0);
        let opened = app.buffers[0].path.as_ref().unwrap();
        assert!(
            Self::paths_name_same_file(opened, &rel),
            "opened {:?} should be Cargo.toml",
            opened
        );
    }
```

`Self::` is invalid in a free test function. Use a local helper instead of that last assert:

```rust
    #[test]
    fn open_relative_and_absolute_path_are_the_same_tab() {
        let rel = PathBuf::from("Cargo.toml");
        let abs = std::fs::canonicalize(&rel).expect("Cargo.toml must exist");
        let mut app = App::new(&[]);
        app.open_file_in_tab(&rel).unwrap();
        app.open_file_in_tab(&abs).unwrap();
        assert_eq!(app.buffers.len(), 1, "relative vs absolute must not duplicate");
        assert_eq!(app.active, 0);
    }
```

If Task 2 already implemented `index_of_open_path` correctly, these tests will PASS on the first run. That violates TDD's "watch it fail" rule. Handle it this way:

1. Temporarily comment out the `if let Some(idx) = self.index_of_open_path` branch in `open_file_in_tab` (keep the helper).
2. Run the two tests — Expected: FAIL, `buffers.len() == 2` (or 3).
3. Restore the branch.
4. Run again — Expected: PASS.

Do **not** skip the red step. If you cannot get a red by commenting the branch, the tests are not actually covering duplicates.

Also add (spec: deleted-on-disk fallback). This one is optional if constructing it is awkward; the helper already falls back on canonicalize `Err`. A focused unit-style check:

```rust
    #[test]
    fn duplicate_check_falls_back_to_raw_path_when_canonicalize_fails() {
        // A buffer whose path no longer exists cannot be canonicalized.
        let mut app = App::new(&[]);
        let gone = PathBuf::from("/definitely/missing/rust-edit-gone-file.rs");
        app.buffers[0].set_path(gone.clone());
        app.buffers[0].is_initial = false;
        // open_file_in_tab will fail at read — that is rule 1, and is correct.
        // Directly exercise the matcher instead:
        assert_eq!(app.index_of_open_path(&gone), Some(0));
    }
```

`index_of_open_path` is private. Either make it `pub(crate)` or skip this test. Prefer `pub(crate)` on `index_of_open_path` and `placeholder_index` so tests can reach them without weakening encapsulation further.

- [ ] **Step 2: Watch duplicates fail, then restore the matcher, then pass**

```bash
"$CARGO" test --bin rust-edit open_same_file_twice_focuses_existing_tab open_relative_and_absolute_path_are_the_same_tab -- --nocapture
```

Expected after restore: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "$(cat <<'EOF'
feat(app): focus existing tab instead of opening a duplicate

Canonicalize both the requested path and each buffer path (raw-path
fallback if canonicalize fails) so relative vs absolute opens of the
same file activate the existing tab.

EOF
)"
```

---

### Task 4: `[+]` never replaced; append when no placeholder; last-tab reset is replaceable

**Files:**
- Modify: `src/app.rs` tests (production already behaves if Tasks 1–2 are correct)

- [ ] **Step 1: Write the remaining spec tests**

```rust
    #[test]
    fn plus_created_empty_tab_is_never_replaced() {
        // Start with a real file so the only empty tab is the [+] one.
        let first = unique_temp_file("first");
        let mut app = App::new(&[first.to_string_lossy().to_string()]);
        assert_eq!(app.buffers.len(), 1);
        assert!(!app.buffers[0].is_initial);
        app.new_tab();
        assert!(!app.buffers[1].is_initial);
        let second = unique_temp_file("second");
        app.open_file_in_tab(&second).unwrap();
        assert_eq!(app.buffers.len(), 3, "[+] empty tab must be kept; file appended");
        assert!(app.buffers[1].path.is_none());
        assert!(!app.buffers[1].is_initial);
        assert_eq!(app.buffers[2].path.as_deref(), Some(second.as_path()));
        assert_eq!(app.active, 2);
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
    }

    #[test]
    fn open_appends_when_app_started_with_a_file() {
        let first = unique_temp_file("one");
        let mut app = App::new(&[first.to_string_lossy().to_string()]);
        let second = unique_temp_file("two");
        app.open_file_in_tab(&second).unwrap();
        assert_eq!(app.buffers.len(), 2);
        assert_eq!(app.active, 1);
        assert_eq!(app.buffers[0].path.as_deref(), Some(first.as_path()));
        assert_eq!(app.buffers[1].path.as_deref(), Some(second.as_path()));
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
    }

    #[test]
    fn placeholder_after_closing_last_tab_is_replaceable() {
        let first = unique_temp_file("closing");
        let mut app = App::new(&[first.to_string_lossy().to_string()]);
        app.close_current_tab(); // unmodified → reset to placeholder
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        let second = unique_temp_file("after-close");
        app.open_file_in_tab(&second).unwrap();
        assert_eq!(app.buffers.len(), 1, "reset placeholder must be replaced");
        assert_eq!(app.buffers[0].path.as_deref(), Some(second.as_path()));
        assert_eq!(app.active, 0);
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
    }
```

If production is already correct, these will pass immediately. To watch red:

- For `plus_created_empty_tab_is_never_replaced`: temporarily treat `path.is_none() && !is_modified` as a placeholder (ignore `is_initial`). Expected FAIL: len == 2 (the `[+]` tab got replaced). Then restore `is_initial` in the predicate.
- For the other two: they already passed in Task 2's replace/append logic. Still add them as spec-required regression tests; do not delete them because they are "already green."

- [ ] **Step 2: Run**

```bash
"$CARGO" test --bin rust-edit plus_created_empty_tab_is_never_replaced open_appends_when_app_started_with_a_file placeholder_after_closing_last_tab_is_replaceable -- --nocapture
"$CARGO" test --bin rust-edit
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "$(cat <<'EOF'
test(app): cover [+] tabs, CLI-started append, and last-tab reset

Lock the spec edge cases: user-created empty tabs are never replaced,
opening a second file from a CLI-started session appends, and the
placeholder recreated after closing the last tab is replaceable.

EOF
)"
```

---

### Task 5: Wire `close_file_dialog` open branch

**Files:**
- Modify: `src/app.rs:905-917` (`close_file_dialog` else / open branch)

- [ ] **Step 1: Write failing dialog tests**

Current open branch always `push`es. After Task 2, `open_file_in_tab` exists but the dialog does not call it, so this test fails (2 tabs):

```rust
    #[test]
    fn file_dialog_open_replaces_placeholder() {
        let mut app = App::new(&[]);
        let path = unique_temp_file("from-dialog");
        app.show_file_dialog(false);
        app.file_path_input = path.to_string_lossy().to_string();
        app.close_file_dialog(0); // Open
        assert!(!app.show_file_dialog, "success must dismiss the dialog");
        assert_eq!(app.buffers.len(), 1, "dialog must use open_file_in_tab");
        assert_eq!(app.buffers[0].text(), "from-dialog");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn file_dialog_open_error_keeps_dialog_and_buffers() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.file_path_input = "/this/does/not/exist/rust-edit-nope.txt".into();
        app.close_file_dialog(0);
        assert!(app.show_file_dialog, "error must keep the dialog open");
        assert!(
            app.file_dialog_message.starts_with("Open error:"),
            "got {:?}",
            app.file_dialog_message
        );
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
    }
```

- [ ] **Step 2: Run to verify fail**

```bash
"$CARGO" test --bin rust-edit file_dialog_open_replaces_placeholder file_dialog_open_error_keeps_dialog -- --nocapture
```

Expected: `file_dialog_open_replaces_placeholder` FAIL with `buffers.len() == 2`. Error test may already PASS (current code already returns on `Err` with that message). If the error test passes immediately, keep it as a regression; do not weaken it.

- [ ] **Step 3: Replace the open branch**

In `close_file_dialog`, replace the `else { match std::fs::read_to_string ... }` block with:

```rust
        } else {
            match self.open_file_in_tab(&path) {
                Ok(()) => {}
                Err(e) => {
                    self.file_dialog_message = format!("Open error: {e}");
                    return;
                }
            }
        }
```

Do not touch the save branch. Do not add a second `record_recent_file` here — `open_file_in_tab` already records.

- [ ] **Step 4: Run tests**

```bash
"$CARGO" test --bin rust-edit file_dialog_open_replaces_placeholder file_dialog_open_error_keeps_dialog -- --nocapture
"$CARGO" test --bin rust-edit
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "$(cat <<'EOF'
feat(app): route File > Open through open_file_in_tab

The file dialog open button (and therefore Browse / Recents, which
only fill file_path_input) now replaces the placeholder and refuses
duplicate tabs. Read errors still keep the dialog open.

EOF
)"
```

---

### Task 6: Docs, fmt, clippy

**Files:**
- Modify: `AGENTS.md` (Buffer fields bullet ~line 141)
- Modify: `docs/Todo-List.md` (line 4)

- [ ] **Step 1: Update AGENTS.md Buffer fields bullet**

Replace the `Buffer` fields sentence with:

```
- `Buffer` fields: `cursor` (byte offset), `selection` (normalized `Range`) + `selection_anchor` (fixed anchor for bidirectional extend), `scroll_offset` (row, col), `visible_lines`/`visible_width` (synced each frame), `generation` (invalidation key for the highlight cache), `is_initial` (true only for the auto-created untitled placeholder at startup or after closing the last tab; `[+]` tabs stay false). Auto-scroll lives in `adjust_scroll` (vertical+horizontal edge clamping) and `ensure_cursor_*`. File open goes through `App::open_file_in_tab` (duplicate-focus, then in-place placeholder replace, else append).
```

- [ ] **Step 2: Check off the bug in Todo-List.md**

Change line 4 from `- [ ]` to `- [x]` for: "When opening a file and the default untitled document is open and not modified…"

- [ ] **Step 3: Copy the plan into the repo if it is not already there**

```bash
# If docs/superpowers/plans/2026-08-29-untitled-tab-replacement.md does not exist,
# write this plan there so future sessions can find it next to the spec.
```

- [ ] **Step 4: fmt + clippy + full test suite**

```bash
"$CARGO" fmt
"$CARGO" clippy -- -D warnings
"$CARGO" test
```

Expected: clippy clean, all tests pass. Fix any new warnings (unused imports, needless `clone`, private-in-public) in this task — do not leave them.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md docs/Todo-List.md docs/superpowers/plans/2026-08-29-untitled-tab-replacement.md
git commit -m "$(cat <<'EOF'
docs: record placeholder-tab open behavior

Note Buffer.is_initial and App::open_file_in_tab in AGENTS.md and
mark the untitled-tab bug fixed in the todo list.

EOF
)"
```

---

## Manual checklist (AGENTS.md §7 — after implementation, 2 terminals)

- [ ] Launch with no args → open a file via dialog, via file browser, and via Recent files → single tab showing the file.
- [ ] Type in the untitled tab → open a file → both tabs remain.
- [ ] Open an already-open file → focuses existing tab, no duplicate.
- [ ] Close all tabs → open a file → single tab (no stray empty tab).
- [ ] `[+]` empty tab + open file → both tabs remain.
- [ ] Type then fully undo in the placeholder → open a file → single tab.
- [ ] Open a missing path → dialog stays open with `Open error: ...`; untitled tab unchanged.

No UI/layout change, so no extra visual QA beyond the tab bar count/labels.

---

## Spec coverage (self-review)

| Spec requirement | Task |
|---|---|
| `is_initial` on `Buffer`, default false in `new` / `from_str` | 1 |
| Set true at startup / last-tab close / confirm-close last tab | 1 |
| `[+]` never auto-closed | 1 + 4 |
| Read failure → error, no change | 2 + 5 |
| Duplicate → focus existing (canonicalize, raw fallback) | 3 |
| Placeholder replace in place, become active | 2 |
| Else append | 4 |
| Recents after success | 2 (inside `open_file_in_tab`) |
| `close_file_dialog` open branch calls `open_file_in_tab` | 5 |
| Typed-then-undone still replaced | 2 |
| Inactive placeholder still replaced | 2 |
| Last-tab reset replaceable | 4 |
| CLI startup unchanged | (no task) |
| Header / highlight cache unchanged | (no task) |
| Unit tests 1–7 | 2, 3, 4 |
| Manual checklist | post-implementation |
| AGENTS.md / Todo-List | 6 |

No placeholders, TBDs, or "similar to Task N" shortcuts. Method names used later (`open_file_in_tab`, `placeholder_index`, `index_of_open_path`, `canonical_or_raw`, `Buffer::placeholder`) are defined in Tasks 1–2.

---

## Execution

Work in an isolated git worktree (using-git-worktrees), not directly on `main`. Follow TDD per task. After all tasks: `cargo fmt && cargo clippy -- -D warnings && cargo test`, then finishing-a-development-branch.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, spec-compliance review then code-quality review between tasks.
2. **Inline Execution** — execute tasks in this session using executing-plans, with checkpoints.

Which approach?
