// Ratatui Layout helpers. Sprint 1/2.
// Keeps layout concerns separate from rendering details.

#![allow(dead_code)]

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Returns the main three areas: header (1), editor (flex), status (1).
pub fn main_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(1),    // editor
            Constraint::Length(1), // status
        ])
        .split(area)
        .to_vec()
}

/// For find bar at bottom (when active).
pub fn editor_with_find_layout(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    (chunks[0], chunks[1])
}

/// Simple centered popup area.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_width = area.width * percent_x / 100;
    let popup_height = area.height * percent_y / 100;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    Rect::new(
        x,
        y,
        popup_width.min(area.width),
        popup_height.min(area.height),
    )
}

/// Split editor area for MD source | preview.
pub fn md_split_preview(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    (chunks[0], chunks[1])
}
