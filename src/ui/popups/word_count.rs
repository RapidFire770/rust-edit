// Word count popup.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, text: &str) {
    let block = Block::default()
        .title("Word Count")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let lines: Vec<Line> = text.lines().map(|l| Line::from(l.to_string())).collect();

    let para = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(Color::from(&theme.editor_fg)));

    let w = 40u16;
    let h = (text.lines().count() as u16 + 4).min(20);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}
