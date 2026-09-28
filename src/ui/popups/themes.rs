// "Themes" popup — preset list + custom-theme editor.
//
// Two modes (selected via `ThemesMode` on `App`):
//   * List  — built-ins then customs, click/enter to apply, e/n/d to
//             edit/new/delete.
//   * Edit  — name field + scrolled color-role list (overflow bar) + live
//             mini-chrome preview (header/tabs/gutter/selection/caret/find/
//             status); Tab cycles Name/Role/Palette/sliders/Hex; Enter saves.
//
// State lives on `App` (state-on-App convention from confirm/file_browser);
// this module only renders + exposes geometry helpers the mouse handler reuses.

#![allow(dead_code)]

use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::{ColorDef, Theme};
use crate::ui::color_picker::{
    adjust_channel, premade, swatch_rect, to_rgb, Channel, SLIDER_BAR_W, SWATCH_COLS, SWATCH_ROWS,
};
use crate::ui::scrollbar::{self, OverflowBar};
use crate::App;

/// Which screen of the Themes popup is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemesMode {
    #[default]
    List,
    Edit,
}

/// Which input the editor is currently typing into. Tab cycles forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditFocus {
    #[default]
    Name,
    Role,
    Palette,
    SliderR,
    SliderG,
    SliderB,
    Hex, // was Color
}

impl EditFocus {
    pub fn is_text(self) -> bool {
        matches!(self, EditFocus::Name | EditFocus::Hex)
    }
}

/// Result of a single editor-mode keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemesEditCmd {
    None,
    Save,
    Cancel,
}

/// Editor-mode state. Owned by `App.themes_edit`; `None` while in list mode.
#[derive(Debug, Clone)]
pub struct ThemeEditState {
    /// Name buffer (user may retype).
    pub name: String,
    /// The in-progress theme. Starts as a copy of the source (built-in copy or
    /// the custom being edited) and is mutated as the user changes roles.
    pub theme: Theme,
    /// Which role index is selected in the right-hand list.
    pub role_selected: usize,
    /// Role list vertical scroll (when the list overflows the popup).
    pub role_scroll: u16,
    /// Which input field currently receives keystrokes.
    pub focus: EditFocus,
    /// Selected premade swatch (0..24).
    pub palette_index: usize,
    /// Color-value text buffer (e.g. `#ff8800`). Validated on each keystroke.
    pub color_buf: String,
    /// Last validation error for `color_buf` (shown inline, dim red).
    pub color_err: Option<String>,
    /// True if the user has typed since entering editor mode (for unsaved guard).
    pub dirty: bool,
}

impl ThemeEditState {
    /// Build editor state from a source theme (a copy the user will mutate).
    /// `name_override` lets "New from built-in" seed a fresh empty name.
    pub fn new(source: Theme, name_override: Option<String>) -> Self {
        let role = 0;
        let theme = source.clone();
        let name = name_override.unwrap_or_else(|| source.name.clone());
        let initial_color = color_buf_for(&field_value(&theme, role));
        Self {
            name,
            theme,
            role_selected: role,
            role_scroll: 0,
            focus: EditFocus::Name,
            palette_index: 0,
            color_buf: initial_color,
            color_err: None,
            dirty: false,
        }
    }

    /// Move focus to the next field, wrapping.
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            EditFocus::Name => EditFocus::Role,
            EditFocus::Role => EditFocus::Palette,
            EditFocus::Palette => EditFocus::SliderR,
            EditFocus::SliderR => EditFocus::SliderG,
            EditFocus::SliderG => EditFocus::SliderB,
            EditFocus::SliderB => EditFocus::Hex,
            EditFocus::Hex => EditFocus::Name,
        };
    }

    /// Apply the current color buffer to the selected role if it parses.
    /// Returns true on success.
    pub fn commit_color(&mut self) -> bool {
        match ColorDef::parse_input(&self.color_buf) {
            Ok(c) => {
                set_field_value(&mut self.theme, self.role_selected, c);
                self.color_err = None;
                self.dirty = true;
                true
            }
            Err(e) => {
                self.color_err = Some(e);
                false
            }
        }
    }

    /// Move role selection by `delta` (wraps), reload the color buffer from the
    /// newly-selected role, and clear any pending error.
    pub fn move_role(&mut self, delta: i32) {
        let n = ROLE_FIELDS.len() as i32;
        let new = ((self.role_selected as i32 + delta) % n + n) % n;
        self.role_selected = new as usize;
        self.color_buf = color_buf_for(&field_value(&self.theme, self.role_selected));
        self.color_err = None;
        if self.role_selected < self.role_scroll as usize {
            self.role_scroll = self.role_selected as u16;
        }
        if self.role_selected >= self.role_scroll as usize + ROLE_VISIBLE as usize {
            self.role_scroll = (self.role_selected + 1 - ROLE_VISIBLE as usize) as u16;
        }
    }

    /// Scroll the role window without changing the selected role (mouse wheel).
    pub fn scroll_roles(&mut self, delta: i32) {
        let max = (ROLE_FIELDS.len() as u16).saturating_sub(ROLE_VISIBLE);
        let next = (i32::from(self.role_scroll) + delta).clamp(0, i32::from(max));
        self.role_scroll = next as u16;
    }

    /// Append a char to whichever field has focus.
    pub fn type_char(&mut self, c: char) {
        match self.focus {
            EditFocus::Name => {
                // Disallow whitespace-only and characters that would break the
                // on-disk filename. We sanitize on save too, but block early.
                if c.is_control() {
                    return;
                }
                self.name.push(c);
                self.dirty = true;
            }
            EditFocus::Hex => {
                self.color_buf.push(c);
                self.dirty = true;
                // Live-preview: try to apply immediately so the preview pane
                // updates as the user types a valid hex/name. Failures just
                // leave the buffer unchanged in the preview.
                if let Ok(c) = ColorDef::parse_input(&self.color_buf) {
                    set_field_value(&mut self.theme, self.role_selected, c);
                    self.color_err = None;
                }
            }
            EditFocus::Role
            | EditFocus::Palette
            | EditFocus::SliderR
            | EditFocus::SliderG
            | EditFocus::SliderB => {
                // Non-text targets don't accept typed characters.
            }
        }
    }

    pub fn backspace(&mut self) {
        match self.focus {
            EditFocus::Name => {
                self.name.pop();
                self.dirty = true;
            }
            EditFocus::Hex => {
                self.color_buf.pop();
                self.dirty = true;
                // Refresh preview from the buffer's last valid prefix.
                if let Ok(c) = ColorDef::parse_input(&self.color_buf) {
                    set_field_value(&mut self.theme, self.role_selected, c);
                    self.color_err = None;
                }
            }
            EditFocus::Role
            | EditFocus::Palette
            | EditFocus::SliderR
            | EditFocus::SliderG
            | EditFocus::SliderB => {}
        }
    }

    /// Apply `color` to the selected role and refresh `color_buf`.
    fn apply_color(&mut self, color: ColorDef) {
        if self.role_selected >= ROLE_FIELDS.len() {
            return;
        }
        set_field_value(&mut self.theme, self.role_selected, color);
        self.color_buf = color_buf_for(&field_value(&self.theme, self.role_selected));
        self.color_err = None;
        self.dirty = true;
    }

    fn apply_palette_swatch(&mut self) {
        let swatches = premade();
        let Some(swatch) = swatches.get(self.palette_index) else {
            return;
        };
        self.apply_color(swatch.color.clone());
    }

    /// Select premade swatch `i` and apply it to the current role.
    pub fn apply_swatch_at(&mut self, i: usize) {
        let swatches = premade();
        let Some(swatch) = swatches.get(i) else {
            return;
        };
        self.palette_index = i;
        self.apply_color(swatch.color.clone());
    }

    /// Set one RGB channel of the selected role, storing the result as `ColorDef::Rgb`.
    pub fn set_channel_value(&mut self, ch: Channel, value: u8) {
        let current = field_value(&self.theme, self.role_selected);
        let (r, g, b) = to_rgb(&current);
        let next = match ch {
            Channel::R => ColorDef::Rgb(value, g, b),
            Channel::G => ColorDef::Rgb(r, value, b),
            Channel::B => ColorDef::Rgb(r, g, value),
        };
        self.apply_color(next);
    }

    fn move_focus_vert(&mut self, dir: i32) {
        match self.focus {
            EditFocus::Role => self.move_role(dir),
            EditFocus::Palette => {
                let n = premade().len();
                self.palette_index = wrap_index(
                    self.palette_index,
                    dir.saturating_mul(i32::from(SWATCH_COLS)),
                    n,
                );
                self.apply_palette_swatch();
            }
            EditFocus::SliderR | EditFocus::SliderG | EditFocus::SliderB => {
                self.focus = cycle_slider(self.focus, dir);
            }
            _ => {}
        }
    }

    fn move_focus_horiz(&mut self, dir: i32) {
        match self.focus {
            EditFocus::Palette => {
                let n = premade().len();
                self.palette_index = wrap_index(self.palette_index, dir, n);
                self.apply_palette_swatch();
            }
            EditFocus::SliderR | EditFocus::SliderG | EditFocus::SliderB => {
                let ch = match self.focus {
                    EditFocus::SliderR => Channel::R,
                    EditFocus::SliderG => Channel::G,
                    EditFocus::SliderB => Channel::B,
                    _ => return,
                };
                let current = field_value(&self.theme, self.role_selected);
                let next = adjust_channel(&current, ch, (dir * 8) as i16);
                self.apply_color(next);
            }
            _ => {}
        }
    }
}

fn wrap_index(idx: usize, delta: i32, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let n = n as i32;
    (idx as i32 + delta).rem_euclid(n) as usize
}

fn cycle_slider(focus: EditFocus, dir: i32) -> EditFocus {
    match (focus, dir.signum()) {
        (EditFocus::SliderR, 1) => EditFocus::SliderG,
        (EditFocus::SliderG, 1) => EditFocus::SliderB,
        (EditFocus::SliderB, 1) => EditFocus::SliderR,
        (EditFocus::SliderR, -1) => EditFocus::SliderB,
        (EditFocus::SliderG, -1) => EditFocus::SliderR,
        (EditFocus::SliderB, -1) => EditFocus::SliderG,
        (other, _) => other,
    }
}

/// Route an editor-mode keystroke. List-mode shortcuts must not be handled here.
pub fn handle_edit_key(edit: &mut ThemeEditState, code: KeyCode) -> ThemesEditCmd {
    match code {
        KeyCode::Esc => ThemesEditCmd::Cancel,
        KeyCode::Enter => {
            let _ = edit.commit_color();
            ThemesEditCmd::Save
        }
        KeyCode::Tab => {
            edit.cycle_focus();
            ThemesEditCmd::None
        }
        KeyCode::Backspace => {
            edit.backspace();
            ThemesEditCmd::None
        }
        KeyCode::Up | KeyCode::Char('k') if !edit.focus.is_text() => {
            edit.move_focus_vert(-1);
            ThemesEditCmd::None
        }
        KeyCode::Down | KeyCode::Char('j') if !edit.focus.is_text() => {
            edit.move_focus_vert(1);
            ThemesEditCmd::None
        }
        KeyCode::Left if !edit.focus.is_text() => {
            edit.move_focus_horiz(-1);
            ThemesEditCmd::None
        }
        KeyCode::Right if !edit.focus.is_text() => {
            edit.move_focus_horiz(1);
            ThemesEditCmd::None
        }
        KeyCode::Char('s') if !edit.focus.is_text() => ThemesEditCmd::Save,
        KeyCode::Char(c) if edit.focus.is_text() => {
            edit.type_char(c);
            ThemesEditCmd::None
        }
        _ => ThemesEditCmd::None,
    }
}

// ===================== List mode geometry =====================

pub const LIST_POPUP_W: u16 = 70;
/// Generous height so the list scrolls only once several custom themes exist.
pub const LIST_POPUP_H: u16 = 16;

pub fn list_rect(area: Rect) -> Rect {
    let x = (area.width.saturating_sub(LIST_POPUP_W)) / 2;
    let y = (area.height.saturating_sub(LIST_POPUP_H)) / 2;
    Rect::new(
        x,
        y,
        LIST_POPUP_W.min(area.width),
        LIST_POPUP_H.min(area.height),
    )
}

/// Y of the first theme row inside the popup (after border + title + blank).
pub fn list_first_row_y(popup: Rect) -> u16 {
    popup.y + 1 + 1 + 1
}

/// Map a screen row `y` to a theme index in the list, accounting for the
/// "── Custom ──" divider that shifts custom themes down by one row.
/// Returns None when the row is not on a theme (title, blank, divider, or
/// out of range).
pub fn list_row_to_index(
    y: u16,
    popup: Rect,
    built_in_count: usize,
    total: usize,
) -> Option<usize> {
    let first = list_first_row_y(popup);
    let rel = y.saturating_sub(first);
    let idx = if (rel as usize) < built_in_count {
        rel as usize
    } else if (rel as usize) == built_in_count {
        return None; // the divider row
    } else {
        rel as usize - 1 // custom section: subtract the divider offset
    };
    if idx < total {
        Some(idx)
    } else {
        None
    }
}

/// Footer button labels for list mode. Used by render + mouse hit-test.
pub fn list_footer_labels() -> [&'static str; 5] {
    [
        "[ Enter=Apply ]",
        "[ e=Edit ]",
        "[ n=New ]",
        "[ d=Delete ]",
        "[ Esc=Close ]",
    ]
}

pub fn list_footer_y(popup: Rect) -> u16 {
    // The footer is the last content line inside the bordered popup.
    // Paragraph content starts at popup.y + 1 (top border).
    // The padding logic results in the footer rendering at popup.y + H - 2.
    popup.y + LIST_POPUP_H - 2
}

/// Compute the `(start_x, end_x)` screen column range of each footer button,
/// matching the actual rendering in [`render_list`] and [`render_edit`]:
/// 1 leading space, then 2-space gaps between buttons. Used by mouse hover/
/// click hit-testing so the detected ranges agree with what the user sees.
pub fn themes_button_x_ranges(x_start: u16, labels: &[&str]) -> Vec<(u16, u16)> {
    let mut ranges = Vec::with_capacity(labels.len());
    let mut cursor = x_start + 1; // " " leading space before first button
    for (i, label) in labels.iter().enumerate() {
        if i > 0 {
            cursor = cursor.saturating_add(2); // "  " gap between buttons
        }
        let len = label.chars().count() as u16;
        ranges.push((cursor, cursor.saturating_add(len)));
        cursor = cursor.saturating_add(len);
    }
    ranges
}

// ===================== Edit mode geometry =====================

pub const EDIT_POPUP_W: u16 = 76;
pub const EDIT_POPUP_H: u16 = 22;
pub const ROLE_VISIBLE: u16 = 8;

/// Left pane width in inner columns (includes 1 cell for the overflow scrollbar).
const ROLE_INNER_COLS: u16 = 38;

pub fn edit_rect(area: Rect) -> Rect {
    let x = (area.width.saturating_sub(EDIT_POPUP_W)) / 2;
    let y = (area.height.saturating_sub(EDIT_POPUP_H)) / 2;
    Rect::new(
        x,
        y,
        EDIT_POPUP_W.min(area.width),
        EDIT_POPUP_H.min(area.height),
    )
}

fn inner_first_y(popup: Rect) -> u16 {
    popup.y.saturating_add(1)
}

fn inner_last_y(popup: Rect) -> u16 {
    popup.y.saturating_add(popup.height.saturating_sub(2))
}

/// Screen Y of inner content row `n` (0-based, after the top border). Clamped
/// into the inner vertical range so helpers stay monotonic on small terminals.
fn inner_row_y(popup: Rect, n: u16) -> u16 {
    let first = inner_first_y(popup);
    let last = inner_last_y(popup);
    first.saturating_add(n).clamp(first, last.max(first))
}

fn inner_width(popup: Rect) -> u16 {
    popup.width.saturating_sub(2)
}

fn inner_x(popup: Rect) -> u16 {
    popup.x.saturating_add(1)
}

fn clamp_inner_height(popup: Rect, y: u16, want: u16) -> u16 {
    let last = inner_last_y(popup);
    let max_h = last.saturating_sub(y).saturating_add(1);
    want.min(max_h)
}

pub fn name_row_y(popup: Rect) -> u16 {
    inner_row_y(popup, 0)
}

pub fn roles_area(popup: Rect) -> Rect {
    let y = inner_row_y(popup, 3);
    let w = ROLE_INNER_COLS.min(inner_width(popup));
    Rect::new(
        inner_x(popup),
        y,
        w,
        clamp_inner_height(popup, y, ROLE_VISIBLE),
    )
}

pub fn preview_area(popup: Rect) -> Rect {
    let roles = roles_area(popup);
    let x = inner_x(popup).saturating_add(roles.width);
    let inner_end = inner_x(popup).saturating_add(inner_width(popup));
    let w = inner_end.saturating_sub(x);
    Rect::new(x, roles.y, w, roles.height)
}

/// Role labels: `roles_area` minus the overflow-bar column when the list
/// does not fit the viewport.
pub fn roles_list_area(popup: Rect, overflowing: bool) -> Rect {
    let roles = roles_area(popup);
    if overflowing && roles.width > 0 {
        Rect::new(
            roles.x,
            roles.y,
            roles.width.saturating_sub(1),
            roles.height,
        )
    } else {
        roles
    }
}

/// Rightmost column of [`roles_area`] — the overflow scrollbar track.
pub fn roles_scrollbar_rect(popup: Rect) -> Rect {
    let roles = roles_area(popup);
    Rect::new(
        roles.x.saturating_add(roles.width.saturating_sub(1)),
        roles.y,
        if roles.width == 0 { 0 } else { 1 },
        roles.height,
    )
}

pub fn roles_overflow_bar(scroll: u16) -> OverflowBar {
    OverflowBar {
        total: ROLE_FIELDS.len(),
        position: scroll as usize,
        viewport: ROLE_VISIBLE as usize,
    }
}

/// True when `(x, y)` is on the role-list overflow track. Always false when
/// the list fits so the last label column stays clickable.
pub fn roles_click_is_scrollbar(x: usize, y: usize, popup: Rect, overflowing: bool) -> bool {
    if !overflowing {
        return false;
    }
    let sb = roles_scrollbar_rect(popup);
    sb.width > 0
        && x >= sb.x as usize
        && x < (sb.x + sb.width) as usize
        && y >= sb.y as usize
        && y < (sb.y + sb.height) as usize
}

pub fn palette_area(popup: Rect) -> Rect {
    let y = inner_row_y(popup, 13);
    Rect::new(
        inner_x(popup),
        y,
        inner_width(popup),
        clamp_inner_height(popup, y, SWATCH_ROWS),
    )
}

pub fn slider_row_y(popup: Rect, channel: u8) -> u16 {
    inner_row_y(popup, 15 + u16::from(channel.min(2)))
}

/// Columns before the slider bar (`"R ["`). Shared by render and hit-testing.
pub const SLIDER_PREFIX_W: u16 = 3;

/// Screen rect of the 16-cell RGB slider bar for `channel` (0=R, 1=G, 2=B).
/// Starts after [`SLIDER_PREFIX_W`]. Shared by render and mouse.
pub fn slider_bar_area(popup: Rect, channel: u8) -> Rect {
    let y = slider_row_y(popup, channel);
    let x = inner_x(popup).saturating_add(SLIDER_PREFIX_W);
    let inner_end = inner_x(popup).saturating_add(inner_width(popup));
    let max_w = inner_end.saturating_sub(x);
    Rect::new(x, y, SLIDER_BAR_W.min(max_w), 1)
}

pub fn hex_row_y(popup: Rect) -> u16 {
    inner_row_y(popup, 18)
}

pub fn footer_row_y(popup: Rect) -> u16 {
    inner_row_y(popup, 19)
}

/// Footer labels for edit mode.
pub fn edit_footer_labels() -> [&'static str; 2] {
    ["[ Save ]", "[ Cancel ]"]
}

// ===================== Field table =====================

/// The editable color roles, in display order. The index here is what
/// `role_selected` refers to. Name is a separate text field, not a role.
pub const ROLE_FIELDS: &[(&str, &str)] = &[
    ("header_bg", "header_bg"),
    ("header_fg", "header_fg"),
    ("editor_bg", "editor_bg"),
    ("editor_fg", "editor_fg"),
    ("status_bg", "status_bg"),
    ("status_fg", "status_fg"),
    ("line_number_fg", "line_number_fg"),
    ("selection_bg", "selection_bg"),
    ("cursor_fg", "cursor_fg"),
    ("keyword", "keyword"),
    ("string", "string"),
    ("comment", "comment"),
    ("function", "function"),
    ("number", "number"),
    ("type_name", "type_name"),
    ("accent", "accent"),
];

pub fn field_value(theme: &Theme, role_idx: usize) -> ColorDef {
    let (_, accessor) = ROLE_FIELDS[role_idx];
    match accessor {
        "header_bg" => theme.header_bg.clone(),
        "header_fg" => theme.header_fg.clone(),
        "editor_bg" => theme.editor_bg.clone(),
        "editor_fg" => theme.editor_fg.clone(),
        "status_bg" => theme.status_bg.clone(),
        "status_fg" => theme.status_fg.clone(),
        "line_number_fg" => theme.line_number_fg.clone(),
        "selection_bg" => theme.selection_bg.clone(),
        "cursor_fg" => theme.cursor_fg.clone(),
        "keyword" => theme.keyword.clone(),
        "string" => theme.string.clone(),
        "comment" => theme.comment.clone(),
        "function" => theme.function.clone(),
        "number" => theme.number.clone(),
        "type_name" => theme.type_name.clone(),
        "accent" => theme.accent.clone(),
        _ => ColorDef::Rgb(0, 0, 0),
    }
}

fn set_field_value(theme: &mut Theme, role_idx: usize, value: ColorDef) {
    let (_, accessor) = ROLE_FIELDS[role_idx];
    match accessor {
        "header_bg" => theme.header_bg = value,
        "header_fg" => theme.header_fg = value,
        "editor_bg" => theme.editor_bg = value,
        "editor_fg" => theme.editor_fg = value,
        "status_bg" => theme.status_bg = value,
        "status_fg" => theme.status_fg = value,
        "line_number_fg" => theme.line_number_fg = value,
        "selection_bg" => theme.selection_bg = value,
        "cursor_fg" => theme.cursor_fg = value,
        "keyword" => theme.keyword = value,
        "string" => theme.string = value,
        "comment" => theme.comment = value,
        "function" => theme.function = value,
        "number" => theme.number = value,
        "type_name" => theme.type_name = value,
        "accent" => theme.accent = value,
        _ => {}
    }
}

/// Render a `ColorDef` as the text shown in the editor's value column.
pub fn color_buf_for(c: &ColorDef) -> String {
    match c {
        ColorDef::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        ColorDef::Named(n) => n.clone(),
    }
}

// ===================== Rendering =====================

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, app: &App) {
    match app.themes_mode {
        ThemesMode::List => render_list(f, area, theme, app),
        ThemesMode::Edit => render_edit(f, area, theme, app),
    }
}

fn render_list(f: &mut Frame, area: Rect, theme: &Theme, app: &App) {
    let popup = list_rect(area);
    let block = Block::default()
        .title("Themes")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let dim_dim = Style::default()
        .fg(Color::from(&theme.editor_fg))
        .add_modifier(Modifier::DIM);

    let registry = crate::themes::ThemeRegistry::load();
    let built_ins = registry.built_ins();
    let customs = registry.customs();

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled("Pick a theme (live)", dim_dim)));
    lines.push(Line::from(""));

    // Built-ins section.
    for (i, t) in built_ins.iter().enumerate() {
        let active = i == app.themes_selected;
        let hovered = app.themes_list_hover == Some(i);
        let is_current = t.name == app.theme.name;
        push_theme_row(
            &mut lines,
            t.name.as_str(),
            "(built-in)",
            RowFlags {
                active,
                hovered,
                is_current,
            },
            &accent,
            &dim,
        );
    }

    // Divider + customs section.
    if !customs.is_empty() {
        lines.push(Line::from(Span::styled("── Custom ──", dim_dim)));
        for (j, t) in customs.iter().enumerate() {
            let idx = built_ins.len() + j;
            let active = idx == app.themes_selected;
            let hovered = app.themes_list_hover == Some(idx);
            let is_current = t.name == app.theme.name;
            push_theme_row(
                &mut lines,
                t.name.as_str(),
                "(custom)",
                RowFlags {
                    active,
                    hovered,
                    is_current,
                },
                &accent,
                &dim,
            );
        }
    }

    // Pad to push the footer toward the bottom for a stable layout.
    let footer_y = list_footer_y(popup);
    let content_lines_used = lines.len() as u16;
    let target = footer_y.saturating_sub(popup.y + 1);
    while (lines.len() as u16) < target.saturating_sub(1) {
        lines.push(Line::from(""));
        let _ = content_lines_used; // (silence: the loop uses lines.len())
    }

    // Footer.
    let labels = list_footer_labels();
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, l) in labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let hovered = app.themes_button_hover == Some(i);
        // No persistent keyboard selection in list footer — hover drives style.
        let style = if hovered { accent } else { dim };
        spans.push(Span::styled(*l, style));
    }
    lines.push(Line::from(spans));

    let para = Paragraph::new(lines).block(block).style(dim);
    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}

/// Row flags bundled so `push_theme_row` stays under clippy's argument limit.
struct RowFlags {
    active: bool,
    hovered: bool,
    is_current: bool,
}

fn push_theme_row(
    lines: &mut Vec<Line>,
    name: &str,
    tag: &str,
    flags: RowFlags,
    accent: &Style,
    dim: &Style,
) {
    let style = if flags.active || flags.hovered {
        *accent
    } else {
        *dim
    };
    let marker = if flags.active { ">" } else { " " };
    let current = if flags.is_current { " •" } else { "" };
    // Pad name to align the tag column.
    let name_padded = format!("{:<20}", name);
    lines.push(Line::from(vec![
        Span::styled(format!("{marker} "), style),
        Span::styled(name_padded, style),
        Span::styled(format!("{tag}{current}"), *dim),
    ]));
}

fn span_cols(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
}

fn pad_spans(mut spans: Vec<Span>, width: u16) -> Vec<Span> {
    let used = span_cols(&spans);
    if (used as u16) < width {
        spans.push(Span::raw(" ".repeat((width as usize).saturating_sub(used))));
    }
    spans
}

fn clip_spans(spans: Vec<Span>, width: u16) -> Vec<Span> {
    let mut out = Vec::new();
    let mut used = 0usize;
    let max = width as usize;
    for s in spans {
        if used >= max {
            break;
        }
        let n = s.content.chars().count();
        if used + n <= max {
            used += n;
            out.push(s);
        } else {
            let take = max - used;
            let clipped: String = s.content.chars().take(take).collect();
            out.push(Span::styled(clipped, s.style));
            break;
        }
    }
    out
}

fn swatch_style(color: Color, selected: bool) -> Style {
    let mut style = Style::default().bg(color);
    if selected {
        // Keep the premade color visible; reverse would swap it away.
        style = style.add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
    }
    style
}

fn palette_row_spans(popup: Rect, edit: &ThemeEditState, row: u16) -> Vec<Span<'static>> {
    let pal = palette_area(popup);
    let width = pal.width as usize;
    if width == 0 {
        return Vec::new();
    }
    let mut styles = vec![Style::default(); width];
    let swatches = premade();
    let start = usize::from(row).saturating_mul(usize::from(SWATCH_COLS));
    let end = start.saturating_add(usize::from(SWATCH_COLS));
    for i in start..end {
        let Some(r) = swatch_rect(pal, i) else {
            continue;
        };
        let Some(swatch) = swatches.get(i) else {
            continue;
        };
        let selected = edit.focus == EditFocus::Palette && edit.palette_index == i;
        let style = swatch_style(Color::from(&swatch.color), selected);
        let local = r.x.saturating_sub(pal.x) as usize;
        for k in 0..usize::from(r.width) {
            if let Some(slot) = styles.get_mut(local.saturating_add(k)) {
                *slot = style;
            }
        }
    }
    coalesce_style_cells(&styles)
}

fn coalesce_style_cells(styles: &[Style]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut i = 0;
    while i < styles.len() {
        let style = styles[i];
        let mut n = 1;
        while i + n < styles.len() && styles[i + n] == style {
            n += 1;
        }
        spans.push(Span::styled(" ".repeat(n), style));
        i += n;
    }
    spans
}

fn slider_line(
    label: char,
    value: u8,
    focused: bool,
    accent: Style,
    dim: Style,
    dim_dim: Style,
) -> Line<'static> {
    let prefix_style = if focused { accent } else { dim };
    let bar_w = usize::from(SLIDER_BAR_W);
    let thumb_at = if bar_w <= 1 {
        0
    } else {
        (usize::from(value) * (bar_w - 1)) / 255
    };
    let thumb_style = if focused { accent } else { dim };
    let mut spans = clip_spans(
        pad_spans(
            vec![
                Span::styled(format!("{label} "), prefix_style),
                Span::styled("[", dim),
            ],
            SLIDER_PREFIX_W,
        ),
        SLIDER_PREFIX_W,
    );
    for i in 0..bar_w {
        if i == thumb_at {
            spans.push(Span::styled("●", thumb_style));
        } else {
            spans.push(Span::styled("─", dim_dim));
        }
    }
    spans.push(Span::styled("]", dim));
    spans.push(Span::styled(format!(" {value}"), dim));
    Line::from(spans)
}

fn role_label(idx: usize) -> &'static str {
    ROLE_FIELDS.get(idx).map(|(label, _)| *label).unwrap_or("")
}

fn mark_selected(style: Style, selected: bool) -> Style {
    if selected {
        style.add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
    } else {
        style
    }
}

fn pad_spans_style(spans: Vec<Span>, width: u16, style: Style) -> Vec<Span> {
    let used = span_cols(&spans);
    let mut spans = clip_spans(spans, width);
    if (used as u16) < width {
        spans.push(Span::styled(
            " ".repeat((width as usize).saturating_sub(used)),
            style,
        ));
    }
    spans
}

/// Mini editor chrome: header, tabs, gutter+code, find, status. One row per
/// [`ROLE_VISIBLE`] slot so the pane fills [`preview_area`].
fn preview_chrome_lines(theme: &Theme, selected_role: usize, width: u16) -> Vec<Line<'static>> {
    let sel = |name: &str| role_label(selected_role) == name;
    let header = Style::default()
        .bg(Color::from(&theme.header_bg))
        .fg(Color::from(&theme.header_fg))
        .add_modifier(Modifier::BOLD);
    let header_mark = mark_selected(header, sel("header_bg") || sel("header_fg"));
    let accent_on_header = mark_selected(
        Style::default()
            .bg(Color::from(&theme.accent))
            .fg(Color::from(&theme.header_fg))
            .add_modifier(Modifier::BOLD),
        sel("accent"),
    );
    let editor = theme.editor_bg_style();
    let editor_pad = mark_selected(editor, sel("editor_bg"));
    let fg = mark_selected(
        Style::default()
            .fg(Color::from(&theme.editor_fg))
            .bg(Color::from(&theme.editor_bg)),
        sel("editor_fg"),
    );
    let gutter = mark_selected(
        Style::default()
            .fg(Color::from(&theme.line_number_fg))
            .bg(Color::from(&theme.editor_bg)),
        sel("line_number_fg"),
    );
    let kw = mark_selected(
        Style::default()
            .fg(Color::from(&theme.keyword))
            .bg(Color::from(&theme.editor_bg))
            .add_modifier(Modifier::BOLD),
        sel("keyword"),
    );
    let fun = mark_selected(
        Style::default()
            .fg(Color::from(&theme.function))
            .bg(Color::from(&theme.editor_bg)),
        sel("function"),
    );
    let num = mark_selected(
        Style::default()
            .fg(Color::from(&theme.number))
            .bg(Color::from(&theme.editor_bg)),
        sel("number"),
    );
    let string = mark_selected(
        Style::default()
            .fg(Color::from(&theme.string))
            .bg(Color::from(&theme.editor_bg)),
        sel("string"),
    );
    let comment = mark_selected(
        Style::default()
            .fg(Color::from(&theme.comment))
            .bg(Color::from(&theme.editor_bg)),
        sel("comment"),
    );
    let ty = mark_selected(
        Style::default()
            .fg(Color::from(&theme.type_name))
            .bg(Color::from(&theme.editor_bg)),
        sel("type_name"),
    );
    let selection = mark_selected(
        Style::default()
            .fg(Color::from(&theme.editor_fg))
            .bg(Color::from(&theme.selection_bg)),
        sel("selection_bg"),
    );
    let cursor = mark_selected(theme.cursor_style(), sel("cursor_fg"));
    let find_q = mark_selected(
        Style::default()
            .fg(Color::from(&theme.accent))
            .bg(Color::from(&theme.editor_bg))
            .add_modifier(Modifier::BOLD),
        sel("accent"),
    );
    let status = mark_selected(theme.status_style(), sel("status_bg") || sel("status_fg"));

    vec![
        Line::from(pad_spans_style(
            vec![
                Span::styled(" File ", accent_on_header),
                Span::styled("│", header_mark),
                Span::styled(" Tools ", header_mark),
                Span::styled("│", header_mark),
                Span::styled(" Settings ", header_mark),
            ],
            width,
            header_mark,
        )),
        Line::from(pad_spans_style(
            vec![
                Span::styled("[main.rs]", accent_on_header),
                Span::styled(" ", header_mark),
                Span::styled("[lib.rs]", header_mark),
                Span::styled(" ", header_mark),
                Span::styled("[+]", header_mark),
            ],
            width,
            header_mark,
        )),
        Line::from(pad_spans_style(
            vec![
                Span::styled("   1| ", gutter),
                Span::styled("fn ", kw),
                Span::styled("main", fun),
                Span::styled("() {", fg),
            ],
            width,
            editor_pad,
        )),
        Line::from(pad_spans_style(
            vec![
                Span::styled("   2| ", gutter),
                Span::styled("    ", fg),
                Span::styled("let ", kw),
                Span::styled("x", selection),
                Span::styled(" = ", fg),
                Span::styled("42", num),
                Span::styled(";", fg),
            ],
            width,
            editor_pad,
        )),
        Line::from(pad_spans_style(
            vec![
                Span::styled("   3| ", gutter),
                Span::styled("    ", fg),
                Span::styled("let ", kw),
                Span::styled("s", fg),
                Span::styled(" = ", fg),
                Span::styled("\"hi\"", string),
                Span::styled("; ", fg),
                Span::styled("// cmt", comment),
                Span::styled(" ", cursor),
            ],
            width,
            editor_pad,
        )),
        Line::from(pad_spans_style(
            vec![
                Span::styled("   4| ", gutter),
                Span::styled("type ", kw),
                Span::styled("T", ty),
                Span::styled(" = ", fg),
                Span::styled("String", ty),
                Span::styled(";", fg),
            ],
            width,
            editor_pad,
        )),
        Line::from(pad_spans_style(
            vec![
                Span::styled(" Find: ", fg),
                Span::styled("[hi]", find_q),
                Span::styled("  1 match", fg),
            ],
            width,
            editor_pad,
        )),
        Line::from(pad_spans_style(
            vec![Span::styled(" main.rs  |  Rust  |  Ln 4", status)],
            width,
            status,
        )),
    ]
}

fn render_edit(f: &mut Frame, area: Rect, theme: &Theme, app: &App) {
    let edit = match &app.themes_edit {
        Some(e) => e,
        None => return, // defensive: should always be set in Edit mode
    };
    let popup = edit_rect(area);
    let roles = roles_area(popup);
    let preview = preview_area(popup);
    let block = Block::default()
        .title(format!("Edit Theme: {}", edit.name))
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let dim_dim = Style::default()
        .fg(Color::from(&theme.editor_fg))
        .add_modifier(Modifier::DIM);
    let err_style = Style::default().fg(Color::from(&theme.keyword));

    let name_style = if edit.focus == EditFocus::Name {
        accent
    } else {
        dim
    };
    let name_line = Line::from(vec![
        Span::styled("Name: ", dim),
        Span::styled("[", name_style),
        Span::styled(format!("{:<16}", edit.name), name_style),
        Span::styled("]", name_style),
    ]);

    let overflowing = roles_overflow_bar(edit.role_scroll).overflowing();
    let list = roles_list_area(popup, overflowing);
    let mut header_spans = pad_spans(vec![Span::styled("Roles", dim_dim)], roles.width);
    header_spans.push(Span::styled("Preview", dim_dim));
    let header_line = Line::from(header_spans);

    let start = edit.role_scroll as usize;
    let vis = ROLE_VISIBLE as usize;
    let mut role_lines: Vec<Line> = Vec::with_capacity(vis);
    for row in 0..vis {
        let idx = start + row;
        let mut left = if let Some((label, _)) = ROLE_FIELDS.get(idx) {
            let active = idx == edit.role_selected;
            let style = if active { accent } else { dim };
            let marker = if active { ">" } else { " " };
            let label_padded = format!("{:<14}", label);
            let value = field_value(&edit.theme, idx);
            let value_str = color_buf_for(&value);
            vec![
                Span::styled(format!("{marker} "), style),
                Span::styled(label_padded, style),
                Span::raw(" "),
                Span::styled("  ", Style::default().bg(Color::from(&value))),
                Span::styled(format!(" {value_str}"), dim),
            ]
        } else {
            Vec::new()
        };
        left = pad_spans(left, list.width);
        role_lines.push(Line::from(left));
    }

    let palette_style = if edit.focus == EditFocus::Palette {
        accent
    } else {
        dim
    };
    let palette_line = Line::from(Span::styled("Palette", palette_style));
    let (sr, sg, sb) = to_rgb(&field_value(&edit.theme, edit.role_selected));
    let r_line = slider_line(
        'R',
        sr,
        edit.focus == EditFocus::SliderR,
        accent,
        dim,
        dim_dim,
    );
    let g_line = slider_line(
        'G',
        sg,
        edit.focus == EditFocus::SliderG,
        accent,
        dim,
        dim_dim,
    );
    let b_line = slider_line(
        'B',
        sb,
        edit.focus == EditFocus::SliderB,
        accent,
        dim,
        dim_dim,
    );

    let hex_style = if edit.focus == EditFocus::Hex {
        accent
    } else {
        dim
    };
    let selected_label = ROLE_FIELDS
        .get(edit.role_selected)
        .map(|(l, _)| *l)
        .unwrap_or("");
    let err_span = match &edit.color_err {
        Some(e) => Span::styled(format!("  ! {e}"), err_style),
        None => Span::raw(""),
    };
    let hex_line = Line::from(vec![
        Span::styled("Hex: ", dim),
        Span::styled("[", hex_style),
        Span::styled(format!("{:<16}", edit.color_buf), hex_style),
        Span::styled("]", hex_style),
        Span::styled(format!("  ({selected_label})"), dim_dim),
        err_span,
    ]);

    let labels = edit_footer_labels();
    let mut footer_spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, l) in labels.iter().enumerate() {
        if i > 0 {
            footer_spans.push(Span::raw("  "));
        }
        let hovered = app.themes_button_hover == Some(i);
        let style = if hovered { accent } else { dim };
        footer_spans.push(Span::styled(*l, style));
    }
    let footer_line = Line::from(footer_spans);

    // Exactly 20 inner lines matching name_row_y / roles_area / hex / footer.
    let mut lines: Vec<Line> = Vec::with_capacity(20);
    lines.push(name_line); // n=0
    lines.push(Line::from("")); // n=1
    lines.push(header_line); // n=2
    lines.extend(role_lines); // n=3..10
    lines.push(Line::from("")); // n=11
    lines.push(palette_line); // n=12
    lines.push(Line::from(palette_row_spans(popup, edit, 0))); // n=13
    lines.push(Line::from(palette_row_spans(popup, edit, 1))); // n=14
    lines.push(r_line); // n=15
    lines.push(g_line); // n=16
    lines.push(b_line); // n=17
    lines.push(hex_line); // n=18
    lines.push(footer_line); // n=19

    let para = Paragraph::new(lines).block(block).style(dim);
    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);

    let bar = roles_overflow_bar(edit.role_scroll);
    scrollbar::render(f, roles_scrollbar_rect(popup), bar, theme);
    if preview.width > 0 && preview.height > 0 {
        let preview_lines = preview_chrome_lines(&edit.theme, edit.role_selected, preview.width);
        f.render_widget(
            Paragraph::new(preview_lines).style(edit.theme.editor_bg_style()),
            preview,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_row_to_index_accounts_for_custom_divider() {
        // popup.y = 0 → list_first_row_y = 3.
        let popup = Rect::new(0, 0, LIST_POPUP_W, LIST_POPUP_H);
        let built_in_count = 6;
        let total = 8; // 6 built-ins + 2 customs

        // Built-ins: index == rel (rows 3..8).
        assert_eq!(list_row_to_index(3, popup, built_in_count, total), Some(0));
        assert_eq!(list_row_to_index(8, popup, built_in_count, total), Some(5));

        // Divider row (9): no theme.
        assert_eq!(list_row_to_index(9, popup, built_in_count, total), None);

        // Customs: index == rel - 1 (rows 10..11).
        assert_eq!(list_row_to_index(10, popup, built_in_count, total), Some(6));
        assert_eq!(list_row_to_index(11, popup, built_in_count, total), Some(7));

        // Out of range below the list.
        assert_eq!(list_row_to_index(12, popup, built_in_count, total), None);
    }

    #[test]
    fn list_row_to_index_no_customs_has_no_divider() {
        let popup = Rect::new(0, 0, LIST_POPUP_W, LIST_POPUP_H);
        let built_in_count = 6;
        let total = 6;

        assert_eq!(list_row_to_index(3, popup, built_in_count, total), Some(0));
        assert_eq!(list_row_to_index(8, popup, built_in_count, total), Some(5));
        // rel == 6 is out of range (no customs) → None.
        assert_eq!(list_row_to_index(9, popup, built_in_count, total), None);
    }

    #[test]
    fn edit_state_seeds_color_buf_from_role_zero() {
        let theme = Theme::dark_red();
        let st = ThemeEditState::new(theme.clone(), None);
        // Role 0 is header_bg (dark-red firebrick).
        assert_eq!(st.color_buf, "#b22222");
        assert_eq!(st.role_selected, 0);
        assert_eq!(st.focus, EditFocus::Name);
    }

    #[test]
    fn move_role_reloads_color_buf() {
        let theme = Theme::dark_red();
        let mut st = ThemeEditState::new(theme.clone(), None);
        st.move_role(1); // role 1 = header_fg
        assert_eq!(st.role_selected, 1);
        // header_fg is RGB(255,255,255) -> #ffffff
        assert_eq!(st.color_buf, "#ffffff");
    }

    #[test]
    fn commit_color_validates_and_applies() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("tmp".to_string()));
        st.role_selected = 1; // header_fg
        st.color_buf = "#00ff00".to_string();
        assert!(st.commit_color());
        assert_eq!(st.theme.header_fg, ColorDef::Rgb(0, 255, 0));
        assert!(st.color_err.is_none());

        st.color_buf = "garbage".to_string();
        assert!(!st.commit_color());
        assert!(st.color_err.is_some());
    }

    #[test]
    fn type_char_in_name_field_appends() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("".to_string()));
        st.focus = EditFocus::Name;
        st.type_char('a');
        st.type_char('b');
        assert_eq!(st.name, "ab");
    }

    #[test]
    fn role_field_table_covers_every_theme_field() {
        // Sanity: every accessor in ROLE_FIELDS must be a real Theme field
        // (the set_field_value match arms enforce this at compile time, but we
        // also assert the count matches the Theme struct's color roles).
        let theme = Theme::light();
        // 16 color roles (name is a separate text field, not a role).
        assert_eq!(ROLE_FIELDS.len(), 16);
        // Reading each must not panic.
        for i in 0..ROLE_FIELDS.len() {
            let _ = field_value(&theme, i);
        }
    }

    #[test]
    fn name_field_accepts_letters_used_as_list_shortcuts() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some(String::new()));
        st.focus = EditFocus::Name;
        for c in ['e', 'n', 'd', 's', 'j', 'k'] {
            let cmd = handle_edit_key(&mut st, KeyCode::Char(c));
            assert_eq!(cmd, ThemesEditCmd::None);
        }
        assert_eq!(st.name, "endsjk");
    }

    #[test]
    fn hex_field_accepts_d_and_does_not_reset_draft() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("tmp".into()));
        st.focus = EditFocus::Hex;
        st.role_selected = 1; // header_fg
        st.color_buf.clear();
        for c in ['#', 'd', 'd', '0', '0', '0', '0'] {
            handle_edit_key(&mut st, KeyCode::Char(c));
        }
        assert_eq!(st.color_buf, "#dd0000");
        assert_eq!(st.name, "tmp");
    }

    #[test]
    fn s_saves_from_role_focus_not_from_name() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("tmp".into()));
        st.focus = EditFocus::Name;
        assert_eq!(
            handle_edit_key(&mut st, KeyCode::Char('s')),
            ThemesEditCmd::None
        );
        assert_eq!(st.name, "tmps");
        st.focus = EditFocus::Role;
        assert_eq!(
            handle_edit_key(&mut st, KeyCode::Char('s')),
            ThemesEditCmd::Save
        );
    }

    #[test]
    fn cycle_focus_visits_all_edit_targets() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some(String::new()));
        assert_eq!(st.focus, EditFocus::Name);
        let order = [
            EditFocus::Role,
            EditFocus::Palette,
            EditFocus::SliderR,
            EditFocus::SliderG,
            EditFocus::SliderB,
            EditFocus::Hex,
            EditFocus::Name,
        ];
        for expected in order {
            st.cycle_focus();
            assert_eq!(st.focus, expected);
        }
    }

    #[test]
    fn palette_right_applies_premade_to_color_role() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("tmp".into()));
        st.focus = EditFocus::Palette;
        let cmd = handle_edit_key(&mut st, KeyCode::Right);
        assert_eq!(cmd, ThemesEditCmd::None);
        assert_eq!(st.palette_index, 1);
        let expected = premade()[1].color.clone();
        assert_eq!(st.theme.header_bg, expected);
        assert_eq!(st.color_buf, color_buf_for(&expected));
        assert!(st.color_err.is_none());
        assert!(st.dirty);
    }

    #[test]
    fn esc_cancels_and_enter_saves() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("tmp".into()));
        assert_eq!(
            handle_edit_key(&mut st, KeyCode::Esc),
            ThemesEditCmd::Cancel
        );
        assert_eq!(
            handle_edit_key(&mut st, KeyCode::Enter),
            ThemesEditCmd::Save
        );
    }

    #[test]
    fn role_fields_are_the_16_color_roles() {
        assert_eq!(ROLE_FIELDS.len(), 16);
        assert!(ROLE_FIELDS.iter().all(|(l, _)| *l != "name"));
    }

    #[test]
    fn edit_inner_widgets_fit_80x24() {
        let popup = edit_rect(Rect::new(0, 0, 80, 24));
        assert_eq!(popup.height, EDIT_POPUP_H);
        assert_eq!(popup.width, EDIT_POPUP_W);
        let inner_last = popup.y + popup.height - 2;
        for y in [
            name_row_y(popup),
            footer_row_y(popup),
            hex_row_y(popup),
            slider_row_y(popup, 0),
            palette_area(popup).y,
            roles_area(popup).y,
        ] {
            assert!(y > popup.y && y <= inner_last, "y={y} popup={popup:?}");
        }
        assert_eq!(roles_area(popup).height, ROLE_VISIBLE);
        assert_eq!(footer_row_y(popup), popup.y + 1 + 19);
        assert_eq!(name_row_y(popup), popup.y + 1);
    }

    #[test]
    fn slider_bar_area_matches_row_and_width() {
        let popup = edit_rect(Rect::new(0, 0, 80, 24));
        for ch in 0u8..3 {
            let bar = slider_bar_area(popup, ch);
            assert_eq!(bar.width, SLIDER_BAR_W, "ch={ch}");
            assert_eq!(bar.y, slider_row_y(popup, ch), "ch={ch}");
            assert_eq!(bar.height, 1, "ch={ch}");
            assert_eq!(bar.x, popup.x + 1 + SLIDER_PREFIX_W, "ch={ch}");
        }
    }

    #[test]
    fn slider_bar_offset_matches_prefix_width() {
        let popup = edit_rect(Rect::new(0, 0, 80, 24));
        let bar = slider_bar_area(popup, 0);
        assert_eq!(bar.x - (popup.x + 1), SLIDER_PREFIX_W);

        let line = slider_line(
            'R',
            128,
            false,
            Style::default(),
            Style::default(),
            Style::default(),
        );
        let prefix: u16 = line
            .spans
            .iter()
            .take(2)
            .map(|s| s.content.chars().count() as u16)
            .sum();
        assert_eq!(prefix, SLIDER_PREFIX_W);
    }

    #[test]
    fn selected_swatch_keeps_bg_without_reverse() {
        let color = Color::Rgb(178, 34, 34);
        let selected = swatch_style(color, true);
        assert_eq!(selected.bg, Some(color));
        assert!(
            selected.add_modifier.contains(Modifier::UNDERLINED),
            "selected swatch should be underlined"
        );
        assert!(
            selected.add_modifier.contains(Modifier::BOLD),
            "selected swatch should be bold"
        );
        assert!(
            !selected.add_modifier.contains(Modifier::REVERSED),
            "REVERSED hides the swatch background"
        );

        let idle = swatch_style(color, false);
        assert_eq!(idle.bg, Some(color));
        assert!(!idle.add_modifier.contains(Modifier::REVERSED));
        assert!(!idle.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn palette_area_hit_swatch_0_and_12() {
        use crate::ui::color_picker::{hit_swatch, swatch_rect};
        let popup = edit_rect(Rect::new(0, 0, 80, 24));
        let pal = palette_area(popup);
        let r0 = swatch_rect(pal, 0).expect("swatch 0 fits");
        let r12 = swatch_rect(pal, 12).expect("swatch 12 fits");
        assert_eq!(hit_swatch(pal, r0.x, r0.y), Some(0));
        assert_eq!(hit_swatch(pal, r12.x, r12.y), Some(12));
        assert_ne!(r0.y, r12.y);
    }

    #[test]
    fn apply_swatch_at_sets_index_and_role() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("tmp".into()));
        st.apply_swatch_at(12);
        assert_eq!(st.palette_index, 12);
        let expected = premade()[12].color.clone();
        assert_eq!(st.theme.header_bg, expected);
        assert_eq!(st.color_buf, color_buf_for(&expected));
        assert!(st.dirty);
    }

    #[test]
    fn set_channel_value_updates_header_bg_and_hex() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("tmp".into()));
        // Role 0 is header_bg: dark-red firebrick #b22222 = (178, 34, 34).
        st.set_channel_value(Channel::R, 10);
        assert_eq!(st.theme.header_bg, ColorDef::Rgb(10, 34, 34));
        assert_eq!(st.color_buf, "#0a2222");
        assert!(st.dirty);
        st.set_channel_value(Channel::G, 200);
        assert_eq!(st.theme.header_bg, ColorDef::Rgb(10, 200, 34));
        assert_eq!(st.color_buf, "#0ac822");
    }

    fn typical_edit_popup() -> Rect {
        edit_rect(Rect::new(0, 0, 80, 24))
    }

    #[test]
    fn roles_scrollbar_rect_is_rightmost_column_of_roles_area() {
        let popup = typical_edit_popup();
        let roles = roles_area(popup);
        let sb = roles_scrollbar_rect(popup);
        assert_eq!(sb.width, 1);
        assert_eq!(sb.height, roles.height);
        assert_eq!(sb.y, roles.y);
        assert_eq!(sb.x, roles.x + roles.width - 1);
        assert_eq!(sb.height, ROLE_VISIBLE);
    }

    #[test]
    fn roles_overflow_bar_overflows_sixteen_roles_in_eight_rows() {
        let bar = roles_overflow_bar(0);
        assert_eq!(bar.total, ROLE_FIELDS.len());
        assert_eq!(bar.viewport, ROLE_VISIBLE as usize);
        assert!(bar.overflowing());
        assert_eq!(
            bar.max_position(),
            ROLE_FIELDS.len() - ROLE_VISIBLE as usize
        );
    }

    #[test]
    fn roles_click_is_scrollbar_only_on_track_when_overflowing() {
        let popup = typical_edit_popup();
        let sb = roles_scrollbar_rect(popup);
        assert!(roles_click_is_scrollbar(
            sb.x as usize,
            sb.y as usize,
            popup,
            true
        ));
        assert!(!roles_click_is_scrollbar(
            sb.x as usize - 1,
            sb.y as usize,
            popup,
            true
        ));
        assert!(
            !roles_click_is_scrollbar(sb.x as usize, sb.y as usize, popup, false),
            "hidden bar must not steal the last role-label column"
        );
    }

    #[test]
    fn roles_list_area_shrinks_by_one_column_when_overflowing() {
        let popup = typical_edit_popup();
        let roles = roles_area(popup);
        let list = roles_list_area(popup, true);
        assert_eq!(list.x, roles.x);
        assert_eq!(list.y, roles.y);
        assert_eq!(list.height, roles.height);
        assert_eq!(list.width, roles.width - 1);
        let full = roles_list_area(popup, false);
        assert_eq!(full, roles);
    }

    #[test]
    fn preview_area_stays_right_of_roles_area() {
        let popup = typical_edit_popup();
        let roles = roles_area(popup);
        let preview = preview_area(popup);
        assert_eq!(preview.y, roles.y);
        assert_eq!(preview.height, roles.height);
        assert_eq!(preview.x, roles.x + roles.width);
    }

    fn unique_preview_theme() -> Theme {
        Theme {
            name: "unique".into(),
            header_bg: ColorDef::Rgb(10, 1, 1),
            header_fg: ColorDef::Rgb(11, 1, 1),
            editor_bg: ColorDef::Rgb(12, 1, 1),
            editor_fg: ColorDef::Rgb(13, 1, 1),
            status_bg: ColorDef::Rgb(14, 1, 1),
            status_fg: ColorDef::Rgb(15, 1, 1),
            line_number_fg: ColorDef::Rgb(16, 1, 1),
            selection_bg: ColorDef::Rgb(17, 1, 1),
            cursor_fg: ColorDef::Rgb(18, 1, 1),
            keyword: ColorDef::Rgb(19, 1, 1),
            string: ColorDef::Rgb(20, 1, 1),
            comment: ColorDef::Rgb(21, 1, 1),
            function: ColorDef::Rgb(22, 1, 1),
            number: ColorDef::Rgb(23, 1, 1),
            type_name: ColorDef::Rgb(24, 1, 1),
            accent: ColorDef::Rgb(25, 1, 1),
        }
    }

    fn line_has_color(line: &Line, color: Color) -> bool {
        line.spans
            .iter()
            .any(|s| s.style.fg == Some(color) || s.style.bg == Some(color))
    }

    fn preview_uses_role(theme: &Theme, role_idx: usize) -> bool {
        let color = Color::from(&field_value(theme, role_idx));
        preview_chrome_lines(theme, role_idx, 40)
            .iter()
            .any(|line| line_has_color(line, color))
    }

    #[test]
    fn preview_chrome_lines_is_eight_rows() {
        let lines = preview_chrome_lines(&Theme::dark_red(), 0, 36);
        assert_eq!(lines.len(), ROLE_VISIBLE as usize);
    }

    #[test]
    fn preview_chrome_lines_uses_every_color_role() {
        let theme = unique_preview_theme();
        for (i, (label, _)) in ROLE_FIELDS.iter().enumerate() {
            assert!(
                preview_uses_role(&theme, i),
                "preview missing role {i} ({label})"
            );
        }
    }

    #[test]
    fn preview_header_row_uses_header_bg() {
        let theme = unique_preview_theme();
        let lines = preview_chrome_lines(&theme, 0, 40);
        let header_bg = Color::from(&theme.header_bg);
        assert!(
            line_has_color(&lines[0], header_bg),
            "row 0 should paint header_bg"
        );
    }

    #[test]
    fn preview_status_row_uses_status_bg() {
        let theme = unique_preview_theme();
        let lines = preview_chrome_lines(&theme, 4, 40);
        let status_bg = Color::from(&theme.status_bg);
        let last = lines.last().expect("preview rows");
        assert!(
            line_has_color(last, status_bg),
            "last row should paint status_bg"
        );
    }

    #[test]
    fn preview_marks_selected_role_with_underline() {
        let theme = unique_preview_theme();
        let status_idx = ROLE_FIELDS
            .iter()
            .position(|(l, _)| *l == "status_bg")
            .unwrap();
        let lines = preview_chrome_lines(&theme, status_idx, 40);
        let last = lines.last().expect("preview rows");
        assert!(
            last.spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::UNDERLINED)),
            "selected status_bg should underline the status row"
        );
    }
}
