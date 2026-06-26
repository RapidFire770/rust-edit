// Open / Save As popup. Sprint 1+.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

/// Popup width scales with the terminal: wide enough to be useful, never
/// overflowing. Kept in one place so the render and the mouse handler agree.
pub fn file_dialog_rect(area: Rect) -> Rect {
    // Up to ~80% of width, clamped to a sane min/max.
    let w = (area.width * 4 / 5).clamp(50, 90);
    let h = 10u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    is_save: bool,
    path_input: &str,
    message: &str,
) {
    let title = if is_save { "Save As" } else { "Open File" };
    let popup = file_dialog_rect(area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    // The path field shows a window of the input that keeps the editing cursor
    // (always at the end here) in view, so long paths no longer overflow off the
    // right edge. Inner text width = popup width minus 2 borders minus "> " prefix.
    let field_w = (popup.width.saturating_sub(4)) as usize;
    let path_chars: Vec<char> = path_input.chars().collect();
    let visible_path: String = if path_chars.len() <= field_w {
        path_input.to_string()
    } else {
        // Keep the right end (where the cursor is); prefix an ellipsis.
        let take = field_w.saturating_sub(1);
        let mut s: String = path_chars[path_chars.len() - take..].iter().collect();
        s.insert(0, '…');
        s
    };

    let accent = Style::default()
        .bg(Color::from(&theme.accent))
        .fg(Color::from(&theme.header_fg))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));

    // Clickable confirm/cancel buttons on the last inner row.
    let confirm_label = if is_save { "[ Save ]" } else { "[ Open ]" };
    let btn_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(confirm_label, accent),
        Span::raw("   "),
        Span::styled("[ Cancel ]", dim),
    ]);

    let lines = vec![
        Line::from("Path:"),
        Line::from(format!("> {}", visible_path)),
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(""),
        btn_line,
    ];

    let para = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(Color::from(&theme.editor_fg)));

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}
