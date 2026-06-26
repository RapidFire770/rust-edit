// Unsaved changes, etc. Sprint 1+.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

/// Compute the centered popup rect for the given message, mirroring `render_confirm`.
/// Exposed so the mouse handler can hit-test the buttons against the same geometry.
pub fn confirm_rect(area: Rect, message: &str) -> Rect {
    let w = (message.len() as u16 + 10).max(44);
    let h = 7u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

/// The two action buttons, as (label, byte-x-start, byte-x-end) in popup-local
/// coordinates (0 = popup's left border). Used by render and the mouse handler.
pub fn confirm_button_rects() -> &'static [(&'static str, bool); 2] {
    &[
        ("[ Y ] Confirm", true), // true = confirming
        ("[ N ] Cancel", false), // false = cancelling
    ]
}

pub fn render_confirm(f: &mut Frame, area: Rect, theme: &Theme, message: &str) {
    let popup = confirm_rect(area, message);
    let block = Block::default()
        .title("Confirm")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let accent = Style::default()
        .bg(Color::from(&theme.accent))
        .fg(Color::from(&theme.header_fg))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));

    // Center the two clickable buttons on the last inner row.
    let btns = confirm_button_rects();
    let btn_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(btns[0].0, accent),
        Span::raw("   "),
        Span::styled(btns[1].0, dim),
    ]);

    let lines = vec![
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
