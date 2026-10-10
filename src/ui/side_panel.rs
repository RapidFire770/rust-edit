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

/// Narrowest panel that still shows the move button, the close button,
/// and a few letters of the title. Includes the scrollbar and the `│`.
pub const MIN_PANEL_COLS: u16 = 12;

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

/// Session width of each panel kind. Defaults match [`panel_cols`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelWidths {
    pub functions: u16,
    pub variables: u16,
    pub project: u16,
}

impl Default for PanelWidths {
    fn default() -> Self {
        Self {
            functions: panel_cols(PanelKind::Functions),
            variables: panel_cols(PanelKind::Variables),
            project: panel_cols(PanelKind::Project),
        }
    }
}

impl PanelWidths {
    /// Floor used when a window shrinks. Opening a panel still uses [`Default`].
    pub fn minimum() -> Self {
        Self {
            functions: MIN_PANEL_COLS,
            variables: MIN_PANEL_COLS,
            project: MIN_PANEL_COLS,
        }
    }

    pub fn get(self, kind: PanelKind) -> u16 {
        match kind {
            PanelKind::Functions => self.functions,
            PanelKind::Variables => self.variables,
            PanelKind::Project => self.project,
        }
    }

    pub fn set(&mut self, kind: PanelKind, cols: u16) {
        match kind {
            PanelKind::Functions => self.functions = cols,
            PanelKind::Variables => self.variables = cols,
            PanelKind::Project => self.project = cols,
        }
    }
}

/// `room` is the widest this panel may be while the editor keeps
/// [`MIN_EDITOR_COLS`] and the other panel keeps the width it is drawing.
/// `None` means there is not enough room for [`MIN_PANEL_COLS`].
pub fn step_panel_cols(current: u16, delta: i16, room: u16) -> Option<u16> {
    if room < MIN_PANEL_COLS {
        return None;
    }
    let next = if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(u16::try_from(delta).unwrap_or(0))
    };
    Some(next.clamp(MIN_PANEL_COLS, room))
}

pub fn clamp_panel_cols(width: u16, room: u16) -> Option<u16> {
    step_panel_cols(width, 0, room)
}

/// Left: the pointer column becomes the panel's last column.
/// Right: the panel's right edge stays put and the pointer becomes its first column.
pub fn width_at_pointer(side: Side, area: Rect, mouse_x: u16) -> u16 {
    match side {
        Side::Left => mouse_x.saturating_sub(area.x).saturating_add(1),
        Side::Right => area.x.saturating_add(area.width).saturating_sub(mouse_x),
    }
}

/// Slots after a width check. `notice` is set only when a panel was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    pub left: Option<PanelKind>,
    pub right: Option<PanelKind>,
    pub notice: Option<&'static str>,
}

fn panels_need(left: Option<PanelKind>, right: Option<PanelKind>, widths: PanelWidths) -> u16 {
    let cols = |kind| widths.get(kind);
    left.map(cols).unwrap_or(0) + right.map(cols).unwrap_or(0) + MIN_EDITOR_COLS
}

/// Drop a panel when the editor would be narrower than [`MIN_EDITOR_COLS`].
/// Two symbol lists drop the right one. A project panel is kept and the
/// other side is dropped, because the project tree is the wider slot.
/// `widths` is the size used for the check: saved widths on open, the
/// 12-column floor while laying out a window that got smaller.
pub fn fit_panels(
    left: Option<PanelKind>,
    right: Option<PanelKind>,
    width: u16,
    widths: PanelWidths,
) -> Fit {
    let mut left = left;
    let mut right = right;
    let mut notice = None;
    if left.is_some() && right.is_some() && width < panels_need(left, right, widths) {
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
    if (left.is_some() || right.is_some()) && width < panels_need(left, right, widths) {
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

/// Columns to draw. When the saved widths overflow, shrink the wider panel
/// first. When they match, shrink the right panel first. Never go below
/// [`MIN_PANEL_COLS`]. This does not write the saved widths.
pub fn painted_widths(
    left: Option<PanelKind>,
    right: Option<PanelKind>,
    widths: PanelWidths,
    term: u16,
) -> (Option<u16>, Option<u16>) {
    let budget = term.saturating_sub(MIN_EDITOR_COLS);
    let mut left_w = left.map(|kind| widths.get(kind).max(MIN_PANEL_COLS));
    let mut right_w = right.map(|kind| widths.get(kind).max(MIN_PANEL_COLS));
    let mut overflow = left_w
        .unwrap_or(0)
        .saturating_add(right_w.unwrap_or(0))
        .saturating_sub(budget);
    if left_w.unwrap_or(0) > right_w.unwrap_or(0) {
        shrink_slot(&mut left_w, &mut overflow);
        shrink_slot(&mut right_w, &mut overflow);
    } else {
        shrink_slot(&mut right_w, &mut overflow);
        shrink_slot(&mut left_w, &mut overflow);
    }
    (left_w, right_w)
}

fn shrink_slot(slot: &mut Option<u16>, overflow: &mut u16) {
    if let Some(cols) = slot.as_mut() {
        let take = cols.saturating_sub(MIN_PANEL_COLS).min(*overflow);
        *cols -= take;
        *overflow -= take;
    }
}

/// Split the editor band (below the header, above the status bar).
/// The find bar stays full width. Panels occupy only the rows beside the editor.
/// `left_cols` and `right_cols` are the painted widths, including the `│`.
pub fn split_editor_band(
    area: Rect,
    left_cols: Option<u16>,
    right_cols: Option<u16>,
    find_open: bool,
) -> SideSplit {
    let (body, find) = if find_open {
        let (body, find_area) = super::layout::editor_with_find_layout(area);
        (body, Some(find_area))
    } else {
        (area, None)
    };

    let mut constraints = Vec::with_capacity(3);
    if let Some(cols) = left_cols {
        constraints.push(Constraint::Length(cols));
    }
    constraints.push(Constraint::Min(MIN_EDITOR_COLS));
    if let Some(cols) = right_cols {
        constraints.push(Constraint::Length(cols));
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(body);

    let mut index = 0;
    let left = if left_cols.is_some() {
        let rect = chunks[index];
        index += 1;
        Some(rect)
    } else {
        None
    };
    let editor = chunks[index];
    let right = if right_cols.is_some() {
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
    /// The inner `│`. A drag on this column resizes the panel.
    Resize,
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
        return PanelHit::Resize;
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
    /// Set by the wheel and the scrollbar. Paint must not pull `scroll`
    /// back onto `selected` while this is set.
    pub free_scroll: bool,
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

pub fn clamp_scroll(scroll: usize, len: usize, viewport: usize) -> usize {
    if len == 0 || viewport == 0 {
        return 0;
    }
    scroll.min(len.saturating_sub(viewport))
}

/// Clamp the highlight into the list. A free scroll stays where the user
/// put it. Otherwise the highlight is brought on screen.
pub fn settle(nav: &mut PanelNav, len: usize, viewport: usize) {
    if len == 0 || viewport == 0 {
        nav.selected = 0;
        nav.scroll = 0;
        return;
    }
    nav.selected = nav.selected.min(len - 1);
    if nav.free_scroll {
        nav.scroll = clamp_scroll(nav.scroll, len, viewport);
    } else {
        reveal(nav.selected, &mut nav.scroll, viewport);
        nav.scroll = clamp_scroll(nav.scroll, len, viewport);
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
pub(crate) fn panel_header_style(theme: &Theme, focused: bool) -> Style {
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
        let start = clamp_scroll(nav.scroll, rows.len(), viewport.max(1));
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
        let split = split_editor_band(band(80, 10), Some(PANEL_COLS), Some(PANEL_COLS), false);
        assert_eq!(split.left, Some(Rect::new(0, 0, 24, 10)));
        assert_eq!(split.editor, Rect::new(24, 0, 32, 10));
        assert_eq!(split.right, Some(Rect::new(56, 0, 24, 10)));
        assert_eq!(split.find, None);
    }

    #[test]
    fn find_bar_stays_full_width_under_the_panels() {
        let split = split_editor_band(band(80, 10), Some(PANEL_COLS), Some(PANEL_COLS), true);
        assert_eq!(split.find, Some(Rect::new(0, 9, 80, 1)));
        assert_eq!(split.left.unwrap().height, 9);
        assert_eq!(split.editor, Rect::new(24, 0, 32, 9));
        assert_eq!(split.right.unwrap().height, 9);
    }

    #[test]
    fn one_panel_takes_its_side_only() {
        let left = split_editor_band(band(80, 10), Some(PANEL_COLS), None, false);
        assert_eq!(left.left, Some(Rect::new(0, 0, 24, 10)));
        assert_eq!(left.editor, Rect::new(24, 0, 56, 10));
        assert_eq!(left.right, None);

        let right = split_editor_band(band(80, 10), None, Some(PANEL_COLS), false);
        assert_eq!(right.left, None);
        assert_eq!(right.editor, Rect::new(0, 0, 56, 10));
        assert_eq!(right.right, Some(Rect::new(56, 0, 24, 10)));
    }

    #[test]
    fn fit_drops_the_right_panel_before_the_left() {
        let both = Some(PanelKind::Functions);
        let vars = Some(PanelKind::Variables);
        let widths = PanelWidths::default();
        assert_eq!(
            fit_panels(both, vars, 80, widths),
            Fit {
                left: both,
                right: vars,
                notice: None,
            }
        );
        assert_eq!(
            fit_panels(both, vars, 72, widths),
            Fit {
                left: both,
                right: vars,
                notice: None,
            }
        );
        assert_eq!(
            fit_panels(both, vars, 71, widths),
            Fit {
                left: both,
                right: None,
                notice: Some("Window is too narrow for both lists"),
            }
        );
        assert_eq!(
            fit_panels(both, vars, 40, widths),
            Fit {
                left: None,
                right: None,
                notice: Some("Window is too narrow"),
            }
        );
        assert_eq!(
            fit_panels(None, vars, 60, widths),
            Fit {
                left: None,
                right: vars,
                notice: None,
            }
        );
        assert_eq!(
            fit_panels(both, None, 40, widths),
            Fit {
                left: None,
                right: None,
                notice: Some("Window is too narrow"),
            }
        );
    }

    #[test]
    fn project_panel_is_36_columns_and_blocks_a_second_panel_on_80() {
        let split = split_editor_band(band(80, 10), Some(PROJECT_PANEL_COLS), None, false);
        assert_eq!(split.left.unwrap().width, 36);
        assert_eq!(split.editor.x, 36);
        assert_eq!(split.right, None);

        let wide = split_editor_band(
            band(100, 10),
            Some(PROJECT_PANEL_COLS),
            Some(PANEL_COLS),
            false,
        );
        assert_eq!(wide.left.unwrap().width, 36);
        assert_eq!(wide.right.unwrap().width, 24);
        assert_eq!(wide.editor.width, 40);

        let widths = PanelWidths::default();
        assert_eq!(
            fit_panels(
                Some(PanelKind::Project),
                Some(PanelKind::Variables),
                80,
                widths
            ),
            Fit {
                left: Some(PanelKind::Project),
                right: None,
                notice: Some("Window is too narrow for both panels"),
            }
        );
        assert_eq!(
            fit_panels(
                Some(PanelKind::Functions),
                Some(PanelKind::Project),
                80,
                widths
            ),
            Fit {
                left: None,
                right: Some(PanelKind::Project),
                notice: Some("Window is too narrow for both panels"),
            }
        );
        assert_eq!(
            fit_panels(Some(PanelKind::Project), None, 59, widths),
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
        assert_eq!(hit_test(geom, 23, 2), PanelHit::Resize);
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
        assert_eq!(hit_test(geom, 56, 2), PanelHit::Resize);
        assert_eq!(hit_test(geom, 74, 1), PanelHit::Move);
        assert_eq!(hit_test(geom, 76, 1), PanelHit::Close);
        assert_eq!(hit_test(geom, 57, 1), PanelHit::Header);
        assert_eq!(hit_test(geom, 57, 3), PanelHit::Row(1));
        assert_eq!(hit_test(geom, 79, 4), PanelHit::Scrollbar);
        assert_eq!(hit_test(geom, 10, 3), PanelHit::Miss);
    }

    #[test]
    fn step_stops_at_12_and_at_the_room_left_for_the_editor() {
        assert_eq!(step_panel_cols(24, -1, 56), Some(23));
        assert_eq!(step_panel_cols(12, -1, 56), Some(12));
        assert_eq!(step_panel_cols(24, 1, 24), Some(24));
        assert_eq!(step_panel_cols(24, 1, 30), Some(25));
        assert_eq!(step_panel_cols(24, 1, 11), None);
    }

    #[test]
    fn pointer_width_uses_the_fixed_outer_edge() {
        let left = Rect::new(0, 1, 24, 8);
        assert_eq!(width_at_pointer(Side::Left, left, 23), 24);
        assert_eq!(width_at_pointer(Side::Left, left, 40), 41);
        let right = Rect::new(56, 1, 24, 8);
        assert_eq!(width_at_pointer(Side::Right, right, 56), 24);
        assert_eq!(width_at_pointer(Side::Right, right, 50), 30);
    }

    #[test]
    fn saved_project_width_of_20_fits_beside_a_symbol_list_on_80() {
        let widths = PanelWidths {
            project: 20,
            ..PanelWidths::default()
        };
        assert_eq!(
            fit_panels(
                Some(PanelKind::Project),
                Some(PanelKind::Variables),
                80,
                widths
            ),
            Fit {
                left: Some(PanelKind::Project),
                right: Some(PanelKind::Variables),
                notice: None,
            }
        );
    }

    #[test]
    fn minimum_widths_keep_both_panels_on_80() {
        let mins = PanelWidths::minimum();
        assert_eq!(
            fit_panels(
                Some(PanelKind::Project),
                Some(PanelKind::Variables),
                80,
                mins
            )
            .right,
            Some(PanelKind::Variables)
        );
        assert_eq!(
            fit_panels(Some(PanelKind::Functions), None, 40, mins).left,
            Some(PanelKind::Functions)
        );
        assert!(fit_panels(Some(PanelKind::Functions), None, 35, mins)
            .left
            .is_none());
    }

    #[test]
    fn painted_widths_shrink_the_wider_panel_and_remember_nothing() {
        let widths = PanelWidths {
            project: 36,
            variables: 24,
            ..PanelWidths::default()
        };
        // Budget on an 80-column screen is 56. 36+24 overflows by 4, so the
        // project draws at 32 and the variable list stays at 24.
        assert_eq!(
            painted_widths(
                Some(PanelKind::Project),
                Some(PanelKind::Variables),
                widths,
                80
            ),
            (Some(32), Some(24))
        );
        assert_eq!(widths.project, 36);
    }

    #[test]
    fn split_uses_the_painted_column_counts() {
        let split = split_editor_band(band(80, 10), Some(30), Some(24), false);
        assert_eq!(split.left.unwrap().width, 30);
        assert_eq!(split.right.unwrap().width, 24);
        assert_eq!(split.editor.width, 26);
    }

    #[test]
    fn clamp_scroll_stops_on_the_last_full_page() {
        assert_eq!(clamp_scroll(0, 40, 10), 0);
        assert_eq!(clamp_scroll(100, 40, 10), 30);
        assert_eq!(clamp_scroll(5, 4, 10), 0);
        assert_eq!(clamp_scroll(3, 0, 10), 0);
    }

    #[test]
    fn settle_keeps_a_free_scroll_away_from_the_highlight() {
        let mut nav = PanelNav {
            selected: 0,
            scroll: 24,
            free_scroll: true,
        };
        settle(&mut nav, 40, 10);
        assert_eq!(nav.scroll, 24);
        assert_eq!(nav.selected, 0);

        nav.scroll = 100;
        settle(&mut nav, 40, 10);
        assert_eq!(nav.scroll, 30);
    }

    #[test]
    fn settle_without_free_scroll_pins_the_highlight() {
        let mut nav = PanelNav {
            selected: 0,
            scroll: 24,
            free_scroll: false,
        };
        settle(&mut nav, 40, 10);
        assert_eq!(nav.scroll, 0);

        nav.selected = 39;
        nav.scroll = 0;
        settle(&mut nav, 40, 10);
        assert_eq!(nav.scroll, 30);
    }
}
