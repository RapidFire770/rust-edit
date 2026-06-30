// Settings form + theme switcher (live). Sprint 2.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, selected: usize, word_wrap: bool) {
    let themes = Theme::built_ins();
    let names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();

    let mut lines = vec![
        Line::from("Settings (live)"),
        Line::from(""),
        Line::from("Theme (j/k arrows Enter):"),
    ];

    for (i, name) in names.iter().enumerate() {
        let prefix = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            Style::default()
                .fg(Color::from(&theme.accent))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::from(&theme.editor_fg))
        };
        lines.push(Line::from(format!("{}{}", prefix, name)).style(style)); // simplified
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Tab size: 4 (config)"));
    lines.push(Line::from(format!(
        "Word wrap: {}",
        if word_wrap { "on" } else { "off" }
    )));
    lines.push(Line::from("Esc to close"));

    let block = Block::default()
        .title("Settings")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let para = Paragraph::new(lines).block(block);

    let w = 45u16;
    let h = 14u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}
