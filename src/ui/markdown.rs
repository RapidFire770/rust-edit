// Basic Markdown preview renderer using pulldown-cmark -> Ratatui Spans/Lines.
// Uses Theme colors for headings (bold + accent), code (bg), lists, etc.

#![allow(dead_code)]

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::Theme;

pub fn render_preview(md: &str, theme: &Theme) -> Vec<Line<'static>> {
    let parser = Parser::new(md);
    let mut lines: Vec<Line> = Vec::new();
    let mut current_line: Vec<Span<'static>> = Vec::new();
    let mut in_code = false;
    let mut code_lang = String::new();
    let accent = theme
        .accent_style()
        .fg
        .unwrap_or(Color::from(&theme.editor_fg));
    let code_bg = Color::Rgb(40, 44, 52); // dark code bg

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_line)));
                }
                // headings use bold + accent
                let mut s = Style::default().fg(accent).add_modifier(Modifier::BOLD);
                // size via more bold or just level note
                if level as u8 <= 2 {
                    s = s.add_modifier(Modifier::UNDERLINED);
                }
                current_line.push(Span::styled(format!("{} ", "#".repeat(level as usize)), s));
            }
            Event::End(TagEnd::Heading(_)) => {
                lines.push(Line::from(std::mem::take(&mut current_line)));
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code = true;
                if let pulldown_cmark::CodeBlockKind::Fenced(l) = kind {
                    code_lang = l.to_string();
                }
                if !current_line.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_line)));
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                code_lang.clear();
                if !current_line.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_line)));
                }
            }
            Event::Start(Tag::List(_)) | Event::End(TagEnd::List(_)) => {
                // simple list handling
            }
            Event::Start(Tag::Item) => {
                current_line.push(Span::raw("• "));
            }
            Event::End(TagEnd::Item) => {
                lines.push(Line::from(std::mem::take(&mut current_line)));
            }
            Event::Text(t) => {
                let style = if in_code {
                    Style::default()
                        .bg(code_bg)
                        .fg(Color::from(&theme.editor_fg))
                } else {
                    Style::default().fg(Color::from(&theme.editor_fg))
                };
                current_line.push(Span::styled(t.to_string(), style));
            }
            Event::Code(t) => {
                current_line.push(Span::styled(
                    format!("`{}`", t),
                    Style::default().bg(code_bg).fg(Color::from(&theme.string)),
                ));
            }
            Event::SoftBreak | Event::HardBreak => {
                lines.push(Line::from(std::mem::take(&mut current_line)));
            }
            Event::Start(Tag::Strong) => {
                // will apply bold to following, simple: wrap or note
            }
            Event::End(TagEnd::Strong) => {}
            Event::Start(Tag::Emphasis) => {}
            Event::End(TagEnd::Emphasis) => {}
            _ => {}
        }
    }
    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::raw("(empty markdown)")));
    }
    lines
}
