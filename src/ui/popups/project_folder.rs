// Project-folder picker. Recent rows and Browse share the file browser's
// geometry helpers so the hit targets match the paint.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::button::ButtonRow;
use crate::ui::popups::file_browser::{
    self, format_recent_label, recent_parent_display, recent_row_click_is_clear, BrowserEntry,
};

/// List focus. Larger than any footer button index.
pub const FOCUS_LIST: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerMode {
    Recent,
    Browse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerState {
    pub mode: PickerMode,
    pub cwd: std::path::PathBuf,
    pub entries: Vec<BrowserEntry>,
    pub selected: usize,
    pub show_hidden: bool,
    pub button: usize,
    pub error: Option<String>,
    pub row_hover: Option<usize>,
}

impl PickerState {
    pub fn recent(cwd: std::path::PathBuf) -> Self {
        Self {
            mode: PickerMode::Recent,
            cwd,
            entries: Vec::new(),
            selected: 0,
            show_hidden: false,
            button: FOCUS_LIST,
            error: None,
            row_hover: None,
        }
    }
}

pub fn footer_labels(mode: PickerMode) -> &'static [&'static str] {
    match mode {
        PickerMode::Recent => &["[ Browse ]", "[ Clear ]", "[ Cancel ]"],
        PickerMode::Browse => &[
            "[ Up ]",
            "[ Use folder ]",
            "[ Hidden ]",
            "[ Recent ]",
            "[ Cancel ]",
        ],
    }
}

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, state: &PickerState) {
    let popup = file_browser::file_browser_rect(area);
    let title = match state.mode {
        PickerMode::Recent => " Open Project Folder ".to_string(),
        PickerMode::Browse => format!(" Project — {} ", state.cwd.display()),
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());
    f.render_widget(Clear, popup);
    f.render_widget(block, popup);

    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let dir_style = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let selected = Style::default()
        .fg(Color::from(&theme.editor_bg))
        .bg(Color::from(&theme.accent));
    let error_style = Style::default()
        .fg(Color::from(&theme.editor_bg))
        .bg(Color::from(&theme.accent));

    let (list_start, list_end) = file_browser::list_area(popup, false);
    let body_h = list_end.saturating_sub(list_start).max(1);
    let inner_w = popup.width.saturating_sub(2) as usize;
    let overflowing = state.entries.len() > body_h;
    let clear_reserve = if state.mode == PickerMode::Recent {
        file_browser::RECENT_CLEAR_COLS + 1
    } else {
        0
    };
    let name_budget = inner_w.saturating_sub(clear_reserve + usize::from(overflowing));

    let mut lines: Vec<Line> = Vec::new();
    let hint = match state.mode {
        PickerMode::Recent => "Enter: open   Del/Bksp/[x]: remove   Esc: cancel",
        PickerMode::Browse => "Enter: open dir   Ctrl+H: hidden   Esc: cancel",
    };
    lines.push(Line::from(Span::styled(hint, dim)));
    lines.push(Line::from(""));

    if let Some(err) = &state.error {
        lines.push(Line::from(Span::styled(
            file_browser::format_recent_label(err, "", name_budget.max(1)),
            error_style,
        )));
    } else if state.entries.is_empty() {
        let msg = match state.mode {
            PickerMode::Recent => "(no recent project folders)",
            PickerMode::Browse => "(empty directory)",
        };
        lines.push(Line::from(Span::styled(msg, dim)));
    } else {
        let scroll = file_browser::compute_scroll(state.selected, body_h)
            .min(state.entries.len().saturating_sub(1));
        let show_to = (scroll + body_h).min(state.entries.len());
        for (offset, entry) in state.entries[scroll..show_to].iter().enumerate() {
            let index = scroll + offset;
            let text = match state.mode {
                PickerMode::Recent => {
                    let parent = recent_parent_display(&entry.path);
                    let mut label = format_recent_label(&entry.name, &parent, name_budget);
                    let pad = name_budget.saturating_sub(label.chars().count());
                    label.push_str(&" ".repeat(pad));
                    label.push_str(" [x]");
                    label
                }
                PickerMode::Browse => {
                    let mut name = entry.name.clone();
                    if entry.is_dir {
                        name.push('/');
                    }
                    file_browser::format_recent_label(&name, "", name_budget.max(1))
                }
            };
            let is_hover = state.row_hover == Some(index);
            let is_sel =
                index == state.selected && state.button == FOCUS_LIST && state.row_hover.is_none();
            let style = if is_hover || is_sel {
                selected
            } else if entry.is_dir {
                dir_style
            } else {
                dim
            };
            lines.push(Line::from(Span::styled(text, style)));
        }
    }

    let inner_h = popup.height.saturating_sub(2) as usize;
    while lines.len() < inner_h.saturating_sub(1) {
        lines.push(Line::from(""));
    }
    let labels = footer_labels(state.mode);
    let mut row = ButtonRow::new(labels, &vec![false; labels.len()]);
    if state.button < labels.len() {
        row.select(Some(state.button));
    } else {
        for button in &mut row.buttons {
            button.state = crate::ui::button::ButtonState::Normal;
        }
    }
    lines.push(row.render(theme));

    let inner = Rect::new(
        popup.x.saturating_add(1),
        popup.y.saturating_add(1),
        popup.width.saturating_sub(2),
        popup.height.saturating_sub(2),
    );
    if inner.width > 0 && inner.height > 0 {
        f.render_widget(Paragraph::new(lines).style(dim), inner);
    }
}

/// Footer button under `x`, if the click is on the footer row.
pub fn footer_button_at(x: u16, y: u16, area: Rect, mode: PickerMode) -> Option<usize> {
    let popup = file_browser::file_browser_rect(area);
    if y != file_browser::footer_button_row(popup) {
        return None;
    }
    let labels = footer_labels(mode);
    let ranges = ButtonRow::button_x_ranges(popup.x.saturating_add(1), labels);
    ranges
        .iter()
        .position(|(start, end)| x >= *start && x < *end)
}

pub fn outside_popup(x: u16, y: u16, area: Rect) -> bool {
    let popup = file_browser::file_browser_rect(area);
    x < popup.x
        || y < popup.y
        || x >= popup.x.saturating_add(popup.width)
        || y >= popup.y.saturating_add(popup.height)
}

pub fn list_row_at(y: u16, area: Rect, selected: usize) -> Option<usize> {
    let popup = file_browser::file_browser_rect(area);
    let (start, end) = file_browser::list_area(popup, false);
    file_browser::list_index_at(y as usize, (start, end), selected)
}

pub fn recent_x_is_clear(x: u16, area: Rect, overflowing: bool) -> bool {
    let popup = file_browser::file_browser_rect(area);
    recent_row_click_is_clear(x as usize, popup, overflowing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn recent_project_label_keeps_the_folder_and_truncates_the_parent() {
        let path = Path::new("/home/josh/src/rust-edit");
        let parent = recent_parent_display(path);
        let wide = format_recent_label("rust-edit", &parent, 40);
        assert_eq!(wide, "rust-edit @ /home/josh/src");
        let narrow = format_recent_label("rust-edit", &parent, 16);
        assert!(narrow.starts_with("rust-edit"));
        assert!(narrow.chars().count() <= 16);
    }
}
