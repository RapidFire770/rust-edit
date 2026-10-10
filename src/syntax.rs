// Syntect wrapper, language detection, highlight cache. Sprint 1.
// Returns ratatui Spans for visible window only. See Design Spec §4 and AGENTS.

#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, HighlightState, ThemeSet};
use syntect::parsing::{ParseState, SyntaxSet};

use crate::buffer::Language;

pub struct SyntaxHighlighter {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

/// Syntect parse + highlight state at a line boundary. Callers cache these so a
/// mid-file window can resume without replaying the whole buffer.
#[derive(Clone, Debug)]
pub struct HighlightSnapshot {
    pub highlight: HighlightState,
    pub parse: ParseState,
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
        // Produced by build.rs from the default newlines set plus assets/syntaxes.
        // A bad dump is a compile artifact, not a runtime condition.
        let syntax_set = syntect::dumps::from_uncompressed_data(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/syntaxes.packdump"
        )))
        .expect("embedded syntax packdump");
        Self {
            syntax_set,
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
            Language::Jsonc => "JSONC",
            Language::Toml => "TOML",
            Language::Yaml => "YAML",
            Language::Ini => "INI",
            Language::Bash => "Bourne Again Shell (bash)",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Javascript => "JavaScript",
            Language::Typescript => "TypeScript",
            Language::Java => "Java",
            Language::CSharp => "C#",
            Language::Go => "Go",
            Language::Zig => "Zig",
            Language::Php => "PHP",
            Language::Ruby => "Ruby",
            Language::Perl => "Perl",
            Language::Sql => "SQL",
            Language::Html => "HTML",
            Language::Css => "CSS",
            Language::Xml => "XML",
            Language::Lua => "Lua",
            Language::Scala => "Scala",
            Language::Haskell => "Haskell",
            Language::R => "R",
            Language::Clojure => "Clojure",
            Language::Erlang => "Erlang",
            Language::Groovy => "Groovy",
            Language::Makefile => "Makefile",
            Language::JavaProperties => "Java Properties",
            Language::ObjectiveC => "Objective-C",
            Language::ObjectiveCpp => "Objective-C++",
            Language::Diff => "Diff",
            Language::Latex => "LaTeX",
            Language::Tex => "TeX",
            Language::Tcl => "Tcl",
            Language::OCaml => "OCaml",
            Language::RestructuredText => "reStructuredText",
            Language::Lisp => "Lisp",
            Language::D => "D",
            Language::Batch => "Batch File",
            Language::Basic => "BASIC",
            Language::KickAsm => "KickAss",
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

    /// Fresh parse/highlight state for `lang` (before any lines are consumed).
    pub fn initial_snapshot(&self, lang: Language) -> HighlightSnapshot {
        let (syntax, theme) = self.syntax_and_theme(lang);
        let h = HighlightLines::new(syntax, theme);
        let (highlight, parse) = h.state();
        HighlightSnapshot { highlight, parse }
    }

    /// Highlight `lines[start..start+count]`, carrying parse state from `lines[0..start]`.
    ///
    /// Returns `None` if `start` is past the end of `lines`. `start == lines.len()`
    /// (including `count == 0`) yields `Some(vec![])`.
    pub fn highlight_range(
        &self,
        lines: &[&str],
        start: usize,
        count: usize,
        lang: Language,
    ) -> Option<Vec<Vec<Span<'static>>>> {
        if start > lines.len() {
            return None;
        }
        let end = start.saturating_add(count).min(lines.len());
        let snap = self.initial_snapshot(lang);
        // Drive from line 0 so prefix state is applied; collect only `[start, end)`.
        let (spans, _end_state) = self.highlight_slice_from_state(lines, start, 0, end, lang, snap);
        Some(spans)
    }

    /// Drive syntect from `from_line` through `end_line` (exclusive).
    /// Spans are collected only for `[span_start, end_line)`.
    /// Checkpoints are the caller's problem; this helper only returns the end snapshot.
    pub fn highlight_slice_from_state(
        &self,
        lines: &[&str],
        span_start: usize,
        from_line: usize,
        end_line: usize,
        lang: Language,
        start_state: HighlightSnapshot,
    ) -> (Vec<Vec<Span<'static>>>, HighlightSnapshot) {
        if from_line >= end_line {
            return (Vec::new(), start_state);
        }
        let (_syntax, theme) = self.syntax_and_theme(lang);
        let mut h = HighlightLines::from_state(theme, start_state.highlight, start_state.parse);
        let mut out = Vec::new();
        let end = end_line.min(lines.len());
        for (i, line) in lines.iter().enumerate().take(end).skip(from_line) {
            let stripped = line.trim_end_matches(['\n', '\r']);
            let highlighted_line = format!("{}\n", stripped);
            let ranges = h
                .highlight_line(&highlighted_line, &self.syntax_set)
                .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), stripped)]);
            if i >= span_start {
                out.push(ranges_to_spans(ranges));
            }
        }
        let (highlight, parse) = h.state();
        (out, HighlightSnapshot { highlight, parse })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Language;
    use ratatui::text::Span;

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

    #[test]
    fn mid_file_window_matches_full_file_slice() {
        let sh = SyntaxHighlighter::new();
        let lines = [
            "fn before() {}",
            "/* comment opener",
            "still inside the block comment",
            "closer */",
            "fn after() {}",
        ];
        let joined = lines.join("\n");
        let full = sh.highlight(&joined, Language::Rust);
        assert_eq!(full.len(), 5);

        // Fresh HighlightLines at line 2 is the CURRENT (buggy) window behavior.
        let isolated = sh.highlight_window(std::iter::once(lines[2]), Language::Rust);
        let full_mid_fg = full[2].first().and_then(|s| s.style.fg);
        let isolated_fg = isolated[0].first().and_then(|s| s.style.fg);
        assert_ne!(
            full_mid_fg, isolated_fg,
            "precondition: starting HighlightLines at the continuation line loses comment state"
        );

        // The new API must resume from state after lines 0..2.
        let resumed = sh
            .highlight_range(lines.as_slice(), 2, 1, Language::Rust)
            .expect("range in bounds");
        assert_eq!(resumed.len(), 1);
        let resumed_fg = resumed[0].first().and_then(|s| s.style.fg);
        assert_eq!(
            resumed_fg, full_mid_fg,
            "resuming from prior parse state must keep the block-comment color"
        );
    }

    #[test]
    fn every_language_resolves_to_a_real_syntax() {
        let sh = SyntaxHighlighter::new();
        for lang in crate::buffer::Language::ALL {
            let name = sh.syntax_name(*lang);
            let found = sh
                .syntax_set
                .find_syntax_by_name(name)
                .unwrap_or_else(|| panic!("{lang:?} looked up {name:?} and missed"));
            assert_eq!(found.name, name, "{lang:?}");
            if *lang != Language::Plain {
                assert_ne!(found.name, "Plain Text", "{lang:?} fell back to plain text");
            }
        }
    }

    fn distinct_fg_count(sh: &SyntaxHighlighter, text: &str, lang: Language) -> usize {
        let mut colors = Vec::new();
        for line in sh.highlight(text, lang) {
            for span in line {
                if span.content.is_empty() {
                    continue;
                }
                if !colors.contains(&span.style.fg) {
                    colors.push(span.style.fg);
                }
            }
        }
        colors.len()
    }

    fn fg_containing(spans: &[Span], piece: &str) -> Option<ratatui::style::Color> {
        spans
            .iter()
            .find(|span| span.content.contains(piece))
            .and_then(|span| span.style.fg)
    }

    #[test]
    fn added_grammars_use_more_than_one_color() {
        let sh = SyntaxHighlighter::new();
        assert!(distinct_fg_count(&sh, "public class A {}", Language::Java) > 1);
        assert!(distinct_fg_count(&sh, "key = \"v\"", Language::Toml) > 1);
        assert!(distinct_fg_count(&sh, "const x: number = 1;", Language::Typescript) > 1);
        assert!(distinct_fg_count(&sh, "pub fn main() void {}", Language::Zig) > 1);

        let ts = sh.highlight("const \"interface\"\n// interface\n", Language::Typescript);
        let keyword_fg = fg_containing(&ts[0], "const");
        let in_string = fg_containing(&ts[0], "interface");
        assert_ne!(
            keyword_fg, in_string,
            "a TypeScript string must not pick up a keyword color"
        );
        let comment_fg = fg_containing(&ts[1], "//");
        let in_comment = fg_containing(&ts[1], "interface");
        assert_eq!(
            comment_fg, in_comment,
            "a word inside a TypeScript comment stays a comment"
        );

        let jsonc = sh.highlight("// c\n{ \"a\": 1 }", Language::Jsonc);
        let comment_fg = fg_containing(&jsonc[0], "c");
        let key_fg = fg_containing(&jsonc[1], "a");
        assert_ne!(comment_fg, key_fg, "JSONC comment and string must differ");

        let quoted = sh.highlight("\"http://x\"", Language::Jsonc);
        let inside = fg_containing(&quoted[0], "http");
        assert_eq!(
            inside, key_fg,
            "a URL inside a JSONC string must stay a string, not a comment"
        );
    }

    #[test]
    fn basic_grammar_colors_code_strings_and_comments() {
        let sh = SyntaxHighlighter::new();
        assert!(distinct_fg_count(&sh, "10 PRINT \"HI\"\nREM note\n", Language::Basic) > 1);

        let quoted = sh.highlight("10 PRINT \"REM HI\"\n", Language::Basic);
        let inside = fg_containing(&quoted[0], "REM");
        let hi = fg_containing(&quoted[0], "HI");
        let keyword = fg_containing(&quoted[0], "PRINT");
        assert_eq!(inside, hi, "quoted REM and HI stay one string color");
        assert_ne!(inside, keyword, "REM inside quotes stays a string");
        assert_ne!(hi, keyword, "HI inside quotes stays a string");

        let comment = sh.highlight("POKE 1,2: REM note\n", Language::Basic);
        let poke = fg_containing(&comment[0], "POKE");
        let note = fg_containing(&comment[0], "note");
        assert_ne!(poke, note, "a trailing REM starts a comment");
        assert_eq!(
            fg_containing(&comment[0], "REM"),
            note,
            "the REM token is part of the comment"
        );

        let remote = sh.highlight("REMOTE = 1\n", Language::Basic);
        assert_ne!(
            fg_containing(&remote[0], "OTE"),
            note,
            "REMOTE is not a REM comment"
        );

        let contents = |spans: &[Span]| {
            spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<Vec<_>>()
        };

        let channel = sh.highlight("PRINT#1\n", Language::Basic);
        assert!(
            channel[0]
                .iter()
                .any(|span| span.content.contains("PRINT#")),
            "PRINT# must be one span, got {:?}",
            contents(&channel[0])
        );

        let rem_colon = sh.highlight("10 REM: teh\n", Language::Basic);
        assert_eq!(
            fg_containing(&rem_colon[0], "teh"),
            fg_containing(&rem_colon[0], "REM"),
            "10 REM: is a comment, spans {:?}",
            contents(&rem_colon[0])
        );

        let print_colon = sh.highlight("10 PRINT:\n", Language::Basic);
        assert_eq!(
            fg_containing(&print_colon[0], "PRINT"),
            poke,
            "10 PRINT: is a keyword, spans {:?}",
            contents(&print_colon[0])
        );

        let label = sh.highlight("labelSetup:\n", Language::Basic);
        assert_ne!(
            fg_containing(&label[0], "labelSetup"),
            poke,
            "labelSetup: is not a keyword, spans {:?}",
            contents(&label[0])
        );
    }

    #[test]
    fn kickass_grammar_colors_mnemonics_numbers_and_comments() {
        let sh = SyntaxHighlighter::new();
        assert!(distinct_fg_count(&sh, "main:\n    lda #$02\n", Language::KickAsm) > 1);

        let quoted = sh.highlight("lda \"// not\"\n", Language::KickAsm);
        let slash = fg_containing(&quoted[0], "//");
        let word = fg_containing(&quoted[0], "not");
        let mnem = fg_containing(&quoted[0], "lda");
        assert_eq!(slash, word, "// inside a string stays a string");
        assert_ne!(slash, mnem);

        let block = sh.highlight("/*\nstill\n*/\n", Language::KickAsm);
        assert_eq!(
            fg_containing(&block[0], "/*"),
            fg_containing(&block[1], "still"),
            "a block comment continues onto the next line"
        );
    }
}
