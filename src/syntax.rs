// Syntect wrapper, language detection, highlight cache. Sprint 1.
// Returns ratatui Spans for visible window only. See Design Spec §4 and AGENTS.

#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::buffer::Language;

pub struct SyntaxHighlighter {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Pick a syntax name for the language. Falls back gracefully.
    fn syntax_name(&self, lang: Language) -> &'static str {
        match lang {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::Markdown => "Markdown",
            Language::Json => "JSON",
            Language::Toml => "TOML",
            Language::Yaml => "YAML",
            Language::Bash => "Bourne Again Shell (bash)",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Javascript => "JavaScript",
            Language::Typescript => "TypeScript",
            Language::Plain => "Plain Text",
        }
    }

    /// Highlight a full document (or slice) into lines of styled Spans.
    /// This is the main function used by the editor for the visible region.
    pub fn highlight(&self, text: &str, lang: Language) -> Vec<Vec<Span<'static>>> {
        let syntax_name = self.syntax_name(lang);
        let syntax = self
            .syntax_set
            .find_syntax_by_name(syntax_name)
            .or_else(|| Some(self.syntax_set.find_syntax_plain_text()))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        // Use a nice dark default theme from the bundled set
        let theme = &self.theme_set.themes["base16-ocean.dark"];

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut out = Vec::new();

        for line in text.lines() {
            // highlight_line expects trailing \n or not? docs say include for accuracy; we add \n
            let highlighted_line = format!("{}\n", line);
            let ranges = highlighter
                .highlight_line(&highlighted_line, &self.syntax_set)
                .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)]);

            let spans: Vec<Span<'static>> = ranges
                .into_iter()
                .map(|(style, text)| {
                    let mut rat_style = Style::default();
                    // foreground only (editor background is provided by the Paragraph/Theme)
                    rat_style = rat_style.fg(Color::Rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    ));
                    // Do NOT apply syntect background here to avoid unwanted per-letter bg.
                    // Only syntax colors + modifiers.
                    // modifiers
                    if style.font_style.contains(FontStyle::BOLD) {
                        rat_style = rat_style.add_modifier(Modifier::BOLD);
                    }
                    if style.font_style.contains(FontStyle::ITALIC) {
                        rat_style = rat_style.add_modifier(Modifier::ITALIC);
                    }
                    if style.font_style.contains(FontStyle::UNDERLINE) {
                        rat_style = rat_style.add_modifier(Modifier::UNDERLINED);
                    }
                    Span::styled(text.to_string(), rat_style)
                })
                .collect();

            out.push(spans);
        }

        // If file was empty, ensure at least one empty line entry
        if out.is_empty() {
            out.push(vec![Span::raw("")]);
        }
        out
    }

    /// Convenience: highlight only a visible window of lines (0-based, count).
    /// For large buffers we pass only the slice we render.
    pub fn highlight_visible(
        &self,
        full_text: &str,
        start_line: usize,
        count: usize,
        lang: Language,
    ) -> Vec<Vec<Span<'static>>> {
        let lines: Vec<&str> = full_text.lines().collect();
        let end = (start_line + count).min(lines.len());
        let slice = if start_line < lines.len() {
            lines[start_line..end].join("\n")
        } else {
            String::new()
        };
        self.highlight(&slice, lang)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Language;

    #[test]
    fn highlights_rust_without_panic() {
        let sh = SyntaxHighlighter::new();
        let code = "fn main() { println!(\"hi\"); }";
        let highlighted = sh.highlight(code, Language::Rust);
        assert!(!highlighted.is_empty());
        // At minimum we produced some spans
        assert!(highlighted.iter().any(|l| !l.is_empty()));
    }

    #[test]
    fn falls_back_on_plain() {
        let sh = SyntaxHighlighter::new();
        let highlighted = sh.highlight("just text", Language::Plain);
        assert_eq!(highlighted.len(), 1);
    }
}
