# Word Wrap — Design Specification

**Date:** 2026-06-29
**Status:** Approved (pending implementation)
**Scope:** Phase 1 enhancement — visual (soft) word wrap for the editor pane

## 1. Goal

When a logical line is longer than the visible editor width, wrap it across
multiple screen rows so all text is visible without horizontal scrolling.

Wrap is **visual only** (soft wrap). It does not insert newlines, does not
change the buffer's byte content, and does not change the logical line count.
Wrapped continuation rows carry no line number and do not increment the
status-bar `Ln`/`lines` counters.

## 2. User-facing behavior

- **Toggle**: `Alt+Z` (matches VS Code convention). Also reachable via the
  **Tools** menu (`"Toggle Word Wrap (Alt+Z)"`) so the mouse path exists
  (AGENTS.md Core Principle 2).
- **Default**: off (matches existing `Config::word_wrap` default).
- **Persistence**: toggle writes through to `config.toml` via
  `Config::set_word_wrap`, mirroring `set_theme`.
- **Wrap boundary**: word boundary, with **character fallback**.
  - Break at the last whitespace at or before the width column.
  - If a single word is longer than the width (e.g. a long URL), break
    mid-word at the width column so no text is hidden.
- **Continuation rows**: blank indent. The first sub-row draws the normal
  gutter (`{:>4}| `, 6 columns); continuation sub-rows draw 6 spaces so
  wrapped text aligns vertically with the text on the first row.
- **Horizontal scroll**: disabled while wrap is on. `scroll_offset.1` is
  ignored by the renderer and the cursor-placement math.
- **Status bar**: unchanged. It already reports logical `Ln`, `Col`, and
  `lines` (`src/ui/status.rs`), which is the desired behavior under wrap.
- **Help popup**: a new `("Alt+Z", "Toggle word wrap")` row is added to the
  `View & Tools` category in `src/ui/popups/help.rs`.
- **Settings popup**: the hardcoded `Line nums: on` line is replaced with a
  live `Word wrap: {on|off}` line so users can see the current state.

## 3. Non-goals

- Hard wrap / fill-paragraph (inserting newlines into the buffer).
- Per-buffer wrap state (wrap is a global editor setting, like theme).
- Wrap-aware reflow of the find/replace match counter — match byte ranges
  remain correct; only the scroll-to-match math changes (see §5.4).
- Re-rendering markdown preview (out of scope; was removed per AGENTS.md §1).

## 4. Architecture

**Approach: render-time mapping (Approach A).**

The wrap layout is computed fresh each frame from `buf.visible_width`. No
new persistent state is added to `Buffer`. When `word_wrap == false`, the
wrap code path is bypassed entirely, so existing behavior — and the existing
byte-offset test suite in `buffer.rs` — is unaffected.

Rationale: `cursor`, `selection`, and `scroll_offset` invariants are heavily
tested and central to the editor. Coupling them to a width-dependent layout
(Approach B) would create a large invalidation surface across every mutation
path. Keeping the layout as a render-time artifact isolates risk to the new
module and a small number of explicitly wrapped call sites.

### 4.1 Key invariant: `scroll_offset.0` reinterpretation

When `word_wrap` is **off**, `scroll_offset.0` is a logical line index
(unchanged from today).

When `word_wrap` is **on**, `scroll_offset.0` is reinterpreted as a
**screen row** (a row in the wrapped layout). This avoids introducing a
parallel scroll field and keeps every scroll/cursor function operating on a
single integer.

The layout helper (§5.1) translates between screen rows and logical lines
whenever needed. Toggling wrap re-clamps `scroll_offset.0` so the cursor
remains visible under the new interpretation.

### 4.2 Sizing: where the layout is computed

- `editor.rs::render` computes the layout once per frame over a window of
  logical lines large enough to fill the viewport (start at
  `scroll_offset.0`'s logical line; expand until `total_screen_rows` covers
  `area.height`).
- Non-render call sites that need screen-row math (wheel scroll, mouse
  click/drag, cursor visibility on key events, find jump) compute a fresh
  layout inline. This is cheap because only the visible window is iterated.

## 5. Component changes

### 5.1 New module: `src/wrap.rs`

Pure, testable, depends only on `ropey::Rope` and a width.

```rust
pub struct WrapLayout {
    /// One entry per visible logical line (indexed relative to `start_line`).
    /// `breaks[i]` is the list of byte offsets (exclusive) that end each
    /// wrapped screen row, excluding the final row (which ends at the line's
    /// end). Empty vec => the line fits on one screen row.
    pub breaks: Vec<Vec<usize>>,
    /// Sum of sub-row counts across all entries in `breaks`.
    pub total_screen_rows: usize,
}

/// Compute the wrap layout for `count` logical lines starting at `start_line`.
/// `width` is in display columns (already excludes gutter).
pub fn compute_layout(rope: &Rope, start_line: usize, count: usize, width: usize)
    -> WrapLayout;

/// Total screen rows across the entire buffer at the given width.
/// Used by `clamp_scroll_wrapped` to compute max scroll.
pub fn total_screen_rows(rope: &Rope, width: usize) -> usize;

/// Screen row (relative to the layout's window start) containing `byte`.
/// `byte` is absolute (buffer-relative); the function uses `rope` to find
/// the logical line and `start_line` to index into `layout.breaks`.
pub fn screen_row_of_byte(
    layout: &WrapLayout,
    rope: &Rope,
    start_line: usize,
    byte: usize,
) -> usize;

/// Byte offset of the cursor placed at (`screen_row`, `col`) within the
/// layout's window. Used for mouse hit-testing.
pub fn byte_of_screen_row(
    layout: &WrapLayout,
    rope: &Rope,
    start_line: usize,
    screen_row: usize,
    col: usize,
) -> usize;
```

**Wrap algorithm per logical line** (word boundary + char fallback):

1. Iterate chars with `rope.line(line).chars().enumerate()` accumulating
   byte offset and display column (tabs expand to next multiple of
   `tab_size`; wide chars count as 2 columns — consistent with the existing
   gutter comment at `editor.rs:65-67`).
2. Track `last_space_byte` (most recent whitespace at column > 0).
3. When the next char would exceed `width`:
   - If `last_space_byte` is past the line's row start, break at
     `last_space_byte`; resume the next row at the next non-whitespace char.
   - Else break at the current char (char fallback for over-long words).
4. A line of length exactly `width` does **not** wrap (break only on
   exceed).

### 5.2 `src/ui/editor.rs` — rendering

The current loop (lines 32-110) emits one `Line` per logical line. Under
wrap it emits one `Line` per **screen row**.

- At top of `render`, if `app.config.word_wrap`, compute
  `let layout = wrap::compute_layout(buf.rope, start_line, count, buf.visible_width);`
  where `count` is grown until `layout.total_screen_rows >= area.height`
  (or buffer ends).
- Iterate the layout's entries; for each visible logical line, slice its
  pre-highlighted `Vec<Span>` into N sub-row span vectors according to
  `layout.breaks[i]`.
  - **First sub-row**: prepend gutter span `Span::styled(format!("{:>4}| ", line_idx + 1), theme.line_number_style())`.
  - **Continuation sub-rows**: prepend `Span::raw("      ")` (6 spaces) to
    align the text column.
- Horizontal scroll (`buf.scroll_offset.1`) is **not consulted** when wrap
  is on.
- Per-character selection highlight (lines 80-94) is preserved byte-for-byte
  as spans are sliced — the existing `abs_byte >= sel.start && abs_byte < sel.end` check runs unchanged.
- Scrollbar calibration (lines 136-142) uses `layout.total_screen_rows`
  instead of `line_count` when wrap is on.

When wrap is off, the existing code path runs unchanged.

### 5.3 `src/buffer.rs` — scroll/cursor helpers

Add screen-row-aware variants that take a `&WrapLayout`. The existing
logical-line variants stay for the wrap-off path.

- `adjust_scroll_wrapped(&mut self, layout: &WrapLayout, cursor_byte: usize)`
  — clamp the cursor's screen row into `[scroll_offset.0, scroll_offset.0 + visible_lines)`.
- `clamp_scroll_wrapped(&mut self, total_screen_rows: usize)`
  — `max = total_screen_rows.saturating_sub(self.visible_lines)`.
- `scroll_wrapped(&mut self, delta: isize)` — `scroll_offset.0 =
  (scroll_offset.0 as isize + delta).max(0) as usize`.
- `page_up_wrapped` / `page_down_wrapped` — move by `visible_lines` screen
  rows, then re-clamp cursor visibility.
- `ensure_cursor_horizontally_visible` — call sites are guarded to skip this
  when wrap is on (it becomes a no-op).

`cursor_position()` (used by the status bar) is unchanged — it returns the
logical `(line, col)`, which is what we want to display.

### 5.4 `src/app.rs` and `src/main.rs` — wiring

- **`refresh_highlight_cache`** (`app.rs:172-210`): keyed on logical lines,
  needs **no change** to its invariants. The caller in `main.rs:1178` may
  pass a larger `count` when wrap is on so the cached window covers every
  logical line whose sub-rows intersect the viewport (use the layout).
- **Wheel scroll** (`main.rs:158-169`): branch on `app.config.word_wrap`;
  call `scroll_wrapped` + `clamp_scroll_wrapped` with a freshly computed
  `total_screen_rows`.
- **Hardware-cursor placement** (`main.rs:87-102`): when wrap is on,
  `screen_y = screen_row_of_byte(&layout, cursor_byte) - scroll_offset.0`
  (note `scroll_offset.0` is a screen row under wrap); `screen_x` ignores
  `scroll_offset.1`.
- **Mouse click/drag** (`main.rs:886-913`, `995-1016`): translate the
  clicked screen row to `(logical_line, col)` via `byte_of_screen_row`,
  then set `buf.cursor` (byte offset) and clear/extend selection. The
  existing `set_cursor_from_visual` / `byte_at_visual` paths remain for the
  wrap-off case.
- **`jump_to_match_index`** (`app.rs:378-404`): when wrap is on, scroll math
  uses `screen_row_of_byte`; skip `ensure_cursor_horizontally_visible`.
- **Toggle key** (`main.rs`, Alt-key match block ~line 334): add an arm for
  `KeyCode::Char('z')` with `KeyModifiers::ALT` that flips
  `app.config.word_wrap` via `set_word_wrap` and triggers a redraw +
  viewport re-sync.
- **Tools menu**: add `"Toggle Word Wrap (Alt+Z)"` to `tools_items`
  (`header.rs:108-113`). Bump the Tools item count from `4` to `5` in
  **four** places in `main.rs` (lines 499, 506, 690, 720). Add match arm
  `("Tools", 4) => toggle` in `handle_menu_action` (`main.rs:1118`).
- **Settings popup**: change `settings.rs::render` to take `word_wrap: bool`
  (update the single call site). Replace the hardcoded `Line nums: on` row
  with `format!("Word wrap: {}", if word_wrap { "on" } else { "off" })`.

### 5.5 `src/config.rs`

`word_wrap: bool` already exists (`config.rs:16`). Add:

```rust
pub fn set_word_wrap(&mut self, on: bool) {
    self.word_wrap = on;
    let _ = self.save();
}
```

mirroring `set_theme` (`config.rs:68`).

## 6. Edge cases

- **Empty buffer / single short line**: layout returns one row; behaves
  exactly as today.
- **Line exactly `width` columns**: no wrap (break only when exceeding).
- **Tab characters**: expand to the next multiple of `tab_size` when
  counting columns (consistent with existing rendering).
- **Wide chars (CJK, emoji)**: count as 2 display columns.
- **Very long single word**: char fallback breaks at `width`.
- **Toggle at runtime**: re-clamp scroll so cursor stays visible.
- **Window resize**: the existing per-frame `visible_width` sync
  (`main.rs:64-75`) recalculates layout automatically.

## 7. Testing

### 7.1 Unit tests (`src/wrap.rs`, pure logic)

- Short line → 1 row, empty `breaks`.
- Line of N chars at width W → ⌈N/W⌉ rows (char fallback path).
- Line with spaces → breaks at word boundaries.
- Over-long word mid-line → word on its own row(s), continuation resumes
  after the word.
- Multibyte (CJK) line → byte-correct breaks; no splitting inside a
  codepoint.
- `screen_row_of_byte` and `byte_of_screen_row` round-trip property test
  over a corpus of generated lines.

### 7.2 Regression

The existing `buffer.rs` byte-offset tests (lines 846-971) must pass
unchanged. Approach A guarantees this: `cursor`, `selection`, and
`scroll_offset` semantics are untouched when wrap is off, and the wrap code
path is fully bypassed.

### 7.3 Build / lint

`cargo fmt && cargo clippy -- -D warnings && cargo test` must pass.

### 7.4 Manual test checklist (run in kitty + alacritty)

- [ ] `Alt+Z` toggles wrap on and off; setting persists across restarts.
- [ ] Long line wraps at word boundaries; continuation rows have no line
      number and align under the text column.
- [ ] Status bar `Ln`, `Col`, and `lines` reflect logical values (unchanged
      by wrap).
- [ ] Clicking on a wrapped continuation row places the cursor on the right
      logical line and column.
- [ ] PgUp / PgDn move by screen rows.
- [ ] Mouse wheel scrolls by screen rows.
- [ ] Find (`Ctrl+F`) jumps to a match on a wrapped line and scrolls it into
      view correctly.
- [ ] Selection across a wrap boundary highlights correctly on every
      sub-row.
- [ ] A line longer than the width containing a single long word wraps at
      `width` (char fallback) rather than overflowing.
- [ ] Tools menu item toggles wrap; help popup lists `Alt+Z`.
- [ ] Turning wrap off restores horizontal scroll and line-per-row layout
      cleanly.
- [ ] No layout breakage on resize (narrow ↔ wide terminal).
- [ ] Large file (10k+ lines, some long) remains responsive while wrap is on.

## 8. Files touched

**New:**
- `src/wrap.rs`

**Substantive:**
- `src/ui/editor.rs` (render loop)
- `src/buffer.rs` (screen-row scroll variants)
- `src/main.rs` (cursor placement, mouse mapping, scroll dispatch, key/menu wiring, highlight-cache count)
- `src/app.rs` (find-jump scroll path)
- `src/ui/popups/settings.rs` (live wrap-state row + signature change)

**Trivial:**
- `src/config.rs` (`set_word_wrap` helper)
- `src/ui/header.rs` (Tools menu item)
- `src/ui/popups/help.rs` (shortcut row)
- `src/ui/mod.rs` (only if `settings::render` signature change ripples)

**Unchanged:**
- `src/ui/status.rs`, `src/ui/find.rs`, `src/syntax.rs`, `src/theme.rs`,
  `src/ui/layout.rs`, all other popups.

## 9. Tradeoffs

- **Render-time layout (Approach A) vs stored layout (B)**: A keeps
  `Buffer`'s invariants intact and the byte-offset test suite safe, at the
  cost of slightly more argument passing and a per-frame recompute (cheap
  because only the visible window is iterated). If profiling later shows a
  hotspot on huge files, upgrade to a cached layout on `App` (Approach C).
- **`scroll_offset.0` reinterpretation vs separate field**: reinterpretation
  avoids a parallel field and dual scroll code paths in the common case; the
  cost is two well-named variants of each scroll helper. A separate
  `scroll_screen_row` field would be cleaner long-term but is more churn
  today for no functional gain.
- **`set_word_wrap` writes to disk on every toggle**: matches `set_theme`
  behavior and is what users expect for a persistent setting. Negligible
  cost (tiny TOML file).
