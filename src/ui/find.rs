// Find / Replace bar or popup logic.
// Presentation + small state helpers.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Which element of the Find/Replace bar currently has focus.
///
/// The three-state model lets the user type any character (including `n`/`N`)
/// into the Query/Replace boxes, then Tab into `Command` mode to drive the
/// navigation/replace keys. While the bar is open the editor stays blocked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FindFocus {
    /// Typing characters edits the query string.
    #[default]
    Query,
    /// Typing characters edits the replace string.
    Replace,
    /// No input box focused; n/N/r/Enter/c/x/w drive nav & replace & options.
    Command,
}

impl FindFocus {
    /// Tab cycles Query → Replace → Command → Query.
    pub fn next(self) -> Self {
        match self {
            FindFocus::Query => FindFocus::Replace,
            FindFocus::Replace => FindFocus::Command,
            FindFocus::Command => FindFocus::Query,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FindState {
    pub query: String,
    pub replace: String,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub matches: Vec<std::ops::Range<usize>>, // actual matched byte ranges (for correct length with regex etc)
    pub current_match: usize,
    /// Which element of the bar has focus (3-state model).
    pub focus: FindFocus,
}

impl FindState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// True when keystrokes should edit a text field rather than navigate.
    pub fn editing_text(&self) -> bool {
        matches!(self.focus, FindFocus::Query | FindFocus::Replace)
    }
}

pub(crate) fn find_field_spans(
    text: &str,
    focused: bool,
    width: usize,
    text_style: Style,
    caret_style: Style,
) -> Vec<Span<'static>> {
    crate::ui::text_field::field_spans(
        text,
        text.chars().count(),
        focused,
        width,
        text_style,
        caret_style,
    )
}

pub fn render_bar(f: &mut Frame, area: Rect, state: &FindState, theme: &crate::theme::Theme) {
    let bg = Style::default().bg(Color::Rgb(50, 50, 70)).fg(Color::White);
    let dim = Style::default()
        .bg(Color::Rgb(50, 50, 70))
        .fg(Color::Rgb(160, 160, 180));
    let accent = Style::default()
        .bg(Color::Rgb(50, 50, 70))
        .fg(Color::Rgb(255, 180, 80))
        .add_modifier(Modifier::BOLD);
    let caret_style = crate::ui::text_field::caret_cell_style(theme);

    // Budget the query/replace fields to a fraction of the bar width so a long
    // query never pushes the toggles and hints off-screen (the bar scales with
    // the window size). Reserve columns for the fixed scaffolding.
    let w = area.width as usize;
    let field_budget = (w.saturating_sub(46) / 3).max(6);

    let qstyle = if state.focus == FindFocus::Query {
        accent
    } else {
        bg
    };
    let rstyle = if state.focus == FindFocus::Replace {
        accent
    } else {
        bg
    };

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(" Find:", dim));
    spans.push(Span::styled(" [", qstyle));
    spans.extend(find_field_spans(
        &state.query,
        state.focus == FindFocus::Query,
        field_budget,
        qstyle,
        caret_style,
    ));
    spans.push(Span::styled("] ", qstyle));

    spans.push(Span::styled("Rep:", dim));
    spans.push(Span::styled(" [", rstyle));
    spans.extend(find_field_spans(
        &state.replace,
        state.focus == FindFocus::Replace,
        field_budget,
        rstyle,
        caret_style,
    ));
    spans.push(Span::styled("] ", rstyle));

    spans.push(Span::styled(
        format!("| {} matches ", state.matches.len()),
        bg,
    ));

    // Render each toggle on its own so an active one stands out (accent) while
    // inactive ones stay dim — makes the c/x/w options easier to discover.
    spans.push(Span::styled("| ", dim));
    for (ch, on, label) in [
        ('c', state.case_sensitive, "case "),
        ('x', state.use_regex, "regex "),
        ('w', state.whole_word, "word "),
    ] {
        let ch = if on { ch.to_ascii_uppercase() } else { ch };
        let style = if on { accent } else { dim };
        spans.push(Span::styled(format!("[{ch}]{label}"), style));
    }

    // Right-side focus/hint depends on mode. In Command mode the nav keys are live.
    match state.focus {
        FindFocus::Query => spans.push(Span::styled("« typing query — Tab=next  Esc=close ", dim)),
        FindFocus::Replace => {
            spans.push(Span::styled("« typing replace — Tab=cmd  Esc=close ", dim))
        }
        FindFocus::Command => spans.push(Span::styled(
            "« Cmd: n/N=nav  r/Enter=rep  Ctrl+A=all  c/x/w=opts  Tab=query  Esc ",
            accent,
        )),
    }

    let bar = Paragraph::new(Line::from(spans)).style(bg);
    f.render_widget(bar, area);
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
    fn focused_query_ends_with_the_caret_cell() {
        let spans = find_field_spans("fn", true, 8, plain(), block());
        assert_eq!(shown(&spans), "fn ");
        assert_eq!(spans.last().unwrap().style.bg, Some(Color::Red));
    }

    #[test]
    fn unfocused_replace_keeps_the_cell_plain() {
        let spans = find_field_spans("fn", false, 8, plain(), block());
        assert_eq!(shown(&spans), "fn ");
        assert!(spans.iter().all(|s| s.style.bg.is_none()));
    }
}
