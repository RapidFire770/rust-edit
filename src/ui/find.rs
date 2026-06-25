// Find / Replace bar or popup logic. MVP basic + Phase 1 full.
// Presentation + small state helpers.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

#[derive(Debug, Clone, Default)]
pub struct FindState {
    pub query: String,
    pub replace: String,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub matches: Vec<std::ops::Range<usize>>, // actual matched byte ranges (for correct length with regex etc)
    pub current_match: usize,
    /// When true, keyboard input appends to replace instead of query.
    pub editing_replace: bool,
}

impl FindState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }
}

pub fn render_bar(f: &mut Frame, area: Rect, state: &FindState) {
    let style = Style::default().bg(Color::Rgb(50, 50, 70)).fg(Color::White);
    let opts = format!(
        " [{}]case [{}]regex [{}]word ",
        if state.case_sensitive { "C" } else { "c" },
        if state.use_regex { "X" } else { "x" },
        if state.whole_word { "W" } else { "w" }
    );
    // Focus indicated with [brackets]
    let qdisp = if !state.editing_replace {
        format!("[{}]", state.query)
    } else {
        format!(" {} ", state.query)
    };
    let rdisp = if state.editing_replace {
        format!("[{}]", state.replace)
    } else {
        format!(" {} ", state.replace)
    };
    let text = format!(
        " Find:{}  Rep:{}  | {} matches {} | n/N or F3/Shift+F3 nav  r/Enter=rep  ^A=all  Tab=focus  Esc ",
        qdisp,
        rdisp,
        state.matches.len(),
        opts
    );
    let bar = Paragraph::new(Line::from(text)).style(style);
    f.render_widget(bar, area);
}
