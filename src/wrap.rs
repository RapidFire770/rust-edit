#![allow(dead_code)]

use ropey::Rope;

#[derive(Debug, Clone, Default)]
pub struct WrapLayout {
    /// `breaks[i]` = byte offsets (absolute, rope-relative) ending each wrapped
    /// screen row for logical line `start_line + i`, excluding the final row.
    /// Empty vec => the line fits on one screen row.
    pub breaks: Vec<Vec<usize>>,
    pub total_screen_rows: usize,
    /// Tab width (in display columns) used when this layout was computed.
    /// `byte_of_screen_row` reads this so its column→byte walk matches the
    /// wrap algorithm's tab accounting exactly.
    pub tab_size: u8,
}

/// Logical line index whose wrapped row range contains `scroll_screen_row`.
/// If `scroll_screen_row` is past the last row, returns the last logical line
/// (same default as `render_wrapped`), not 0.
pub fn first_logical_line_for_screen_row(layout: &WrapLayout, scroll_screen_row: usize) -> usize {
    if layout.breaks.is_empty() {
        return 0;
    }
    let mut prefix = 0usize;
    for (i, line_breaks) in layout.breaks.iter().enumerate() {
        let rows = line_breaks.len() + 1;
        if prefix + rows > scroll_screen_row {
            return i;
        }
        prefix += rows;
    }
    layout.breaks.len() - 1
}

/// Given a screen-row scroll offset and a visible screen-row count, return the
/// `(logical_start_line, logical_count)` window that covers all visible screen
/// rows. Used to feed `refresh_highlight_cache` correct logical-line arguments
/// under word wrap (where `scroll_offset.0` is a screen row, not a logical
/// line — see final-review Critical-1).
///
/// Scroll past the wrapped total is clamped to the last full page
/// (`total.saturating_sub(visible_screen_rows)`), matching `clamp_scroll_wrapped`,
/// so cache fill and the renderer describe the same window.
pub fn visible_logical_window(
    rope: &Rope,
    scroll_screen_row: usize,
    visible_screen_rows: usize,
    width: usize,
    tab_size: u8,
) -> (usize, usize) {
    let n = rope.len_lines();
    let layout = compute_layout(rope, 0, n, width, tab_size);
    if layout.total_screen_rows == 0 {
        return (0, 1);
    }
    let vis = visible_screen_rows.max(1);
    // Last page, not last row: vis=5 and total=10 → scroll 5, lines [5, 10).
    let max_scroll = layout.total_screen_rows.saturating_sub(vis);
    let scroll_screen_row = scroll_screen_row.min(max_scroll);
    let start_line = first_logical_line_for_screen_row(&layout, scroll_screen_row);
    let mut prefix = 0usize;
    for line_breaks in layout.breaks.iter().take(start_line) {
        prefix += line_breaks.len() + 1;
    }
    // Extend the window until we've covered `vis` screen rows from the
    // (possibly clamped) scroll origin.
    let target_end_row = scroll_screen_row + vis;
    let mut acc = prefix;
    let mut end_line = start_line;
    for (i, line_breaks) in layout.breaks.iter().enumerate().skip(start_line) {
        let rows = line_breaks.len() + 1;
        if acc + rows >= target_end_row {
            end_line = i + 1;
            break;
        }
        acc += rows;
        end_line = i + 1;
    }
    let count = end_line
        .saturating_sub(start_line)
        .max(1)
        .min(n.saturating_sub(start_line).max(1));
    (start_line, count)
}

/// Total screen rows across the entire buffer at the given width.
pub fn total_screen_rows(rope: &Rope, width: usize, tab_size: u8) -> usize {
    let n = rope.len_lines();
    compute_layout(rope, 0, n, width, tab_size).total_screen_rows
}

/// Compute the wrap layout for `count` logical lines starting at `start_line`.
/// `width` is in display columns (excludes the gutter). Tabs advance to the next
/// multiple of `tab_size`; ASCII chars count as 1 column; all other chars count
/// as 1 column (CJK-width approximation deferred — see "Future" in spec §6).
pub fn compute_layout(
    rope: &Rope,
    start_line: usize,
    count: usize,
    width: usize,
    tab_size: u8,
) -> WrapLayout {
    let width = width.max(1);
    let tab = tab_size.max(1) as usize;
    let n_lines = rope.len_lines();
    let mut breaks: Vec<Vec<usize>> = Vec::with_capacity(count);
    let mut total_screen_rows = 0usize;

    for i in 0..count {
        let lidx = start_line + i;
        if lidx >= n_lines {
            break;
        }
        let line_start = rope.line_to_byte(lidx);
        let line_slice = rope.line(lidx);

        let row_breaks = wrap_one_line(&line_slice, line_start, width, tab);
        total_screen_rows += row_breaks.len() + 1;
        breaks.push(row_breaks);
    }

    WrapLayout {
        breaks,
        total_screen_rows,
        tab_size: tab_size.max(1),
    }
}

/// Wrap a single logical line. Returns the absolute byte offsets ending each
/// non-final wrapped row. The final row ends at the line terminator (not included).
fn wrap_one_line(
    line: &ropey::RopeSlice,
    line_start_byte: usize,
    width: usize,
    tab: usize,
) -> Vec<usize> {
    let mut breaks: Vec<usize> = Vec::new();
    let mut col = 0usize; // display column since the start of the current row
    let mut last_space_byte: Option<usize> = None; // byte of the most recent space in THIS row
    let mut col_after_last_space: usize = 0; // display col value after the most recent space's advance was added
    let mut byte_in_line = 0usize; // bytes consumed so far in this line

    for ch in line.chars() {
        if ch == '\n' || ch == '\r' {
            continue;
        }
        let advance = if ch == '\t' { tab - (col % tab) } else { 1 };
        let abs_byte = line_start_byte + byte_in_line;

        if col + advance > width {
            // Wrap before this char.
            if let Some(space_byte) = last_space_byte {
                // Word break: end row at the byte after the space (exclusive).
                breaks.push(space_byte + 1);
                // Resume on the current char; the new row's width is the width of
                // the chars from (space+1) to current_char inclusive.
                // col - col_after_last_space = width of (space, current_char) exclusive;
                // adding advance (current char) gives the new row's col.
                col = (col - col_after_last_space) + advance;
                last_space_byte = None;
            } else {
                // Char fallback: break right before this char.
                breaks.push(abs_byte);
                col = advance;
                last_space_byte = None;
            }
        } else {
            col += advance;
            if ch == ' ' || ch == '\t' {
                last_space_byte = Some(abs_byte);
                col_after_last_space = col;
            }
        }
        byte_in_line += ch.len_utf8();
    }

    breaks
}

/// Screen row (relative to the layout window's first row, 0-indexed) containing
/// `byte`. `byte` is rope-relative (absolute). Clamps to
/// `[0, total_screen_rows - 1]` when out of range.
pub fn screen_row_of_byte(
    layout: &WrapLayout,
    rope: &Rope,
    start_line: usize,
    byte: usize,
) -> usize {
    if layout.total_screen_rows == 0 {
        return 0;
    }
    let mut row = 0usize;
    for (i, line_breaks) in layout.breaks.iter().enumerate() {
        let lidx = start_line + i;
        if lidx >= rope.len_lines() {
            break;
        }
        let line_end = if lidx + 1 < rope.len_lines() {
            rope.line_to_byte(lidx + 1)
        } else {
            rope.len_bytes()
        };

        if byte < line_end {
            // Byte is within this logical line. Find its sub-row.
            let mut sub = 0usize;
            for &brk in line_breaks.iter() {
                if byte < brk {
                    break;
                }
                sub += 1;
            }
            return row + sub;
        }
        // Byte is past this line — advance by this line's row count.
        row += line_breaks.len() + 1;
    }
    // Byte is past the window end.
    layout.total_screen_rows.saturating_sub(1)
}

/// Byte offset of the cursor placed at (`screen_row`, `col`) within the layout's
/// window. `screen_row` is relative to window start; `col` is display columns
/// from the start of that sub-row (already gutter-adjusted; wrap has no hscroll).
/// Clamps `col` to the sub-row's actual width.
pub fn byte_of_screen_row(
    layout: &WrapLayout,
    rope: &Rope,
    start_line: usize,
    screen_row: usize,
    col: usize,
) -> usize {
    let mut row_remaining = screen_row.min(layout.total_screen_rows.saturating_sub(1));
    for (i, line_breaks) in layout.breaks.iter().enumerate() {
        let lidx = start_line + i;
        if lidx >= rope.len_lines() {
            return rope.len_bytes();
        }
        let rows_in_line = line_breaks.len() + 1;
        if row_remaining >= rows_in_line {
            row_remaining -= rows_in_line;
            continue;
        }
        let line_start = rope.line_to_byte(lidx);
        let line_end = if lidx + 1 < rope.len_lines() {
            rope.line_to_byte(lidx + 1)
        } else {
            rope.len_bytes()
        };
        // Per Task 1's convention: a break byte is the first byte of the NEW
        // sub-row, so the previous break IS the start of this sub-row.
        let sub_row_start_byte = if row_remaining == 0 {
            line_start
        } else {
            line_breaks[row_remaining - 1]
        };
        let sub_row_end_byte = if row_remaining < line_breaks.len() {
            line_breaks[row_remaining]
        } else {
            line_end
        };
        return walk_sub_row_to_col(
            rope,
            sub_row_start_byte,
            sub_row_end_byte,
            col,
            layout.tab_size as usize,
        );
    }
    rope.len_bytes()
}

/// Walk the sub-row `[start_byte, end_byte)`, advancing one display column per
/// non-newline char (mirrors `wrap_one_line`'s column accounting) until `col`
/// columns have been consumed. Returns the byte offset at that column, clamped
/// to the sub-row's end (i.e. the byte offset just past the last visible char).
///
/// `tab` must match the `tab_size` used to compute the layout: a tab advances
/// to the next multiple of `tab` columns, so a `col` that lands inside a tab
/// maps to the tab's start byte (the cursor does not split a tab).
fn walk_sub_row_to_col(
    rope: &Rope,
    start_byte: usize,
    end_byte: usize,
    col: usize,
    tab: usize,
) -> usize {
    if start_byte >= end_byte {
        return start_byte;
    }
    let slice = rope.byte_slice(start_byte..end_byte);
    let mut byte = start_byte;
    let mut cur_col = 0usize;
    for ch in slice.chars() {
        if cur_col >= col {
            break;
        }
        // Newlines/CR do not occupy a display column in a wrapped row (see
        // `wrap_one_line`); they terminate the visible content of the sub-row.
        if ch == '\n' || ch == '\r' {
            break;
        }
        // Mirror `wrap_one_line`'s tab accounting: a tab advances to the next
        // multiple of `tab` columns. If `col` falls inside the tab's span we
        // stop at the tab's start byte (the cursor cannot sit mid-tab).
        let advance = if ch == '\t' { tab - (cur_col % tab) } else { 1 };
        if cur_col + advance > col {
            break;
        }
        cur_col += advance;
        byte += ch.len_utf8();
    }
    byte
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn layout(text: &str, start: usize, count: usize, width: usize, tab: u8) -> WrapLayout {
        compute_layout(&Rope::from_str(text), start, count, width, tab)
    }

    #[test]
    fn short_line_is_single_row() {
        // "hello\n" -> line 0 = "hello" (5 cols). At width 80, no wrap.
        let l = layout("hello\n", 0, 1, 80, 4);
        let empty: Vec<Vec<usize>> = vec![vec![]];
        assert_eq!(l.breaks, empty);
        assert_eq!(l.total_screen_rows, 1);
    }

    #[test]
    fn exact_width_does_not_wrap() {
        // 5 chars at width 5: no wrap.
        let l = layout("hello\n", 0, 1, 5, 4);
        let empty: Vec<Vec<usize>> = vec![vec![]];
        assert_eq!(l.breaks, empty);
        assert_eq!(l.total_screen_rows, 1);
    }

    #[test]
    fn over_width_single_word_char_fallback() {
        // "abcdef" (no spaces), width 3. The line is only 6 chars long; the
        // algorithm fills row 0 with "abc" (col reaches 3 == width, OK), then
        // char 'd' would push col to 4 > 3 → char-fallback break at byte 3.
        // Row 1 = "def" (col reaches 3 == width, OK); no further chars. So 1
        // break at byte 3, 2 rows total. (Step 4 corrected the original
        // expectation of [3, 6]/3 rows: that assumed a break would fire again
        // at the line terminator, but col only exceeds width when another char
        // would be appended — at end-of-line there is no such char, so no
        // trailing break. The `over_width_single_word_two_breaks` test below
        // exercises a 9-char line that actually does produce two breaks.)
        let l = layout("abcdef\n", 0, 1, 3, 4);
        assert_eq!(l.breaks, vec![vec![3]]);
        assert_eq!(l.total_screen_rows, 2);
    }

    #[test]
    fn over_width_single_word_two_breaks() {
        // "abcdefghi" (9 chars) width 3 → breaks at byte 3, 6. Row 0 "abc", row 1 "def", row 2 "ghi".
        let l = layout("abcdefghi\n", 0, 1, 3, 4);
        assert_eq!(l.breaks, vec![vec![3, 6]]);
        assert_eq!(l.total_screen_rows, 3);
    }

    #[test]
    fn word_boundary_breaks_at_last_space() {
        // "alpha beta gamma\n" width 11. "alpha beta" = 10 chars, next space at byte 10,
        // then "gamma" wouldn't fit. Break at the space after "beta" (byte 10) so row 0
        // = "alpha beta" and row 1 = "gamma".
        // bytes: a(0)l(1)p(2)h(3)a(4) (5)b(6)e(7)t(8)a(9) (10)g(11)a(12)m(13)m(14)a(15)
        let l = layout("alpha beta gamma\n", 0, 1, 11, 4);
        assert_eq!(l.breaks, vec![vec![11]]);
        // row 0 = "alpha beta " trimmed to end at the space's position; row 1 = "gamma"
        assert_eq!(l.total_screen_rows, 2);
    }

    #[test]
    fn over_long_word_mid_line_uses_char_fallback() {
        // "ab verylongwordnotfitting cd\n" width 6.
        // row 0: "ab" (2 cols), then "verylo" doesn't fit after the space => break at
        //   the space (byte 3, after "ab "), row 1 starts at "verylongword..."
        // Actually: walk: 'a'(0)'b'(1)' '(2)'v'(3)'e'(4)'r'(5)'y'(6)...
        // At width 6: after "ab ver" (cols 0..6) we're at col 6, next 'y' would exceed.
        // last_space_byte is 2 (the space after "ab"). Break at byte 3 (after the space).
        // Row 1 = "verylongwordnotfitting..." — single word longer than width 6 →
        //   char fallback: break at byte 9 (after "verylo"), 15 ("wordno"), 21 ("tfitte"),
        //   25 ("ng"), then " cd" fits.
        // Just assert total rows and that the first break is at byte 3.
        let l = layout("ab verylongwordnotfitting cd\n", 0, 1, 6, 4);
        assert_eq!(
            l.breaks[0][0], 3,
            "first break must be at the space after 'ab '"
        );
        assert_eq!(l.total_screen_rows, 6);
        // Full breaks: word-break at byte 3 (after "ab "), then char-fallback
        // breaks inside the long word at bytes 9, 15, 21, then a final
        // word-break at byte 26 (after the space between the word and "cd").
        assert_eq!(l.breaks[0], vec![3, 9, 15, 21, 26]);
    }

    #[test]
    fn multiple_logical_lines_window() {
        // "aaaa\nbbbbbb\n" width 3, count 2 lines starting at 0.
        // line 0 "aaaa" (4 chars, width 3): a(1)a(2)a(3) col==3 OK; 4th 'a' would
        //   push col to 4 > 3 → char-fallback break at byte 3. Row 1 = "a". 2 rows.
        // line 1 "bbbbbb" (6 chars) starts at byte 5 (after "aaaa\n"): b(1)b(2)b(3)
        //   col==3 OK; 4th 'b' → break at byte 8. Remaining 3 'b's fill col to 3
        //   exactly — no 7th char exists to trigger another break, so only ONE
        //   break at byte 8 (the brief's original [8, 11]/3-row expectation was
        //   wrong: it assumed a trailing break fires at end-of-row, but the
        //   algorithm breaks only when a char WOULD exceed width). 2 rows.
        // Total: 2 + 2 = 4 screen rows.
        let l = layout("aaaa\nbbbbbb\n", 0, 2, 3, 4);
        assert_eq!(l.breaks.len(), 2);
        assert_eq!(l.breaks[0], vec![3]);
        assert_eq!(l.breaks[1], vec![8]);
        assert_eq!(l.total_screen_rows, 4);
    }

    #[test]
    fn empty_line_is_single_row() {
        let l = layout("\n\n", 0, 2, 80, 4);
        let empty: Vec<Vec<usize>> = vec![vec![], vec![]];
        assert_eq!(l.breaks, empty);
        assert_eq!(l.total_screen_rows, 2);
    }

    #[test]
    fn window_start_offset() {
        // "aaaa\nbbbb\ncccc\ndddd\n" — start at line 2 ("cccc"), width 3.
        // Line 2 starts at byte 10 ("aaaa\n"(5) "bbbb\n"(5)). "cccc" width 3 → break at byte 13.
        let l = layout("aaaa\nbbbb\ncccc\ndddd\n", 2, 1, 3, 4);
        assert_eq!(l.breaks, vec![vec![13]]);
    }

    #[test]
    fn tab_expands_to_next_tab_stop() {
        // "\thello\n" width 8 with tab_size 4. Tab expands to col 4, then "hello" (5) = col 9 > 8.
        // Break: last_space_byte is None (tab is whitespace but at col 0). Char fallback at
        // the char that would exceed: after tab (col 4), 'h'(5)'e'(6)'l'(7)'l'(8) — col 8 is OK
        // (exact), 'o' would make col 9 > 8 → break at byte 5 (after "    " + "hell").
        // Actually we count col after consuming char. See step 3 algorithm.
        let l = layout("\thello\n", 0, 1, 8, 4);
        assert_eq!(l.total_screen_rows, 2);
        assert_eq!(l.breaks[0].len(), 1);
    }

    #[test]
    fn wrapped_rows_never_exceed_width() {
        let cases = [
            ("ab verylongwordnotfitting cd\n", 6),
            ("alpha beta gamma delta epsilon zeta\n", 11),
            ("short\n", 80),
            ("abcdefghijklmnopqrstuvwxyz\n", 5),
            ("one two three four five six seven eight nine ten\n", 10),
        ];
        for (text, width) in cases {
            let rope = Rope::from_str(text);
            let l = compute_layout(&rope, 0, rope.len_lines(), width, 4);
            for (i, line_breaks) in l.breaks.iter().enumerate() {
                let line_start = rope.line_to_byte(i);
                let line_end = if i + 1 < rope.len_lines() {
                    rope.line_to_byte(i + 1)
                } else {
                    rope.len_bytes()
                };
                let mut boundaries = line_breaks.clone();
                boundaries.push(line_end);
                let mut prev = line_start;
                for &brk in &boundaries {
                    let slice = rope.byte_slice(prev..brk);
                    let mut cols = 0usize;
                    for ch in slice.chars() {
                        if ch == '\n' || ch == '\r' {
                            continue;
                        }
                        cols += if ch == '\t' { 4 - (cols % 4) } else { 1 };
                    }
                    assert!(
                        cols <= width,
                        "row in line {i} (bytes {prev}..{brk}) is {cols} cols wide, exceeds width {width}; breaks={line_breaks:?}"
                    );
                    prev = brk;
                }
            }
        }
    }

    #[test]
    fn screen_row_of_byte_basic() {
        // "abcdef\n" width 3 → breaks [3]. byte 0,1,2 → row 0; byte 3,4,5 → row 1.
        let rope = Rope::from_str("abcdef\n");
        let l = compute_layout(&rope, 0, 1, 3, 4);
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 0), 0);
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 2), 0);
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 3), 1);
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 5), 1);
    }

    #[test]
    fn screen_row_of_byte_multi_line_window() {
        // "aaaa\nbbbbbb\n" width 3, count 2.
        // Per Task 1's corrected `multiple_logical_lines_window` test, each
        // logical line yields exactly ONE break (the trailing-row break does
        // NOT fire because no char tries to overflow after the last sub-row is
        // filled). So line 0 = [3] (2 rows), line 1 = [8] (2 rows); total 4.
        // The brief's original "line 1 = [8, 11]/3 rows" assumption was stale.
        let rope = Rope::from_str("aaaa\nbbbbbb\n");
        let l = compute_layout(&rope, 0, 2, 3, 4);
        // line 0 spans bytes 0..5; breaks[0] = [3]. byte 0..3 → row 0; byte 3..5 (incl.
        // '\n' at 4) → row 1. line 1 spans bytes 5..12; breaks[1] = [8]. Within line 1:
        // bytes 5..8 → row 2, bytes 8..12 (incl. '\n' at 11) → row 3.
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 4), 1); // '\n' of line 0, still row 1
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 5), 2);
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 7), 2);
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 8), 3);
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 10), 3);
        // Past the window: clamp to last screen row.
        assert_eq!(screen_row_of_byte(&l, &rope, 0, 999), 3);
    }

    #[test]
    fn byte_of_screen_row_inside_tab_returns_tab_start() {
        // Regression: walk_sub_row_to_col used to count a tab as 1 column,
        // mismatching wrap_one_line (which counts `tab - col % tab` columns).
        // On "\tab cd\n" width 12 tab 4 the tab occupies cols 0..4 of row 0;
        // clicking inside that span must keep the cursor at the tab's start
        // byte rather than advancing into the following characters.
        let rope = Rope::from_str("\tab cd\n");
        let l = compute_layout(&rope, 0, 1, 12, 4);
        // Sanity: the line fits on one screen row.
        assert_eq!(l.total_screen_rows, 1);
        assert!(l.breaks[0].is_empty());
        // Click col 2 (inside the tab) → byte 0 (tab start).
        assert_eq!(byte_of_screen_row(&l, &rope, 0, 0, 2), 0);
        // Click col 3 (still inside the tab) → byte 0.
        assert_eq!(byte_of_screen_row(&l, &rope, 0, 0, 3), 0);
        // Click col 4 (exactly at tab end = start of 'a') → byte 1.
        assert_eq!(byte_of_screen_row(&l, &rope, 0, 0, 4), 1);
        // Click col 5 → byte 2 ('b').
        assert_eq!(byte_of_screen_row(&l, &rope, 0, 0, 5), 2);
    }

    #[test]
    fn byte_of_screen_row_round_trip() {
        // "alpha beta gamma\n" width 11. The space after "beta" is byte 10;
        // Task 1 word-break pushes the break byte to 10 + 1 = 11, so row 0 =
        // "alpha beta " (bytes 0..11) and row 1 = "gamma" (bytes 11..16).
        let rope = Rope::from_str("alpha beta gamma\n");
        let l = compute_layout(&rope, 0, 1, 11, 4);
        // Click row 0, col 6 → byte 6 ('e' in "beta").
        assert_eq!(byte_of_screen_row(&l, &rope, 0, 0, 6), 6);
        // Click row 1, col 0 → byte 11 ('g').
        assert_eq!(byte_of_screen_row(&l, &rope, 0, 1, 0), 11);
        // Click row 1, col 4 → byte 15 ('a', last char of "gamma").
        assert_eq!(byte_of_screen_row(&l, &rope, 0, 1, 4), 15);
        // Click past end of row 1 → clamp to line end (byte 16, the position of
        // '\n' — cursor sits before the newline).
        assert_eq!(byte_of_screen_row(&l, &rope, 0, 1, 99), 16);
    }

    /// Regression for Critical-1: under wrap, `ui()` must translate the
    /// screen-row `scroll_offset.0` back to a logical-line window before
    /// refreshing the syntax highlight cache. `visible_logical_window` is the
    /// helper that performs this translation.
    #[test]
    fn visible_logical_window_no_wrap_passthrough() {
        // Three short lines at width 80 — none wrap. Screen rows equal logical
        // lines. A scroll of 1 with 2 visible rows should map to lines [1..3).
        let rope = Rope::from_str("aaa\nbbb\nccc\n");
        let (start, count) = visible_logical_window(&rope, 1, 2, 80, 4);
        assert_eq!(start, 1);
        assert_eq!(count, 2);
    }

    #[test]
    fn visible_logical_window_wrapped_mid_line() {
        // 3 logical lines: line 0 short, line 1 long enough to wrap to 5 screen
        // rows at width 3, line 2 short.
        //
        // "abcdefghijkmnop" (15 chars) at width 3 → 5 screen rows (3 chars each),
        // i.e. 4 breaks. Screen-row layout:
        //   line 0 "x"            -> 1 row  (row 0)
        //   line 1 (5 rows)       -> rows 1..6
        //   line 2 "y"            -> row 6
        // scroll_screen_row = 3, visible = 2 → covers rows 3..5, both inside
        // line 1. So the window must start at line 1 and include at least line 1.
        let rope = Rope::from_str("x\nabcdefghijkmnop\ny\n");
        // Sanity-check the wrap math: 15 chars / 3 = 5 rows => 4 breaks.
        let layout = compute_layout(&rope, 0, rope.len_lines(), 3, 4);
        assert_eq!(
            layout.breaks[1].len(),
            4,
            "line 1 must wrap to 5 rows (4 breaks) at width 3"
        );
        let (start, count) = visible_logical_window(&rope, 3, 2, 3, 4);
        assert_eq!(
            start, 1,
            "window must start at the logical line containing screen row 3"
        );
        assert!(count >= 1, "window must include at least one line");
    }

    #[test]
    fn visible_logical_window_spans_into_next_line() {
        // "aaaa\nbb\n" width 3. Line 0 "aaaa" wraps to 2 rows; line 1 "bb" = 1 row.
        // scroll_screen_row = 1 (2nd row of line 0), visible = 2 → covers rows 1
        // and 2 → lines 0 and 1.
        let rope = Rope::from_str("aaaa\nbb\n");
        let (start, count) = visible_logical_window(&rope, 1, 2, 3, 4);
        assert_eq!(start, 0);
        assert_eq!(count, 2);
    }

    #[test]
    fn visible_logical_window_clamps_scroll_past_end() {
        // 10 short lines, width 80 → 1 screen row each. Scroll far past the
        // last row must still describe the last visible window, not line 0.
        // No trailing newline so ropey reports exactly 10 lines.
        let rope = Rope::from_str("a\nb\nc\nd\ne\nf\ng\nh\ni\nj");
        assert_eq!(rope.len_lines(), 10);
        let (start, count) = visible_logical_window(&rope, 999, 5, 80, 4);
        assert_eq!(start, 5, "past-end scroll must start at the last 5 lines");
        assert_eq!(count, 5);
    }

    #[test]
    fn visible_logical_window_matches_render_start_when_narrowing() {
        // 80-char lines: width 80 → 1 row/line; width 20 → 4 rows/line.
        // Same screen-row scroll must not jump to a later logical start
        // than the renderer would paint after a narrower wrap.
        let text = (0..80)
            .map(|_| "x".repeat(80))
            .collect::<Vec<_>>()
            .join("\n");
        let rope = Rope::from_str(&text);
        let scroll = 100usize;
        let vis = 20usize;
        let (wide_start, _) = visible_logical_window(&rope, scroll, vis, 80, 4);
        let (narrow_start, narrow_count) = visible_logical_window(&rope, scroll, vis, 20, 4);
        assert!(
            narrow_start <= wide_start,
            "narrower wrap must not start later than the wide layout (narrow={narrow_start}, wide={wide_start})"
        );
        assert!(narrow_count >= 1, "window must include at least one line");
        let narrow_layout = compute_layout(&rope, 0, rope.len_lines(), 20, 4);
        let max_scroll = narrow_layout.total_screen_rows.saturating_sub(vis);
        let clamped = scroll.min(max_scroll);
        assert_eq!(
            narrow_start,
            first_logical_line_for_screen_row(&narrow_layout, clamped),
            "visible_logical_window start must match render_wrapped's first painted line"
        );
    }
}
