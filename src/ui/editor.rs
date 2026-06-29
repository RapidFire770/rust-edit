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
use crate::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let buf = app.active_buffer();
    let theme = app.current_theme();

    let start_line = buf.scroll_offset.0;
    let visible_lines = area.height.saturating_sub(0) as usize;

    // Use the cached spans computed once per edit/scroll/language change by
    // app.refresh_highlight_cache (AGENTS §6: "Do not re-highlight the entire
    // buffer on every frame"). The cache covers [cache_start_line, cache_start_line
    // + count); we index into it for the rows we actually render. If the cache is
    // somehow missing or stale (e.g. the preview path skipped refresh), fall back
    // to a single-window highlight so we never fail to render.
    let cache = app.cached_visible_spans();
    let mut highlighted: Vec<Vec<Span<'static>>> = Vec::new();
    let n_lines = buf.line_count();
    for ii in 0..visible_lines.max(1) {
        let lidx = start_line + ii;
        if lidx >= n_lines {
            break;
        }
        let hl = if let Some(c) = cache {
            if c.start_line <= lidx {
                let rel = lidx - c.start_line;
                if let Some(line_spans) = c.spans.get(rel) {
                    line_spans.clone()
                } else {
                    vec![Span::raw(buf.line(lidx))]
                }
            } else {
                vec![Span::raw(buf.line(lidx))]
            }
        } else {
            // Fallback (cache not populated): highlight just this window.
            let h = app.highlighter.highlight(&buf.line(lidx), buf.language);
            if h.is_empty() {
                vec![Span::raw(buf.line(lidx))]
            } else {
                h.into_iter().next().unwrap_or_default()
            }
        };
        highlighted.push(hl);
    }

    let mut lines: Vec<Line> = Vec::new();

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
        // If entire empty line (or start) is covered by selection, add a space to show the bg highlight
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

    while lines.len() < area.height as usize {
        lines.push(Line::from(""));
    }

    // Split for scrollbar on right (1 col)
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

    // Render scrollbar (vertical).
    //
    // Calibration: ratatui treats `position` as ranging `[0, content_length-1]`,
    // but the editor's real max scroll offset is `line_count - visible_lines`.
    // Passing `content_length = line_count` therefore leaves the thumb stranded
    // in the middle of the track when the last content line reaches the viewport
    // bottom. We instead derive `content_length` so that `content_length - 1`
    // equals our max scroll offset, making the thumb span the full track over
    // the genuine scrollable range, and set `viewport_content_length` explicitly
    // so the thumb size reflects the viewport/content ratio (rather than relying
    // on the widget-area-height fallback, which is fragile).
    let total_lines = buf.line_count();
    let vis = (scrollbar_area.height as usize).max(1);
    let scroll_pos = buf.scroll_offset.0;
    let content_length = total_lines.saturating_sub(vis).saturating_add(1);
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
