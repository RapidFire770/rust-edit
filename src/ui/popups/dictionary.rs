// Personal-dictionary popup. The list and the word editor share one focus
// value; the editor is painted on top and owns input while it is open.
// Geometry helpers here are the only paint and hit-test source.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::button::ButtonRow;
use crate::ui::scrollbar::{self, OverflowBar};
use crate::App;

pub const POPUP_W: u16 = 52;
pub const POPUP_H: u16 = 16;
pub const EDITOR_W: u16 = 42;
pub const EDITOR_H: u16 = 7;
pub const CLEAR_LABEL: &str = "[x]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictionaryFocus {
    List,
    Buttons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorFocus {
    Field,
    Buttons,
}

#[derive(Debug, Clone)]
pub struct DictionaryEditor {
    pub draft: String,
    /// Char index into `draft`.
    pub caret: usize,
    pub adding: bool,
    /// Spelling selected when Edit opened. Empty for Add.
    pub origin: String,
    pub button: usize,
    pub button_hover: Option<usize>,
    pub focus: EditorFocus,
}

#[derive(Debug, Clone)]
pub struct DictionaryState {
    pub words: Vec<String>,
    pub selected: usize,
    pub scroll: usize,
    pub focus: DictionaryFocus,
    pub button: usize,
    pub row_hover: Option<usize>,
    pub button_hover: Option<usize>,
    pub editor: Option<DictionaryEditor>,
}

impl DictionaryState {
    pub fn new(words: Vec<String>) -> Self {
        let empty = words.is_empty();
        Self {
            words,
            selected: 0,
            scroll: 0,
            focus: if empty {
                DictionaryFocus::Buttons
            } else {
                DictionaryFocus::List
            },
            button: 0,
            row_hover: None,
            button_hover: None,
            editor: None,
        }
    }

    pub fn begin_add(&mut self) {
        self.editor = Some(DictionaryEditor {
            draft: String::new(),
            caret: 0,
            adding: true,
            origin: String::new(),
            button: 0,
            button_hover: None,
            focus: EditorFocus::Field,
        });
        self.row_hover = None;
        self.button_hover = None;
    }

    pub fn begin_edit(&mut self) {
        let Some(word) = self.words.get(self.selected).cloned() else {
            return;
        };
        let caret = word.chars().count();
        self.editor = Some(DictionaryEditor {
            draft: word.clone(),
            caret,
            adding: false,
            origin: word,
            button: 0,
            button_hover: None,
            focus: EditorFocus::Field,
        });
        self.row_hover = None;
        self.button_hover = None;
    }
}

impl DictionaryEditor {
    pub fn insert_char(&mut self, ch: char) {
        if ch.is_control() || self.focus != EditorFocus::Field {
            return;
        }
        let byte = byte_at(&self.draft, self.caret);
        self.draft.insert(byte, ch);
        self.caret += 1;
    }

    pub fn backspace(&mut self) {
        if self.focus != EditorFocus::Field || self.caret == 0 {
            return;
        }
        self.caret -= 1;
        let start = byte_at(&self.draft, self.caret);
        let next = self.draft[start..].chars().next().map(|ch| ch.len_utf8());
        if let Some(len) = next {
            self.draft.replace_range(start..start + len, "");
        }
    }

    pub fn delete_forward(&mut self) {
        if self.focus != EditorFocus::Field {
            return;
        }
        let start = byte_at(&self.draft, self.caret);
        let next = self.draft[start..].chars().next().map(|ch| ch.len_utf8());
        if let Some(len) = next {
            self.draft.replace_range(start..start + len, "");
        }
    }

    pub fn move_caret(&mut self, delta: i32) {
        if self.focus != EditorFocus::Field {
            return;
        }
        let len = self.draft.chars().count() as i32;
        self.caret = (self.caret as i32 + delta).clamp(0, len) as usize;
    }

    pub fn caret_home(&mut self) {
        if self.focus == EditorFocus::Field {
            self.caret = 0;
        }
    }

    pub fn caret_end(&mut self) {
        if self.focus == EditorFocus::Field {
            self.caret = self.draft.chars().count();
        }
    }
}

fn byte_at(text: &str, caret: usize) -> usize {
    text.chars().take(caret).map(|ch| ch.len_utf8()).sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictionaryHit {
    Row(usize),
    Clear(usize),
    Scrollbar,
    Button(usize),
    Inside,
    Outside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorHit {
    Field,
    Button(usize),
    Inside,
    Outside,
}

pub fn footer_labels() -> [&'static str; 3] {
    ["[ Add ]", "[ Edit ]", "[ Close ]"]
}

pub fn editor_labels() -> [&'static str; 2] {
    ["[ Save ]", "[ Cancel ]"]
}

pub fn dictionary_rect(area: Rect) -> Rect {
    let width = POPUP_W.min(area.width);
    let height = POPUP_H.min(area.height);
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

pub fn editor_rect(area: Rect) -> Rect {
    let width = EDITOR_W.min(area.width);
    let height = EDITOR_H.min(area.height);
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

/// Word rows that fit above the blank line and the footer.
pub fn list_viewport(popup: Rect) -> usize {
    (popup.height as usize).saturating_sub(4).max(1)
}

pub fn footer_y(popup: Rect) -> u16 {
    popup.y + list_viewport(popup) as u16 + 2
}

pub fn reveal(selected: usize, scroll: usize, viewport: usize) -> usize {
    let viewport = viewport.max(1);
    if selected < scroll {
        selected
    } else if selected >= scroll + viewport {
        selected + 1 - viewport
    } else {
        scroll
    }
}

pub fn list_overflow(word_count: usize, scroll: usize, popup: Rect) -> OverflowBar {
    OverflowBar {
        total: word_count,
        position: scroll,
        viewport: list_viewport(popup),
    }
}

pub fn scrollbar_rect(popup: Rect) -> Rect {
    Rect::new(
        popup.x + popup.width.saturating_sub(2),
        popup.y + 1,
        1,
        list_viewport(popup) as u16,
    )
}

/// Exclusive screen-x range of `[x]`. When the list overflows, the range
/// ends at the scrollbar column.
pub fn clear_x_range(popup: Rect, overflowing: bool) -> (u16, u16) {
    let inner_end = if overflowing {
        popup.x + popup.width.saturating_sub(2)
    } else {
        popup.x + popup.width.saturating_sub(1)
    };
    let start = inner_end.saturating_sub(CLEAR_LABEL.chars().count() as u16);
    (start, inner_end)
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

pub fn hit_test(area: Rect, words: &[String], scroll: usize, x: u16, y: u16) -> DictionaryHit {
    let popup = dictionary_rect(area);
    if !contains(popup, x, y) {
        return DictionaryHit::Outside;
    }
    let viewport = list_viewport(popup);
    let scroll = scroll.min(words.len().saturating_sub(viewport));
    if y == footer_y(popup) {
        let labels = footer_labels();
        let ranges = ButtonRow::button_x_ranges(popup.x.saturating_add(1), &labels);
        if let Some(index) = ranges
            .iter()
            .position(|(start, end)| x >= *start && x < *end)
        {
            return DictionaryHit::Button(index);
        }
        return DictionaryHit::Inside;
    }
    let list_top = popup.y + 1;
    let list_bottom = list_top + viewport as u16;
    if y >= list_top && y < list_bottom {
        let overflowing = words.len() > viewport;
        if overflowing && x == popup.x + popup.width.saturating_sub(2) {
            return DictionaryHit::Scrollbar;
        }
        let index = scroll + (y - list_top) as usize;
        if index < words.len() {
            let (start, end) = clear_x_range(popup, overflowing);
            if x >= start && x < end {
                return DictionaryHit::Clear(index);
            }
            return DictionaryHit::Row(index);
        }
    }
    DictionaryHit::Inside
}

/// Inner text row of the editor field. One column in from each side border,
/// and one column in from the left padding space.
pub fn editor_field_rect(popup: Rect) -> Rect {
    Rect::new(
        popup.x.saturating_add(2),
        popup.y.saturating_add(2),
        popup.width.saturating_sub(4),
        1,
    )
}

pub fn editor_button_y(popup: Rect) -> u16 {
    popup.y.saturating_add(4)
}

pub fn field_scroll(caret: usize, width: usize) -> usize {
    let width = width.max(1);
    (caret + 1).saturating_sub(width)
}

pub fn caret_from_click(draft: &str, caret: usize, field: Rect, x: u16) -> usize {
    let len = draft.chars().count();
    let width = (field.width as usize).max(1);
    let scroll = field_scroll(caret, width);
    if x < field.x {
        return scroll.min(len);
    }
    let rel = (x - field.x) as usize;
    (scroll + rel).min(len)
}

pub fn editor_hit(area: Rect, x: u16, y: u16) -> EditorHit {
    let popup = editor_rect(area);
    if !contains(popup, x, y) {
        return EditorHit::Outside;
    }
    let field = editor_field_rect(popup);
    if y == field.y && x >= field.x && x < field.x.saturating_add(field.width.max(1)) {
        return EditorHit::Field;
    }
    if y == editor_button_y(popup) {
        let labels = editor_labels();
        let ranges = ButtonRow::button_x_ranges(popup.x.saturating_add(1), &labels);
        if let Some(index) = ranges
            .iter()
            .position(|(start, end)| x >= *start && x < *end)
        {
            return EditorHit::Button(index);
        }
    }
    EditorHit::Inside
}

fn ellipsize(word: &str, width: usize) -> String {
    if word.chars().count() <= width {
        return word.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut out: String = word.chars().take(width - 1).collect();
    out.push('…');
    out
}

fn button_line(
    labels: &[&'static str],
    buttons_focused: bool,
    selected: usize,
    hover: Option<usize>,
    accent: Style,
    dim: Style,
) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("   "));
        }
        let hovered = hover == Some(index);
        let chosen = buttons_focused && hover.is_none() && selected == index;
        let style = if hovered || chosen { accent } else { dim };
        spans.push(Span::styled(*label, style));
    }
    Line::from(spans)
}

fn field_line(
    draft: &str,
    caret: usize,
    width: usize,
    dim: Style,
    caret_style: Style,
) -> Line<'static> {
    let width = width.max(1);
    let scroll = field_scroll(caret, width);
    let chars: Vec<char> = draft.chars().skip(scroll).take(width).collect();
    let mut spans = vec![Span::raw(" ")];
    let caret_col = caret.saturating_sub(scroll);
    if chars.is_empty() {
        spans.push(Span::styled(" ", caret_style));
        return Line::from(spans);
    }
    for (index, ch) in chars.iter().enumerate() {
        let style = if index == caret_col { caret_style } else { dim };
        spans.push(Span::styled(ch.to_string(), style));
    }
    if caret_col >= chars.len() {
        spans.push(Span::styled(" ", caret_style));
    }
    Line::from(spans)
}

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, app: &App) {
    let Some(state) = &app.dictionary else {
        return;
    };
    let popup = dictionary_rect(area);
    let viewport = list_viewport(popup);
    let scroll = state.scroll.min(state.words.len().saturating_sub(viewport));
    let overflowing = state.words.len() > viewport;
    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let inner_cols = (popup.width as usize).saturating_sub(2);
    let line_cols = inner_cols.saturating_sub(if overflowing { 1 } else { 0 });
    let word_cols = line_cols.saturating_sub(2 + CLEAR_LABEL.chars().count());

    let mut lines = Vec::new();
    for row in 0..viewport {
        let index = scroll + row;
        if let Some(word) = state.words.get(index) {
            let hovered = state.row_hover == Some(index);
            let chosen = state.focus == DictionaryFocus::List
                && state.row_hover.is_none()
                && state.selected == index;
            let active = hovered || chosen;
            let style = if active { accent } else { dim };
            let prefix = if active { "> " } else { "  " };
            let shown = ellipsize(word, word_cols);
            let gap = word_cols.saturating_sub(shown.chars().count());
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(shown, style),
                Span::raw(" ".repeat(gap)),
                Span::styled(CLEAR_LABEL, style),
            ]));
        } else if state.words.is_empty() && row == 0 {
            lines.push(Line::from(Span::styled("  No custom words", dim)));
        } else {
            lines.push(Line::from(""));
        }
    }
    lines.push(Line::from(""));
    lines.push(button_line(
        &footer_labels(),
        state.focus == DictionaryFocus::Buttons && state.editor.is_none(),
        state.button,
        state.button_hover,
        accent,
        dim,
    ));

    let block = Block::default()
        .title(" Dictionary ")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());
    f.render_widget(Clear, popup);
    f.render_widget(Paragraph::new(lines).block(block).style(dim), popup);
    if overflowing {
        scrollbar::render(
            f,
            scrollbar_rect(popup),
            list_overflow(state.words.len(), scroll, popup),
            theme,
        );
    }

    if let Some(editor) = &state.editor {
        render_editor(f, area, theme, editor, accent, dim);
    }
}

fn render_editor(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    editor: &DictionaryEditor,
    accent: Style,
    dim: Style,
) {
    let popup = editor_rect(area);
    let caret_style = Style::default()
        .fg(Color::from(&theme.editor_bg))
        .bg(Color::from(&theme.accent));
    let field = editor_field_rect(popup);
    let mut lines = vec![
        Line::from(""),
        field_line(
            &editor.draft,
            editor.caret,
            field.width as usize,
            dim,
            caret_style,
        ),
        Line::from(""),
        button_line(
            &editor_labels(),
            editor.focus == EditorFocus::Buttons,
            editor.button,
            editor.button_hover,
            accent,
            dim,
        ),
        Line::from(""),
    ];
    // A short terminal clips the box; keep the field and the buttons first.
    let inner = (popup.height as usize).saturating_sub(2);
    lines.truncate(inner);

    let title = if editor.adding {
        " Add word "
    } else {
        " Edit word "
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());
    f.render_widget(Clear, popup);
    f.render_widget(Paragraph::new(lines).block(block).style(dim), popup);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::new(0, 0, 80, 24)
    }

    #[test]
    fn clear_x_range_sits_left_of_the_border_when_the_list_fits() {
        let popup = dictionary_rect(area());
        let (start, end) = clear_x_range(popup, false);
        assert_eq!(end, popup.x + popup.width - 1);
        assert_eq!(end - start, 3);
    }

    #[test]
    fn clear_x_range_sits_left_of_the_scrollbar_when_overflowing() {
        let popup = dictionary_rect(area());
        let (start, end) = clear_x_range(popup, true);
        assert_eq!(end, popup.x + popup.width - 2);
        assert_eq!(end - start, 3);
    }

    #[test]
    fn hit_test_distinguishes_row_clear_buttons_and_outside() {
        let popup = dictionary_rect(area());
        let words = vec!["alpha".to_string(), "beta".to_string()];
        let row_y = popup.y + 1;
        assert_eq!(
            hit_test(area(), &words, 0, popup.x + 4, row_y),
            DictionaryHit::Row(0)
        );
        let (start, _) = clear_x_range(popup, false);
        assert_eq!(
            hit_test(area(), &words, 0, start, row_y),
            DictionaryHit::Clear(0)
        );
        let labels = footer_labels();
        let ranges = ButtonRow::button_x_ranges(popup.x + 1, &labels);
        let footer = footer_y(popup);
        for (index, (x, _)) in ranges.iter().enumerate() {
            assert_eq!(
                hit_test(area(), &words, 0, *x, footer),
                DictionaryHit::Button(index)
            );
        }
        assert_eq!(hit_test(area(), &words, 0, 0, 0), DictionaryHit::Outside);
    }

    #[test]
    fn editor_hit_maps_the_field_save_cancel_and_outside() {
        let popup = editor_rect(area());
        let editor = DictionaryEditor {
            draft: "abcd".to_string(),
            caret: 4,
            adding: true,
            origin: String::new(),
            button: 0,
            button_hover: None,
            focus: EditorFocus::Field,
        };
        let field = editor_field_rect(popup);
        assert_eq!(editor_hit(area(), field.x, field.y), EditorHit::Field);
        assert_eq!(
            caret_from_click(&editor.draft, editor.caret, field, field.x + 1),
            1
        );
        let labels = editor_labels();
        let ranges = ButtonRow::button_x_ranges(popup.x + 1, &labels);
        let y = editor_button_y(popup);
        assert_eq!(editor_hit(area(), ranges[0].0, y), EditorHit::Button(0));
        assert_eq!(editor_hit(area(), ranges[1].0, y), EditorHit::Button(1));
        assert_eq!(editor_hit(area(), 0, 0), EditorHit::Outside);
    }

    #[test]
    fn reveal_keeps_the_selection_on_screen() {
        assert_eq!(reveal(0, 0, 12), 0);
        assert_eq!(reveal(14, 0, 12), 3);
        assert_eq!(reveal(2, 5, 12), 2);
    }

    #[test]
    fn editor_keys_insert_and_delete_around_the_caret() {
        let mut editor = DictionaryEditor {
            draft: String::new(),
            caret: 0,
            adding: true,
            origin: String::new(),
            button: 0,
            button_hover: None,
            focus: EditorFocus::Field,
        };
        editor.insert_char('a');
        editor.insert_char('e');
        assert_eq!(editor.draft, "ae");
        editor.move_caret(-1);
        editor.delete_forward();
        assert_eq!(editor.draft, "a");
        editor.backspace();
        assert_eq!(editor.draft, "");
    }
}
