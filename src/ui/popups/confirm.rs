// Unsaved changes, etc. Sprint 1+.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::button::ButtonGroup;

/// Compute the centered popup rect for the given message, mirroring `render_confirm`.
/// Exposed so the mouse handler can hit-test the buttons against the same geometry.
pub fn confirm_rect(area: Rect, message: &str) -> Rect {
    let w = (message.len() as u16 + 10).max(44);
    let h = 7u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

/// Returns the screen y-coordinate of the button row in the confirm popup.
/// The paragraph has 5 lines (indices 0–4), button is at index 4.
/// The Block border adds +1 to the y offset.
pub fn confirm_button_row(popup: Rect) -> u16 {
    popup.y + 1 + 4
}

/// The two action buttons, as (label, byte-x-start, byte-x-end) in popup-local
/// coordinates (0 = popup's left border). Used by render and the mouse handler.
pub fn confirm_button_rects() -> &'static [(&'static str, bool); 2] {
    &[
        ("[ Y ] Confirm", true), // true = confirming
        ("[ N ] Cancel", false), // false = cancelling
    ]
}

pub fn render_confirm(f: &mut Frame, area: Rect, theme: &Theme, message: &str, app: &crate::App) {
    let popup = confirm_rect(area, message);
    let block = Block::default()
        .title("Confirm")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let labels = ["[ Y ] Confirm", "[ N ] Cancel"];
    let is_primary = [true, false];

    let mut btn_group = ButtonGroup::new(labels, is_primary);

    if app.confirm_selected == 0 {
        btn_group.left();
    } else {
        btn_group.right();
    }

    if let Some(hover_idx) = app.confirm_button_hover {
        btn_group.set_hover(Some(hover_idx));
    }

    let btn_line = btn_group.render(theme);

    let lines = vec![
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(message),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(""),
        btn_line,
    ];

    let para = Paragraph::new(lines)
        .block(block)
        .style(ratatui::style::Style::default().fg(ratatui::style::Color::from(&theme.editor_fg)));

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}
