#![allow(dead_code)]

use ropey::Rope;

#[derive(Debug, Clone, Default)]
pub struct WrapLayout {
    /// `breaks[i]` = byte offsets (absolute, rope-relative) ending each wrapped
    /// screen row for logical line `start_line + i`, excluding the final row.
    /// Empty vec => the line fits on one screen row.
    pub breaks: Vec<Vec<usize>>,
    pub total_screen_rows: usize,
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
                // Resume on the current char; reset col & last_space.
                col = advance;
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
            }
        }
        byte_in_line += ch.len_utf8();
    }

    breaks
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
        assert!(l.total_screen_rows >= 5);
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
}
