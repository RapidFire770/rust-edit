// Unsaved changes, etc. Sprint 1+.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

pub fn render_confirm(f: &mut Frame, area: Rect, theme: &Theme, message: &str) {
    let block = Block::default()
        .title("Confirm")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let lines = vec![
        Line::from(message),
        Line::from(""),
        Line::from("Press Y to confirm, N or Esc to cancel"),
    ];

    let para = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(Color::from(&theme.editor_fg)));

    // Reuse centered logic, approximate
    let w = (message.len() as u16 + 10).max(40);
    let h = 6u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}
