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
use crate::buffer::Language;
use crate::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let buf = app.active_buffer();
    let theme = app.current_theme();

    if app.markdown_preview && buf.language == Language::Markdown {
        // Rendered preview using pulldown-cmark + theme
        let md_lines = crate::ui::markdown::render_preview(&buf.text(), theme);
        let mut lines: Vec<Line> = md_lines.into_iter().take(area.height as usize).collect();
        while lines.len() < area.height as usize {
            lines.push(Line::from(""));
        }
        let editor = Paragraph::new(lines)
            .style(theme.editor_bg_style())
            .block(Block::default().borders(Borders::NONE));
        f.render_widget(editor, area);
        return;
    }

    let start_line = buf.scroll_offset.0;
    let visible_lines = area.height.saturating_sub(0) as usize;

    // Use rope's line model (via buf.line) to ensure line indexing matches
    // cursor_position() and line_count(). This fixes off-by-one in gutter/status
    // for cases involving line breaks (e.g. first Enter on empty buffer).
    let mut highlighted: Vec<Vec<Span<'static>>> = Vec::new();
    let n_lines = buf.line_count();
    for ii in 0..visible_lines.max(1) {
        let lidx = start_line + ii;
        if lidx >= n_lines {
            break;
        }
        let ltext = buf.line(lidx);
        let mut h = app.highlighter.highlight(&ltext, buf.language);
        let hl = if h.is_empty() {
            vec![Span::raw(ltext)]
        } else {
            h.remove(0)
        };
        highlighted.push(hl);
    }

    let mut lines: Vec<Line> = Vec::new();

    let hscroll = buf.scroll_offset.1;
    for (i, hline) in highlighted.iter().enumerate() {
        let line_idx = start_line + i;
        let gutter = format!("{:>4}│ ", line_idx + 1);
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

    // Render scrollbar (vertical)
    let total_lines = buf.line_count();
    let scroll_pos = buf.scroll_offset.0;
    let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll_pos);
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
        let gutter = format!("{:>4}│ ", line_idx + 1);
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
