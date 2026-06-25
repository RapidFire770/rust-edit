// Open / Save As popup. Sprint 1+.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    is_save: bool,
    path_input: &str,
    message: &str,
) {
    let title = if is_save { "Save As" } else { "Open File" };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let lines = vec![
        Line::from("Path:"),
        Line::from(format!("> {}", path_input)),
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from("Type path, Enter to confirm, Esc cancel"),
        Line::from("(Simple: no dir listing yet)"),
    ];

    let para = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(Color::from(&theme.editor_fg)));

    let w = 50u16;
    let h = 10u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}
