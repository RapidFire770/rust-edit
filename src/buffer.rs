// Buffer struct + Ropey wrapper + core editing ops (insert, delete, undo, indent, etc.)
// Sprint 1 core implementation target. See Design Spec §6 and AGENTS §6.

#![allow(dead_code)]

use ropey::Rope;
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Plain,
    Rust,
    Python,
    Markdown,
    Json,
    Toml,
    Yaml,
    Bash,
    C,
    Cpp,
    Javascript,
    Typescript,
}

impl Language {
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "rs" => Language::Rust,
            "py" | "pyw" => Language::Python,
            "md" | "markdown" => Language::Markdown,
            "json" => Language::Json,
            "toml" => Language::Toml,
            "yaml" | "yml" => Language::Yaml,
            "sh" | "bash" | "zsh" => Language::Bash,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" => Language::Cpp,
            "js" | "mjs" => Language::Javascript,
            "ts" => Language::Typescript,
            _ => Language::Plain,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Buffer {
    pub rope: Rope,
    pub path: Option<PathBuf>,
    pub language: Language,
    pub is_modified: bool,

    /// Byte offset of cursor in the rope.
    pub cursor: usize,
    /// Optional selection [start, end) with start <= end. When None, no selection.
    pub selection: Option<Range<usize>>,
    /// (row, col_offset) for viewport scrolling (rows are 0-based).
    pub scroll_offset: (usize, usize),
    /// Last known height (in rows) of the editor viewport. Refreshed each frame
    /// by the run loop so adjust_scroll can clamp against the *real* height
    /// instead of a guessed constant. Fixes half-screen scroll jumping on long files.
    pub visible_lines: usize,
    /// Bumped on every text mutation (insert/backspace/delete/newline/indent/dedent/undo/redo).
    /// Used as an invalidation key by the highlight cache so spans are only recomputed
    /// on edit or scroll into new lines (see AGENTS §6 "Syntax Highlighting").
    pub generation: u64,

    /// Undo stack (full state snapshots for robust undo of text + caret + view).
    #[allow(clippy::type_complexity)]
    undo_stack: Vec<(Rope, usize, Option<std::ops::Range<usize>>, (usize, usize))>,
    /// Redo stack.
    #[allow(clippy::type_complexity)]
    redo_stack: Vec<(Rope, usize, Option<std::ops::Range<usize>>, (usize, usize))>,
    /// Max undo depth (keeps memory reasonable).
    max_undo: usize,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    pub fn new() -> Self {
        let rope = Rope::new();
        Self {
            rope,
            path: None,
            language: Language::Plain,
            is_modified: false,
            cursor: 0,
            selection: None,
            scroll_offset: (0, 0),
            visible_lines: 20,
            generation: 0,
            undo_stack: Vec::with_capacity(32),
            redo_stack: Vec::new(),
            max_undo: 100,
        }
    }

    pub fn from_str(s: &str, path: Option<PathBuf>) -> Self {
        let rope = Rope::from_str(s);
        let language = path
            .as_ref()
            .map(|p| Language::from_path(p.as_ref()))
            .unwrap_or_default();
        Self {
            rope,
            path,
            language,
            is_modified: false,
            cursor: 0,
            selection: None,
            scroll_offset: (0, 0),
            visible_lines: 20,
            generation: 0,
            undo_stack: Vec::with_capacity(32),
            redo_stack: Vec::new(),
            max_undo: 100,
        }
    }

    /// Total number of lines (at least 1).
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get a line by 0-based index (without the trailing newline).
    pub fn line(&self, idx: usize) -> String {
        if idx >= self.line_count() {
            return String::new();
        }
        self.rope.line(idx).to_string()
    }

    /// Current cursor as (line, col) 0-based.
    pub fn cursor_position(&self) -> (usize, usize) {
        if self.rope.len_chars() == 0 {
            return (0, 0);
        }
        let line = self.rope.byte_to_line(self.cursor);
        let line_start_byte = self.rope.line_to_byte(line);
        let col = self
            .rope
            .byte_slice(line_start_byte..self.cursor)
            .to_string()
            .chars()
            .count();
        (line, col)
    }

    /// Gutter width used for rendering line numbers (used for mouse hit testing).
    pub const GUTTER_WIDTH: usize = 6;

    /// Set cursor from a visual line (absolute buffer line, 0-based) and visual column (chars after gutter).
    /// Used for mouse clicks in editor.
    pub fn set_cursor_from_visual(&mut self, visual_line: usize, visual_col: usize) {
        let n = self.line_count();
        if visual_line >= n {
            // Support clicking in padded blank rows below content to place at EOF (append).
            if n == 0 {
                self.cursor = 0;
            } else {
                let last = n - 1;
                let ls = self.rope.line_to_byte(last);
                let lstr = self.line(last);
                let full = lstr.chars().count();
                let mut byte = ls;
                for (i, c) in lstr.chars().enumerate() {
                    if i >= full {
                        break;
                    }
                    byte += c.len_utf8();
                }
                self.cursor = byte;
            }
            self.selection = None;
            return;
        }
        let line_start_byte = self.rope.line_to_byte(visual_line);
        let line_str = self.line(visual_line);
        let mut byte = line_start_byte;
        for (chars, ch) in line_str.chars().enumerate() {
            if chars >= visual_col {
                break;
            }
            byte += ch.len_utf8();
        }
        self.cursor = byte;
        self.selection = None;
        // Do not auto-adjust here; callers (mouse place) decide whether to ensure_visible
        // to avoid unwanted jumps when clicking inside the current viewport.
    }

    /// Scroll the view by delta lines (positive = down).
    pub fn scroll(&mut self, delta: isize) {
        let current = self.scroll_offset.0 as isize;
        let new_scroll = (current + delta).max(0) as usize;
        let max_scroll = self.line_count().saturating_sub(1);
        self.scroll_offset.0 = new_scroll.min(max_scroll);
    }

    /// Clamp scroll to valid range given approximate visible lines.
    pub fn clamp_scroll(&mut self, visible_lines: usize) {
        let max = self.line_count().saturating_sub(visible_lines.max(1));
        if self.scroll_offset.0 > max {
            self.scroll_offset.0 = max;
        }
    }

    /// Compute the byte offset for a visual line and column (does not mutate).
    pub fn byte_at_visual(&self, visual_line: usize, visual_col: usize) -> usize {
        if visual_line >= self.line_count() {
            return self.rope.len_bytes();
        }
        let line_start_byte = self.rope.line_to_byte(visual_line);
        let line_str = self.line(visual_line);
        let mut byte = line_start_byte;
        for (chars, ch) in line_str.chars().enumerate() {
            if chars >= visual_col {
                break;
            }
            byte += ch.len_utf8();
        }
        byte
    }

    /// Save snapshot for undo (call before a mutating operation).
    pub fn push_undo(&mut self) {
        if self.undo_stack.len() >= self.max_undo {
            self.undo_stack.remove(0);
        }
        let state = (
            self.rope.clone(),
            self.cursor,
            self.selection.clone(),
            self.scroll_offset,
        );
        self.undo_stack.push(state);
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some((prev_rope, prev_cur, prev_sel, prev_scroll)) = self.undo_stack.pop() {
            let current = (
                self.rope.clone(),
                self.cursor,
                self.selection.clone(),
                self.scroll_offset,
            );
            self.redo_stack.push(current);
            self.rope = prev_rope;
            self.cursor = prev_cur.min(self.rope.len_bytes());
            self.selection = prev_sel;
            self.scroll_offset = prev_scroll;
            self.is_modified = !self.undo_stack.is_empty(); // rough
            self.bump_generation();
        }
    }

    pub fn redo(&mut self) {
        if let Some((next_rope, next_cur, next_sel, next_scroll)) = self.redo_stack.pop() {
            let current = (
                self.rope.clone(),
                self.cursor,
                self.selection.clone(),
                self.scroll_offset,
            );
            self.undo_stack.push(current);
            self.rope = next_rope;
            self.cursor = next_cur.min(self.rope.len_bytes());
            self.selection = next_sel;
            self.scroll_offset = next_scroll;
            self.bump_generation();
        }
    }

    /// Insert text at cursor. Clears selection if present (replace selection behavior later).
    pub fn insert(&mut self, text: &str) {
        self.push_undo();

        if let Some(sel) = self.selection.take() {
            // Replace selection with text
            let start = sel.start.min(self.rope.len_bytes());
            let end = sel.end.min(self.rope.len_bytes());
            self.rope.remove(start..end);
            self.rope.insert(start, text);
            self.cursor = start + text.len();
        } else {
            let pos = self.cursor.min(self.rope.len_bytes());
            self.rope.insert(pos, text);
            self.cursor = pos + text.len();
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Insert a single char (convenience for typing).
    pub fn insert_char(&mut self, ch: char) {
        self.insert(&ch.to_string());
    }

    /// Delete the char before cursor (backspace), or selection if present.
    pub fn backspace(&mut self) {
        self.push_undo();

        if let Some(sel) = self.selection.take() {
            let start = sel.start.min(self.rope.len_bytes());
            let end = sel.end.min(self.rope.len_bytes());
            self.rope.remove(start..end);
            self.cursor = start;
        } else if self.cursor > 0 {
            let pos = self.cursor;
            // Delete previous char (handle multi-byte)
            let prev_char_idx = self.rope.byte_to_char(pos) - 1;
            let remove_start = self.rope.char_to_byte(prev_char_idx);
            self.rope.remove(remove_start..pos);
            self.cursor = remove_start;
        }
        self.is_modified = true;
        self.bump_generation();
    }

    /// Delete the char after cursor (delete key).
    pub fn delete(&mut self) {
        self.push_undo();

        if let Some(sel) = self.selection.take() {
            let start = sel.start.min(self.rope.len_bytes());
            let end = sel.end.min(self.rope.len_bytes());
            self.rope.remove(start..end);
            self.cursor = start;
        } else if self.cursor < self.rope.len_bytes() {
            let pos = self.cursor;
            // Find end of next char
            let ch_idx = self.rope.byte_to_char(pos);
            if ch_idx < self.rope.len_chars() {
                let next_byte = self.rope.char_to_byte(ch_idx + 1);
                self.rope.remove(pos..next_byte);
            }
        }
        self.is_modified = true;
        self.bump_generation();
    }

    /// Move cursor by char delta (clamped).
    pub fn move_cursor(&mut self, delta_chars: isize, extend_selection: bool) {
        let current_char = self.rope.byte_to_char(self.cursor);
        let new_char = (current_char as isize + delta_chars)
            .max(0)
            .min(self.rope.len_chars() as isize) as usize;
        let new_byte = self.rope.char_to_byte(new_char);

        if extend_selection {
            let start = self
                .selection
                .as_ref()
                .map(|r| r.start)
                .unwrap_or(self.cursor);
            let end = new_byte;
            let (s, e) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            self.selection = Some(s..e);
        } else {
            self.selection = None;
        }
        self.cursor = new_byte;
        self.adjust_scroll();
    }

    /// Move to start of current line.
    pub fn move_home(&mut self, extend_selection: bool) {
        let line = self.rope.byte_to_line(self.cursor);
        let line_start = self.rope.line_to_byte(line);
        if extend_selection {
            // compute range similar to above
            let start = self
                .selection
                .as_ref()
                .map(|r| r.start)
                .unwrap_or(self.cursor);
            let (s, e) = if start <= line_start {
                (start, line_start)
            } else {
                (line_start, start)
            };
            self.selection = Some(s..e);
        } else {
            self.selection = None;
        }
        self.cursor = line_start;
        self.adjust_scroll();
    }

    /// Move to end of current line.
    pub fn move_end(&mut self, extend_selection: bool) {
        let line = self.rope.byte_to_line(self.cursor);
        let next_line_start = if line + 1 < self.rope.len_lines() {
            self.rope.line_to_byte(line + 1)
        } else {
            self.rope.len_bytes()
        };
        // end of line is before the \n if present
        let line_end = if next_line_start > 0 {
            // walk back over \n if any
            let mut end = next_line_start;
            if end > 0 && self.rope.char(self.rope.byte_to_char(end - 1)) == '\n' {
                end -= 1;
            }
            end
        } else {
            0
        };

        if extend_selection {
            let start = self
                .selection
                .as_ref()
                .map(|r| r.start)
                .unwrap_or(self.cursor);
            let (s, e) = if start <= line_end {
                (start, line_end)
            } else {
                (line_end, start)
            };
            self.selection = Some(s..e);
        } else {
            self.selection = None;
        }
        self.cursor = line_end;
        self.adjust_scroll();
    }

    /// Move cursor up by a number of lines (for arrows/PgUp), preserving column.
    pub fn move_lines_up(&mut self, count: usize, extend_selection: bool) {
        if count == 0 {
            return;
        }
        let (row, col) = self.cursor_position();
        if row == 0 {
            return;
        }
        let target_row = row.saturating_sub(count);
        self.move_to_line(target_row, col, extend_selection);
    }

    /// Move cursor down by a number of lines (for arrows/PgDn), preserving column.
    pub fn move_lines_down(&mut self, count: usize, extend_selection: bool) {
        if count == 0 {
            return;
        }
        let (row, col) = self.cursor_position();
        let last = self.line_count().saturating_sub(1);
        if row >= last {
            return;
        }
        let target_row = (row + count).min(last);
        self.move_to_line(target_row, col, extend_selection);
    }

    fn move_to_line(&mut self, target_row: usize, preferred_col: usize, extend_selection: bool) {
        if target_row >= self.line_count() {
            return;
        }
        let line_start_byte = self.rope.line_to_byte(target_row);
        let line_str = self.line(target_row);
        let target_col = preferred_col.min(line_str.chars().count());
        let mut byte_off = line_start_byte;
        for (i, c) in line_str.chars().enumerate() {
            if i >= target_col {
                break;
            }
            byte_off += c.len_utf8();
        }
        if extend_selection {
            let start = self
                .selection
                .as_ref()
                .map(|r| r.start)
                .unwrap_or(self.cursor);
            let end = byte_off;
            let (s, e) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            self.selection = Some(s..e);
        } else {
            self.selection = None;
        }
        self.cursor = byte_off;
        self.adjust_scroll();
    }

    /// Scroll view and cursor up by roughly a page (visible_lines - 1).
    /// Force the scroll_offset to move by the page delta so the *screen* visibly pages.
    pub fn page_up(&mut self, visible_lines: usize, extend_selection: bool) {
        let (cur_row, col) = self.cursor_position();
        let page = visible_lines.saturating_sub(1).max(1) as isize;
        let old_scroll = self.scroll_offset.0 as isize;
        let new_scroll = (old_scroll - page).max(0) as usize;
        self.scroll_offset.0 = new_scroll;
        // Place caret at a reasonable position in the newly paged view (approx same relative or top).
        let target_row = new_scroll + ((cur_row as isize - old_scroll).max(0) as usize);
        let target_row = target_row.min(self.line_count().saturating_sub(1));
        self.move_to_line(target_row, col, extend_selection);
    }

    /// Scroll view and cursor down by roughly a page (visible_lines - 1).
    /// Force the scroll_offset to move by the page delta so the *screen* visibly pages.
    pub fn page_down(&mut self, visible_lines: usize, extend_selection: bool) {
        let (cur_row, col) = self.cursor_position();
        let page = visible_lines.saturating_sub(1).max(1) as isize;
        let old_scroll = self.scroll_offset.0 as isize;
        let max_s = self.line_count().saturating_sub(1) as isize;
        let new_scroll = (old_scroll + page).min(max_s) as usize;
        self.scroll_offset.0 = new_scroll;
        // Place caret at a reasonable position in the newly paged view.
        let target_row = new_scroll + ((cur_row as isize - old_scroll).max(0) as usize);
        let target_row = target_row.min(self.line_count().saturating_sub(1));
        self.move_to_line(target_row, col, extend_selection);
    }

    /// Mark the buffer's text as changed so the highlight cache invalidates.
    /// Called from every mutation. Pure view/navigation ops must NOT call this.
    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Auto-scroll to keep the cursor visible, using the *real* viewport height.
    ///
    /// Scrolls ONLY when the cursor crosses the true top or bottom edge of the
    /// viewport; mid-window movement leaves the view untouched. This replaces an
    /// earlier version that used a hardcoded `20`/`10` guess, which caused the
    /// view to start scrolling ~halfway down the screen and to jump on every
    /// keystroke in long files. The logic mirrors `ensure_cursor_visible`.
    pub fn adjust_scroll(&mut self) {
        let v = self.visible_lines.max(1);
        let (row, _) = self.cursor_position();
        let s = self.scroll_offset.0;
        if row < s {
            // Cursor moved above the top edge → bring it to the top line.
            self.scroll_offset.0 = row;
        } else if row >= s + v {
            // Cursor moved below the bottom edge → recenter so it sits near the bottom.
            self.scroll_offset.0 = row.saturating_sub(v.saturating_sub(2));
        }
        // else: cursor inside viewport → do nothing (no half-screen jumping).
    }

    /// Ensure the cursor is at least minimally visible within the given viewport height.
    /// Only scrolls if the cursor row is completely outside the current view window.
    /// Used after mouse placement (don't jump view for in-view clicks) and edge cases.
    pub fn ensure_cursor_visible(&mut self, visible_lines: usize) {
        let (row, _) = self.cursor_position();
        let s = self.scroll_offset.0;
        let v = visible_lines.max(1);
        if row < s {
            self.scroll_offset.0 = row; // bring to top
        } else if row >= s + v {
            self.scroll_offset.0 = row.saturating_sub(v.saturating_sub(2));
        }
    }

    /// Smart indent on newline: insert \n + copy leading ws from current line + extra indent if appropriate.
    pub fn newline_with_indent(&mut self) {
        self.push_undo();

        let pos = self.cursor.min(self.rope.len_bytes());
        let line_idx = self.rope.byte_to_line(pos);
        let line_text = self.line(line_idx);

        // Leading whitespace of current line
        let leading_ws: String = line_text
            .chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .collect();

        // Decide whether to increase indent
        let trimmed = line_text.trim_end();
        let increase = matches!(trimmed.chars().last(), Some('{' | '[' | '(' | ':'));

        let mut insert_text = String::from("\n") + &leading_ws;
        if increase {
            // Use 4 spaces or tab? For now always 4 spaces (config driven later).
            insert_text.push_str("    ");
        }

        if let Some(sel) = self.selection.take() {
            let start = sel.start.min(self.rope.len_bytes());
            let end = sel.end.min(self.rope.len_bytes());
            self.rope.remove(start..end);
            self.rope.insert(start, &insert_text);
            self.cursor = start + insert_text.len();
        } else {
            self.rope.insert(pos, &insert_text);
            self.cursor = pos + insert_text.len();
        }
        self.is_modified = true;
        self.bump_generation();
    }

    /// Indent current line or selection by one level (4 spaces).
    pub fn indent(&mut self) {
        self.push_undo();
        // Simplified: indent whole lines touched by selection or cursor line
        let (start_line, end_line) = if let Some(sel) = &self.selection {
            (
                self.rope.byte_to_line(sel.start),
                self.rope.byte_to_line(sel.end.saturating_sub(1)),
            )
        } else {
            let l = self.rope.byte_to_line(self.cursor);
            (l, l)
        };

        for l in (start_line..=end_line).rev() {
            let line_start = self.rope.line_to_byte(l);
            self.rope.insert(line_start, "    ");
        }

        // Adjust cursor/selection roughly
        self.cursor += 4;
        self.is_modified = true;
        self.bump_generation();
    }

    /// Dedent current line or selection (remove up to 4 leading spaces).
    pub fn dedent(&mut self) {
        self.push_undo();
        let (start_line, end_line) = if let Some(sel) = &self.selection {
            (
                self.rope.byte_to_line(sel.start),
                self.rope.byte_to_line(sel.end.saturating_sub(1)),
            )
        } else {
            let l = self.rope.byte_to_line(self.cursor);
            (l, l)
        };

        for l in (start_line..=end_line).rev() {
            let line_start = self.rope.line_to_byte(l);
            let line_text = self.line(l);
            let leading = line_text.chars().take_while(|c| *c == ' ').count().min(4);
            if leading > 0 {
                let remove_end = line_start + leading;
                self.rope.remove(line_start..remove_end);
            }
        }
        self.is_modified = true;
        self.bump_generation();
    }

    /// Return full content as String (for save).
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Set path + re-detect language + clear modified.
    pub fn set_path(&mut self, path: PathBuf) {
        self.language = Language::from_path(&path);
        self.path = Some(path);
        self.is_modified = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_buffer_is_empty() {
        let b = Buffer::new();
        assert_eq!(b.line_count(), 1);
        assert_eq!(b.text(), "");
        assert_eq!(b.cursor_position(), (0, 0));
    }

    #[test]
    fn insert_and_cursor() {
        let mut b = Buffer::new();
        b.insert("hello");
        assert_eq!(b.text(), "hello");
        assert_eq!(b.cursor_position(), (0, 5));

        b.insert_char('\n');
        b.insert("world");
        assert_eq!(b.text(), "hello\nworld");
        assert_eq!(b.cursor_position(), (1, 5));
    }

    #[test]
    fn backspace_and_delete() {
        let mut b = Buffer::from_str("abc", None);
        b.cursor = 1; // after 'a'
        b.backspace();
        assert_eq!(b.text(), "bc");
        assert_eq!(b.cursor, 0);

        b.cursor = 1; // between b|c
        b.delete();
        assert_eq!(b.text(), "b");
    }

    #[test]
    fn smart_indent_basic() {
        let mut b = Buffer::from_str("def foo():", None);
        b.cursor = b.rope.len_bytes();
        b.newline_with_indent();
        assert!(b.text().starts_with("def foo():\n    "));
        assert!(b.cursor_position().1 >= 4);
    }

    #[test]
    fn undo_redo() {
        let mut b = Buffer::new();
        b.insert("hello");
        assert!(b.can_undo());
        b.undo();
        assert_eq!(b.text(), "");
        assert!(b.can_redo());
        b.redo();
        assert_eq!(b.text(), "hello");
    }

    #[test]
    fn language_detection() {
        assert_eq!(
            Language::from_path(&PathBuf::from("foo.rs")),
            Language::Rust
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("bar.py")),
            Language::Python
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("notes.md")),
            Language::Markdown
        );
    }

    // Investigation tests for regressions (page, mouse-like set on blank/eof, insert row stability, undo after moves)
    #[test]
    fn page_and_set_visual_on_blank_and_eof() {
        let mut b = Buffer::from_str("line0\nline1\n", None); // 3 lines, last empty
        assert_eq!(b.line_count(), 3);
        // place like mouse on last (blank) line col 0
        b.set_cursor_from_visual(2, 0);
        assert_eq!(b.cursor_position(), (2, 0));
        // insert on blank should keep row
        b.insert_char('X');
        assert_eq!(b.cursor_position().0, 2); // no jump down
        assert!(b.text().contains("X"));

        // eof placement (click "below")
        b.set_cursor_from_visual(99, 0); // >= count should go to end of last
        let (r, c) = b.cursor_position();
        assert_eq!(r, 2);
        assert!(c >= 1); // after the X or at end

        // page down from top with vis=5 should change scroll and caret
        b.scroll_offset = (0, 0);
        b.cursor = 0;
        let _before_scroll = b.scroll_offset.0;
        let _before_row = b.cursor_position().0;
        b.page_down(5, false);
        let (row_after, _) = b.cursor_position();
        let after_scroll = b.scroll_offset.0;
        assert!(
            row_after > 0 || after_scroll > 0,
            "page should advance view or caret"
        );

        // Stronger multi-line case: many lines, small vis, page should move scroll by ~page
        let many = (0..30)
            .map(|i| format!("L{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let mut b2 = Buffer::from_str(&many, None);
        assert_eq!(b2.line_count(), 30);
        b2.scroll_offset = (0, 0);
        b2.cursor = 0;
        b2.page_down(10, false);
        assert!(
            b2.scroll_offset.0 >= 5,
            "page should have scrolled the view by a substantial amount"
        );
    }

    #[test]
    fn undo_after_various_non_edit_nav_and_insert_on_blank() {
        let mut b = Buffer::new();
        b.insert("hello\n\nworld");
        let snap = b.text();
        b.set_cursor_from_visual(1, 0); // blank line
        b.insert_char('X'); // edit on blank
        assert!(b.can_undo());
        b.undo();
        assert_eq!(b.text(), snap);
        assert!(b.can_redo());
    }

    // --- adjust_scroll edge clamping (Fix 1 & 2) ---
    // Cursor movement *inside* the viewport must NOT move the view; scrolling
    // only happens at the true top/bottom edges, using the real viewport height.
    fn make_long_buffer(n: usize) -> Buffer {
        let text = (0..n)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        Buffer::from_str(&text, None)
    }

    #[test]
    fn adjust_scroll_only_at_edges() {
        let mut b = make_long_buffer(200);
        b.visible_lines = 20; // viewport rows 0..19
        b.scroll_offset = (40, 0); // view spans rows 40..59

        // Cursor in the middle of the viewport → no scroll.
        b.cursor = b.rope.line_to_byte(50); // row 50
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0, 40,
            "mid-window cursor must not scroll the view"
        );

        // Cursor on the last visible row (row 59 = 40+20-1) → still in view, no scroll.
        b.cursor = b.rope.line_to_byte(59);
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0, 40,
            "cursor on bottom edge row must not scroll"
        );

        // Cursor one row past the bottom edge → recenter so cursor sits near bottom.
        b.cursor = b.rope.line_to_byte(60);
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0,
            60usize.saturating_sub(18),
            "cursor past bottom should recenter to row - (v-2)"
        );

        // Cursor above the top edge → bring to top.
        b.cursor = b.rope.line_to_byte(5);
        b.adjust_scroll();
        assert_eq!(b.scroll_offset.0, 5, "cursor above top should snap to top");
    }

    #[test]
    fn insert_on_blank_line_when_scrolled_keeps_view_and_row() {
        // Reproduces Bug 2: typing on a blank line in a scrolled long file must
        // land on that line, without the viewport (and thus perceived cursor)
        // jumping. The buffer content was always correct; the bug was the view.
        let mut b = Buffer::from_str("a\n\nb\n\nc\n\nd\n\ne", None);
        b.visible_lines = 3; // tiny viewport to force scrolling quickly
        b.scroll_offset = (6, 0);
        // Place cursor like a mouse click on the blank line at row 7 (which is row 6+1 in view).
        b.set_cursor_from_visual(7, 0);
        assert_eq!(b.cursor_position(), (7, 0));
        let scroll_before = b.scroll_offset.0;
        b.insert_char('X');
        assert_eq!(
            b.cursor_position().0,
            7,
            "row must stay 7 after typing on blank line"
        );
        assert_eq!(
            b.scroll_offset.0, scroll_before,
            "view must not jump when typing inside the viewport"
        );
        assert!(b.text().contains("X"), "character was actually inserted");
    }

    // --- generation invalidation key (Fix 3c) ---
    #[test]
    fn generation_bumps_on_edit_not_on_nav() {
        let mut b = Buffer::from_str("hello", None);
        let g0 = b.generation;

        // Navigation must NOT bump generation (no spurious cache invalidation).
        b.move_cursor(1, false);
        b.move_home(false);
        b.move_end(false);
        assert_eq!(b.generation, g0, "navigation must not bump generation");

        // Every text mutation bumps exactly once.
        b.insert_char('!');
        assert_eq!(b.generation, g0 + 1, "insert bumps generation");
        b.backspace();
        assert_eq!(b.generation, g0 + 2, "backspace bumps generation");
        b.newline_with_indent();
        assert_eq!(b.generation, g0 + 3, "newline bumps generation");
        b.indent();
        assert_eq!(b.generation, g0 + 4, "indent bumps generation");
        b.dedent();
        assert_eq!(b.generation, g0 + 5, "dedent bumps generation");

        // delete bumps only when it actually removes something.
        b.cursor = 0;
        let before = b.generation;
        b.delete();
        assert_eq!(b.generation, before + 1, "delete bumps generation");

        // undo/redo are mutations of content → bump.
        let before = b.generation;
        b.undo();
        assert_eq!(b.generation, before + 1, "undo bumps generation");
        let before = b.generation;
        b.redo();
        assert_eq!(b.generation, before + 1, "redo bumps generation");
    }
}
