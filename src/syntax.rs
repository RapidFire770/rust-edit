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

/// Convert one line's syntect `(Style, &str)` ranges into ratatui `Span`s.
/// Foreground color + font modifiers only; the editor background is supplied by
/// the Paragraph/Theme, so we deliberately drop syntect's per-range background.
///
/// Defense in depth: we also strip any trailing `\n`/`\r` off the final emitted
/// span. The highlighter normalizes its input, but if a future caller ever feeds
/// syntect text that ends in a newline we must not let that leak into the render
/// loop (it desyncs the visible cursor from the logical buffer cursor).
fn ranges_to_spans(ranges: Vec<(syntect::highlighting::Style, &str)>) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = ranges
        .into_iter()
        .map(|(style, text)| {
            let mut rat_style = Style::default().fg(Color::Rgb(
                style.foreground.r,
                style.foreground.g,
                style.foreground.b,
            ));
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

    // Trim trailing newlines off the last span only (they can only appear at the
    // end of the line's text). `Span::content` is a `Cow<'static, str>`.
    if let Some(last) = spans.last_mut() {
        let trimmed = last.content.trim_end_matches(['\n', '\r']);
        if trimmed.len() != last.content.len() {
            last.content = trimmed.to_string().into();
        }
    }
    spans
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
    /// Uses a single `HighlightLines` for the whole text so multi-line syntax
    /// state (block comments, raw strings) carries across lines correctly.
    pub fn highlight(&self, text: &str, lang: Language) -> Vec<Vec<Span<'static>>> {
        let (syntax, theme) = self.syntax_and_theme(lang);
        let mut highlighter = HighlightLines::new(syntax, theme);
        self.highlight_lines(text.lines(), &mut highlighter)
    }

    /// Resolve the syntect SyntaxReference + Theme for a language (shared by both
    /// highlight paths). Falls back to plain text if the language is unknown.
    fn syntax_and_theme(
        &self,
        lang: Language,
    ) -> (
        &syntect::parsing::SyntaxReference,
        &syntect::highlighting::Theme,
    ) {
        let syntax_name = self.syntax_name(lang);
        let syntax = self
            .syntax_set
            .find_syntax_by_name(syntax_name)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        let theme = &self.theme_set.themes["base16-ocean.dark"];
        (syntax, theme)
    }

    /// Drive a single `HighlightLines` across a sequence of lines and convert the
    /// syntect output to ratatui `Span`s. Reusing one highlighter (built once per
    /// visible window) avoids the per-line `HighlightLines::new` cost that caused
    /// lag on large files, and keeps multi-line constructs highlighted correctly.
    fn highlight_lines(
        &self,
        lines: impl Iterator<Item = impl AsRef<str>>,
        highlighter: &mut HighlightLines,
    ) -> Vec<Vec<Span<'static>>> {
        let mut out = Vec::new();
        for line in lines {
            // ropey's `.line(idx)` INCLUDES the trailing terminator (`\n`, and on
            // CRLF files a stray `\r` precedes it). Callers (the cache path) feed
            // us those raw lines. We must normalize to the bare content first,
            // then append exactly one `\n`, so (a) syntect keeps getting the
            // terminator it expects for correct multi-line state, and (b) the
            // spans we emit never carry a newline into the editor render loop —
            // which previously desynced the visible cursor from the logical one
            // on multi-line/highlighted buffers.
            let stripped = line.as_ref().trim_end_matches(['\n', '\r']);
            let highlighted_line = format!("{}\n", stripped);
            let ranges = highlighter
                .highlight_line(&highlighted_line, &self.syntax_set)
                .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), stripped)]);

            out.push(ranges_to_spans(ranges));
        }

        // If input was empty, ensure at least one empty line entry.
        if out.is_empty() {
            out.push(vec![Span::raw("")]);
        }
        out
    }

    /// Highlight an ordered window of lines using ONE `HighlightLines`.
    ///
    /// This is the entry point the editor should use for the visible region: it
    /// builds the parser state machine once (not once per line) and feeds lines in
    /// order, so both performance and multi-line correctness are correct. Lines are
    /// passed as an iterator so callers can slice a rope without allocating a joined
    /// `String`.
    pub fn highlight_window<'a>(
        &self,
        lines: impl Iterator<Item = &'a str>,
        lang: Language,
    ) -> Vec<Vec<Span<'static>>> {
        let (syntax, theme) = self.syntax_and_theme(lang);
        let mut highlighter = HighlightLines::new(syntax, theme);
        self.highlight_lines(lines, &mut highlighter)
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
        if start_line >= lines.len() {
            return vec![vec![Span::raw("")]];
        }
        self.highlight_window(lines[start_line..end].iter().copied(), lang)
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

    #[test]
    fn highlight_window_uses_shared_multiline_state() {
        // A Rust block comment spans three lines. When the window is highlighted
        // with a single HighlightLines, the comment style must carry across lines
        // (the continuation lines should share the same foreground as the opener,
        // not revert to default). This guards both correctness and the single-
        // highlighter-per-window performance path.
        let sh = SyntaxHighlighter::new();
        let code = [
            "/* this is a",
            "multiline block comment",
            "still inside*/ fn x(){}",
        ];
        let highlighted = sh.highlight_window(code.into_iter(), Language::Rust);
        assert_eq!(highlighted.len(), 3);

        // Opener and the middle line must use the same (comment) foreground.
        let first_fg = highlighted[0]
            .first()
            .map(|s| s.style.fg)
            .expect("first line has a span");
        let mid_fg = highlighted[1]
            .first()
            .map(|s| s.style.fg)
            .expect("middle line has a span");
        assert_eq!(
            first_fg, mid_fg,
            "block comment continuation must keep the comment color across lines"
        );

        // Compare against the buggy old approach: highlighting each line in
        // isolation would lose the comment state on the middle line.
        let isolated_mid = sh.highlight(code[1], Language::Rust);
        let isolated_mid_fg = isolated_mid[0].first().map(|s| s.style.fg).unwrap_or(None);
        assert_ne!(
            mid_fg, isolated_mid_fg,
            "isolated highlighting would drop block-comment state; window must not"
        );
    }

    /// Regression: rendered spans must NEVER contain `\n` or `\r`. The highlight
    /// pipeline historically fed syntect a line that already carried a trailing
    /// newline (ropey `.line()` includes it) and then appended another, leaking
    /// newline bytes into the emitted `Span` text. That shifted rendered content
    /// relative to the logical cursor on multi-line files (backspace deleting the
    /// wrong character, typing landing ahead of the cursor, blank-line spill-over).
    #[test]
    fn rendered_spans_contain_no_newlines() {
        let sh = SyntaxHighlighter::new();
        // Simulate the cache path: lines already include trailing newlines (as
        // `buf.line(i)` / `rope.line(i).to_string()` does for non-final lines).
        let lines = ["fn main() {\n", "    let x = 1;\n", "}\n"];
        let highlighted = sh.highlight_window(lines.into_iter(), Language::Rust);
        assert_eq!(highlighted.len(), 3);
        for (i, line_spans) in highlighted.iter().enumerate() {
            let joined: String = line_spans.iter().map(|s| &s.content[..]).collect();
            assert!(
                !joined.contains('\n') && !joined.contains('\r'),
                "line {} spans contain a newline/CRLF: {:?}",
                i,
                joined
            );
        }
    }
}
