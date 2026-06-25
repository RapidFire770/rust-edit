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

    // Build menu part with possible highlight for open dropdown
    let menu_base_style = theme.header_style();
    let menu_highlight = Style::default()
        .bg(Color::from(&theme.accent))
        .fg(Color::from(&theme.header_fg))
        .add_modifier(Modifier::BOLD);

    let mut menu_spans = vec![];
    let menu_parts = [
        ("File", " File  "),
        ("Tools", " Tools  "),
        ("Settings", " Settings  "),
    ];
    for (name, label) in menu_parts {
        let style = if active_menu == Some(name) {
            menu_highlight
        } else {
            menu_base_style
        };
        menu_spans.push(Span::styled(label, style));
    }
    let mut header_spans = menu_spans;

    for (i, buf) in app.buffers.iter().enumerate() {
        let mut name = buf
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[untitled]")
            .to_string();
        if buf.is_modified {
            name.push('•');
        }
        let is_active = i == app.active;
        let style = if is_active {
            theme.header_style()
        } else {
            Style::default()
                .bg(Color::from(&theme.header_bg))
                .fg(Color::from(&theme.header_fg))
        };
        header_spans.push(Span::styled(format!(" {} ", name), style));
    }
    header_spans.push(Span::styled(" [+] ", theme.header_style()));

    // Pad the rest of the line with header background so the color spans the entire top row
    let used_width: u16 = header_spans.iter().map(|s| s.content.len() as u16).sum();
    if used_width < area.width {
        let padding = " ".repeat((area.width - used_width) as usize);
        header_spans.push(Span::styled(padding, theme.header_style()));
    }

    let header = Paragraph::new(Line::from(header_spans));
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
        "Save As (Ctrl+Shift+S)",
        "Close Tab (Ctrl+W)",
        "Quit (Ctrl+Q)",
    ];
    let tools_items = vec![
        "Find (Ctrl+F)",
        "Find & Replace (Ctrl+H)",
        "Word Count (Shift+W)",
        "Format Document",
    ];
    let settings_items = vec!["Settings...", "Themes"];

    let (item_strs, menu_x) = match menu {
        "File" => (file_items, 1u16),
        "Tools" => (tools_items, 8u16),
        "Settings" => (settings_items, 15u16),
        _ => (vec!["..."], 1u16),
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
        1, // below header
        width.min(area.width),
        height.min(area.height.saturating_sub(1)),
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
