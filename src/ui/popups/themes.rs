// "Themes" popup — preset list + custom-theme editor.
//
// Two modes (selected via `ThemesMode` on `App`):
//   * List  — built-ins then customs, click/enter to apply, e/n/d to
//             edit/new/delete.
//   * Edit  — name field + scrolled color-role list + live preview, with
//             reserved palette/slider rows; Tab cycles Name/Role/Palette/
//             sliders/Hex; Enter saves.
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

/// Left pane width in inner columns (includes 2 cells reserved for a future swatch).
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

/// Screen rect of the 16-cell RGB slider bar for `channel` (0=R, 1=G, 2=B).
/// Starts after the `"R ["` prefix (3 columns). Shared by render and mouse.
pub fn slider_bar_area(popup: Rect, channel: u8) -> Rect {
    let y = slider_row_y(popup, channel);
    let x = inner_x(popup).saturating_add(3);
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

fn palette_row_spans(
    popup: Rect,
    edit: &ThemeEditState,
    row: u16,
    accent: Style,
) -> Vec<Span<'static>> {
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
        let mut style = Style::default().bg(Color::from(&swatch.color));
        if edit.focus == EditFocus::Palette && edit.palette_index == i {
            style = style.patch(accent).add_modifier(Modifier::REVERSED);
        }
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
    let mut spans = vec![
        Span::styled(format!("{label} "), prefix_style),
        Span::styled("[", dim),
    ];
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

fn preview_sample_lines(preview_theme: &Theme) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled(
                "fn ",
                Style::default()
                    .fg(Color::from(&preview_theme.keyword))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "main",
                Style::default().fg(Color::from(&preview_theme.function)),
            ),
            Span::styled(
                "() {",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                "let ",
                Style::default()
                    .fg(Color::from(&preview_theme.keyword))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "x",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
            Span::styled(
                " = ",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
            Span::styled(
                "42",
                Style::default().fg(Color::from(&preview_theme.number)),
            ),
            Span::styled(
                ";",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                "let ",
                Style::default()
                    .fg(Color::from(&preview_theme.keyword))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "s",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
            Span::styled(
                " = ",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
            Span::styled(
                "\"hi\"",
                Style::default().fg(Color::from(&preview_theme.string)),
            ),
            Span::styled(
                "; ",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
            Span::styled(
                "// comment",
                Style::default().fg(Color::from(&preview_theme.comment)),
            ),
        ]),
        Line::from(Span::styled(
            "}",
            Style::default().fg(Color::from(&preview_theme.editor_fg)),
        )),
        Line::from(vec![
            Span::styled(
                "type ",
                Style::default()
                    .fg(Color::from(&preview_theme.keyword))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "T",
                Style::default().fg(Color::from(&preview_theme.type_name)),
            ),
            Span::styled(
                " = ",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
            Span::styled(
                "String",
                Style::default().fg(Color::from(&preview_theme.type_name)),
            ),
            Span::styled(
                ";",
                Style::default().fg(Color::from(&preview_theme.editor_fg)),
            ),
        ]),
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

    let mut header_spans = pad_spans(vec![Span::styled("Roles", dim_dim)], roles.width);
    header_spans.push(Span::styled("Preview", dim_dim));
    let header_line = Line::from(header_spans);

    let preview_lines = preview_sample_lines(&edit.theme);
    let start = edit.role_scroll as usize;
    let vis = ROLE_VISIBLE as usize;
    let mut role_preview_lines: Vec<Line> = Vec::with_capacity(vis);
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
        left = pad_spans(left, roles.width);
        if let Some(prev) = preview_lines.get(row) {
            left.extend(clip_spans(prev.spans.clone(), preview.width));
        }
        role_preview_lines.push(Line::from(left));
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
    lines.extend(role_preview_lines); // n=3..10
    lines.push(Line::from("")); // n=11
    lines.push(palette_line); // n=12
    lines.push(Line::from(palette_row_spans(popup, edit, 0, accent))); // n=13
    lines.push(Line::from(palette_row_spans(popup, edit, 1, accent))); // n=14
    lines.push(r_line); // n=15
    lines.push(g_line); // n=16
    lines.push(b_line); // n=17
    lines.push(hex_line); // n=18
    lines.push(footer_line); // n=19

    let para = Paragraph::new(lines).block(block).style(dim);
    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(y >= popup.y + 1 && y <= inner_last, "y={y} popup={popup:?}");
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
            assert_eq!(bar.x, popup.x + 1 + 3, "ch={ch}");
        }
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
}
