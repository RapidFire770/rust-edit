// Function List and Variables List side panels.
// Geometry is the paint and hit-test source. Scanning lives in `symbols`.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::Theme;

/// Columns reserved for one symbol list, including its separator and scrollbar.
pub const PANEL_COLS: u16 = 24;

/// Columns reserved for the project tree. Wider than a symbol list so nested
/// names stay readable. A project panel plus one symbol list needs 84 columns.
pub const PROJECT_PANEL_COLS: u16 = 36;

/// Editor rect (gutter + text + vertical bar) that must remain when a panel is open.
pub const MIN_EDITOR_COLS: u16 = 24;

const MOVE_BUTTON: &str = "[>]";
const MOVE_BUTTON_LEFT: &str = "[<]";
const CLOSE_BUTTON: &str = "[x]";
const BUTTON_COLS: u16 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn other(self) -> Self {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Functions,
    Variables,
    Project,
}

impl PanelKind {
    pub fn title(self) -> &'static str {
        match self {
            PanelKind::Functions => "Functions",
            PanelKind::Variables => "Variables",
            PanelKind::Project => "Project",
        }
    }

    /// Side used when the other list is not already there.
    pub fn preferred_side(self) -> Side {
        match self {
            PanelKind::Functions | PanelKind::Project => Side::Left,
            PanelKind::Variables => Side::Right,
        }
    }
}

pub fn panel_cols(kind: PanelKind) -> u16 {
    match kind {
        PanelKind::Functions | PanelKind::Variables => PANEL_COLS,
        PanelKind::Project => PROJECT_PANEL_COLS,
    }
}

/// Slots after a width check. `notice` is set only when a panel was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    pub left: Option<PanelKind>,
    pub right: Option<PanelKind>,
    pub notice: Option<&'static str>,
}

fn panels_need(left: Option<PanelKind>, right: Option<PanelKind>) -> u16 {
    left.map(panel_cols).unwrap_or(0) + right.map(panel_cols).unwrap_or(0) + MIN_EDITOR_COLS
}

/// Drop a panel when the editor would be narrower than [`MIN_EDITOR_COLS`].
/// Two symbol lists drop the right one. A project panel is kept and the
/// other side is dropped, because the project tree is the wider slot.
pub fn fit_panels(left: Option<PanelKind>, right: Option<PanelKind>, width: u16) -> Fit {
    let mut left = left;
    let mut right = right;
    let mut notice = None;
    if left.is_some() && right.is_some() && width < panels_need(left, right) {
        let project = left == Some(PanelKind::Project) || right == Some(PanelKind::Project);
        if project && left == Some(PanelKind::Project) {
            right = None;
            notice = Some("Window is too narrow for both panels");
        } else if project {
            left = None;
            notice = Some("Window is too narrow for both panels");
        } else {
            right = None;
            notice = Some("Window is too narrow for both lists");
        }
    }
    if (left.is_some() || right.is_some()) && width < panels_need(left, right) {
        left = None;
        right = None;
        notice = Some("Window is too narrow");
    }
    Fit {
        left,
        right,
        notice,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SideSplit {
    pub left: Option<Rect>,
    pub editor: Rect,
    pub right: Option<Rect>,
    /// Full-width find bar under the panels. `None` when find is closed.
    pub find: Option<Rect>,
}

/// Split the editor band (below the header, above the status bar).
/// The find bar stays full width. Panels occupy only the rows beside the editor.
pub fn split_editor_band(
    area: Rect,
    left: Option<PanelKind>,
    right: Option<PanelKind>,
    find_open: bool,
) -> SideSplit {
    let (body, find) = if find_open {
        let (body, find_area) = super::layout::editor_with_find_layout(area);
        (body, Some(find_area))
    } else {
        (area, None)
    };

    let mut constraints = Vec::with_capacity(3);
    if let Some(kind) = left {
        constraints.push(Constraint::Length(panel_cols(kind)));
    }
    constraints.push(Constraint::Min(MIN_EDITOR_COLS));
    if let Some(kind) = right {
        constraints.push(Constraint::Length(panel_cols(kind)));
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(body);

    let mut index = 0;
    let left = if left.is_some() {
        let rect = chunks[index];
        index += 1;
        Some(rect)
    } else {
        None
    };
    let editor = chunks[index];
    let right = if right.is_some() {
        index += 1;
        Some(chunks[index])
    } else {
        None
    };

    SideSplit {
        left,
        editor,
        right,
        find,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelGeom {
    pub outer: Rect,
    pub separator_x: u16,
    pub content: Rect,
    pub scrollbar: Rect,
    pub header: Rect,
    pub list: Rect,
    /// Inclusive start, exclusive end, in screen columns.
    pub move_button: (u16, u16),
    pub close_button: (u16, u16),
}

/// Left panel: names, scrollbar, then `│`. Right panel: `│`, names, scrollbar.
pub fn panel_geom(outer: Rect, side: Side) -> PanelGeom {
    let (separator_x, content_x, scrollbar_x, inner_w) = match side {
        Side::Left => {
            let sep = outer.x.saturating_add(outer.width.saturating_sub(1));
            let bar = outer.x.saturating_add(outer.width.saturating_sub(2));
            (sep, outer.x, bar, outer.width.saturating_sub(2))
        }
        Side::Right => {
            let content_x = outer.x.saturating_add(1);
            let bar = outer.x.saturating_add(outer.width.saturating_sub(1));
            (outer.x, content_x, bar, outer.width.saturating_sub(2))
        }
    };
    let content = Rect::new(content_x, outer.y, inner_w, outer.height);
    let header_h = u16::from(outer.height > 0 && inner_w > 0);
    let header = Rect::new(content.x, content.y, content.width, header_h);
    let list_h = outer.height.saturating_sub(header_h);
    let list = Rect::new(
        content.x,
        content.y.saturating_add(header_h),
        content.width,
        list_h,
    );
    let scrollbar = Rect::new(
        scrollbar_x,
        content.y.saturating_add(header_h),
        u16::from(outer.width >= 2 && list_h > 0),
        list_h,
    );
    let (move_button, close_button) = header_buttons(header);
    PanelGeom {
        outer,
        separator_x,
        content,
        scrollbar,
        header,
        list,
        move_button,
        close_button,
    }
}

fn header_buttons(header: Rect) -> ((u16, u16), (u16, u16)) {
    let right = header.x.saturating_add(header.width);
    let close_w = BUTTON_COLS.min(header.width);
    let move_w = BUTTON_COLS.min(header.width.saturating_sub(close_w));
    let close_start = right.saturating_sub(close_w);
    let move_start = close_start.saturating_sub(move_w);
    ((move_start, close_start), (close_start, right))
}

/// Header text for `width` columns. The last six columns are `[>][x]` or `[<][x]`
/// when they fit, matching [`PanelGeom::move_button`] and `close_button`.
pub fn header_label(title: &str, side: Side, width: usize) -> String {
    let move_txt = match side {
        Side::Left => MOVE_BUTTON,
        Side::Right => MOVE_BUTTON_LEFT,
    };
    let buttons = format!("{move_txt}{CLOSE_BUTTON}");
    if width <= buttons.len() {
        return buttons.chars().take(width).collect();
    }
    let title_room = width - buttons.len();
    let mut shown: String = title.chars().take(title_room).collect();
    while shown.chars().count() < title_room {
        shown.push(' ');
    }
    shown.push_str(&buttons);
    shown
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelHit {
    Move,
    Close,
    Header,
    /// Offset of the visible list row. Add the list scroll to get a symbol index.
    Row(usize),
    Scrollbar,
    Miss,
}

pub fn hit_test(geom: PanelGeom, x: u16, y: u16) -> PanelHit {
    if !point_in(geom.outer, x, y) {
        return PanelHit::Miss;
    }
    if x == geom.separator_x {
        return PanelHit::Miss;
    }
    if point_in(geom.scrollbar, x, y) {
        return PanelHit::Scrollbar;
    }
    if point_in(geom.header, x, y) {
        if x >= geom.move_button.0 && x < geom.move_button.1 {
            return PanelHit::Move;
        }
        if x >= geom.close_button.0 && x < geom.close_button.1 {
            return PanelHit::Close;
        }
        return PanelHit::Header;
    }
    if point_in(geom.list, x, y) {
        return PanelHit::Row((y - geom.list.y) as usize);
    }
    PanelHit::Miss
}

pub fn point_in(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}

/// One visible list entry. `line_no` is 1-based, matching the gutter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRow {
    pub label: String,
    pub line_no: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelNav {
    pub selected: usize,
    pub scroll: usize,
}

/// Keep `selected` inside the visible rows.
pub fn reveal(selected: usize, scroll: &mut usize, viewport: usize) {
    if viewport == 0 {
        return;
    }
    if selected < *scroll {
        *scroll = selected;
    } else if selected >= scroll.saturating_add(viewport) {
        *scroll = selected + 1 - viewport;
    }
}

/// Name on the left, line number on the right, ellipsis when the name does not fit.
pub fn row_line(label: &str, line_no: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let num = line_no.to_string();
    if num.len() + 1 < width {
        let name_w = width - num.len() - 1;
        let name = ellipsize(label, name_w);
        let gap = width - name.chars().count() - num.len();
        format!("{name}{}{num}", " ".repeat(gap))
    } else {
        ellipsize(label, width)
    }
}

pub(crate) fn ellipsize(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let keep = width.saturating_sub(1);
    let mut out: String = text.chars().take(keep).collect();
    out.push('…');
    out
}

pub struct PanelView<'a> {
    pub side: Side,
    pub kind: PanelKind,
    pub rows: &'a [ListRow],
    pub nav: PanelNav,
    pub message: &'a str,
    pub focused: bool,
}

/// Accent background while the list has the keyboard, matching the active tab.
fn panel_header_style(theme: &Theme, focused: bool) -> Style {
    if focused {
        Style::default()
            .bg(Color::from(&theme.accent))
            .fg(Color::from(&theme.header_fg))
            .add_modifier(Modifier::BOLD)
    } else {
        theme.header_style()
    }
}

pub fn render(f: &mut Frame, area: Rect, view: PanelView<'_>, theme: &Theme) {
    let PanelView {
        side,
        kind,
        rows,
        nav,
        message,
        focused,
    } = view;
    if area.width == 0 || area.height == 0 {
        return;
    }
    let geom = panel_geom(area, side);
    let body = theme.editor_bg_style();
    f.render_widget(Paragraph::new("").style(body), area);

    if geom.header.width > 0 && geom.header.height > 0 {
        let label = header_label(kind.title(), side, geom.header.width as usize);
        f.render_widget(
            Paragraph::new(label).style(panel_header_style(theme, focused)),
            geom.header,
        );
    }

    let width = geom.list.width as usize;
    let viewport = geom.list.height as usize;
    let selected_style = Style::default()
        .fg(Color::from(&theme.editor_fg))
        .bg(Color::from(&theme.selection_bg));
    let plain = Style::default()
        .fg(Color::from(&theme.editor_fg))
        .bg(Color::from(&theme.editor_bg));
    let dim = Style::default()
        .fg(Color::from(&theme.line_number_fg))
        .bg(Color::from(&theme.editor_bg));

    let mut lines: Vec<Line> = Vec::new();
    if rows.is_empty() {
        if viewport > 0 {
            lines.push(Line::from(Span::styled(ellipsize(message, width), dim)));
        }
    } else {
        let start = nav.scroll.min(rows.len().saturating_sub(1));
        for (offset, row) in rows.iter().skip(start).take(viewport).enumerate() {
            let style = if start + offset == nav.selected {
                selected_style
            } else {
                plain
            };
            lines.push(Line::from(Span::styled(
                row_line(&row.label, row.line_no, width),
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

    fn band(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    #[test]
    fn row_line_keeps_the_line_number_and_ellipsizes_the_name() {
        assert_eq!(row_line("main", 12, 10), "main    12");
        let long = row_line("parse_symbols", 140, 12);
        assert!(long.ends_with("140"), "{long}");
        assert!(long.contains('…'), "{long}");
        assert_eq!(long.chars().count(), 12);
    }

    #[test]
    fn lists_prefer_opposite_sides() {
        assert_eq!(PanelKind::Functions.preferred_side(), Side::Left);
        assert_eq!(PanelKind::Variables.preferred_side(), Side::Right);
        assert_eq!(PanelKind::Project.preferred_side(), Side::Left);
    }

    #[test]
    fn focused_header_uses_the_accent_background() {
        let theme = Theme::dark_red();
        let focused = panel_header_style(&theme, true);
        let idle = panel_header_style(&theme, false);
        assert_eq!(focused.bg, Some(Color::from(&theme.accent)));
        assert_eq!(focused.fg, Some(Color::from(&theme.header_fg)));
        assert!(focused.add_modifier.contains(Modifier::BOLD));
        assert_eq!(idle, theme.header_style());
    }

    #[test]
    fn both_panels_on_80_leave_32_for_the_editor() {
        let split = split_editor_band(
            band(80, 10),
            Some(PanelKind::Functions),
            Some(PanelKind::Variables),
            false,
        );
        assert_eq!(split.left, Some(Rect::new(0, 0, 24, 10)));
        assert_eq!(split.editor, Rect::new(24, 0, 32, 10));
        assert_eq!(split.right, Some(Rect::new(56, 0, 24, 10)));
        assert_eq!(split.find, None);
    }

    #[test]
    fn find_bar_stays_full_width_under_the_panels() {
        let split = split_editor_band(
            band(80, 10),
            Some(PanelKind::Functions),
            Some(PanelKind::Variables),
            true,
        );
        assert_eq!(split.find, Some(Rect::new(0, 9, 80, 1)));
        assert_eq!(split.left.unwrap().height, 9);
        assert_eq!(split.editor, Rect::new(24, 0, 32, 9));
        assert_eq!(split.right.unwrap().height, 9);
    }

    #[test]
    fn one_panel_takes_its_side_only() {
        let left = split_editor_band(band(80, 10), Some(PanelKind::Functions), None, false);
        assert_eq!(left.left, Some(Rect::new(0, 0, 24, 10)));
        assert_eq!(left.editor, Rect::new(24, 0, 56, 10));
        assert_eq!(left.right, None);

        let right = split_editor_band(band(80, 10), None, Some(PanelKind::Variables), false);
        assert_eq!(right.left, None);
        assert_eq!(right.editor, Rect::new(0, 0, 56, 10));
        assert_eq!(right.right, Some(Rect::new(56, 0, 24, 10)));
    }

    #[test]
    fn fit_drops_the_right_panel_before_the_left() {
        let both = Some(PanelKind::Functions);
        let vars = Some(PanelKind::Variables);
        assert_eq!(
            fit_panels(both, vars, 80),
            Fit {
                left: both,
                right: vars,
                notice: None,
            }
        );
        assert_eq!(
            fit_panels(both, vars, 72),
            Fit {
                left: both,
                right: vars,
                notice: None,
            }
        );
        assert_eq!(
            fit_panels(both, vars, 71),
            Fit {
                left: both,
                right: None,
                notice: Some("Window is too narrow for both lists"),
            }
        );
        assert_eq!(
            fit_panels(both, vars, 40),
            Fit {
                left: None,
                right: None,
                notice: Some("Window is too narrow"),
            }
        );
        assert_eq!(
            fit_panels(None, vars, 60),
            Fit {
                left: None,
                right: vars,
                notice: None,
            }
        );
        assert_eq!(
            fit_panels(both, None, 40),
            Fit {
                left: None,
                right: None,
                notice: Some("Window is too narrow"),
            }
        );
    }

    #[test]
    fn project_panel_is_36_columns_and_blocks_a_second_panel_on_80() {
        let split = split_editor_band(band(80, 10), Some(PanelKind::Project), None, false);
        assert_eq!(split.left.unwrap().width, 36);
        assert_eq!(split.editor.x, 36);
        assert_eq!(split.right, None);

        let wide = split_editor_band(
            band(100, 10),
            Some(PanelKind::Project),
            Some(PanelKind::Variables),
            false,
        );
        assert_eq!(wide.left.unwrap().width, 36);
        assert_eq!(wide.right.unwrap().width, 24);
        assert_eq!(wide.editor.width, 40);

        assert_eq!(
            fit_panels(Some(PanelKind::Project), Some(PanelKind::Variables), 80),
            Fit {
                left: Some(PanelKind::Project),
                right: None,
                notice: Some("Window is too narrow for both panels"),
            }
        );
        assert_eq!(
            fit_panels(Some(PanelKind::Functions), Some(PanelKind::Project), 80),
            Fit {
                left: None,
                right: Some(PanelKind::Project),
                notice: Some("Window is too narrow for both panels"),
            }
        );
        assert_eq!(
            fit_panels(Some(PanelKind::Project), None, 59),
            Fit {
                left: None,
                right: None,
                notice: Some("Window is too narrow"),
            }
        );
    }

    #[test]
    fn left_geom_puts_the_separator_on_the_inner_edge() {
        let outer = Rect::new(0, 1, PANEL_COLS, 8);
        let geom = panel_geom(outer, Side::Left);
        assert_eq!(geom.separator_x, 23);
        assert_eq!(geom.content, Rect::new(0, 1, 22, 8));
        assert_eq!(geom.header, Rect::new(0, 1, 22, 1));
        assert_eq!(geom.list, Rect::new(0, 2, 22, 7));
        assert_eq!(geom.scrollbar, Rect::new(22, 2, 1, 7));
        assert_eq!(geom.move_button, (16, 19));
        assert_eq!(geom.close_button, (19, 22));
        let label = header_label("Functions", Side::Left, 22);
        assert_eq!(label.chars().count(), 22);
        assert!(label.ends_with("[>][x]"), "{label}");
        assert!(label.starts_with("Functions"), "{label}");
    }

    #[test]
    fn right_geom_puts_the_separator_on_the_outer_edge() {
        let outer = Rect::new(56, 1, PANEL_COLS, 8);
        let geom = panel_geom(outer, Side::Right);
        assert_eq!(geom.separator_x, 56);
        assert_eq!(geom.content, Rect::new(57, 1, 22, 8));
        assert_eq!(geom.scrollbar, Rect::new(79, 2, 1, 7));
        assert_eq!(geom.move_button, (73, 76));
        assert_eq!(geom.close_button, (76, 79));
        let label = header_label("Variables", Side::Right, 22);
        assert!(label.ends_with("[<][x]"), "{label}");
        assert_eq!(hit_test(geom, 56, 2), PanelHit::Miss);
        assert_eq!(hit_test(geom, 74, 1), PanelHit::Move);
        assert_eq!(hit_test(geom, 76, 1), PanelHit::Close);
        assert_eq!(hit_test(geom, 57, 1), PanelHit::Header);
        assert_eq!(hit_test(geom, 57, 3), PanelHit::Row(1));
        assert_eq!(hit_test(geom, 79, 4), PanelHit::Scrollbar);
        assert_eq!(hit_test(geom, 10, 3), PanelHit::Miss);
    }
}
