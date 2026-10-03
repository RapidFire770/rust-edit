// Spelling suggestion popup. State lives on `App`; this module paints and
// exposes the geometry the mouse handler hit-tests.

#![allow(dead_code)]

use std::ops::Range;

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::button::ButtonRow;
use crate::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellingFocus {
    List,
    Buttons,
}

#[derive(Debug, Clone)]
pub struct SpellingState {
    pub word: String,
    pub range: Range<usize>,
    pub suggestions: Vec<String>,
    pub selected: usize,
    pub button: usize,
    pub focus: SpellingFocus,
    pub row_hover: Option<usize>,
    pub button_hover: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellingHit {
    Suggestion(usize),
    Button(usize),
    Inside,
    Outside,
}

pub fn footer_labels() -> [&'static str; 3] {
    ["[ Add word ]", "[ Ignore ]", "[ Cancel ]"]
}

fn content_rows(count: usize) -> u16 {
    count.max(1) as u16
}

pub fn spelling_rect(area: Rect, suggestions: &[String]) -> Rect {
    let rows = content_rows(suggestions.len());
    let height = rows.saturating_add(4).min(area.height);
    let longest = suggestions
        .iter()
        .map(|word| word.chars().count())
        .max()
        .unwrap_or(0)
        .max("No suggestions".chars().count());
    let labels = footer_labels();
    let footer = labels
        .iter()
        .map(|label| label.chars().count())
        .sum::<usize>()
        + 2
        + 3 * labels.len().saturating_sub(1);
    let inner = longest.saturating_add(4).max(footer).max(24);
    let width = (inner as u16 + 2).clamp(42, 72).min(area.width.max(1));
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

pub fn suggestion_ys(popup: Rect, count: usize) -> Vec<u16> {
    (0..count).map(|index| popup.y + 1 + index as u16).collect()
}

pub fn footer_y(popup: Rect, count: usize) -> u16 {
    popup.y + content_rows(count) + 2
}

pub fn hit_test(area: Rect, suggestions: &[String], x: u16, y: u16) -> SpellingHit {
    let popup = spelling_rect(area, suggestions);
    let inside = x >= popup.x
        && x < popup.x.saturating_add(popup.width)
        && y >= popup.y
        && y < popup.y.saturating_add(popup.height);
    if !inside {
        return SpellingHit::Outside;
    }
    if y == footer_y(popup, suggestions.len()) {
        let labels = footer_labels();
        // The paragraph sits inside the border, so the button row starts one
        // column in from the popup's left edge.
        let ranges = ButtonRow::button_x_ranges(popup.x.saturating_add(1), &labels);
        if let Some(index) = ranges
            .iter()
            .position(|(start, end)| x >= *start && x < *end)
        {
            return SpellingHit::Button(index);
        }
        return SpellingHit::Inside;
    }
    for (index, row_y) in suggestion_ys(popup, suggestions.len())
        .into_iter()
        .enumerate()
    {
        if y == row_y {
            return SpellingHit::Suggestion(index);
        }
    }
    SpellingHit::Inside
}

fn title_for(word: &str) -> String {
    let full = format!("Spelling: {word}");
    if full.chars().count() <= 32 {
        full
    } else {
        let mut short: String = full.chars().take(31).collect();
        short.push('…');
        short
    }
}

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, app: &App) {
    let Some(state) = &app.spelling else {
        return;
    };
    let popup = spelling_rect(area, &state.suggestions);
    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let mut lines = Vec::new();
    if state.suggestions.is_empty() {
        lines.push(Line::from(Span::styled("  No suggestions", dim)));
    } else {
        for (index, word) in state.suggestions.iter().enumerate() {
            let hovered = state.row_hover == Some(index);
            let chosen = state.focus == SpellingFocus::List
                && state.row_hover.is_none()
                && state.selected == index;
            let active = hovered || chosen;
            let style = if active { accent } else { dim };
            let prefix = if active { "> " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(word.clone(), style),
            ]));
        }
    }
    lines.push(Line::from(""));
    let labels = footer_labels();
    let mut spans = vec![Span::raw("  ")];
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("   "));
        }
        let hovered = state.button_hover == Some(index);
        let chosen = state.focus == SpellingFocus::Buttons
            && state.button_hover.is_none()
            && state.button == index;
        let style = if hovered || chosen { accent } else { dim };
        spans.push(Span::styled(*label, style));
    }
    lines.push(Line::from(spans));

    let block = Block::default()
        .title(title_for(&state.word))
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());
    let para = Paragraph::new(lines).block(block).style(dim);
    f.render_widget(Clear, popup);
    f.render_widget(para, popup);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_finds_a_suggestion_and_each_button() {
        let area = Rect::new(0, 0, 80, 24);
        let suggestions = vec!["the".to_string(), "teh".to_string()];
        let popup = spelling_rect(area, &suggestions);
        let rows = suggestion_ys(popup, suggestions.len());
        assert_eq!(
            hit_test(area, &suggestions, popup.x + 4, rows[0]),
            SpellingHit::Suggestion(0)
        );
        assert_eq!(
            hit_test(area, &suggestions, popup.x + 4, rows[1]),
            SpellingHit::Suggestion(1)
        );
        let footer = footer_y(popup, suggestions.len());
        // Painted labels start after the border and the two leading spaces.
        let mut painted = popup.x + 3;
        for (index, label) in footer_labels().iter().enumerate() {
            let len = label.chars().count() as u16;
            assert_eq!(
                hit_test(area, &suggestions, painted, footer),
                SpellingHit::Button(index)
            );
            assert_eq!(
                hit_test(area, &suggestions, painted + len - 1, footer),
                SpellingHit::Button(index)
            );
            painted = painted + len + 3;
        }
        assert_eq!(hit_test(area, &suggestions, 0, 0), SpellingHit::Outside);
    }
}
