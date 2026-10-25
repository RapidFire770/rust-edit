//! One-cell caret for single-line fields.
//!
//! The hardware cursor is the editor caret and is hidden while a popup or
//! menu is open. Fields paint this cell instead.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::theme::Theme;

/// Block for a focused field.
///
/// `Theme::cursor_style` sets `Modifier::REVERSED`. Terminals swap the
/// foreground and background when they draw that bit, so the cell's
/// background becomes `editor_bg` and disappears on a popup. The colors
/// stay; the reverse bit does not.
pub fn caret_cell_style(theme: &Theme) -> Style {
    theme.cursor_style().remove_modifier(Modifier::REVERSED)
}

/// First visible char index so `caret` fits in `width` columns.
/// `width` includes the caret cell. Matches the dictionary click scroller.
pub fn field_scroll(caret: usize, width: usize) -> usize {
    let width = width.max(1);
    (caret + 1).saturating_sub(width)
}

/// Spans for one single-line field.
///
/// `caret` is a char index. `width` counts the visible text and the caret
/// cell. An unfocused field still reserves that cell as a plain space when
/// the caret is at the end, so focus does not change the width.
pub fn field_spans(
    text: &str,
    caret: usize,
    focused: bool,
    width: usize,
    text_style: Style,
    caret_style: Style,
) -> Vec<Span<'static>> {
    let width = width.max(1);
    let chars: Vec<char> = text.chars().collect();
    let caret = caret.min(chars.len());
    let scroll = field_scroll(caret, width);
    let caret_col = caret - scroll;
    let visible: Vec<char> = chars.into_iter().skip(scroll).take(width).collect();
    let mark = if focused { caret_style } else { text_style };

    let mut spans = Vec::new();
    if visible.is_empty() {
        spans.push(Span::styled(" ".to_string(), mark));
        return spans;
    }
    for (index, ch) in visible.iter().enumerate() {
        let style = if focused && index == caret_col {
            caret_style
        } else {
            text_style
        };
        spans.push(Span::styled(ch.to_string(), style));
    }
    if caret_col >= visible.len() {
        spans.push(Span::styled(" ".to_string(), mark));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn plain() -> Style {
        Style::default()
    }

    fn block() -> Style {
        Style::default().bg(Color::Red)
    }

    fn shown(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn focused_empty_field_is_one_caret_cell() {
        let spans = field_spans("", 0, true, 8, plain(), block());
        assert_eq!(shown(&spans), " ");
        assert_eq!(spans[0].style.bg, Some(Color::Red));
    }

    #[test]
    fn unfocused_empty_field_is_a_plain_space() {
        let spans = field_spans("", 0, false, 8, plain(), block());
        assert_eq!(shown(&spans), " ");
        assert_eq!(spans[0].style.bg, None);
    }

    #[test]
    fn caret_at_end_follows_the_text() {
        let spans = field_spans("hi", 2, true, 8, plain(), block());
        assert_eq!(shown(&spans), "hi ");
        assert_eq!(spans[2].style.bg, Some(Color::Red));
        assert_eq!(spans[0].style.bg, None);
    }

    #[test]
    fn caret_on_a_character_styles_that_character() {
        let spans = field_spans("hi", 0, true, 8, plain(), block());
        assert_eq!(shown(&spans), "hi");
        assert_eq!(spans[0].style.bg, Some(Color::Red));
        assert_eq!(spans[1].style.bg, None);
    }

    #[test]
    fn long_text_keeps_the_end_caret_inside_the_width() {
        let text = "abcdefghijklmnopqrstuvwxyz";
        let spans = field_spans(text, 26, true, 4, plain(), block());
        assert_eq!(shown(&spans).chars().count(), 4);
        assert!(shown(&spans).ends_with(' '));
        assert_eq!(spans.last().unwrap().style.bg, Some(Color::Red));
        assert_eq!(field_scroll(26, 4), 23);
    }

    #[test]
    fn unfocused_field_does_not_use_the_caret_style() {
        let spans = field_spans("hi", 2, false, 8, plain(), block());
        assert_eq!(shown(&spans), "hi ");
        assert!(spans.iter().all(|s| s.style.bg.is_none()));
    }

    #[test]
    fn painted_caret_uses_cursor_colors_without_reverse() {
        let theme = Theme::dark_red();
        let caret = caret_cell_style(&theme);
        let spans = field_spans("hi", 2, true, 8, plain(), caret);
        let cell = spans.last().unwrap();
        assert_eq!(cell.content.as_ref(), " ");
        assert_eq!(cell.style.bg, Some(Color::from(&theme.cursor_fg)));
        assert_eq!(cell.style.fg, Some(Color::from(&theme.editor_bg)));
        assert!(!cell.style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn caret_past_the_end_clamps() {
        let spans = field_spans("hi", 9, true, 8, plain(), block());
        assert_eq!(shown(&spans), "hi ");
        assert_eq!(spans.last().unwrap().style.bg, Some(Color::Red));
    }
}
