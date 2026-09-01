// Main editing view, line numbers, cursor, selection, syntax spans. Sprint 1/2.
// Presentation only. Receives state from App/Buffer + highlighter.

#![allow(dead_code)]

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

use crate::app::App;
use crate::buffer::Buffer;
use crate::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let buf = app.active_buffer();
    let theme = app.current_theme();
    let visible_lines = area.height as usize;
    let cache = app.cached_visible_spans();

    let mut lines: Vec<Line<'static>> = if app.config.word_wrap {
        render_wrapped(buf, app, visible_lines, cache)
    } else {
        render_unwrapped(buf, app, visible_lines, cache)
    };

    while lines.len() < area.height as usize {
        lines.push(Line::from(""));
    }

    // Split for scrollbar on right (1 col).
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    let content_area = inner_chunks[0];
    let scrollbar_area = inner_chunks[1];

    let editor = Paragraph::new(lines)
        .style(theme.editor_bg_style())
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(editor, content_area);

    // Scrollbar calibration (see existing comment). When wrap is on, the total
    // is screen rows (not logical lines) and scroll_offset.0 is a screen row.
    let vis = (scrollbar_area.height as usize).max(1);
    let (total_rows, scroll_pos) = if app.config.word_wrap {
        let total =
            crate::wrap::total_screen_rows(&buf.rope, buf.visible_width, app.config.tab_size);
        (total, buf.scroll_offset.0.min(total.saturating_sub(1)))
    } else {
        (buf.line_count(), buf.scroll_offset.0)
    };
    let content_length = total_rows.saturating_sub(vis).saturating_add(1);
    let mut scrollbar_state = ScrollbarState::new(content_length)
        .position(scroll_pos)
        .viewport_content_length(vis);
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"))
        .track_symbol(Some("│"))
        .thumb_symbol("█");
    f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
}

/// Original 1-line-per-row render path. Unchanged from the pre-wrap version.
fn render_unwrapped(
    buf: &Buffer,
    app: &App,
    visible_lines: usize,
    cache: &crate::highlight_cache::HighlightCache,
) -> Vec<Line<'static>> {
    let theme = app.current_theme();
    let start_line = buf.scroll_offset.0;
    let n_lines = buf.line_count();

    // Use the cached spans computed by app.refresh_highlight_cache (AGENTS §6).
    // Span::raw fallback is temporary until Task 5 batches uncached lines.
    let mut highlighted: Vec<Vec<Span<'static>>> = Vec::new();
    for ii in 0..visible_lines.max(1) {
        let lidx = start_line + ii;
        if lidx >= n_lines {
            break;
        }
        let hl = cache
            .line_spans(lidx)
            .map(|s| s.to_vec())
            .unwrap_or_else(|| vec![Span::raw(buf.line(lidx))]);
        highlighted.push(hl);
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    let hscroll = buf.scroll_offset.1;
    for (i, hline) in highlighted.iter().enumerate() {
        let line_idx = start_line + i;
        // Gutter MUST be pure ASCII: `│` (U+2502) is East-Asian-Width Ambiguous
        // and renders as 2 cells in some terminals/fonts, which shifts content
        // right by 1 vs GUTTER_WIDTH and desyncs the hardware cursor. AGENTS §1.
        let gutter = format!("{:>4}| ", line_idx + 1);
        let mut spans = vec![Span::styled(gutter, theme.line_number_style())];

        let line_start_byte = buf.rope.line_to_byte(line_idx);
        let mut byte_in_line = 0usize;
        let mut char_in_line = 0usize;

        for sp in hline {
            let txt = sp.content.clone();
            for ch in txt.chars() {
                let ch_len = ch.len_utf8();
                let abs_byte = line_start_byte + byte_in_line;
                let is_selected = if let Some(sel) = &buf.selection {
                    abs_byte >= sel.start && abs_byte < sel.end
                } else {
                    false
                };
                let mut ch_style = sp.style;
                if is_selected {
                    ch_style = ch_style.patch(theme.selection_style());
                }
                if char_in_line >= hscroll {
                    spans.push(Span::styled(ch.to_string(), ch_style));
                }
                byte_in_line += ch_len;
                char_in_line += 1;
            }
        }
        // If entire empty line (or start) is covered by selection, add a space
        // to show the bg highlight.
        if hline.is_empty() {
            if let Some(sel) = &buf.selection {
                let line_start = buf.rope.line_to_byte(line_idx);
                if sel.start <= line_start && sel.end > line_start && char_in_line >= hscroll {
                    spans.push(Span::styled(" ", theme.selection_style()));
                }
            }
        }
        lines.push(Line::from(spans));
    }
    lines
}

/// Wrapped render path. Emits one `Line` per screen row. The first sub-row of
/// each logical line carries the gutter; continuation rows carry 6 spaces.
/// Hscroll is ignored (wrap replaces horizontal scrolling).
fn render_wrapped(
    buf: &Buffer,
    app: &App,
    visible_lines: usize,
    cache: &crate::highlight_cache::HighlightCache,
) -> Vec<Line<'static>> {
    let theme = app.current_theme();
    let n_lines = buf.line_count();
    let width = buf.visible_width.max(1);
    let tab = app.config.tab_size;
    let scroll_screen_row = buf.scroll_offset.0; // under wrap, this is a SCREEN ROW

    // NOTE: full-buffer layout = O(buffer bytes) per frame. Acceptable for normal
    // editing; a 50k+ line file with many long lines may stutter. Future: window
    // the compute to the visible logical-line range (spec §9 Approach C).
    let full_layout = crate::wrap::compute_layout(&buf.rope, 0, n_lines, width, tab);
    if full_layout.total_screen_rows == 0 {
        return Vec::new();
    }

    // Prefix sum: prefix[lidx] = first global screen row of logical line `lidx`.
    let mut prefix: Vec<usize> = Vec::with_capacity(full_layout.breaks.len() + 1);
    prefix.push(0);
    let mut prev = 0usize;
    for line_breaks in &full_layout.breaks {
        prev += line_breaks.len() + 1;
        prefix.push(prev);
    }

    // Find the first logical line whose screen-row range contains scroll_screen_row.
    let mut start_line = full_layout.breaks.len().saturating_sub(1);
    for (i, line_breaks) in full_layout.breaks.iter().enumerate() {
        let rows_in_line = line_breaks.len() + 1;
        if prefix[i] + rows_in_line > scroll_screen_row {
            start_line = i;
            break;
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(visible_lines);

    'outer: for (lidx, line_breaks) in full_layout.breaks.iter().enumerate().skip(start_line) {
        let rows_in_line = line_breaks.len() + 1;
        let line_start_byte = buf.rope.line_to_byte(lidx);
        let line_end_byte = if lidx + 1 < n_lines {
            buf.rope.line_to_byte(lidx + 1)
        } else {
            buf.rope.len_bytes()
        };

        // Fetch highlighted spans for this line (cache or Span::raw fallback).
        let hline: Vec<Span<'static>> = cache
            .line_spans(lidx)
            .map(|s| s.to_vec())
            .unwrap_or_else(|| vec![Span::raw(buf.line(lidx))]);

        // Sub-row byte boundaries. sub_starts[0] = line_start_byte; sub_starts[k]
        // = breaks[k-1]. sub_ends[k] = breaks[k] (or line_end_byte for the last).
        let mut sub_starts: Vec<usize> = Vec::with_capacity(rows_in_line);
        sub_starts.push(line_start_byte);
        for &brk in line_breaks.iter() {
            sub_starts.push(brk);
        }
        let mut sub_ends: Vec<usize> = line_breaks.clone();
        sub_ends.push(line_end_byte);

        let line_first_global_row = prefix[lidx];
        for sub in 0..rows_in_line {
            let global_row = line_first_global_row + sub;
            if global_row < scroll_screen_row {
                continue;
            }
            if lines.len() >= visible_lines {
                break 'outer;
            }
            let sub_start_byte = sub_starts[sub];
            let sub_end_byte = sub_ends[sub];
            let is_first_sub = sub == 0;
            let gutter = if is_first_sub {
                Span::styled(format!("{:>4}| ", lidx + 1), theme.line_number_style())
            } else {
                Span::raw("      ") // 6 spaces, aligned with the gutter
            };
            let mut spans = vec![gutter];
            // Walk hline, slicing chars whose absolute byte is in this sub-row.
            let mut byte_in_line = 0usize;
            for sp in &hline {
                let txt = &sp.content;
                for ch in txt.chars() {
                    let abs_byte = line_start_byte + byte_in_line;
                    if abs_byte >= sub_start_byte && abs_byte < sub_end_byte {
                        let is_selected = if let Some(sel) = &buf.selection {
                            abs_byte >= sel.start && abs_byte < sel.end
                        } else {
                            false
                        };
                        let mut ch_style = sp.style;
                        if is_selected {
                            ch_style = ch_style.patch(theme.selection_style());
                        }
                        spans.push(Span::styled(ch.to_string(), ch_style));
                    }
                    byte_in_line += ch.len_utf8();
                }
            }
            // If an empty line is covered by selection, push a styled space so
            // the selection background is visible. Mirrors `render_unwrapped`,
            // minus the hscroll check (wrap has no hscroll). Only the first
            // sub-row needs it (an empty line has a single sub-row).
            if hline.is_empty() && sub == 0 {
                if let Some(sel) = &buf.selection {
                    if sel.start <= line_start_byte && sel.end > line_start_byte {
                        spans.push(Span::styled(" ", theme.selection_style()));
                    }
                }
            }
            lines.push(Line::from(spans));
        }
    }

    lines
}

/// For future: accept precomputed highlights or scroll info for better decoupling.
pub fn render_with_highlights(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    highlighted: &[Vec<Span<'static>>],
    cursor_line: usize,
    cursor_col: usize,
    start_line: usize,
) {
    // Simplified version if needed later.
    let mut lines: Vec<Line> = Vec::new();
    for (i, hline) in highlighted.iter().enumerate() {
        let line_idx = start_line + i;
        // ASCII gutter — see note above (U+2502 width ambiguity shifts cursor).
        let gutter = format!("{:>4}| ", line_idx + 1);
        let mut spans = vec![Span::styled(gutter, theme.line_number_style())];
        let is_cursor = line_idx == cursor_line;
        let mut char_count = 0usize;
        for sp in hline {
            let txt = sp.content.clone();
            if is_cursor {
                let mut chars: Vec<char> = txt.chars().collect();
                for (j, ch) in chars.drain(..).enumerate() {
                    if char_count + j == cursor_col {
                        spans.push(Span::styled(
                            ch.to_string(),
                            sp.style.patch(theme.selection_style()),
                        ));
                    } else {
                        spans.push(Span::styled(ch.to_string(), sp.style));
                    }
                }
                char_count += txt.chars().count();
            } else {
                spans.push(sp.clone());
            }
        }
        lines.push(Line::from(spans));
    }
    while lines.len() < area.height as usize {
        lines.push(Line::from(""));
    }
    let editor = Paragraph::new(lines)
        .style(theme.editor_bg_style())
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(editor, area);
}
