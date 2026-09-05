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
    let settings_items = vec!["Configure...", "Themes...", "Help (F1)"];

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
        "Settings" => 5,
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

pub const MENU_BLOCK_COLS: usize = 27;
pub const PLUS_COLS: usize = 3;
pub const ARROW_COLS: usize = 3; // "[<]" / "[>]"
pub const ARROW_PAD: usize = 1; // trailing space after an arrow

pub struct TabGeom {
    pub index: usize,
    pub x: usize,     // screen column of '['
    pub width: usize, // includes trailing space; may be clipped
    pub clipped: bool,
}

pub struct TabStripLayout {
    pub overflowing: bool,
    pub first_visible: usize, // clamped
    pub left_arrow_x: Option<usize>,
    pub right_arrow_x: Option<usize>,
    pub plus_x: usize,
    pub tabs: Vec<TabGeom>,
    pub can_scroll_left: bool,
    pub can_scroll_right: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderHit {
    Menu(&'static str),
    Tab(usize),
    Plus,
    ScrollLeft,
    ScrollRight,
    Miss,
}

fn tab_width(name: &str) -> usize {
    name.chars().count() + 5
}

/// Walk from the end, packing whole tab widths into `viewport`. A single
/// too-wide tab still occupies the viewport (clipped). The first index of
/// that packed-from-end window is the max scroll.
fn max_first_visible(names: &[String], viewport: usize) -> usize {
    if names.is_empty() {
        return 0;
    }
    let mut acc: usize = 0;
    let mut first = names.len() - 1;
    for i in (0..names.len()).rev() {
        let w = tab_width(&names[i]);
        if acc.saturating_add(w) <= viewport {
            acc += w;
            first = i;
        } else if acc == 0 {
            first = i;
            break;
        } else {
            break;
        }
    }
    first
}

fn pack_visible_tabs(
    names: &[String],
    first_visible: usize,
    start_x: usize,
    viewport: usize,
) -> Vec<TabGeom> {
    let mut tabs = Vec::new();
    let mut used: usize = 0;
    for (offset, name) in names.get(first_visible..).unwrap_or(&[]).iter().enumerate() {
        let index = first_visible + offset;
        let w = tab_width(name);
        if used.saturating_add(w) <= viewport {
            tabs.push(TabGeom {
                index,
                x: start_x + used,
                width: w,
                clipped: false,
            });
            used += w;
        } else if tabs.is_empty() {
            // First visible tab does not fit: clip it to the remaining viewport.
            if viewport > 0 {
                tabs.push(TabGeom {
                    index,
                    x: start_x,
                    width: viewport,
                    clipped: true,
                });
            }
            break;
        } else {
            break;
        }
    }
    tabs
}

/// `names` are the inner labels (`untitled`, `main.rs`, `foo.rs•`), no brackets.
pub fn layout_tab_strip(
    names: &[String],
    header_width: usize,
    first_visible: usize,
) -> TabStripLayout {
    let remaining = header_width.saturating_sub(MENU_BLOCK_COLS);
    let total_tabs: usize = names.iter().map(|n| tab_width(n)).sum();
    let fits = total_tabs.saturating_add(PLUS_COLS) <= remaining;

    if fits {
        let mut tabs = Vec::with_capacity(names.len());
        let mut x = MENU_BLOCK_COLS;
        for (index, name) in names.iter().enumerate() {
            let width = tab_width(name);
            tabs.push(TabGeom {
                index,
                x,
                width,
                clipped: false,
            });
            x += width;
        }
        return TabStripLayout {
            overflowing: false,
            first_visible: 0,
            left_arrow_x: None,
            right_arrow_x: None,
            plus_x: x,
            tabs,
            can_scroll_left: false,
            can_scroll_right: false,
        };
    }

    let chrome = (ARROW_COLS + ARROW_PAD) * 2 + PLUS_COLS;
    if remaining < chrome {
        return layout_degraded_overflow(names, header_width, first_visible);
    }

    let viewport = remaining.saturating_sub(chrome);
    let first_visible = first_visible.min(max_first_visible(names, viewport));
    let tab_start = MENU_BLOCK_COLS + ARROW_COLS + ARROW_PAD;
    let tabs = pack_visible_tabs(names, first_visible, tab_start, viewport);
    let right_arrow_x = tab_start + viewport;
    let plus_x = right_arrow_x + ARROW_COLS + ARROW_PAD;
    let can_scroll_right = can_scroll_right(&tabs, names, first_visible);

    TabStripLayout {
        overflowing: true,
        first_visible,
        left_arrow_x: Some(MENU_BLOCK_COLS),
        right_arrow_x: Some(right_arrow_x),
        plus_x,
        tabs,
        can_scroll_left: first_visible > 0,
        can_scroll_right,
    }
}

/// Overflow chrome (`[<] [>] [+]`) does not fit. Drop arrows rather than sliding
/// `[+]` onto menus, and pack tabs into `[MENU_BLOCK_COLS, plus_x)`.
fn layout_degraded_overflow(
    names: &[String],
    header_width: usize,
    first_visible: usize,
) -> TabStripLayout {
    let plus_x = if header_width >= MENU_BLOCK_COLS + PLUS_COLS {
        header_width.saturating_sub(PLUS_COLS).max(MENU_BLOCK_COLS)
    } else {
        MENU_BLOCK_COLS
    };
    let leftover = plus_x.saturating_sub(MENU_BLOCK_COLS);
    let first_visible = first_visible.min(max_first_visible(names, leftover));
    let tabs = pack_visible_tabs(names, first_visible, MENU_BLOCK_COLS, leftover);
    let can_scroll_right = can_scroll_right(&tabs, names, first_visible);

    TabStripLayout {
        overflowing: true,
        first_visible,
        left_arrow_x: None,
        right_arrow_x: None,
        plus_x,
        tabs,
        can_scroll_left: first_visible > 0,
        can_scroll_right,
    }
}

fn can_scroll_right(tabs: &[TabGeom], names: &[String], first_visible: usize) -> bool {
    match tabs.last().map(|g| g.index) {
        Some(i) => i < names.len().saturating_sub(1),
        None => !names.is_empty() && first_visible < names.len().saturating_sub(1),
    }
}

pub fn hit_test_header(x: usize, layout: &TabStripLayout) -> HeaderHit {
    // Menu ranges are independent of the tab-strip layout and are checked first.
    if x < 6 {
        return HeaderHit::Menu("File");
    }
    if (7..14).contains(&x) {
        return HeaderHit::Menu("Tools");
    }
    if (15..25).contains(&x) {
        return HeaderHit::Menu("Settings");
    }

    if let Some(ax) = layout.left_arrow_x {
        if x >= ax && x < ax + ARROW_COLS {
            return HeaderHit::ScrollLeft;
        }
    }
    for tab in &layout.tabs {
        if x >= tab.x && x < tab.x + tab.width {
            return HeaderHit::Tab(tab.index);
        }
    }
    if let Some(ax) = layout.right_arrow_x {
        if x >= ax && x < ax + ARROW_COLS {
            return HeaderHit::ScrollRight;
        }
    }
    if x >= layout.plus_x && x < layout.plus_x + PLUS_COLS {
        return HeaderHit::Plus;
    }
    HeaderHit::Miss
}

/// Filename or `"untitled"`, plus `'•'` if the buffer is modified.
pub fn buffer_tab_name(buf: &crate::buffer::Buffer) -> String {
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
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn untitled_names(n: usize) -> Vec<String> {
        vec!["untitled".to_string(); n]
    }

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
        assert_eq!(rect.height, 5);
    }

    #[test]
    fn tabs_that_fit_have_no_arrows_and_plus_after_last() {
        // 1 untitled: tabs start at 27, width 13, plus at 40
        let l = layout_tab_strip(&untitled_names(1), 80, 0);
        assert!(!l.overflowing);
        assert_eq!(l.left_arrow_x, None);
        assert_eq!(l.plus_x, 27 + 13);
        assert_eq!(l.tabs.len(), 1);
        assert_eq!(hit_test_header(40, &l), HeaderHit::Plus); // middle of [+]
        assert_eq!(hit_test_header(27, &l), HeaderHit::Tab(0));
    }

    #[test]
    fn four_untitled_tabs_overflow_80_cols() {
        // 27 + 4*13 + 3 = 82 > 80
        let l = layout_tab_strip(&untitled_names(4), 80, 0);
        assert!(l.overflowing);
        assert_eq!(l.left_arrow_x, Some(27));
        assert!(l.can_scroll_right);
        assert!(!l.can_scroll_left);
        assert!(l.plus_x + PLUS_COLS <= 80);
        assert_eq!(hit_test_header(l.plus_x, &l), HeaderHit::Plus);
        assert_eq!(hit_test_header(27, &l), HeaderHit::ScrollLeft);
        assert_eq!(
            hit_test_header(l.right_arrow_x.unwrap(), &l),
            HeaderHit::ScrollRight
        );
    }

    #[test]
    fn first_visible_clamps_so_viewport_stays_packed() {
        let l = layout_tab_strip(&untitled_names(4), 80, 99);
        assert!(l.first_visible > 0);
        assert!(l.can_scroll_left);
        assert!(!l.can_scroll_right);
    }

    #[test]
    fn too_wide_single_tab_is_clipped_and_plus_stays_on_screen() {
        let names = vec!["a".repeat(80)];
        let l = layout_tab_strip(&names, 40, 0);
        assert!(l.overflowing);
        assert_eq!(l.tabs.len(), 1);
        assert!(l.tabs[0].clipped);
        assert!(l.plus_x + PLUS_COLS <= 40);
    }

    #[test]
    fn menus_still_win_hit_test() {
        let l = layout_tab_strip(&untitled_names(1), 80, 0);
        assert_eq!(hit_test_header(1, &l), HeaderHit::Menu("File"));
        assert_eq!(hit_test_header(8, &l), HeaderHit::Menu("Tools"));
        assert_eq!(hit_test_header(16, &l), HeaderHit::Menu("Settings"));
    }

    #[test]
    fn modified_name_width_counts_bullet_as_one_char() {
        let names = vec!["main.rs•".to_string()];
        let l = layout_tab_strip(&names, 80, 0);
        assert!(!l.overflowing);
        assert_eq!(l.tabs.len(), 1);
        assert_eq!(l.tabs[0].width, "main.rs•".chars().count() + 5);
        assert_eq!(l.plus_x, MENU_BLOCK_COLS + l.tabs[0].width);
    }

    #[test]
    fn buffer_tab_name_untitled_unmodified() {
        let buf = crate::buffer::Buffer::new();
        assert_eq!(buffer_tab_name(&buf), "untitled");
    }

    #[test]
    fn buffer_tab_name_appends_bullet_when_modified() {
        let mut buf = crate::buffer::Buffer::new();
        buf.is_modified = true;
        assert_eq!(buffer_tab_name(&buf), "untitled•");
    }

    #[test]
    fn four_untitled_at_80_packs_three_tabs_and_stable_plus() {
        let l = layout_tab_strip(&untitled_names(4), 80, 0);
        assert_eq!(l.tabs.len(), 3);
        assert_eq!(l.first_visible, 0);
        assert_eq!(l.tabs[0].index, 0);
        assert_eq!(l.tabs[0].x, 31);
        assert_eq!(l.tabs[0].width, 13);
        assert!(!l.tabs.iter().any(|t| t.clipped));
        assert_eq!(l.right_arrow_x, Some(73));
        assert_eq!(l.plus_x, 77);
        assert_on_screen_widgets_disjoint(&l, 80);
    }

    fn occupy(owner: &mut [Option<&'static str>], start: usize, end: usize, label: &'static str) {
        let header_width = owner.len();
        let start = start.min(header_width);
        let end = end.min(header_width);
        for x in start..end {
            assert!(
                owner[x].is_none(),
                "column {x} already {:?} would also be {label}",
                owner[x]
            );
            owner[x] = Some(label);
        }
    }

    /// Geometric ranges of Menu / arrows / tabs / plus must not share a column
    /// inside `0..header_width`. `hit_test_header` is first-match and would hide
    /// overlaps, so this inspects the layout rects directly.
    fn assert_on_screen_widgets_disjoint(layout: &TabStripLayout, header_width: usize) {
        let mut owner = vec![None; header_width];
        occupy(&mut owner, 0, 6, "Menu");
        occupy(&mut owner, 7, 14, "Menu");
        occupy(&mut owner, 15, 25, "Menu");
        if let Some(ax) = layout.left_arrow_x {
            occupy(&mut owner, ax, ax + ARROW_COLS, "ScrollLeft");
        }
        for tab in &layout.tabs {
            occupy(&mut owner, tab.x, tab.x + tab.width, "Tab");
        }
        if let Some(ax) = layout.right_arrow_x {
            occupy(&mut owner, ax, ax + ARROW_COLS, "ScrollRight");
        }
        occupy(&mut owner, layout.plus_x, layout.plus_x + PLUS_COLS, "Plus");
    }

    #[test]
    fn empty_names_place_plus_at_menu_block() {
        let l = layout_tab_strip(&[], 80, 0);
        assert!(!l.overflowing);
        assert!(l.tabs.is_empty());
        assert_eq!(l.plus_x, MENU_BLOCK_COLS);
        assert_eq!(l.left_arrow_x, None);
        assert_eq!(l.right_arrow_x, None);
        assert_eq!(hit_test_header(MENU_BLOCK_COLS, &l), HeaderHit::Plus);
        assert_on_screen_widgets_disjoint(&l, 80);
    }

    #[test]
    fn four_untitled_at_width_38_arrows_with_empty_viewport() {
        // remaining = 11 == chrome, viewport = 0. Arrows stay; plus at 35.
        let l = layout_tab_strip(&untitled_names(4), 38, 0);
        assert!(l.overflowing);
        assert_eq!(l.left_arrow_x, Some(27));
        assert_eq!(l.right_arrow_x, Some(31));
        assert_eq!(l.plus_x, 35);
        assert!(l.tabs.is_empty());
        assert_eq!(hit_test_header(27, &l), HeaderHit::ScrollLeft);
        assert_eq!(hit_test_header(31, &l), HeaderHit::ScrollRight);
        assert_eq!(hit_test_header(35, &l), HeaderHit::Plus);
        assert_on_screen_widgets_disjoint(&l, 38);
    }

    #[test]
    fn width_30_drops_arrows_and_keeps_plus_off_menus() {
        // remaining = 3 < chrome: no arrows, leftover tab span is 0, plus at 27.
        let l = layout_tab_strip(&untitled_names(4), 30, 0);
        assert!(l.overflowing);
        assert_eq!(l.left_arrow_x, None);
        assert_eq!(l.right_arrow_x, None);
        assert_eq!(l.plus_x, MENU_BLOCK_COLS);
        assert!(l.tabs.is_empty());
        assert_eq!(hit_test_header(16, &l), HeaderHit::Menu("Settings"));
        assert_eq!(hit_test_header(27, &l), HeaderHit::Plus);
        assert_on_screen_widgets_disjoint(&l, 30);
    }

    #[test]
    fn width_20_plus_stays_off_menus() {
        // remaining = 0: not a fit. plus sits at 27 (off-screen), never on Settings.
        let l = layout_tab_strip(&untitled_names(4), 20, 0);
        assert!(l.overflowing);
        assert_eq!(l.left_arrow_x, None);
        assert_eq!(l.right_arrow_x, None);
        assert_eq!(l.plus_x, MENU_BLOCK_COLS);
        assert!(l.tabs.is_empty());
        assert_eq!(hit_test_header(16, &l), HeaderHit::Menu("Settings"));
        assert_ne!(hit_test_header(19, &l), HeaderHit::Plus);
        assert_on_screen_widgets_disjoint(&l, 20);
    }
}
