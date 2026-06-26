// Find / Replace bar or popup logic. MVP basic + Phase 1 full.
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

/// Truncate a string for display so it never exceeds `max` visible cells,
/// keeping its right end (where the editing cursor is). Prefixes with `…` when
/// truncated. Operates on chars, not bytes, so multi-byte text is handled.
fn truncate_keep_end(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = chars[chars.len() - (max - 1)..].iter().collect();
    out.insert(0, '…');
    out
}

pub fn render_bar(f: &mut Frame, area: Rect, state: &FindState) {
    let bg = Style::default().bg(Color::Rgb(50, 50, 70)).fg(Color::White);
    let dim = Style::default()
        .bg(Color::Rgb(50, 50, 70))
        .fg(Color::Rgb(160, 160, 180));
    let accent = Style::default()
        .bg(Color::Rgb(50, 50, 70))
        .fg(Color::Rgb(255, 180, 80))
        .add_modifier(Modifier::BOLD);

    // Budget the query/replace fields to a fraction of the bar width so a long
    // query never pushes the toggles and hints off-screen (the bar scales with
    // the window size). Reserve columns for the fixed scaffolding.
    let w = area.width as usize;
    let field_budget = (w.saturating_sub(46) / 3).max(6);

    let q = truncate_keep_end(&state.query, field_budget);
    let r = truncate_keep_end(&state.replace, field_budget);

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(" Find:", dim));

    let qstyle = if state.focus == FindFocus::Query {
        accent
    } else {
        bg
    };
    spans.push(Span::styled(format!(" [{q}] "), qstyle));

    spans.push(Span::styled("Rep:", dim));
    let rstyle = if state.focus == FindFocus::Replace {
        accent
    } else {
        bg
    };
    spans.push(Span::styled(format!(" [{r}] "), rstyle));

    spans.push(Span::styled(
        format!("| {} matches ", state.matches.len()),
        bg,
    ));

    let case_c = if state.case_sensitive { 'C' } else { 'c' };
    let re_x = if state.use_regex { 'X' } else { 'x' };
    let word_w = if state.whole_word { 'W' } else { 'w' };
    spans.push(Span::styled(
        format!("| [{case_c}]case [{re_x}]regex [{word_w}]word "),
        dim,
    ));

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
