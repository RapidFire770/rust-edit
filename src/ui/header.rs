// Menu bar + file tabs + [+] button. Sprint 1/2.
// Presentation only.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let theme = app.current_theme();
    let active_menu = app.active_menu.as_deref();

    // Menu buttons with a consistent single-space pad on each side and a thin
    // `│` separator between them, so each label reads as its own clickable button.
    let menu_base_style = theme.header_style();
    let menu_highlight = Style::default()
        .bg(Color::from(&theme.accent))
        .fg(Color::from(&theme.header_fg))
        .add_modifier(Modifier::BOLD);
    let sep_style = Style::default()
        .bg(Color::from(&theme.header_bg))
        .fg(Color::from(&theme.header_fg));

    let mut header_spans: Vec<Span> = Vec::new();
    let menus = [
        ("File", " File "),
        ("Tools", " Tools "),
        ("Settings", " Settings "),
    ];
    for (i, (name, label)) in menus.iter().enumerate() {
        if i > 0 {
            header_spans.push(Span::styled("│", sep_style));
        }
        let style = if active_menu == Some(*name) {
            menu_highlight
        } else {
            menu_base_style
        };
        header_spans.push(Span::styled(*label, style));
    }
    // A slightly wider gap between the menu block and the file tabs.
    header_spans.push(Span::styled("  ", menu_base_style));

    // File tabs — all bracketed for consistency with [untitled] / [+]. The active
    // tab uses the accent background so the current buffer is unmistakable.
    let active_tab_style = Style::default()
        .bg(Color::from(&theme.accent))
        .fg(Color::from(&theme.header_fg))
        .add_modifier(Modifier::BOLD);
    let inactive_tab_style = menu_base_style;
    for (i, buf) in app.buffers.iter().enumerate() {
        let mut name = buf
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();
        if buf.is_modified {
            name.push('•');
        }
        let is_active = i == app.active;
        let style = if is_active {
            active_tab_style
        } else {
            inactive_tab_style
        };
        header_spans.push(Span::styled(format!("[ {name} ]"), style));
        header_spans.push(Span::styled(" ", menu_base_style));
    }
    header_spans.push(Span::styled("[+]", menu_base_style));

    // Base style fills trailing cells to the right edge regardless of content
    // (the old byte-length padding under-counted on non-ASCII chars).
    let header = Paragraph::new(Line::from(header_spans)).style(theme.header_style());
    f.render_widget(header, area);
}

/// Simple text for menus (can be expanded for dropdown anchors later).
pub fn menu_labels() -> &'static str {
    " File  Tools  Settings  "
}

/// Render a dropdown popup for a menu.
/// Positioned below header. Uses simple bordered list.
pub fn render_dropdown(
    f: &mut Frame,
    menu: &str,
    area: Rect,
    theme: &crate::theme::Theme,
    selected: usize,
) {
    use ratatui::widgets::{Block, Borders, List, ListItem};

    let file_items = vec![
        "New (Ctrl+N)",
        "Open (Ctrl+O)",
        "Save (Ctrl+S)",
        "Save As (Ctrl+Alt+S)",
        "Close Tab (Ctrl+W)",
        "Quit (Ctrl+Q)",
    ];
    let tools_items = vec![
        "Find (Ctrl+F)",
        "Find & Replace (Ctrl+H)",
        "Word Count (Alt+W)",
        "Format Document",
        "Toggle Word Wrap (Alt+Z)",
    ];
    let settings_items = vec!["Settings...", "Themes"];

    // x offset where each dropdown should appear, matching the header layout:
    // " File "│" Tools "│" Settings "  → File starts col 0, Tools col 7, Settings col 15.
    let (item_strs, menu_x) = match menu {
        "File" => (file_items, 0u16),
        "Tools" => (tools_items, 7u16),
        "Settings" => (settings_items, 15u16),
        _ => (vec!["..."], 0u16),
    };

    // Compute width to fit longest item + padding + borders
    let max_item_len = item_strs.iter().map(|s| s.len()).max().unwrap_or(20);
    let width = (max_item_len as u16 + 4).max(26);
    let height = (item_strs.len() as u16 + 2).min(10);

    let items: Vec<ListItem> = item_strs
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            if i == selected {
                ListItem::new(Span::styled(
                    format!("> {}", s),
                    Style::default()
                        .fg(Color::from(&theme.accent))
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                ListItem::new(format!("  {}", s))
            }
        })
        .collect();

    let dropdown_area = Rect::new(
        menu_x,
        crate::ui::layout::HEADER_ROWS, // below header (menu row + separator rule)
        width.min(area.width),
        height.min(area.height.saturating_sub(crate::ui::layout::HEADER_ROWS)),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .style(theme.editor_bg_style()); // or header-ish

    let list = List::new(items).block(block).style(
        Style::default()
            .fg(Color::from(&theme.editor_fg))
            .bg(Color::from(&theme.editor_bg)),
    );

    // Clear under to avoid bleed
    f.render_widget(ratatui::widgets::Clear, dropdown_area);
    f.render_widget(list, dropdown_area);
}

/// Get the dropdown popup rect for hit-testing
pub fn dropdown_rect(area: Rect, menu_name: &str) -> Rect {
    let header_height = crate::ui::layout::HEADER_ROWS;
    let h = match menu_name {
        "File" => 8,
        "Tools" => 7,
        "Settings" => 4,
        _ => 4,
    };
    let w = 26;
    let y = header_height;

    let x = match menu_name {
        "File" => 0,
        "Tools" => 7,
        "Settings" => 15,
        _ => 0,
    };

    Rect::new(x, y, w, h.min(area.height.saturating_sub(y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dropdown_rect_file() {
        let area = Rect::new(0, 0, 100, 50);
        let rect = dropdown_rect(area, "File");
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, crate::ui::layout::HEADER_ROWS);
        assert_eq!(rect.width, 26);
        assert_eq!(rect.height, 8);
    }

    #[test]
    fn test_dropdown_rect_tools() {
        let area = Rect::new(0, 0, 100, 50);
        let rect = dropdown_rect(area, "Tools");
        assert_eq!(rect.x, 7);
        assert_eq!(rect.y, crate::ui::layout::HEADER_ROWS);
        assert_eq!(rect.width, 26);
        assert_eq!(rect.height, 7);
    }

    #[test]
    fn test_dropdown_rect_settings() {
        let area = Rect::new(0, 0, 100, 50);
        let rect = dropdown_rect(area, "Settings");
        assert_eq!(rect.x, 15);
        assert_eq!(rect.y, crate::ui::layout::HEADER_ROWS);
        assert_eq!(rect.width, 26);
        assert_eq!(rect.height, 4);
    }
}
