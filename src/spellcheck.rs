// English spellcheck. Hunspell lookup via spellbook, scoped to prose or to
// comments and strings. The dictionary is compiled into the binary.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};

use crate::buffer::Language;
use crate::highlight_cache::HighlightCache;
use crate::syntax::{HighlightSnapshot, SyntaxHighlighter};

pub const MAX_SUGGESTIONS: usize = 8;

/// Which parts of a buffer are eligible for spellcheck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellKind {
    /// JSON and Diff. Nothing is checked.
    Off,
    /// JSONC. Comment text only.
    Comments,
    /// Source code. Comment and string text.
    CommentsAndStrings,
    /// Plain text, Markdown, and reStructuredText. Everything except raw or
    /// embedded code spans.
    Prose,
}

pub fn spell_kind(language: Language) -> SpellKind {
    match language {
        Language::Plain | Language::Markdown | Language::RestructuredText => SpellKind::Prose,
        Language::Jsonc => SpellKind::Comments,
        Language::Json | Language::Diff => SpellKind::Off,
        _ => SpellKind::CommentsAndStrings,
    }
}

struct ScopePrefixes {
    comment: Scope,
    string: Scope,
    raw: Scope,
    embedded: Scope,
    source: Scope,
    variable: Scope,
    interpolation: Scope,
}

fn prefixes() -> &'static ScopePrefixes {
    static PREFIXES: OnceLock<ScopePrefixes> = OnceLock::new();
    PREFIXES.get_or_init(|| ScopePrefixes {
        comment: Scope::new("comment").expect("comment scope"),
        string: Scope::new("string").expect("string scope"),
        raw: Scope::new("markup.raw").expect("markup.raw scope"),
        embedded: Scope::new("meta.embedded").expect("meta.embedded scope"),
        source: Scope::new("source").expect("source scope"),
        variable: Scope::new("variable").expect("variable scope"),
        interpolation: Scope::new("meta.interpolation").expect("meta.interpolation scope"),
    })
}

fn base_dict() -> Result<&'static spellbook::Dictionary, &'static str> {
    static DICT: OnceLock<Result<spellbook::Dictionary, String>> = OnceLock::new();
    match DICT.get_or_init(|| {
        spellbook::Dictionary::new(
            include_str!("../assets/dictionaries/en_US.aff"),
            include_str!("../assets/dictionaries/en_US.dic"),
        )
        .map_err(|err| err.to_string())
    }) {
        Ok(dict) => Ok(dict),
        Err(message) => Err(message.as_str()),
    }
}

/// Straight apostrophe so Hunspell sees the form the word list uses.
fn normalize_apostrophe(word: &str) -> String {
    word.replace('’', "'")
}

fn fold_key(word: &str) -> String {
    normalize_apostrophe(word).to_lowercase()
}

/// One word per line. Blank lines and `#` comments are ignored.
pub fn personal_words_from_str(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                None
            } else {
                Some(fold_key(line))
            }
        })
        .collect()
}

#[cfg_attr(test, allow(dead_code))]
pub fn personal_dictionary_path() -> Option<PathBuf> {
    crate::config::Config::config_dir().map(|dir| dir.join("dictionary.txt"))
}

pub fn append_personal_word(path: &Path, word: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{word}")?;
    Ok(())
}

#[cfg(not(test))]
fn load_personal_words() -> HashSet<String> {
    let Some(path) = personal_dictionary_path() else {
        return HashSet::new();
    };
    match fs::read_to_string(path) {
        Ok(text) => personal_words_from_str(&text),
        Err(_) => HashSet::new(),
    }
}

pub struct SpellEngine {
    dict: &'static spellbook::Dictionary,
    personal: HashSet<String>,
    ignored: HashSet<String>,
}

impl SpellEngine {
    /// Embedded dictionary plus the user's personal word list.
    /// Tests skip the personal file so they cannot read or rewrite it.
    pub fn load() -> Result<Self, String> {
        let dict = base_dict().map_err(str::to_string)?;
        let personal = {
            #[cfg(test)]
            {
                HashSet::new()
            }
            #[cfg(not(test))]
            {
                load_personal_words()
            }
        };
        Ok(Self {
            dict,
            personal,
            ignored: HashSet::new(),
        })
    }

    pub fn suggestions(&self, word: &str) -> Vec<String> {
        let key = normalize_apostrophe(word);
        let mut out = Vec::new();
        self.dict.suggest(&key, &mut out);
        out.truncate(MAX_SUGGESTIONS);
        out
    }

    /// Remember `word` for this process and append it to `dictionary.txt`.
    /// The in-memory add sticks even when the write fails. Tests do not write.
    pub fn add_personal(&mut self, word: &str) -> Result<(), String> {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            return Err("empty word".to_string());
        }
        self.personal.insert(fold_key(trimmed));
        #[cfg(not(test))]
        if let Some(path) = personal_dictionary_path() {
            append_personal_word(&path, trimmed)
                .map_err(|_| "Could not write dictionary.txt".to_string())?;
        }
        Ok(())
    }

    pub fn ignore(&mut self, word: &str) {
        let trimmed = word.trim();
        if !trimmed.is_empty() {
            self.ignored.insert(fold_key(trimmed));
        }
    }

    pub fn is_misspelled(&self, word: &str) -> bool {
        if skip_token(word) {
            return false;
        }
        let key = normalize_apostrophe(word);
        let folded = key.to_lowercase();
        if self.ignored.contains(&folded) || self.personal.contains(&folded) {
            return false;
        }
        !self.dict.check(&key)
    }

    /// Misspelled words whose whole span sits inside one `regions` entry.
    /// Ranges are byte offsets into `text`.
    pub fn misspellings_in(&self, text: &str, regions: &[Range<usize>]) -> Vec<Range<usize>> {
        if regions.is_empty() {
            return Vec::new();
        }
        word_ranges(text)
            .into_iter()
            .filter(|word| {
                regions
                    .iter()
                    .any(|region| region.start <= word.start && word.end <= region.end)
            })
            .filter(|word| self.is_misspelled(&text[word.clone()]))
            .collect()
    }
}

/// All-ASCII capitals, digits, and one-letter tokens are not misspellings.
fn skip_token(word: &str) -> bool {
    if word.len() > spellbook::MAX_WORD_LEN {
        return true;
    }
    let mut letters = 0usize;
    let mut all_ascii_upper = true;
    for ch in word.chars() {
        if ch.is_ascii_digit() {
            return true;
        }
        if ch.is_alphabetic() {
            letters += 1;
            if !ch.is_ascii_uppercase() {
                all_ascii_upper = false;
            }
        }
    }
    letters < 2 || all_ascii_upper
}

/// Maximal letter runs. An apostrophe or hyphen stays inside the word when a
/// letter follows it (`don't`, `well-known`).
pub fn word_ranges(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let ch = match text[i..].chars().next() {
            Some(ch) => ch,
            None => break,
        };
        let len = ch.len_utf8();
        if ch.is_alphabetic() {
            let start = i;
            i += len;
            while i < text.len() {
                let c = match text[i..].chars().next() {
                    Some(c) => c,
                    None => break,
                };
                let n = c.len_utf8();
                if c.is_alphabetic() || c.is_ascii_digit() {
                    i += n;
                    continue;
                }
                if matches!(c, '\'' | '-' | '’') {
                    let next = i + n;
                    if text[next..]
                        .chars()
                        .next()
                        .is_some_and(|nxt| nxt.is_alphabetic() || nxt.is_ascii_digit())
                    {
                        i = next;
                        continue;
                    }
                }
                break;
            }
            out.push(start..i);
        } else {
            i += len;
        }
    }
    out
}

/// `TEH` → `THE`, `Teh` → `The`, otherwise the suggestion as the dictionary
/// returned it.
pub fn apply_case(original: &str, suggestion: &str) -> String {
    let letters: Vec<char> = original.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 2 && letters.iter().all(|c| c.is_ascii_uppercase()) {
        return suggestion.to_uppercase();
    }
    let mut chars = original.chars();
    if let Some(first) = chars.next() {
        let rest_lower = chars.all(|c| !c.is_alphabetic() || c.is_lowercase());
        if first.is_uppercase() && rest_lower {
            let mut suggested = suggestion.chars();
            if let Some(head) = suggested.next() {
                let mut out: String = head.to_uppercase().collect();
                out.extend(suggested.flat_map(|c| c.to_lowercase()));
                return out;
            }
        }
    }
    suggestion.to_string()
}

/// The misspelling that contains `cursor`, or that ends exactly on it.
pub fn misspelling_at(ranges: &[Range<usize>], cursor: usize) -> Option<Range<usize>> {
    ranges
        .iter()
        .find(|range| cursor >= range.start && cursor <= range.end)
        .cloned()
}

fn stack_checkable(stack: &ScopeStack, kind: SpellKind) -> bool {
    let prefixes = prefixes();
    match kind {
        SpellKind::Off => false,
        SpellKind::Comments => scoped_text(stack, false),
        SpellKind::CommentsAndStrings => scoped_text(stack, true),
        SpellKind::Prose => !stack.scopes.iter().any(|scope| {
            prefixes.raw.is_prefix_of(*scope)
                || prefixes.embedded.is_prefix_of(*scope)
                || prefixes.source.is_prefix_of(*scope)
        }),
    }
}

/// Outside-in: a comment (and a string, when `strings` is set) turns checking
/// on. A later variable, embedded, interpolation, or source scope turns it
/// off, so `$user` inside a shell string is not a misspelling.
fn scoped_text(stack: &ScopeStack, strings: bool) -> bool {
    let prefixes = prefixes();
    let mut on = false;
    for scope in &stack.scopes {
        if prefixes.comment.is_prefix_of(*scope)
            || (strings && prefixes.string.is_prefix_of(*scope))
        {
            on = true;
        }
        if prefixes.variable.is_prefix_of(*scope)
            || prefixes.source.is_prefix_of(*scope)
            || prefixes.embedded.is_prefix_of(*scope)
            || prefixes.interpolation.is_prefix_of(*scope)
        {
            on = false;
        }
    }
    on
}

fn note_span(
    stack: &ScopeStack,
    from: usize,
    to: usize,
    kind: SpellKind,
    open: &mut Option<usize>,
    regions: &mut Vec<Range<usize>>,
) {
    if from >= to {
        return;
    }
    let on = stack_checkable(stack, kind);
    match (*open, on) {
        (None, true) => *open = Some(from),
        (Some(start), false) => {
            if from > start {
                regions.push(start..from);
            }
            *open = None;
        }
        _ => {}
    }
}

/// Advance `parse` and `stack` across one rope line. Returned ranges are byte
/// offsets into the line with its trailing newline removed. The line is fed
/// to syntect the same way the highlighter does (`content` + one `\n`).
pub fn regions_on_line(
    parse: &mut ParseState,
    stack: &mut ScopeStack,
    syntax_set: &SyntaxSet,
    raw_line: &str,
    kind: SpellKind,
) -> Vec<Range<usize>> {
    let stripped_len = raw_line.trim_end_matches(['\n', '\r']).len();
    let owned = format!("{}\n", &raw_line[..stripped_len]);
    let ops = match parse.parse_line(&owned, syntax_set) {
        Ok(ops) => ops,
        Err(_) => return Vec::new(),
    };
    if kind == SpellKind::Off {
        return Vec::new();
    }
    let mut regions = Vec::new();
    let mut cursor = 0usize;
    let mut open: Option<usize> = None;
    for (pos, op) in &ops {
        let pos = (*pos).min(stripped_len);
        if pos > cursor {
            note_span(stack, cursor, pos, kind, &mut open, &mut regions);
        }
        let _ = stack.apply(op);
        cursor = pos;
    }
    if stripped_len > cursor {
        note_span(stack, cursor, stripped_len, kind, &mut open, &mut regions);
    }
    if let Some(start) = open.take() {
        if start < stripped_len {
            regions.push(start..stripped_len);
        }
    }
    regions
}

/// Checkable byte ranges for each line, starting from a fresh parse.
#[cfg(test)]
fn checkable_regions(
    highlighter: &SyntaxHighlighter,
    language: Language,
    lines: &[&str],
) -> Vec<Vec<Range<usize>>> {
    let kind = spell_kind(language);
    if kind == SpellKind::Off {
        return lines.iter().map(|_| Vec::new()).collect();
    }
    let snap = highlighter.initial_snapshot(language);
    let mut parse = snap.parse;
    let mut stack = snap.highlight.path;
    lines
        .iter()
        .map(|line| regions_on_line(&mut parse, &mut stack, &highlighter.syntax_set, line, kind))
        .collect()
}

/// Misspelling ranges for the highlight cache's current window.
/// Keyed like the highlight cache plus the window, so a scroll that stays
/// inside the overscan does not reparse.
#[derive(Debug, Default)]
pub struct SpellCache {
    key: Option<(Language, u64, u64, usize, usize)>,
    start_line: usize,
    lines: Vec<Vec<Range<usize>>>,
}

impl SpellCache {
    pub fn clear(&mut self) {
        self.key = None;
        self.start_line = 0;
        self.lines.clear();
    }

    pub fn covers_line(&self, line: usize) -> bool {
        self.key.is_some() && line >= self.start_line && line < self.start_line + self.lines.len()
    }

    pub fn line_ranges(&self, line: usize) -> &[Range<usize>] {
        line.checked_sub(self.start_line)
            .and_then(|idx| self.lines.get(idx))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[allow(clippy::too_many_arguments)] // window key plus the two line callbacks
    pub fn sync<L, B>(
        &mut self,
        engine: &SpellEngine,
        highlighter: &SyntaxHighlighter,
        highlight: &HighlightCache,
        language: Language,
        generation: u64,
        source: u64,
        mut line_at: L,
        mut line_byte: B,
    ) where
        L: FnMut(usize) -> String,
        B: FnMut(usize) -> usize,
    {
        let start = highlight.start_line();
        let count = highlight.spans().len();
        let end = start + count;
        let key = (language, generation, source, start, end);
        if self.key == Some(key) {
            return;
        }
        let kind = spell_kind(language);
        if count == 0 || kind == SpellKind::Off {
            self.key = Some(key);
            self.start_line = start;
            self.lines = vec![Vec::new(); count];
            return;
        }
        let Some((checkpoint, snap)) = highlight.resume_at(start) else {
            self.clear();
            return;
        };
        self.lines = misspellings_from_checkpoint(
            engine,
            highlighter,
            snap,
            checkpoint,
            start,
            end,
            kind,
            &mut line_at,
            &mut line_byte,
        );
        self.key = Some(key);
        self.start_line = start;
    }
}

#[allow(clippy::too_many_arguments)] // checkpoint replay needs the window and both callbacks
pub(crate) fn misspellings_from_checkpoint<L, B>(
    engine: &SpellEngine,
    highlighter: &SyntaxHighlighter,
    snap: HighlightSnapshot,
    checkpoint: usize,
    start: usize,
    end: usize,
    kind: SpellKind,
    line_at: &mut L,
    line_byte: &mut B,
) -> Vec<Vec<Range<usize>>>
where
    L: FnMut(usize) -> String,
    B: FnMut(usize) -> usize,
{
    let mut parse = snap.parse;
    let mut stack = snap.highlight.path;
    let mut lines = Vec::with_capacity(end.saturating_sub(start));
    for idx in checkpoint..end {
        let raw = line_at(idx);
        let regions = regions_on_line(&mut parse, &mut stack, &highlighter.syntax_set, &raw, kind);
        if idx >= start {
            let base = line_byte(idx);
            let stripped_len = raw.trim_end_matches(['\n', '\r']).len();
            let stripped = &raw[..stripped_len];
            let misses = engine.misspellings_in(stripped, &regions);
            lines.push(
                misses
                    .into_iter()
                    .map(|range| (range.start + base)..(range.end + base))
                    .collect(),
            );
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> SpellEngine {
        SpellEngine::load().expect("embedded dictionary")
    }

    #[test]
    fn teh_is_misspelled_and_suggests_the() {
        let engine = engine();
        assert!(engine.is_misspelled("teh"));
        assert!(!engine.is_misspelled("hello"));
        assert!(!engine.is_misspelled("don't"));
        let suggestions = engine.suggestions("teh");
        assert!(
            suggestions.iter().any(|word| word == "the"),
            "{suggestions:?}"
        );
        assert!(suggestions.len() <= MAX_SUGGESTIONS);
    }

    #[test]
    fn tokenizer_keeps_apostrophes_and_hyphens_and_skips_noise() {
        let text = "don't well-known TODO i18n a zz";
        let words: Vec<&str> = word_ranges(text)
            .iter()
            .map(|range| &text[range.clone()])
            .collect();
        assert_eq!(
            words,
            vec!["don't", "well-known", "TODO", "i18n", "a", "zz"]
        );
        let engine = engine();
        #[allow(clippy::single_range_in_vec_init)]
        let regions = vec![0..text.len()];
        let misses = engine.misspellings_in(text, &regions);
        let flagged: Vec<&str> = misses.iter().map(|range| &text[range.clone()]).collect();
        assert_eq!(flagged, vec!["zz"]);
    }

    #[test]
    fn apply_case_preserves_capitalization() {
        assert_eq!(apply_case("teh", "the"), "the");
        assert_eq!(apply_case("Teh", "the"), "The");
        assert_eq!(apply_case("TEH", "the"), "THE");
    }

    #[test]
    fn personal_list_skips_comments_and_add_accepts_the_word() {
        let words = personal_words_from_str("# note\n\nZzzxnotaword\n");
        assert!(words.contains("zzzxnotaword"));
        assert_eq!(words.len(), 1);
        let mut engine = engine();
        assert!(engine.is_misspelled("Zzzxnotaword"));
        engine.add_personal("Zzzxnotaword").unwrap();
        assert!(!engine.is_misspelled("zzzxnotaword"));
        engine.ignore("Qqqnotaword");
        assert!(!engine.is_misspelled("QQQnotaword"));
    }

    #[test]
    fn append_personal_word_writes_a_line() {
        let dir = std::env::temp_dir().join(format!(
            "rust-edit-dict-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("dictionary.txt");
        append_personal_word(&path, "Zzzxnotaword").unwrap();
        append_personal_word(&path, "Anotherword").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "Zzzxnotaword\nAnotherword\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rust_checks_comments_and_strings_only() {
        let engine = engine();
        let highlighter = SyntaxHighlighter::new();
        let line = "fn zzzxnotaword() { let s = \"zzzxnotaword\"; } // teh";
        let regions = checkable_regions(&highlighter, Language::Rust, &[line]);
        let misses = engine.misspellings_in(line, &regions[0]);
        let words: Vec<&str> = misses.iter().map(|range| &line[range.clone()]).collect();
        assert!(words.contains(&"teh"), "{words:?} regions {:?}", regions[0]);
        assert_eq!(
            words.iter().filter(|word| **word == "zzzxnotaword").count(),
            1
        );
    }

    #[test]
    fn block_comment_continuation_stays_in_scope() {
        let engine = engine();
        let highlighter = SyntaxHighlighter::new();
        let lines = ["/* start", "stillbad here", "end */"];
        let regions = checkable_regions(&highlighter, Language::Rust, &lines);
        let misses = engine.misspellings_in(lines[1], &regions[1]);
        let words: Vec<&str> = misses
            .iter()
            .map(|range| &lines[1][range.clone()])
            .collect();
        assert_eq!(words, vec!["stillbad"]);
    }

    #[test]
    fn shell_string_skips_the_interpolated_variable() {
        let engine = engine();
        let highlighter = SyntaxHighlighter::new();
        let line = "echo \"teh $userName\"";
        let regions = checkable_regions(&highlighter, Language::Bash, &[line]);
        let misses = engine.misspellings_in(line, &regions[0]);
        let words: Vec<&str> = misses.iter().map(|range| &line[range.clone()]).collect();
        assert!(words.contains(&"teh"), "{words:?} regions {:?}", regions[0]);
        assert!(!words.iter().any(|word| word.contains("user")), "{words:?}");
    }

    #[test]
    fn markdown_skips_fenced_code_and_checks_prose() {
        let engine = engine();
        let highlighter = SyntaxHighlighter::new();
        let lines = ["teh word", "", "```", "zzzxnotaword", "```"];
        let regions = checkable_regions(&highlighter, Language::Markdown, &lines);
        let prose = engine.misspellings_in(lines[0], &regions[0]);
        let prose_words: Vec<&str> = prose.iter().map(|range| &lines[0][range.clone()]).collect();
        assert_eq!(prose_words, vec!["teh"]);
        let code = engine.misspellings_in(lines[3], &regions[3]);
        assert!(
            code.is_empty(),
            "fenced code should be skipped, regions {:?}",
            regions[3]
        );
    }

    #[test]
    fn json_is_off_and_jsonc_checks_comments() {
        let engine = engine();
        let highlighter = SyntaxHighlighter::new();
        let json = "{ \"zzzxnotaword\": 1 }";
        let json_regions = checkable_regions(&highlighter, Language::Json, &[json]);
        assert!(json_regions[0].is_empty());
        assert!(engine.misspellings_in(json, &json_regions[0]).is_empty());

        let lines = ["// teh", "{ \"zzzxnotaword\": 1 }"];
        let regions = checkable_regions(&highlighter, Language::Jsonc, &lines);
        let comment = engine.misspellings_in(lines[0], &regions[0]);
        let words: Vec<&str> = comment
            .iter()
            .map(|range| &lines[0][range.clone()])
            .collect();
        assert_eq!(words, vec!["teh"]);
        assert!(engine.misspellings_in(lines[1], &regions[1]).is_empty());
    }

    #[test]
    fn cursor_on_or_just_after_a_word_selects_it() {
        #[allow(clippy::single_range_in_vec_init)]
        let ranges = vec![2..5];
        assert_eq!(misspelling_at(&ranges, 2), Some(2..5));
        assert_eq!(misspelling_at(&ranges, 4), Some(2..5));
        assert_eq!(misspelling_at(&ranges, 5), Some(2..5));
        assert_eq!(misspelling_at(&ranges, 6), None);
        assert_eq!(misspelling_at(&ranges, 1), None);
    }
}
