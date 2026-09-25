// Titled info popup (Word Count, format errors, …).

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, title: &str, text: &str) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let lines: Vec<Line> = text.lines().map(|l| Line::from(l.to_string())).collect();

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::from(&theme.editor_fg)));

    let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let w = (longest.saturating_add(4).clamp(40, 72) as u16).min(area.width);
    let inner_w = (w.saturating_sub(2) as usize).max(1);
    let visual_rows: usize = text
        .lines()
        .map(|line| {
            let n = line.chars().count();
            if n == 0 {
                1
            } else {
                n.div_ceil(inner_w)
            }
        })
        .sum();
    let h = (visual_rows as u16 + 4).min(20);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}
