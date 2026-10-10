// Project-tree rows. Geometry and hit-testing stay in `side_panel`.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::project_tree::FlatRow;
use crate::theme::Theme;
use crate::ui::side_panel::{self, header_label, panel_geom, PanelNav, Side};

pub fn project_row_text(row: &FlatRow, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let indent = "  ".repeat(row.depth);
    let marker = if row.is_dir {
        if row.expanded {
            "- "
        } else {
            "+ "
        }
    } else {
        "  "
    };
    let prefix = format!("{indent}{marker}");
    let prefix_cols = prefix.chars().count();
    if prefix_cols >= width {
        return side_panel::ellipsize(&prefix, width);
    }
    let mut name = row.name.clone();
    if row.is_dir {
        name.push('/');
    }
    let room = width - prefix_cols;
    format!("{prefix}{}", side_panel::ellipsize(&name, room))
}

pub fn render_project(
    f: &mut Frame,
    area: Rect,
    side: Side,
    title: &str,
    rows: &[FlatRow],
    nav: PanelNav,
    message: &str,
    focused: bool,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let geom = panel_geom(area, side);
    let body = theme.editor_bg_style();
    f.render_widget(Paragraph::new("").style(body), area);

    if geom.header.width > 0 && geom.header.height > 0 {
        let label = header_label(title, side, geom.header.width as usize);
        f.render_widget(
            Paragraph::new(label).style(side_panel::panel_header_style(theme, focused)),
            geom.header,
        );
    }

    let width = geom.list.width as usize;
    let viewport = geom.list.height as usize;
    let selected_style = Style::default()
        .fg(Color::from(&theme.editor_fg))
        .bg(Color::from(&theme.selection_bg));
    let file_style = Style::default()
        .fg(Color::from(&theme.editor_fg))
        .bg(Color::from(&theme.editor_bg));
    let dir_style = Style::default()
        .fg(Color::from(&theme.accent))
        .bg(Color::from(&theme.editor_bg))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default()
        .fg(Color::from(&theme.line_number_fg))
        .bg(Color::from(&theme.editor_bg));

    let mut lines: Vec<Line> = Vec::new();
    if rows.is_empty() {
        if viewport > 0 {
            lines.push(Line::from(Span::styled(
                side_panel::ellipsize(message, width),
                dim,
            )));
        }
    } else {
        let start = nav.scroll.min(rows.len().saturating_sub(1));
        for (offset, row) in rows.iter().skip(start).take(viewport).enumerate() {
            let style = if start + offset == nav.selected {
                selected_style
            } else if row.is_dir {
                dir_style
            } else {
                file_style
            };
            lines.push(Line::from(Span::styled(
                project_row_text(row, width),
                style,
            )));
        }
    }
    if geom.list.width > 0 && geom.list.height > 0 {
        f.render_widget(Paragraph::new(lines).style(body), geom.list);
    }

    if !rows.is_empty() {
        let bar = crate::ui::scrollbar::OverflowBar {
            total: rows.len(),
            position: nav.scroll,
            viewport: viewport.max(1),
        };
        crate::ui::scrollbar::render(f, geom.scrollbar, bar, theme);
    }

    let sep_lines: Vec<Line> = (0..area.height).map(|_| Line::from("│")).collect();
    let sep_style = Style::default()
        .fg(Color::from(&theme.line_number_fg))
        .bg(Color::from(&theme.editor_bg));
    f.render_widget(
        Paragraph::new(sep_lines).style(sep_style),
        Rect::new(geom.separator_x, area.y, 1, area.height),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn row(is_dir: bool, expanded: bool, depth: usize, name: &str) -> FlatRow {
        FlatRow {
            name: name.to_string(),
            path: PathBuf::from(name),
            is_dir,
            depth,
            expanded,
        }
    }

    #[test]
    fn project_row_text_indents_and_marks_directories() {
        let dir = row(true, true, 1, "src");
        assert_eq!(project_row_text(&dir, 20), "  - src/");
        let collapsed = row(true, false, 0, "src");
        assert_eq!(project_row_text(&collapsed, 20), "+ src/");
        let file = row(false, false, 1, "main.rs");
        assert_eq!(project_row_text(&file, 20), "    main.rs");
    }

    #[test]
    fn project_row_text_ellipsizes_inside_the_width() {
        let file = row(false, false, 2, "very_long_module_name.rs");
        let text = project_row_text(&file, 16);
        assert_eq!(text.chars().count(), 16);
        assert!(text.ends_with('…'));
        assert!(text.starts_with("      "));
    }
}
