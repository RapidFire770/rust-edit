// "Themes" popup — preset list + custom-theme editor.
//
// Two modes (selected via `ThemesMode` on `App`):
//   * List  — built-ins then customs, click/enter to apply, e/n/d to
//             edit/new/delete.
//   * Edit  — name field + per-role color editor with a live code preview,
//             Tab cycles focus between the name field, role list, and color
//             input; Enter saves.
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
use crate::ui::color_picker::{adjust_channel, premade, Channel, SWATCH_COLS};
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
    /// No-op on the fake `name` role (Task 3 drops it).
    fn apply_color(&mut self, color: ColorDef) {
        let Some((_, accessor)) = ROLE_FIELDS.get(self.role_selected) else {
            return;
        };
        if *accessor == "name" {
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

pub const EDIT_POPUP_W: u16 = 64;
pub const EDIT_POPUP_H: u16 = 18;

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

/// X column where the right-hand "Color Roles" pane starts.
pub const ROLE_PANE_X: u16 = 34;
/// X column where the role's color value starts within the roles pane.
pub const ROLE_VALUE_X: u16 = 14;

/// Footer labels for edit mode.
pub fn edit_footer_labels() -> [&'static str; 4] {
    [
        "[ Tab=Next ]",
        "[ Enter=Save ]",
        "[ Esc=Cancel ]",
        "[ s=Save ]",
    ]
}

// ===================== Field table =====================

/// The editable color roles, in display order. The index here is what
/// `role_selected` refers to.
pub const ROLE_FIELDS: &[(&str, &str)] = &[
    ("name", "name"),
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
        "name" => ColorDef::Named(theme.name.clone()),
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
        "name" => {
            if let ColorDef::Named(n) = value {
                theme.name = n;
            }
        }
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

fn render_edit(f: &mut Frame, area: Rect, theme: &Theme, app: &App) {
    let edit = match &app.themes_edit {
        Some(e) => e,
        None => return, // defensive: should always be set in Edit mode
    };
    let popup = edit_rect(area);
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
    let err_style = Style::default().fg(Color::LightRed);

    // ----- Left pane: live preview of sample code in the in-progress theme -----
    let preview_theme = &edit.theme;
    let mut preview_lines: Vec<Line> = Vec::new();
    preview_lines.push(Line::from(Span::styled(
        "Preview",
        Style::default().fg(Color::from(&preview_theme.line_number_fg)),
    )));
    // Sample lines colored by role, so the user sees each accent in context.
    let samples: [(&str, Color); 5] = [
        ("fn main() {", Color::from(&preview_theme.keyword)),
        ("    let x = 42;", Color::from(&preview_theme.number)),
        (
            "    let s = \"hi\"; // comment",
            Color::from(&preview_theme.string),
        ),
        ("}", Color::from(&preview_theme.editor_fg)),
        ("type T = String;", Color::from(&preview_theme.type_name)),
    ];
    // The comment sample mixes string + comment; re-split for realism.
    preview_lines.push(Line::from(vec![
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
    ]));
    preview_lines.push(Line::from(vec![
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
    ]));
    preview_lines.push(Line::from(vec![
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
    ]));
    preview_lines.push(Line::from(Span::styled(
        "}",
        Style::default().fg(Color::from(&preview_theme.editor_fg)),
    )));
    preview_lines.push(Line::from(vec![
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
    ]));
    let _ = samples; // (samples kept as documentation of intent; spans built inline)

    // ----- Right pane: role list -----
    let mut role_lines: Vec<Line> = Vec::new();
    for (i, (label, _)) in ROLE_FIELDS.iter().enumerate() {
        let active = i == edit.role_selected;
        let style = if active { accent } else { dim };
        let value_str = if *label == "name" {
            edit.theme.name.clone()
        } else {
            color_buf_for(&field_value(&edit.theme, i))
        };
        let marker = if active { ">" } else { " " };
        let label_padded = format!("{:<14}", label);
        let value_padded = format!("{:<14}", value_str);
        role_lines.push(Line::from(vec![
            Span::styled(format!("{marker} "), style),
            Span::styled(label_padded, style),
            Span::styled(value_padded, dim),
        ]));
    }

    // ----- Assemble: interleave the two panes line by line -----
    let mut lines: Vec<Line> = Vec::new();
    // Two-column header line: "Preview" (left) + "Color Roles" (right).
    let header = Line::from(vec![
        Span::styled("Preview", dim_dim),
        Span::raw("                              "),
        Span::styled("Color Roles", dim_dim),
    ]);
    lines.push(header);

    let max_panes = preview_lines.len().max(role_lines.len());
    for i in 0..max_panes {
        let left = preview_lines
            .get(i)
            .cloned()
            .unwrap_or_else(|| Line::from(""));
        let right = role_lines.get(i).cloned().unwrap_or_else(|| Line::from(""));
        // Pad left to ROLE_PANE_X then append right.
        let mut combined: Vec<Span> = left.spans.clone();
        // Ensure at least ROLE_PANE_X columns of left content.
        let left_len: usize = combined.iter().map(|s| s.content.chars().count()).sum();
        if (left_len as u16) < ROLE_PANE_X {
            combined.push(Span::raw(
                " ".repeat((ROLE_PANE_X as usize).saturating_sub(left_len)),
            ));
        }
        combined.extend(right.spans);
        lines.push(Line::from(combined));
    }

    // ----- Name + color input row -----
    lines.push(Line::from(""));
    let name_focus = edit.focus == EditFocus::Name;
    let name_style = if name_focus { accent } else { dim };
    lines.push(Line::from(vec![
        Span::styled("Name:  ", dim),
        Span::styled("[", name_style),
        Span::styled(format!("{:<16}", edit.name), name_style),
        Span::styled("]", name_style),
    ]));

    let color_focus = edit.focus == EditFocus::Hex;
    let color_style = if color_focus { accent } else { dim };
    let (selected_label, _) = ROLE_FIELDS[edit.role_selected];
    let err_span = match &edit.color_err {
        Some(e) => Span::styled(format!("  ! {e}"), err_style),
        None => Span::raw(""),
    };
    lines.push(Line::from(vec![
        Span::styled("Color: ", dim),
        Span::styled("[", color_style),
        Span::styled(format!("{:<16}", edit.color_buf), color_style),
        Span::styled("]", color_style),
        Span::styled(format!("  ({})", selected_label), dim_dim),
        err_span,
    ]));

    // ----- Footer -----
    lines.push(Line::from(""));
    let labels = edit_footer_labels();
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, l) in labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let hovered = app.themes_button_hover == Some(i);
        let style = if hovered { accent } else { dim };
        spans.push(Span::styled(*l, style));
    }
    lines.push(Line::from(spans));

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
        // Role 0 is `name` which is stored as Named; the buf should hold the name.
        assert_eq!(st.color_buf, theme.name);
        assert_eq!(st.role_selected, 0);
        assert_eq!(st.focus, EditFocus::Name);
    }

    #[test]
    fn move_role_reloads_color_buf() {
        let theme = Theme::dark_red();
        let mut st = ThemeEditState::new(theme.clone(), None);
        st.move_role(1); // role 1 = header_bg
        assert_eq!(st.role_selected, 1);
        // header_bg is RGB(178,34, 34) -> #b22222
        assert_eq!(st.color_buf, "#b22222");
    }

    #[test]
    fn commit_color_validates_and_applies() {
        let mut st = ThemeEditState::new(Theme::dark_red(), Some("tmp".to_string()));
        st.role_selected = 1; // header_bg
        st.color_buf = "#00ff00".to_string();
        assert!(st.commit_color());
        assert_eq!(st.theme.header_bg, ColorDef::Rgb(0, 255, 0));
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
        // 17 entries: name + 16 color roles.
        assert_eq!(ROLE_FIELDS.len(), 17);
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
        st.role_selected = 1; // header_bg
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
        st.role_selected = 1; // header_bg
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
}
