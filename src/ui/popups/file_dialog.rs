// Open / Save As popup. Sprint 1+.
//
// The dialog is a typed-path field with three footer buttons:
//   index 0 — Save / Open   (primary action)
//   index 1 — Browse…       (opens the file-system browser popup)
//   index 2 — Cancel
//
// Activating Browse (1) does NOT close this dialog — `App::close_file_dialog`
// instead opens the browser on top of it, and confirming in the browser
// populates `file_path_input` here.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::button::ButtonRow;

/// Footer button labels in display order. Used by both the renderer and the
/// mouse hit-test so they always agree.
pub fn button_labels(is_save: bool) -> [&'static str; 3] {
    let action = if is_save { "[ Save ]" } else { "[ Open ]" };
    [action, "[ Browse… ]", "[ Cancel ]"]
}

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

/// Returns the screen y-coordinate of the button row in the file dialog popup.
/// The paragraph has 7 lines (indices 0–6), button is at index 6.
/// The Block border adds +1 to the y offset.
pub fn file_dialog_button_row(popup: Rect) -> u16 {
    popup.y + 1 + 6
}

pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    is_save: bool,
    path_input: &str,
    message: &str,
    app: &crate::App,
) {
    let title = if is_save { "Save As" } else { "Open File" };
    let popup = file_dialog_rect(area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let field_w = (popup.width.saturating_sub(4)) as usize;
    let path_chars: Vec<char> = path_input.chars().collect();
    let visible_path: String = if path_chars.len() <= field_w {
        path_input.to_string()
    } else {
        let take = field_w.saturating_sub(1);
        let mut s: String = path_chars[path_chars.len() - take..].iter().collect();
        s.insert(0, '…');
        s
    };

    let labels = button_labels(is_save);
    let is_primary = [true, false, false];

    let mut btn_row = ButtonRow::new(&labels, &is_primary);

    // `file_dialog_selected` is 0..=2; map directly onto the row.
    btn_row.select(Some(app.file_dialog_selected.min(2)));
    if let Some(hover_idx) = app.file_dialog_button_hover {
        btn_row.set_hover(Some(hover_idx));
    }

    let btn_line = btn_row.render(theme);

    let lines = vec![
        ratatui::text::Line::from("Path:"),
        ratatui::text::Line::from(format!("> {}", visible_path)),
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
