// External-change reload prompt. State lives on `App`; this module is paint
// and hit-test geometry only (AGENTS §6, one popup = one file).

use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::button::ButtonRow;
use crate::App;

pub fn button_labels() -> [&'static str; 3] {
    ["[ Reload ]", "[ Always ]", "[ Keep ]"]
}

/// Centered popup sized to the message and the three-button row.
pub fn reload_rect(area: Rect, message: &str) -> Rect {
    let labels = button_labels();
    let btn_w: usize = labels.iter().map(|s| s.chars().count()).sum::<usize>()
        + 2
        + 3 * labels.len().saturating_sub(1);
    let msg_w = message.chars().count() + 4;
    let w = ((msg_w.max(btn_w) as u16) + 4).max(50).min(area.width.max(1));
    let h = 7u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

/// Screen y of the button row (matches `render`).
pub fn button_row_y(popup: Rect) -> u16 {
    popup.y + 1 + 4
}

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, app: &App) {
    let popup = reload_rect(area, &app.reload_message);
    let block = Block::default()
        .title("Reload")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let labels = button_labels();
    let is_primary = [true, false, false];
    let mut row = ButtonRow::new(&labels, &is_primary);
    row.select(Some(app.reload_selected.min(labels.len() - 1)));
    row.set_hover(app.reload_button_hover);

    let lines = vec![
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(app.reload_message.clone()),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(""),
        row.render(theme),
    ];

    let para = Paragraph::new(lines).block(block).style(
        ratatui::style::Style::default().fg(ratatui::style::Color::from(&theme.editor_fg)),
    );

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_labels_are_reload_always_keep() {
        assert_eq!(button_labels(), ["[ Reload ]", "[ Always ]", "[ Keep ]"]);
    }
}
