// Buffer struct + Ropey wrapper + core editing ops (insert, delete, undo, indent, etc.)
// Sprint 1 core implementation target. See Design Spec §6 and AGENTS §6.

#![allow(dead_code)]

use ropey::Rope;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_BUFFER_ID: AtomicU64 = AtomicU64::new(1);

fn next_buffer_id() -> u64 {
    NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed)
}

fn indent_unit(tab_size: u8) -> String {
    " ".repeat(tab_size.max(1) as usize)
}

/// Byte length of the leading ASCII spaces and tabs. Other whitespace stays
/// with the line body so the comment token is not inserted mid-glyph.
fn leading_ascii_ws_len(body: &str) -> usize {
    body.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| c.len_utf8())
        .sum()
}

/// Prefix and suffix byte lengths of a same-line block wrap after indent.
/// Open/close optional single spaces are included in the lengths.
/// `None` if the body is not wrapped.
fn block_wrap_lens(after_ws: &str, open: &str, close: &str) -> Option<(usize, usize)> {
    if open.is_empty() || close.is_empty() {
        return None;
    }
    let rest = after_ws.strip_prefix(open)?;
    let open_extra = usize::from(rest.starts_with(' '));
    let rest = rest.get(open_extra..)?;
    let rest = rest.strip_suffix(close)?;
    let close_extra = usize::from(rest.ends_with(' '));
    Some((open.len() + open_extra, close.len() + close_extra))
}

/// Byte length of `prefix` at the start of `after`, plus one following space
/// when that space is present.
///
/// Alphabetic prefixes (`REM`) match any ASCII case and must end on a word
/// boundary, so `REMote` is not a comment. Other prefixes match exactly.
fn comment_prefix_len(after: &str, prefix: &str) -> Option<usize> {
    if prefix.is_empty() {
        return None;
    }
    let rest = if prefix.chars().any(|c| c.is_ascii_alphabetic()) {
        let mut rest = after;
        for expected in prefix.chars() {
            let mut chars = rest.chars();
            let got = chars.next()?;
            if !got.eq_ignore_ascii_case(&expected) {
                return None;
            }
            rest = chars.as_str();
        }
        if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        rest
    } else {
        after.strip_prefix(prefix)?
    };
    let matched = after.len() - rest.len();
    let extra = usize::from(rest.starts_with(' '));
    Some(matched + extra)
}

fn line_body(raw: &str) -> &str {
    raw.trim_end_matches(['\n', '\r'])
}

fn line_is_wrapper(raw: &str, token: &str) -> bool {
    line_body(raw).trim() == token
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Plain,
    Rust,
    Python,
    Markdown,
    Json,
    Jsonc,
    Toml,
    Yaml,
    Ini,
    Bash,
    C,
    Cpp,
    Javascript,
    Typescript,
    Java,
    CSharp,
    Go,
    Zig,
    Php,
    Ruby,
    Perl,
    Sql,
    Html,
    Css,
    Xml,
    Lua,
    Scala,
    Haskell,
    R,
    Clojure,
    Erlang,
    Groovy,
    Makefile,
    JavaProperties,
    ObjectiveC,
    ObjectiveCpp,
    Diff,
    Latex,
    Tex,
    Tcl,
    OCaml,
    RestructuredText,
    Lisp,
    D,
    Batch,
}

impl Language {
    /// Every variant, for tests that must not skip a newly added language.
    pub const ALL: &[Language] = &[
        Language::Plain,
        Language::Rust,
        Language::Python,
        Language::Markdown,
        Language::Json,
        Language::Jsonc,
        Language::Toml,
        Language::Yaml,
        Language::Ini,
        Language::Bash,
        Language::C,
        Language::Cpp,
        Language::Javascript,
        Language::Typescript,
        Language::Java,
        Language::CSharp,
        Language::Go,
        Language::Zig,
        Language::Php,
        Language::Ruby,
        Language::Perl,
        Language::Sql,
        Language::Html,
        Language::Css,
        Language::Xml,
        Language::Lua,
        Language::Scala,
        Language::Haskell,
        Language::R,
        Language::Clojure,
        Language::Erlang,
        Language::Groovy,
        Language::Makefile,
        Language::JavaProperties,
        Language::ObjectiveC,
        Language::ObjectiveCpp,
        Language::Diff,
        Language::Latex,
        Language::Tex,
        Language::Tcl,
        Language::OCaml,
        Language::RestructuredText,
        Language::Lisp,
        Language::D,
        Language::Batch,
    ];

    /// Status-bar name. Matches the Help list, shortened where that row is a sentence.
    pub fn label(self) -> &'static str {
        match self {
            Language::Plain => "Plain",
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::Markdown => "Markdown",
            Language::Json => "JSON",
            Language::Jsonc => "JSONC",
            Language::Toml => "TOML",
            Language::Yaml => "YAML",
            Language::Ini => "INI",
            Language::Bash => "Shell",
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
            Language::JavaProperties => "Properties",
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
            Language::Batch => "Batch",
        }
    }

    pub fn from_path(path: &Path) -> Self {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            match name.to_ascii_lowercase().as_str() {
                "makefile" | "gnumakefile" => return Language::Makefile,
                "gemfile" | "rakefile" | "vagrantfile" | "config.ru" => return Language::Ruby,
                ".bashrc" | ".bash_profile" | ".bash_login" | ".bash_logout" | ".profile"
                | ".zshrc" | ".zprofile" => return Language::Bash,
                _ => {}
            }
        }
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "rs" => Language::Rust,
            "py" | "pyw" | "pyi" | "py3" => Language::Python,
            "md" | "markdown" => Language::Markdown,
            "json" => Language::Json,
            "jsonc" => Language::Jsonc,
            "toml" => Language::Toml,
            "yaml" | "yml" => Language::Yaml,
            "ini" | "cfg" => Language::Ini,
            "sh" | "bash" | "zsh" | "fish" => Language::Bash,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "inl" | "ipp" | "cp" => Language::Cpp,
            "js" | "mjs" | "jsx" => Language::Javascript,
            "ts" | "tsx" => Language::Typescript,
            "java" => Language::Java,
            "cs" | "csx" => Language::CSharp,
            "go" => Language::Go,
            "zig" => Language::Zig,
            "php" | "phtml" | "php3" | "php4" | "php5" | "php7" | "phps" | "phpt" => Language::Php,
            "rb" | "rake" | "gemspec" => Language::Ruby,
            "pl" | "pm" | "pod" => Language::Perl,
            "sql" | "ddl" | "dml" => Language::Sql,
            "html" | "htm" | "shtml" | "xhtml" => Language::Html,
            "css" => Language::Css,
            "xml" | "xsd" | "xslt" | "svg" | "rss" | "opml" => Language::Xml,
            "lua" => Language::Lua,
            "scala" | "sbt" => Language::Scala,
            "hs" => Language::Haskell,
            "r" => Language::R,
            "clj" | "cljs" | "cljc" => Language::Clojure,
            "erl" | "hrl" => Language::Erlang,
            "groovy" | "gvy" | "gradle" => Language::Groovy,
            "mk" | "mak" => Language::Makefile,
            "properties" => Language::JavaProperties,
            "m" => Language::ObjectiveC,
            "mm" => Language::ObjectiveCpp,
            "diff" | "patch" => Language::Diff,
            "tex" | "ltx" => Language::Latex,
            "sty" | "cls" => Language::Tex,
            "tcl" => Language::Tcl,
            "ml" | "mli" => Language::OCaml,
            "rst" | "rest" => Language::RestructuredText,
            "lisp" | "cl" | "el" | "scm" | "ss" => Language::Lisp,
            "d" | "di" => Language::D,
            "bat" | "cmd" => Language::Batch,
            _ => Language::Plain,
        }
    }

    /// Token that starts a line comment, without the space inserted on comment.
    /// Languages with only a block comment return `None` here. Plain text,
    /// JSON, Diff, and reStructuredText have neither.
    ///
    /// A language that has both a line comment and a block comment uses the
    /// line token. `/* */` is not toggled for Java, C#, Go, and the other
    /// `//` languages.
    pub fn line_comment_prefix(self) -> Option<&'static str> {
        match self {
            Language::Rust
            | Language::C
            | Language::Cpp
            | Language::Javascript
            | Language::Typescript
            | Language::Java
            | Language::CSharp
            | Language::Go
            | Language::Php
            | Language::Scala
            | Language::Groovy
            | Language::D
            | Language::ObjectiveC
            | Language::ObjectiveCpp
            | Language::Zig
            | Language::Jsonc => Some("//"),
            Language::Python
            | Language::Toml
            | Language::Yaml
            | Language::Bash
            | Language::Ruby
            | Language::Perl
            | Language::R
            | Language::Tcl
            | Language::JavaProperties
            | Language::Makefile => Some("#"),
            Language::Sql | Language::Lua | Language::Haskell => Some("--"),
            Language::Ini | Language::Clojure | Language::Lisp => Some(";"),
            Language::Erlang | Language::Latex | Language::Tex => Some("%"),
            Language::Batch => Some("REM"),
            Language::Plain
            | Language::Markdown
            | Language::Json
            | Language::Html
            | Language::Css
            | Language::Xml
            | Language::OCaml
            | Language::Diff
            | Language::RestructuredText => None,
        }
    }

    /// Block wrap for languages that have no line comment. Open/close have no
    /// surrounding spaces; the toggle inserts those on a single line, or
    /// puts each token on its own line around a multi-line span.
    pub fn block_comment_pair(self) -> Option<(&'static str, &'static str)> {
        match self {
            Language::Markdown | Language::Html | Language::Xml => Some(("<!--", "-->")),
            Language::Css => Some(("/*", "*/")),
            Language::OCaml => Some(("(*", "*)")),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Buffer {
    pub rope: Rope,
    pub path: Option<PathBuf>,
    pub language: Language,
    pub is_modified: bool,
    /// True only for the editor-created empty untitled buffer (startup with no
    /// files, or the reset buffer after closing the last tab). `[+]` / `Ctrl+N`
    /// tabs stay false so they are never auto-replaced on File > Open.
    pub is_initial: bool,

    /// Byte offset of cursor in the rope.
    pub cursor: usize,
    /// Optional selection [start, end) with start <= end. When None, no selection.
    pub selection: Option<Range<usize>>,
    /// Fixed anchor byte for selection extension. Set when a shift-extend sequence
    /// begins (no selection yet) and kept stable across subsequent moves, so the
    /// selection can grow/shrink in EITHER direction relative to this anchor.
    /// Without it, the normalized `selection.start` drifts with the cursor and
    /// leftward extension collapses to one char.
    pub selection_anchor: Option<usize>,
    /// (row, col_offset) for viewport scrolling (rows are 0-based).
    pub scroll_offset: (usize, usize),
    /// Last known height (in rows) of the editor viewport. Refreshed each frame
    /// by the run loop so adjust_scroll can clamp against the *real* height
    /// instead of a guessed constant. Fixes half-screen scroll jumping on long files.
    pub visible_lines: usize,
    /// Last known width (in columns) of the editor text area (excludes gutter +
    /// scrollbar). Refreshed each frame so horizontal auto-scroll can follow the
    /// cursor when a line is longer than the window.
    pub visible_width: usize,
    /// Mirrors `App.config.word_wrap`, refreshed each frame with `visible_lines`.
    /// When true, `scroll_offset.0` is a screen row and `adjust_scroll` must use
    /// wrap math (wrap spec §4.1). Default false so existing tests are unchanged.
    pub word_wrap: bool,
    /// Mirrors `App.config.tab_size`, needed to compute wrap layout on edit.
    pub tab_size: u8,
    /// Bumped on every text mutation (insert/backspace/delete/newline/indent/dedent/undo/redo).
    /// Used as an invalidation key by the highlight cache so spans are only recomputed
    /// on edit or scroll into new lines (see AGENTS §6 "Syntax Highlighting").
    pub generation: u64,
    /// Longest line in chars (no `\n`/`\r`). Refreshed when `generation` changes.
    pub max_line_width: usize,
    max_line_width_gen: u64,
    /// Unique identity for this buffer instance. Used as the highlight-cache
    /// `source` so two gen-0 files cannot alias each other.
    pub id: u64,
    /// Last accepted on-disk identity. `None` for untitled buffers, or after
    /// the file disappeared and we have already recorded that.
    pub disk_stamp: Option<crate::file_watch::FileStamp>,
    /// Session-only: skip the reload prompt for future *clean* external
    /// changes to this buffer. Set by `[ Always ]`. Never persisted.
    pub auto_reload: bool,
    /// True after we have shown a "deleted on disk" notice for the current
    /// missing streak, so we do not spam the status bar.
    pub disk_gone_notified: bool,
    /// This process cannot write `path`. Edits are allowed. Save must warn
    /// instead of failing quietly. Untitled buffers are false.
    pub read_only: bool,

    /// Undo stack (full state snapshots for robust undo of text + caret + view).
    /// The last field is `selection_anchor`, which line-comment edits shift.
    /// Restoring it keeps the next Shift-extend on the undone selection.
    #[allow(clippy::type_complexity)]
    undo_stack: Vec<(
        Rope,
        usize,
        Option<std::ops::Range<usize>>,
        (usize, usize),
        Option<usize>,
    )>,
    /// Redo stack.
    #[allow(clippy::type_complexity)]
    redo_stack: Vec<(
        Rope,
        usize,
        Option<std::ops::Range<usize>>,
        (usize, usize),
        Option<usize>,
    )>,
    /// Max undo depth (keeps memory reasonable).
    max_undo: usize,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    pub fn new() -> Self {
        let rope = Rope::new();
        Self {
            rope,
            path: None,
            language: Language::Plain,
            is_modified: false,
            is_initial: false,
            cursor: 0,
            selection: None,
            selection_anchor: None,
            scroll_offset: (0, 0),
            visible_lines: 20,
            visible_width: 80,
            word_wrap: false,
            tab_size: 4,
            generation: 0,
            max_line_width: 0,
            max_line_width_gen: u64::MAX,
            id: next_buffer_id(),
            disk_stamp: None,
            auto_reload: false,
            disk_gone_notified: false,
            read_only: false,
            undo_stack: Vec::with_capacity(32),
            redo_stack: Vec::new(),
            max_undo: 100,
        }
    }

    /// Auto-created untitled placeholder (startup / last-tab close).
    pub fn placeholder() -> Self {
        let mut b = Self::new();
        b.is_initial = true;
        b
    }

    pub fn from_str(s: &str, path: Option<PathBuf>) -> Self {
        let rope = Rope::from_str(s);
        let language = path
            .as_ref()
            .map(|p| Language::from_path(p.as_ref()))
            .unwrap_or_default();
        let disk_stamp = path
            .as_ref()
            .and_then(|p| crate::file_watch::FileStamp::of(p));
        Self {
            rope,
            path,
            language,
            is_modified: false,
            is_initial: false,
            cursor: 0,
            selection: None,
            selection_anchor: None,
            scroll_offset: (0, 0),
            visible_lines: 20,
            visible_width: 80,
            word_wrap: false,
            tab_size: 4,
            generation: 0,
            max_line_width: 0,
            max_line_width_gen: u64::MAX,
            id: next_buffer_id(),
            disk_stamp,
            auto_reload: false,
            disk_gone_notified: false,
            read_only: false,
            undo_stack: Vec::with_capacity(32),
            redo_stack: Vec::new(),
            max_undo: 100,
        }
    }

    /// Total number of lines (at least 1).
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get a line by 0-based index (without the trailing newline).
    pub fn line(&self, idx: usize) -> String {
        if idx >= self.line_count() {
            return String::new();
        }
        self.rope.line(idx).to_string()
    }

    /// Current cursor as (line, col) 0-based.
    pub fn cursor_position(&self) -> (usize, usize) {
        if self.rope.len_chars() == 0 {
            return (0, 0);
        }
        let line = self.rope.byte_to_line(self.cursor);
        let line_start_byte = self.rope.line_to_byte(line);
        let col = self
            .rope
            .byte_slice(line_start_byte..self.cursor)
            .to_string()
            .chars()
            .count();
        (line, col)
    }

    /// Gutter width used for rendering line numbers when they are shown.
    /// When `config.show_line_numbers` is false, `App::gutter_width()` returns 0.
    /// Used for paint, mouse hit testing, hardware cursor X, and viewport width.
    pub const GUTTER_WIDTH: usize = 6;

    /// Set cursor from a visual line (absolute buffer line, 0-based) and visual column (chars after gutter).
    /// Used for mouse clicks in editor.
    pub fn set_cursor_from_visual(&mut self, visual_line: usize, visual_col: usize) {
        let n = self.line_count();
        if visual_line >= n {
            // Support clicking in padded blank rows below content to place at EOF (append).
            if n == 0 {
                self.cursor = 0;
            } else {
                let last = n - 1;
                let ls = self.rope.line_to_byte(last);
                let lstr = self.line(last);
                let full = lstr.chars().count();
                let mut byte = ls;
                for (i, c) in lstr.chars().enumerate() {
                    if i >= full {
                        break;
                    }
                    byte += c.len_utf8();
                }
                self.cursor = byte;
            }
            self.clear_selection();
            return;
        }
        let line_start_byte = self.rope.line_to_byte(visual_line);
        let line_str = self.line(visual_line);
        // Exclude the trailing `\n`/`\r` from column counting (ropey keeps it in
        // `.line()`). Without this, a click past the last real character would
        // place the cursor on the newline byte instead of at end-of-content.
        let line_str = line_str.trim_end_matches(['\n', '\r']);
        let mut byte = line_start_byte;
        for (chars, ch) in line_str.chars().enumerate() {
            if chars >= visual_col {
                break;
            }
            byte += ch.len_utf8();
        }
        self.cursor = byte;
        self.clear_selection();
        // Do not auto-adjust here; callers (mouse place) decide whether to ensure_visible
        // to avoid unwanted jumps when clicking inside the current viewport.
    }

    /// Scroll the view by delta lines (positive = down).
    pub fn scroll(&mut self, delta: isize) {
        let current = self.scroll_offset.0 as isize;
        let new_scroll = (current + delta).max(0) as usize;
        // Clamp to the last position where content still fills the viewport.
        // Using `visible_lines` (synced each frame in run_app) keeps this
        // consistent with `clamp_scroll`; the previous `line_count - 1` clamp
        // let the wheel push content entirely off-screen, leaving blank rows
        // and desyncing the scrollbar thumb from the real content range.
        let max_scroll = self.line_count().saturating_sub(self.visible_lines.max(1));
        self.scroll_offset.0 = new_scroll.min(max_scroll);
    }

    /// Clamp scroll to valid range given approximate visible lines.
    pub fn clamp_scroll(&mut self, visible_lines: usize) {
        let max = self.line_count().saturating_sub(visible_lines.max(1));
        if self.scroll_offset.0 > max {
            self.scroll_offset.0 = max;
        }
    }

    pub fn compute_max_line_width(&self) -> usize {
        let n = self.line_count();
        let mut max = 0usize;
        for i in 0..n {
            let w = self
                .rope
                .line(i)
                .chars()
                .filter(|&c| c != '\n' && c != '\r')
                .count();
            if w > max {
                max = w;
            }
        }
        max
    }

    pub fn refresh_max_line_width(&mut self) {
        if self.max_line_width_gen == self.generation {
            return;
        }
        self.max_line_width = self.compute_max_line_width();
        self.max_line_width_gen = self.generation;
    }

    /// Max `scroll_offset.1` such that the viewport still shows content.
    pub fn max_hscroll(&self) -> usize {
        // Extra column so an EOL caret (col == max_line_width) stays in the
        // text area instead of sitting on the vertical scrollbar.
        self.max_line_width
            .saturating_sub(self.visible_width.max(1).saturating_sub(1))
    }

    pub fn clamp_hscroll(&mut self) {
        let max = self.max_hscroll();
        if self.scroll_offset.1 > max {
            self.scroll_offset.1 = max;
        }
    }

    pub fn scroll_horizontal(&mut self, delta: isize) {
        let cur = self.scroll_offset.1 as isize;
        self.scroll_offset.1 = (cur + delta).max(0) as usize;
        self.clamp_hscroll();
    }

    /// Compute the byte offset for a visual line and column (does not mutate).
    pub fn byte_at_visual(&self, visual_line: usize, visual_col: usize) -> usize {
        if visual_line >= self.line_count() {
            return self.rope.len_bytes();
        }
        let line_start_byte = self.rope.line_to_byte(visual_line);
        // `rope.line()` includes the trailing `\n` (and a stray `\r` on CRLF
        // files). Exclude the terminator from column counting so a click past the
        // last real char lands at end-of-content, not on the newline byte.
        let line_str = self.line(visual_line);
        let line_str = line_str.trim_end_matches(['\n', '\r']);
        let mut byte = line_start_byte;
        for (chars, ch) in line_str.chars().enumerate() {
            if chars >= visual_col {
                break;
            }
            byte += ch.len_utf8();
        }
        byte
    }

    /// Save snapshot for undo (call before a mutating operation).
    pub fn push_undo(&mut self) {
        if self.undo_stack.len() >= self.max_undo {
            self.undo_stack.remove(0);
        }
        let state = (
            self.rope.clone(),
            self.cursor,
            self.selection.clone(),
            self.scroll_offset,
            self.selection_anchor,
        );
        self.undo_stack.push(state);
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some((prev_rope, prev_cur, prev_sel, prev_scroll, prev_anchor)) =
            self.undo_stack.pop()
        {
            let current = (
                self.rope.clone(),
                self.cursor,
                self.selection.clone(),
                self.scroll_offset,
                self.selection_anchor,
            );
            self.redo_stack.push(current);
            self.rope = prev_rope;
            self.cursor = prev_cur.min(self.rope.len_bytes());
            self.selection = prev_sel;
            self.scroll_offset = prev_scroll;
            self.selection_anchor = prev_anchor;
            self.is_modified = !self.undo_stack.is_empty(); // rough
            self.bump_generation();
        }
    }

    pub fn redo(&mut self) {
        if let Some((next_rope, next_cur, next_sel, next_scroll, next_anchor)) =
            self.redo_stack.pop()
        {
            let current = (
                self.rope.clone(),
                self.cursor,
                self.selection.clone(),
                self.scroll_offset,
                self.selection_anchor,
            );
            self.undo_stack.push(current);
            self.rope = next_rope;
            self.cursor = next_cur.min(self.rope.len_bytes());
            self.selection = next_sel;
            self.scroll_offset = next_scroll;
            self.selection_anchor = next_anchor;
            self.bump_generation();
        }
    }

    /// Insert text at a BYTE offset. Ropey's `Rope::insert` is char-indexed, but
    /// every position in this crate (cursor, selection, computed offsets) is a
    /// byte offset — feeding bytes directly to `insert` lands edits at the wrong
    /// (later) position for multi-byte UTF-8 and panics once `byte > len_chars`.
    /// This helper bridges that. `byte_pos` is clamped to the rope's byte length.
    pub(crate) fn insert_at_byte(&mut self, byte_pos: usize, text: &str) {
        let p = byte_pos.min(self.rope.len_bytes());
        self.rope.insert(self.rope.byte_to_char(p), text);
    }

    /// Remove the byte range `[byte_start, byte_end)`. Bridges the byte→char
    /// mismatch with `Rope::remove` (which is char-indexed). Inputs clamped.
    pub(crate) fn remove_byte_range(&mut self, byte_start: usize, byte_end: usize) {
        let len = self.rope.len_bytes();
        let s = byte_start.min(len);
        let e = byte_end.min(len);
        self.rope
            .remove(self.rope.byte_to_char(s)..self.rope.byte_to_char(e));
    }

    /// Insert text at cursor. Clears selection if present (replace selection behavior later).
    pub fn insert(&mut self, text: &str) {
        self.push_undo();
        self.selection_anchor = None;

        if let Some(sel) = self.selection.take() {
            // Replace selection with text
            let start = sel.start.min(self.rope.len_bytes());
            let end = sel.end.min(self.rope.len_bytes());
            self.remove_byte_range(start, end);
            self.insert_at_byte(start, text);
            self.cursor = start + text.len();
        } else {
            let pos = self.cursor.min(self.rope.len_bytes());
            self.insert_at_byte(pos, text);
            self.cursor = pos + text.len();
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Insert a single char (convenience for typing).
    pub fn insert_char(&mut self, ch: char) {
        self.insert(&ch.to_string());
    }

    /// Delete the char before cursor (backspace), or selection if present.
    pub fn backspace(&mut self) {
        self.push_undo();
        self.selection_anchor = None;

        if let Some(sel) = self.selection.take() {
            let start = sel.start.min(self.rope.len_bytes());
            let end = sel.end.min(self.rope.len_bytes());
            self.remove_byte_range(start, end);
            self.cursor = start;
        } else if self.cursor > 0 {
            let pos = self.cursor;
            // Delete previous char (handle multi-byte)
            let prev_char_idx = self.rope.byte_to_char(pos) - 1;
            let remove_start = self.rope.char_to_byte(prev_char_idx);
            self.remove_byte_range(remove_start, pos);
            self.cursor = remove_start;
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Replace `[range.start, range.end)` with `text` as one undo step.
    /// The cursor lands just after the new text.
    pub fn replace_byte_range(&mut self, range: Range<usize>, text: &str) {
        let len = self.rope.len_bytes();
        let start = range.start.min(len);
        let end = range.end.min(len).max(start);
        self.push_undo();
        self.selection = None;
        self.selection_anchor = None;
        self.remove_byte_range(start, end);
        self.insert_at_byte(start, text);
        self.cursor = start + text.len();
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Delete the char after cursor (delete key).
    pub fn delete(&mut self) {
        self.push_undo();
        self.selection_anchor = None;

        if let Some(sel) = self.selection.take() {
            let start = sel.start.min(self.rope.len_bytes());
            let end = sel.end.min(self.rope.len_bytes());
            self.remove_byte_range(start, end);
            self.cursor = start;
        } else if self.cursor < self.rope.len_bytes() {
            let pos = self.cursor;
            // Find end of next char
            let ch_idx = self.rope.byte_to_char(pos);
            if ch_idx < self.rope.len_chars() {
                let next_byte = self.rope.char_to_byte(ch_idx + 1);
                self.remove_byte_range(pos, next_byte);
            }
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Extend the selection to `new_cursor`, keeping a stable anchor. If there is
    /// no anchor yet (start of a shift-extend sequence), the current cursor becomes
    /// the anchor. The selection always spans [min(anchor,new_cursor), max(...)),
    /// so it grows/shrinks correctly in BOTH directions — fixing the old bug where
    /// extending leftward collapsed the selection to one character.
    fn extend_selection_to(&mut self, new_cursor: usize) {
        let anchor = self
            .selection_anchor
            .unwrap_or_else(|| self.cursor.min(self.rope.len_bytes()));
        self.selection_anchor = Some(anchor);
        let (s, e) = if anchor <= new_cursor {
            (anchor, new_cursor)
        } else {
            (new_cursor, anchor)
        };
        self.selection = Some(s..e);
    }

    /// Clear the selection AND its anchor (used by non-extend moves and edits).
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_anchor = None;
    }

    /// Select the entire buffer. Cursor lands at EOF (so a following left-arrow
    /// collapses to the end of the document, matching common editor behavior).
    pub fn select_all(&mut self) {
        let end = self.rope.len_bytes();
        self.selection = Some(0..end);
        self.selection_anchor = Some(0);
        self.cursor = end;
        self.adjust_scroll();
    }

    /// Extend the selection to a target byte, anchoring at the current cursor if
    /// no selection is in progress (mouse-drag entry point). Used by mouse drag so
    /// it shares the same fixed-anchor semantics as keyboard shift-extend.
    pub fn mouse_extend_to(&mut self, target_byte: usize) {
        self.extend_selection_to(target_byte);
        self.cursor = target_byte;
    }

    /// Move cursor by char delta (clamped).
    pub fn move_cursor(&mut self, delta_chars: isize, extend_selection: bool) {
        let current_char = self.rope.byte_to_char(self.cursor);
        let new_char = (current_char as isize + delta_chars)
            .max(0)
            .min(self.rope.len_chars() as isize) as usize;
        let new_byte = self.rope.char_to_byte(new_char);

        if extend_selection {
            self.extend_selection_to(new_byte);
        } else {
            self.clear_selection();
        }
        self.cursor = new_byte;
        self.adjust_scroll();
    }

    /// Move to start of current line.
    pub fn move_home(&mut self, extend_selection: bool) {
        let line = self.rope.byte_to_line(self.cursor);
        let line_start = self.rope.line_to_byte(line);
        if extend_selection {
            self.extend_selection_to(line_start);
        } else {
            self.clear_selection();
        }
        self.cursor = line_start;
        self.adjust_scroll();
    }

    /// Move to end of current line.
    pub fn move_end(&mut self, extend_selection: bool) {
        let line = self.rope.byte_to_line(self.cursor);
        let next_line_start = if line + 1 < self.rope.len_lines() {
            self.rope.line_to_byte(line + 1)
        } else {
            self.rope.len_bytes()
        };
        // end of line is before the \n if present
        let line_end = if next_line_start > 0 {
            // walk back over \n if any
            let mut end = next_line_start;
            if end > 0 && self.rope.char(self.rope.byte_to_char(end - 1)) == '\n' {
                end -= 1;
            }
            end
        } else {
            0
        };

        if extend_selection {
            self.extend_selection_to(line_end);
        } else {
            self.clear_selection();
        }
        self.cursor = line_end;
        self.adjust_scroll();
    }

    /// Move cursor up by a number of lines (for arrows/PgUp), preserving column.
    pub fn move_lines_up(&mut self, count: usize, extend_selection: bool) {
        if count == 0 {
            return;
        }
        let (row, col) = self.cursor_position();
        if row == 0 {
            return;
        }
        let target_row = row.saturating_sub(count);
        self.move_to_line(target_row, col, extend_selection);
    }

    /// Move cursor down by a number of lines (for arrows/PgDn), preserving column.
    pub fn move_lines_down(&mut self, count: usize, extend_selection: bool) {
        if count == 0 {
            return;
        }
        let (row, col) = self.cursor_position();
        let last = self.line_count().saturating_sub(1);
        if row >= last {
            return;
        }
        let target_row = (row + count).min(last);
        self.move_to_line(target_row, col, extend_selection);
    }

    fn move_to_line(&mut self, target_row: usize, preferred_col: usize, extend_selection: bool) {
        if target_row >= self.line_count() {
            return;
        }
        let line_start_byte = self.rope.line_to_byte(target_row);
        let line_str = self.line(target_row);
        // Exclude the trailing `\n`/`\r` (kept by ropey's `.line()`) from the
        // column clamp. Otherwise a preferred column >= real width would clamp to
        // the newline's position and land the cursor ON the terminator byte.
        let line_str = line_str.trim_end_matches(['\n', '\r']);
        let target_col = preferred_col.min(line_str.chars().count());
        let mut byte_off = line_start_byte;
        for (i, c) in line_str.chars().enumerate() {
            if i >= target_col {
                break;
            }
            byte_off += c.len_utf8();
        }
        if extend_selection {
            self.extend_selection_to(byte_off);
        } else {
            self.clear_selection();
        }
        self.cursor = byte_off;
        self.adjust_scroll();
    }

    /// Scroll view and cursor up by roughly a page (visible_lines - 1).
    /// Force the scroll_offset to move by the page delta so the *screen* visibly pages.
    pub fn page_up(&mut self, visible_lines: usize, extend_selection: bool) {
        let (cur_row, col) = self.cursor_position();
        let page = visible_lines.saturating_sub(1).max(1) as isize;
        let old_scroll = self.scroll_offset.0 as isize;
        let new_scroll = (old_scroll - page).max(0) as usize;
        self.scroll_offset.0 = new_scroll;
        // Place caret at a reasonable position in the newly paged view (approx same relative or top).
        let target_row = new_scroll + ((cur_row as isize - old_scroll).max(0) as usize);
        let target_row = target_row.min(self.line_count().saturating_sub(1));
        self.move_to_line(target_row, col, extend_selection);
    }

    /// Scroll view and cursor down by roughly a page (visible_lines - 1).
    /// Force the scroll_offset to move by the page delta so the *screen* visibly pages.
    pub fn page_down(&mut self, visible_lines: usize, extend_selection: bool) {
        let (cur_row, col) = self.cursor_position();
        let page = visible_lines.saturating_sub(1).max(1) as isize;
        let old_scroll = self.scroll_offset.0 as isize;
        let max_s = self.line_count().saturating_sub(1) as isize;
        let new_scroll = (old_scroll + page).min(max_s) as usize;
        self.scroll_offset.0 = new_scroll;
        // Place caret at a reasonable position in the newly paged view.
        let target_row = new_scroll + ((cur_row as isize - old_scroll).max(0) as usize);
        let target_row = target_row.min(self.line_count().saturating_sub(1));
        self.move_to_line(target_row, col, extend_selection);
    }

    /// Mark the buffer's text as changed so the highlight cache invalidates.
    /// Called from every mutation. Pure view/navigation ops must NOT call this.
    pub(crate) fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Auto-scroll to keep the cursor visible, using the *real* viewport height.
    ///
    /// Scrolls ONLY when the cursor crosses the true top or bottom edge of the
    /// viewport; mid-window movement leaves the view untouched. This replaces an
    /// earlier version that used a hardcoded `20`/`10` guess, which caused the
    /// view to start scrolling ~halfway down the screen and to jump on every
    /// keystroke in long files. The logic mirrors `ensure_cursor_visible`.
    pub fn adjust_scroll(&mut self) {
        if self.word_wrap {
            let layout = crate::wrap::compute_layout(
                &self.rope,
                0,
                self.line_count(),
                self.visible_width.max(1),
                self.tab_size,
            );
            self.ensure_cursor_visible_wrapped(&layout, self.visible_lines.max(1));
            return;
        }
        let v = self.visible_lines.max(1);
        let (row, col) = self.cursor_position();
        let s = self.scroll_offset.0;
        if row < s {
            // Cursor moved above the top edge → bring it to the top line.
            self.scroll_offset.0 = row;
        } else if row >= s + v {
            // Cursor moved below the bottom edge → recenter so it sits near the bottom.
            self.scroll_offset.0 = row.saturating_sub(v.saturating_sub(2));
        }
        // else: cursor inside viewport → do nothing (no half-screen jumping).
        // Keep the cursor column in view horizontally too (long lines / wide cursor moves).
        self.ensure_cursor_horizontally_visible_with(col);
    }

    /// Recompute horizontal scroll so the cursor column stays inside the visible
    /// text width. Recenters the window when the cursor leaves either edge.
    pub fn ensure_cursor_horizontally_visible(&mut self) {
        let (_, col) = self.cursor_position();
        self.ensure_cursor_horizontally_visible_with(col);
    }

    fn ensure_cursor_horizontally_visible_with(&mut self, col: usize) {
        let w = self.visible_width.max(1);
        let h = self.scroll_offset.1;
        if col < h {
            // Cursor left of the window → snap window to the cursor.
            self.scroll_offset.1 = col;
        } else if col >= h + w {
            // Cursor right of the window → recenter so the cursor sits near the right edge.
            self.scroll_offset.1 = col.saturating_sub(w.saturating_sub(2));
        }
    }

    /// Ensure the cursor is at least minimally visible within the given viewport height.
    /// Only scrolls if the cursor row is completely outside the current view window.
    /// Used after mouse placement (don't jump view for in-view clicks) and edge cases.
    pub fn ensure_cursor_visible(&mut self, visible_lines: usize) {
        let (row, _) = self.cursor_position();
        let s = self.scroll_offset.0;
        let v = visible_lines.max(1);
        if row < s {
            self.scroll_offset.0 = row; // bring to top
        } else if row >= s + v {
            self.scroll_offset.0 = row.saturating_sub(v.saturating_sub(2));
        }
    }

    /// Insert `tab_size` spaces at the caret. A non-empty selection indents
    /// every touched line instead.
    pub fn insert_tab(&mut self, tab_size: u8) {
        if let Some(sel) = &self.selection {
            if sel.start != sel.end {
                self.indent(tab_size);
                return;
            }
        }
        self.insert(&indent_unit(tab_size));
    }

    /// Smart indent on newline: insert \n + copy leading ws from current line + extra indent if appropriate.
    pub fn newline_with_indent(&mut self, tab_size: u8) {
        self.push_undo();
        self.selection_anchor = None;

        let pos = self.cursor.min(self.rope.len_bytes());
        let line_idx = self.rope.byte_to_line(pos);
        let line_text = self.line(line_idx);

        // Leading whitespace of current line
        let leading_ws: String = line_text
            .chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .collect();

        // Decide whether to increase indent
        let trimmed = line_text.trim_end();
        let increase = matches!(trimmed.chars().last(), Some('{' | '[' | '(' | ':'));

        let mut insert_text = String::from("\n") + &leading_ws;
        if increase {
            insert_text.push_str(&indent_unit(tab_size));
        }

        if let Some(sel) = self.selection.take() {
            let start = sel.start.min(self.rope.len_bytes());
            let end = sel.end.min(self.rope.len_bytes());
            self.remove_byte_range(start, end);
            self.insert_at_byte(start, &insert_text);
            self.cursor = start + insert_text.len();
        } else {
            self.insert_at_byte(pos, &insert_text);
            self.cursor = pos + insert_text.len();
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Indent current line or selection by one level (`tab_size` spaces).
    pub fn indent(&mut self, tab_size: u8) {
        self.push_undo();
        // Simplified: indent whole lines touched by selection or cursor line
        let cursor_line = if let Some(sel) = &self.selection {
            self.rope.byte_to_line(sel.start)
        } else {
            self.rope.byte_to_line(self.cursor)
        };
        let (start_line, end_line) = if let Some(sel) = &self.selection {
            (
                self.rope.byte_to_line(sel.start),
                self.rope.byte_to_line(sel.end.saturating_sub(1)),
            )
        } else {
            (cursor_line, cursor_line)
        };

        let unit = indent_unit(tab_size);
        let n = unit.len();

        for l in (start_line..=end_line).rev() {
            let line_start = self.rope.line_to_byte(l);
            self.insert_at_byte(line_start, &unit);
            if self.cursor >= line_start {
                self.cursor += n;
            }
            if let Some(sel) = &mut self.selection {
                if sel.start >= line_start {
                    sel.start += n;
                }
                if sel.end >= line_start {
                    sel.end += n;
                }
            }
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Dedent current line or selection (remove up to `tab_size` leading spaces).
    pub fn dedent(&mut self, tab_size: u8) {
        self.push_undo();
        let cursor_line = if let Some(sel) = &self.selection {
            self.rope.byte_to_line(sel.start)
        } else {
            self.rope.byte_to_line(self.cursor)
        };
        let (start_line, end_line) = if let Some(sel) = &self.selection {
            (
                self.rope.byte_to_line(sel.start),
                self.rope.byte_to_line(sel.end.saturating_sub(1)),
            )
        } else {
            (cursor_line, cursor_line)
        };

        let cursor_byte = self.cursor;

        for l in (start_line..=end_line).rev() {
            let line_start = self.rope.line_to_byte(l);
            let line_text = self.line(l);
            let leading = line_text
                .chars()
                .take_while(|c| *c == ' ')
                .count()
                .min(tab_size.max(1) as usize);
            if leading > 0 {
                let remove_end = line_start + leading;
                self.remove_byte_range(line_start, remove_end);
                // Adjust cursor if it's on this line and after the removed spaces
                if cursor_line == l && cursor_byte > line_start {
                    if cursor_byte >= remove_end {
                        self.cursor = cursor_byte - leading;
                    } else {
                        // Cursor is in the middle of the spaces being removed
                        self.cursor = line_start;
                    }
                }
            }
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Toggle line comments on the cursor line, or on every line the selection
    /// touches. `prefix` is the language token (`//`, `#`, `--`, `;`, `%`,
    /// `REM`) with no trailing space. Returns whether the text changed.
    pub fn toggle_line_comment(&mut self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return false;
        }
        let (start_line, end_line) = self.comment_line_span();
        let uncomment = self.span_is_fully_commented(start_line, end_line, prefix);
        self.push_undo();
        let marker = format!("{prefix} ");
        for line_idx in (start_line..=end_line).rev() {
            let line_start = self.rope.line_to_byte(line_idx);
            let raw = self.line(line_idx);
            let body = raw.trim_end_matches(['\n', '\r']);
            let ws_len = leading_ascii_ws_len(body);
            if uncomment {
                if body[ws_len..].trim().is_empty() {
                    continue;
                }
                let after = &body[ws_len..];
                let Some(n) = comment_prefix_len(after, prefix) else {
                    continue;
                };
                let remove_at = line_start + ws_len;
                self.remove_byte_range(remove_at, remove_at + n);
                self.shift_positions_after_edit(remove_at, n, false);
            } else {
                let at = line_start + ws_len;
                self.insert_at_byte(at, &marker);
                self.shift_positions_after_edit(at, marker.len(), true);
            }
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
        true
    }

    /// Toggle a block comment. A one-line span wraps that line as
    /// `{open} text {close}`. A multi-line span puts `open` on its own line
    /// before the first line and `close` on its own line after the last.
    /// Used for Markdown, HTML, and XML (`<!--` / `-->`), CSS (`/*` / `*/`),
    /// and OCaml (`(*` / `*)`). Returns whether the text changed.
    pub fn toggle_block_comment(&mut self, open: &str, close: &str) -> bool {
        if open.is_empty() || close.is_empty() {
            return false;
        }
        let (start_line, end_line) = self.comment_line_span();
        let uncomment = self.span_is_block_wrapped(start_line, end_line, open, close);
        self.push_undo();
        if start_line == end_line {
            self.toggle_single_line_block(start_line, open, close, uncomment);
        } else if uncomment {
            self.unwrap_block_lines(start_line, end_line);
        } else {
            self.wrap_block_lines(start_line, end_line, open, close);
        }
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
        true
    }

    fn toggle_single_line_block(
        &mut self,
        line_idx: usize,
        open: &str,
        close: &str,
        uncomment: bool,
    ) {
        let line_start = self.rope.line_to_byte(line_idx);
        let raw = self.line(line_idx);
        let body = line_body(&raw);
        let ws_len = leading_ascii_ws_len(body);
        if uncomment {
            let Some((pre_n, suf_n)) = block_wrap_lens(&body[ws_len..], open, close) else {
                return;
            };
            let suf_at = line_start + body.len() - suf_n;
            self.remove_byte_range(suf_at, suf_at + suf_n);
            self.shift_positions_after_edit(suf_at, suf_n, false);
            let pre_at = line_start + ws_len;
            self.remove_byte_range(pre_at, pre_at + pre_n);
            self.shift_positions_after_edit(pre_at, pre_n, false);
        } else {
            let prefix = format!("{open} ");
            let suffix = format!(" {close}");
            let suf_at = line_start + body.len();
            self.insert_at_byte(suf_at, &suffix);
            self.shift_positions_after_edit(suf_at, suffix.len(), true);
            let pre_at = line_start + ws_len;
            self.insert_at_byte(pre_at, &prefix);
            self.shift_positions_after_edit(pre_at, prefix.len(), true);
        }
    }

    fn wrap_block_lines(&mut self, start_line: usize, end_line: usize, open: &str, close: &str) {
        let indent = self.span_block_indent(start_line, end_line);
        let last_start = self.rope.line_to_byte(end_line);
        let last_raw = self.line(end_line);
        let last_body = line_body(&last_raw);
        let close_at = last_start + last_body.len();
        let close_text = format!("\n{indent}{close}");
        self.insert_at_byte(close_at, &close_text);
        self.shift_positions_after_edit(close_at, close_text.len(), true);
        let open_at = self.rope.line_to_byte(start_line);
        let open_text = format!("{indent}{open}\n");
        self.insert_at_byte(open_at, &open_text);
        self.shift_positions_after_edit(open_at, open_text.len(), true);
        // Inserting the open line at the selection start would otherwise leave
        // the range on the inner lines only, so a second toggle would wrap again.
        let sel_start = open_at;
        let close_line = end_line + 2;
        let close_line_start = self.rope.line_to_byte(close_line);
        let close_line_len = self.line(close_line).len();
        let sel_end = close_line_start + close_line_len;
        self.selection = Some(sel_start..sel_end);
        if self.selection_anchor.is_some() {
            self.selection_anchor = Some(sel_start);
        }
    }

    fn unwrap_block_lines(&mut self, start_line: usize, end_line: usize) {
        let close_start = self.rope.line_to_byte(end_line);
        let close_raw = self.line(end_line);
        if end_line > 0 && !close_raw.ends_with('\n') && !close_raw.ends_with('\r') {
            let prev_raw = self.line(end_line - 1);
            let extra = prev_raw.len() - line_body(&prev_raw).len();
            let at = close_start - extra;
            let n = close_raw.len() + extra;
            self.remove_byte_range(at, at + n);
            self.shift_positions_after_edit(at, n, false);
        } else {
            let n = close_raw.len();
            self.remove_byte_range(close_start, close_start + n);
            self.shift_positions_after_edit(close_start, n, false);
        }
        let open_start = self.rope.line_to_byte(start_line);
        let open_n = self.line(start_line).len();
        self.remove_byte_range(open_start, open_start + open_n);
        self.shift_positions_after_edit(open_start, open_n, false);
    }

    /// Cursor line, or the lines a non-empty selection touches. A selection
    /// that ends on a newline does not include the following line.
    fn comment_line_span(&self) -> (usize, usize) {
        if let Some(sel) = &self.selection {
            if sel.start != sel.end {
                let len = self.rope.len_bytes();
                let start = self.rope.byte_to_line(sel.start.min(len));
                let end = self.rope.byte_to_line(sel.end.saturating_sub(1).min(len));
                return (start, end.max(start));
            }
        }
        let line = self
            .rope
            .byte_to_line(self.cursor.min(self.rope.len_bytes()));
        (line, line)
    }

    /// True when every non-blank line in the span already starts with `prefix`
    /// at its first non-whitespace character. An all-blank span is not commented.
    fn span_is_fully_commented(&self, start: usize, end: usize, prefix: &str) -> bool {
        let mut saw_code = false;
        for line_idx in start..=end {
            let raw = self.line(line_idx);
            let body = raw.trim_end_matches(['\n', '\r']);
            let ws_len = leading_ascii_ws_len(body);
            if body[ws_len..].trim().is_empty() {
                continue;
            }
            saw_code = true;
            if comment_prefix_len(&body[ws_len..], prefix).is_none() {
                return false;
            }
        }
        saw_code
    }

    /// Leading ASCII spaces/tabs of the least-indented non-blank line in the span.
    /// Copied verbatim so a tab-indented block keeps tab wrappers.
    fn span_block_indent(&self, start: usize, end: usize) -> String {
        let mut best: Option<String> = None;
        for line_idx in start..=end {
            let raw = self.line(line_idx);
            let body = line_body(&raw);
            if body.trim().is_empty() {
                continue;
            }
            let ws_len = leading_ascii_ws_len(body);
            let prefix = body[..ws_len].to_string();
            match &best {
                None => best = Some(prefix),
                Some(cur) if ws_len < cur.len() => best = Some(prefix),
                _ => {}
            }
        }
        best.unwrap_or_default()
    }

    fn span_is_block_wrapped(&self, start: usize, end: usize, open: &str, close: &str) -> bool {
        if start == end {
            let raw = self.line(start);
            let body = line_body(&raw);
            let ws_len = leading_ascii_ws_len(body);
            if body[ws_len..].trim().is_empty() {
                return false;
            }
            return block_wrap_lens(&body[ws_len..], open, close).is_some();
        }
        line_is_wrapper(&self.line(start), open) && line_is_wrapper(&self.line(end), close)
    }

    /// Shift caret, selection, and anchor after inserting or removing `n` bytes at `at`.
    fn shift_positions_after_edit(&mut self, at: usize, n: usize, insert: bool) {
        let shift = |pos: &mut usize| {
            if insert {
                if *pos >= at {
                    *pos += n;
                }
            } else if *pos >= at.saturating_add(n) {
                *pos -= n;
            } else if *pos > at {
                *pos = at;
            }
        };
        shift(&mut self.cursor);
        if let Some(sel) = &mut self.selection {
            shift(&mut sel.start);
            shift(&mut sel.end);
            if sel.start > sel.end {
                std::mem::swap(&mut sel.start, &mut sel.end);
            }
        }
        if let Some(anchor) = &mut self.selection_anchor {
            shift(anchor);
        }
    }

    /// Return full content as String (for save).
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Replace the whole document with `new_text` as a single undoable edit.
    /// Keeps `id`, `path`, `language`, and `is_initial`. Cursor is remapped by
    /// the pre-edit `(line, col)` and clamped to the new text. No-op (no undo
    /// push, no modified flag) when the text is unchanged.
    pub fn replace_document(&mut self, new_text: &str) {
        if new_text == self.text() {
            return;
        }
        let (line, col) = self.cursor_position();
        self.push_undo();
        self.rope = Rope::from_str(new_text);
        self.selection = None;
        self.selection_anchor = None;
        let last_line = self.line_count().saturating_sub(1);
        let mapped_line = line.min(last_line);
        self.cursor = self.byte_at_visual(mapped_line, col);
        self.is_modified = true;
        self.bump_generation();
        self.adjust_scroll();
    }

    /// Replace the document from disk contents. One undo snapshot, cursor
    /// remapped like `replace_document`, then `is_modified = false`.
    pub fn reload_from(&mut self, new_text: &str) {
        self.replace_document(new_text);
        self.is_modified = false;
    }

    /// Set path + re-detect language + clear modified.
    pub fn set_path(&mut self, path: PathBuf) {
        self.language = Language::from_path(&path);
        self.disk_stamp = crate::file_watch::FileStamp::of(&path);
        self.path = Some(path);
        self.is_modified = false;
        self.disk_gone_notified = false;
    }

    // ===== Word-wrap-aware scroll/cursor helpers =====
    //
    // These consult a WrapLayout computed by the caller (render path or main.rs
    // event handler). When word_wrap is off, the caller must use the original
    // (unwrapped) helpers instead.

    /// Scroll by `delta` screen rows. `total_screen_rows` is the buffer's total
    /// wrapped height at the current width.
    pub fn scroll_wrapped(&mut self, delta: isize, total_screen_rows: usize) {
        let current = self.scroll_offset.0 as isize;
        let max = total_screen_rows.saturating_sub(self.visible_lines.max(1)) as isize;
        let new = (current + delta).max(0).min(max) as usize;
        self.scroll_offset.0 = new;
    }

    /// Clamp scroll_offset.0 to the wrapped range.
    pub fn clamp_scroll_wrapped(&mut self, total_screen_rows: usize) {
        let max = total_screen_rows.saturating_sub(self.visible_lines.max(1));
        if self.scroll_offset.0 > max {
            self.scroll_offset.0 = max;
        }
    }

    /// Minimal visibility clamp (used by mouse placement & find jumps).
    pub fn ensure_cursor_visible_wrapped(&mut self, layout: &crate::wrap::WrapLayout, vis: usize) {
        let v = vis.max(1);
        let cursor_row = crate::wrap::screen_row_of_byte(layout, &self.rope, 0, self.cursor);
        let s = self.scroll_offset.0;
        if cursor_row < s {
            self.scroll_offset.0 = cursor_row;
        } else if cursor_row >= s + v {
            self.scroll_offset.0 = cursor_row.saturating_sub(v.saturating_sub(2));
        }
    }

    /// Move by `delta` screen rows (negative is up). Keeps the display column
    /// inside the current screen row. A no-op at the first or last screen row
    /// leaves the selection alone.
    pub fn move_screen_rows(
        &mut self,
        layout: &crate::wrap::WrapLayout,
        delta: isize,
        extend_selection: bool,
    ) {
        if delta == 0 || layout.total_screen_rows == 0 {
            return;
        }
        let row = crate::wrap::screen_row_of_byte(layout, &self.rope, 0, self.cursor);
        let col = crate::wrap::screen_col_of_byte(layout, &self.rope, 0, self.cursor);
        let last = layout.total_screen_rows - 1;
        let target = if delta < 0 {
            row.saturating_sub(delta.unsigned_abs())
        } else {
            row.saturating_add(delta as usize).min(last)
        };
        if target == row {
            return;
        }
        self.place_on_screen_row(layout, target, col, extend_selection);
        self.ensure_cursor_visible_wrapped(layout, self.visible_lines.max(1));
    }

    /// Page up/down under wrap. `layout` is the buffer's full wrap layout.
    ///
    /// Scrolls by a page of screen rows, then puts the caret on the screen row
    /// that keeps its distance from the top of the viewport, at the same
    /// display column. Does not go through `move_to_line`: that seeks a
    /// logical line and skips wrapped continuations.
    pub fn page_wrapped(
        &mut self,
        layout: &crate::wrap::WrapLayout,
        down: bool,
        vis: usize,
        extend_selection: bool,
    ) {
        if layout.total_screen_rows == 0 {
            return;
        }
        let vis = vis.max(1);
        let page = vis.saturating_sub(1).max(1) as isize;
        let old_scroll = self.scroll_offset.0;
        let row = crate::wrap::screen_row_of_byte(layout, &self.rope, 0, self.cursor);
        let col = crate::wrap::screen_col_of_byte(layout, &self.rope, 0, self.cursor);
        let rel = row.saturating_sub(old_scroll);
        let total = layout.total_screen_rows as isize;
        let max = total.saturating_sub(1);
        let new_scroll = if down {
            (old_scroll as isize + page).min(max)
        } else {
            (old_scroll as isize - page).max(0)
        } as usize;
        let last = layout.total_screen_rows - 1;
        let place = new_scroll.saturating_add(rel).min(last);
        self.scroll_offset.0 = new_scroll;
        self.place_on_screen_row(layout, place, col, extend_selection);
        self.ensure_cursor_visible_wrapped(layout, vis);
    }

    fn place_on_screen_row(
        &mut self,
        layout: &crate::wrap::WrapLayout,
        screen_row: usize,
        col: usize,
        extend_selection: bool,
    ) {
        let byte = crate::wrap::byte_of_screen_row(layout, &self.rope, 0, screen_row, col);
        // An empty screen row (a tab wider than the wrap width) has no byte
        // of its own. Leave the caret and the selection where they are.
        let landed = crate::wrap::screen_row_of_byte(layout, &self.rope, 0, byte);
        if landed != screen_row {
            return;
        }
        if extend_selection {
            self.extend_selection_to(byte);
        } else {
            self.clear_selection();
        }
        self.cursor = byte;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_buffer_is_empty() {
        let b = Buffer::new();
        assert_eq!(b.line_count(), 1);
        assert_eq!(b.text(), "");
        assert_eq!(b.cursor_position(), (0, 0));
        assert!(
            !b.is_initial,
            "Buffer::new is a user tab, not a placeholder"
        );
        assert!(!b.is_modified);
        assert!(b.path.is_none());
    }

    #[test]
    fn from_str_is_not_initial() {
        let b = Buffer::from_str("hi", None);
        assert!(!b.is_initial);
    }

    #[test]
    fn placeholder_is_initial_untitled_unmodified() {
        let b = Buffer::placeholder();
        assert!(b.is_initial);
        assert!(b.path.is_none());
        assert!(!b.is_modified);
        assert_eq!(b.text(), "");
    }

    #[test]
    fn insert_and_cursor() {
        let mut b = Buffer::new();
        b.insert("hello");
        assert_eq!(b.text(), "hello");
        assert_eq!(b.cursor_position(), (0, 5));

        b.insert_char('\n');
        b.insert("world");
        assert_eq!(b.text(), "hello\nworld");
        assert_eq!(b.cursor_position(), (1, 5));
    }

    #[test]
    fn backspace_and_delete() {
        let mut b = Buffer::from_str("abc", None);
        b.cursor = 1; // after 'a'
        b.backspace();
        assert_eq!(b.text(), "bc");
        assert_eq!(b.cursor, 0);

        b.cursor = 1; // between b|c
        b.delete();
        assert_eq!(b.text(), "b");
    }

    /// Regression (root cause of the loaded-file cursor desync + paste panic):
    /// ropey's `Rope::insert`/`remove`/`slice` are CHAR-indexed, but this crate
    /// stores `cursor`/`selection` as BYTE offsets. For multi-byte UTF-8 the byte
    /// offset exceeds the char index, so the edit lands at the wrong (later)
    /// position or panics (`char range out of bounds`). These tests hold the
    /// byte→char bridging honest. Content uses '—' (U+2014, 3 bytes) and '€'
    /// (U+20AC, 3 bytes).
    #[test]
    fn insert_after_multibyte_lands_at_cursor() {
        // bytes: a(1) —(3) b(1) = 5 bytes, 3 chars. cursor between '—' and 'b'.
        let mut b = Buffer::from_str("a—b", None);
        b.cursor = 4; // byte offset right after '—' (a=0, —=1..4, b=4)
        b.insert("X");
        assert_eq!(b.text(), "a—Xb");
        // cursor should be just after the inserted X (byte 5)
        assert_eq!(b.cursor, 5);
        assert_eq!(b.cursor_position(), (0, 3));
    }

    #[test]
    fn backspace_after_multibyte_deletes_the_multibyte_char() {
        let mut b = Buffer::from_str("a—b", None);
        b.cursor = 4; // between '—' and 'b'
        b.backspace();
        // backspace must delete the '—' (the char before the cursor), NOT 'b'.
        assert_eq!(b.text(), "ab");
        assert_eq!(b.cursor, 1);
    }

    #[test]
    fn delete_before_multibyte_deletes_the_multibyte_char() {
        let mut b = Buffer::from_str("a—b", None);
        b.cursor = 1; // right after 'a', before '—'
        b.delete();
        // delete must remove the '—' (char after cursor), not 'b' or part of it.
        assert_eq!(b.text(), "ab");
        assert_eq!(b.cursor, 1);
    }

    #[test]
    fn insert_selection_replace_across_multibyte() {
        // Select the '—' (bytes 1..4) and replace with "==". Exercises
        // remove(byte_range)+insert on multi-byte (the panic-prone path).
        let mut b = Buffer::from_str("a—b", None);
        b.selection = Some(1..4);
        b.cursor = 4;
        b.insert("==");
        assert_eq!(b.text(), "a==b");
        assert_eq!(b.cursor, 3); // 1 + "==" (2 bytes)
    }

    #[test]
    fn paste_large_multibyte_no_panic() {
        // Reproduces the user's crash: a byte offset that exceeds len_chars().
        // Build content where byte_len > char_len by a lot, then insert at EOF
        // (byte offset == len_bytes > len_chars) and at a valid mid-buffer
        // boundary. Must not panic and must preserve content.
        let unit = "a—b€c\n"; // 3 multi-byte chars per line
        let big: String = std::iter::repeat_n(unit, 500).collect();
        let mut b = Buffer::from_str(&big, None);
        let eof = b.rope.len_bytes();
        b.cursor = eof;
        b.insert("TAIL—€");
        assert!(b.text().ends_with("TAIL—€"));

        // Insert mid-buffer at a byte offset that is past len_chars() but on a
        // valid char boundary: byte 1 = start of the first '—' (a=0, —=1..4).
        b.cursor = 1;
        b.insert("Z");
        let txt = b.text();
        assert!(
            txt.starts_with("aZ—b€c\n"),
            "got: {}",
            &txt[..txt.len().min(40)]
        );
    }

    #[test]
    fn byte_slice_selection_extracts_correct_multibyte_substring() {
        // rope.slice() is char-indexed; selecting by byte range must use
        // byte_slice or the wrong substring is returned for multi-byte text.
        let b = Buffer::from_str("a—b€c", None);
        // Select bytes 1..4 which is exactly the '—'
        let sub = b.rope.byte_slice(1..4).to_string();
        assert_eq!(sub, "—");
        // Select bytes 4..7 which is exactly the '€' (b is 1 byte at 4? no: a=0,—=1..4,b=4,€=5..8)
        // recompute: a(0) —(1..4) b(4) €(5..8) c(8)
        let sub2 = b.rope.byte_slice(5..8).to_string();
        assert_eq!(sub2, "€");
    }

    /// Regression: on a multi-line buffer (the case that reproduces the reported
    /// editing desync), `cursor_position()` must report exactly the visual
    /// (row,col) for every interesting offset, and `byte_at_visual` must round-trip
    /// it. Guards against off-by-one at line starts, on the `\n`, and at EOF.
    #[test]
    fn cursor_position_roundtrip_multiline() {
        // "abc\ndef\n" -> bytes: 0..4 line0 (abc + \n@3), 4..8 line1 (def + \n@7)
        let mut b = Buffer::from_str("abc\ndef\n", None);
        let n = b.rope.len_bytes();

        // Start of line 0.
        b.cursor = 0;
        assert_eq!(b.cursor_position(), (0, 0));

        // End of line 0 content (just before its newline).
        b.cursor = 3;
        assert_eq!(b.cursor_position(), (0, 3));

        // On the newline of line 0 -> still reported as end of line 0.
        b.cursor = b.rope.line_to_byte(1);
        // line_to_byte(1) points AT the first byte of line 1, not the newline.
        assert_eq!(b.cursor_position(), (1, 0));

        // Round-trip via byte_at_visual for clicks at row1 col0 and col2.
        assert_eq!(b.byte_at_visual(1, 0), b.rope.line_to_byte(1));
        assert_eq!(b.byte_at_visual(1, 2), b.rope.line_to_byte(1) + 2);

        // EOF.
        b.cursor = n;
        let (r, _c) = b.cursor_position();
        assert!(
            r == 2 || r == 1,
            "EOF row should not overflow past the rendered line range, got {r}"
        );

        // move_to_line must land on the start of the requested line, not past it.
        b.cursor = 0;
        b.move_lines_down(1, false);
        assert_eq!(b.cursor_position().0, 1);
        // Column preserved at 0.
        assert_eq!(b.cursor_position().1, 0);
    }

    #[test]
    fn smart_indent_basic() {
        let mut b = Buffer::from_str("def foo():", None);
        b.cursor = b.rope.len_bytes();
        b.newline_with_indent(4);
        assert!(b.text().starts_with("def foo():\n    "));
        assert!(b.cursor_position().1 >= 4);
    }

    #[test]
    fn insert_tab_at_caret_mid_line_inserts_full_tab_size() {
        let mut b = Buffer::from_str("abcd", None);
        b.cursor = 2; // "ab|cd"
        b.insert_tab(4);
        assert_eq!(b.text(), "ab    cd");
        assert_eq!(b.cursor, 6);
    }

    #[test]
    fn insert_tab_at_odd_column_still_inserts_full_width() {
        let mut b = Buffer::from_str("abc", None);
        b.cursor = 1; // "a|bc"
        b.insert_tab(4);
        assert_eq!(b.text(), "a    bc");
        assert_eq!(b.cursor, 5);
    }

    #[test]
    fn insert_tab_at_eol_extends_line() {
        let mut b = Buffer::from_str("abcd", None);
        b.cursor = 4; // "abcd|"
        b.insert_tab(4);
        assert_eq!(b.text(), "abcd    ");
        assert_eq!(b.cursor, 8);
    }

    #[test]
    fn insert_tab_honors_configured_width() {
        let mut b = Buffer::from_str("ab", None);
        b.cursor = 2;
        b.insert_tab(2);
        assert_eq!(b.text(), "ab  ");
        assert_eq!(b.cursor, 4);
    }

    #[test]
    fn insert_tab_at_col_zero_inserts_at_start() {
        let mut b = Buffer::from_str("hello", None);
        b.cursor = 0;
        b.insert_tab(4);
        assert_eq!(b.text(), "    hello");
        assert_eq!(b.cursor, 4);
    }

    #[test]
    fn indent_and_smart_newline_use_tab_size() {
        let mut b = Buffer::from_str("hello", None);
        b.cursor = 0;
        b.indent(2);
        assert_eq!(b.text(), "  hello");

        let mut b = Buffer::from_str("def foo():", None);
        b.cursor = b.rope.len_bytes();
        b.newline_with_indent(2);
        assert!(b.text().starts_with("def foo():\n  "));
    }

    #[test]
    fn insert_tab_with_multiline_selection_indents_lines() {
        let mut b = Buffer::from_str("one\ntwo\n", None);
        b.selection = Some(0..7); // both lines
        b.cursor = 7;
        b.insert_tab(4);
        assert_eq!(b.text(), "    one\n    two\n");
    }

    #[test]
    fn insert_tab_with_single_line_selection_indents_not_replaces() {
        let mut b = Buffer::from_str("hello", None);
        b.selection = Some(0..5);
        b.cursor = 5;
        b.insert_tab(4);
        assert_eq!(b.text(), "    hello");
    }

    #[test]
    fn insert_tab_selection_ending_on_newline_still_indents() {
        // Shift+Down from col 0 on "one\ntwo" selects "one\n" (still one
        // logical end-line after saturating_sub(1)). Must indent, not join.
        let mut b = Buffer::from_str("one\ntwo", None);
        b.selection = Some(0..4);
        b.cursor = 4;
        b.insert_tab(4);
        assert_eq!(b.text(), "    one\ntwo");
    }

    #[test]
    fn dedent_strips_up_to_tab_size() {
        let mut b = Buffer::from_str("      hello", None);
        b.cursor = 6;
        b.dedent(4);
        assert_eq!(b.text(), "  hello");
    }

    #[test]
    fn comment_tokens_cover_every_language() {
        type CommentRow = (
            Language,
            Option<&'static str>,
            Option<(&'static str, &'static str)>,
        );
        let table: &[CommentRow] = &[
            (Language::Rust, Some("//"), None),
            (Language::C, Some("//"), None),
            (Language::Cpp, Some("//"), None),
            (Language::Javascript, Some("//"), None),
            (Language::Typescript, Some("//"), None),
            (Language::Java, Some("//"), None),
            (Language::CSharp, Some("//"), None),
            (Language::Go, Some("//"), None),
            (Language::Php, Some("//"), None),
            (Language::Scala, Some("//"), None),
            (Language::Groovy, Some("//"), None),
            (Language::D, Some("//"), None),
            (Language::ObjectiveC, Some("//"), None),
            (Language::ObjectiveCpp, Some("//"), None),
            (Language::Zig, Some("//"), None),
            (Language::Jsonc, Some("//"), None),
            (Language::Python, Some("#"), None),
            (Language::Toml, Some("#"), None),
            (Language::Yaml, Some("#"), None),
            (Language::Bash, Some("#"), None),
            (Language::Ruby, Some("#"), None),
            (Language::Perl, Some("#"), None),
            (Language::R, Some("#"), None),
            (Language::Tcl, Some("#"), None),
            (Language::JavaProperties, Some("#"), None),
            (Language::Makefile, Some("#"), None),
            (Language::Sql, Some("--"), None),
            (Language::Lua, Some("--"), None),
            (Language::Haskell, Some("--"), None),
            (Language::Ini, Some(";"), None),
            (Language::Clojure, Some(";"), None),
            (Language::Lisp, Some(";"), None),
            (Language::Erlang, Some("%"), None),
            (Language::Latex, Some("%"), None),
            (Language::Tex, Some("%"), None),
            (Language::Batch, Some("REM"), None),
            (Language::Markdown, None, Some(("<!--", "-->"))),
            (Language::Html, None, Some(("<!--", "-->"))),
            (Language::Xml, None, Some(("<!--", "-->"))),
            (Language::Css, None, Some(("/*", "*/"))),
            (Language::OCaml, None, Some(("(*", "*)"))),
            (Language::Plain, None, None),
            (Language::Json, None, None),
            (Language::Diff, None, None),
            (Language::RestructuredText, None, None),
        ];
        let mut unseen: Vec<Language> = Language::ALL.to_vec();
        for (lang, line, block) in table {
            assert_eq!(lang.line_comment_prefix(), *line, "{lang:?} line");
            assert_eq!(lang.block_comment_pair(), *block, "{lang:?} block");
            let before = unseen.len();
            unseen.retain(|item| item != lang);
            assert_eq!(
                unseen.len(),
                before - 1,
                "{lang:?} missing from Language::ALL or duplicated"
            );
        }
        assert!(unseen.is_empty(), "no comment expectation for {unseen:?}");
        for lang in Language::ALL {
            assert!(!lang.label().is_empty(), "{lang:?}");
        }
        assert_eq!(Language::CSharp.label(), "C#");
        assert_eq!(Language::Json.label(), "JSON");
        assert_eq!(Language::ObjectiveCpp.label(), "Objective-C++");
    }

    #[test]
    fn toggle_line_comment_round_trips_other_prefixes() {
        for (src, prefix, commented) in [
            ("select 1", "--", "-- select 1"),
            ("(def x 1)", ";", "; (def x 1)"),
            ("io:format", "%", "% io:format"),
            ("echo hi", "REM", "REM echo hi"),
        ] {
            let mut b = Buffer::from_str(src, None);
            assert!(b.toggle_line_comment(prefix), "{prefix}");
            assert_eq!(b.text(), commented, "{prefix}");
            assert!(b.toggle_line_comment(prefix), "{prefix}");
            assert_eq!(b.text(), src, "{prefix}");
        }
    }

    #[test]
    fn toggle_batch_comment_matches_any_case_but_not_a_longer_word() {
        let mut lower = Buffer::from_str("rem echo", None);
        assert!(lower.toggle_line_comment("REM"));
        assert_eq!(lower.text(), "echo");

        let mut word = Buffer::from_str("REMote", None);
        assert!(word.toggle_line_comment("REM"));
        assert_eq!(word.text(), "REM REMote");
        assert!(word.toggle_line_comment("REM"));
        assert_eq!(word.text(), "REMote");
    }

    #[test]
    fn toggle_block_comment_css_and_ocaml_round_trip() {
        let mut css = Buffer::from_str("color: red;", None);
        assert!(css.toggle_block_comment("/*", "*/"));
        assert_eq!(css.text(), "/* color: red; */");
        assert!(css.toggle_block_comment("/*", "*/"));
        assert_eq!(css.text(), "color: red;");

        let mut ml = Buffer::from_str("let x = 1", None);
        assert!(ml.toggle_block_comment("(*", "*)"));
        assert_eq!(ml.text(), "(* let x = 1 *)");
        assert!(ml.toggle_block_comment("(*", "*)"));
        assert_eq!(ml.text(), "let x = 1");
    }

    fn commented(src: &str, prefix: &str) -> String {
        let mut b = Buffer::from_str(src, None);
        b.cursor = 0;
        assert!(b.toggle_line_comment(prefix));
        b.text()
    }

    #[test]
    fn toggle_line_comment_rust_round_trip_keeps_indent() {
        let mut b = Buffer::from_str("    let x = 1;\n", None);
        b.cursor = 4;
        assert!(b.toggle_line_comment("//"));
        assert_eq!(b.text(), "    // let x = 1;\n");
        assert_eq!(b.cursor, 7); // three bytes ("// ") inserted at the cursor
        assert!(b.toggle_line_comment("//"));
        assert_eq!(b.text(), "    let x = 1;\n");
        assert_eq!(b.cursor, 4);
    }

    #[test]
    fn toggle_line_comment_on_cursor_line_only() {
        let mut b = Buffer::from_str("one\ntwo\nthree\n", None);
        b.cursor = b.rope.line_to_byte(1);
        b.toggle_line_comment("#");
        assert_eq!(b.text(), "one\n# two\nthree\n");
    }

    #[test]
    fn toggle_line_comment_selection_covers_touched_lines_only() {
        let mut b = Buffer::from_str("one\ntwo\nthree\n", None);
        b.selection = Some(0..7); // "one\ntw"
        b.cursor = 7;
        b.selection_anchor = Some(0);
        b.toggle_line_comment("//");
        assert_eq!(b.text(), "// one\n// two\nthree\n");
        assert!(b.selection.is_some());
        assert_eq!(b.selection_anchor, Some(3));
    }

    #[test]
    fn toggle_line_comment_selection_ending_on_newline_skips_next_line() {
        let mut b = Buffer::from_str("one\ntwo", None);
        b.selection = Some(0..4); // "one\n"
        b.cursor = 4;
        b.toggle_line_comment("//");
        assert_eq!(b.text(), "// one\ntwo");
    }

    #[test]
    fn toggle_line_comment_mixed_selection_comments_every_line() {
        let mut b = Buffer::from_str("// a\nb\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.toggle_line_comment("//");
        assert_eq!(b.text(), "// // a\n// b\n");
    }

    #[test]
    fn toggle_line_comment_all_commented_selection_uncomments() {
        let mut b = Buffer::from_str("// a\n// b\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.toggle_line_comment("//");
        assert_eq!(b.text(), "a\nb\n");
    }

    #[test]
    fn toggle_line_comment_blank_line_in_commented_block_stays_blank() {
        let mut b = Buffer::from_str("// a\n\n// b\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.toggle_line_comment("//");
        assert_eq!(b.text(), "a\n\nb\n");
    }

    #[test]
    fn toggle_line_comment_empty_line_gains_a_token() {
        let mut b = Buffer::from_str("\n", None);
        b.toggle_line_comment("#");
        assert_eq!(b.text(), "# \n");
    }

    #[test]
    fn toggle_line_comment_removes_token_without_space_and_rustdoc_once() {
        let mut hash = Buffer::from_str("#foo\n", None);
        hash.toggle_line_comment("#");
        assert_eq!(hash.text(), "foo\n");

        let mut docs = Buffer::from_str("/// doc\n", None);
        docs.toggle_line_comment("//");
        assert_eq!(docs.text(), "/ doc\n");
    }

    #[test]
    fn toggle_line_comment_cursor_in_indent_does_not_move() {
        let mut b = Buffer::from_str("    code\n", None);
        b.cursor = 2;
        b.toggle_line_comment("//");
        assert_eq!(b.text(), "    // code\n");
        assert_eq!(b.cursor, 2);
    }

    #[test]
    fn toggle_line_comment_is_one_undo_step() {
        let mut b = Buffer::from_str("a\nb\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.selection_anchor = Some(0);
        b.toggle_line_comment("//");
        assert_eq!(b.text(), "// a\n// b\n");
        assert_eq!(b.selection_anchor, Some(3));
        b.undo();
        assert_eq!(b.text(), "a\nb\n");
        assert_eq!(b.selection, Some(0..4));
        assert_eq!(b.selection_anchor, Some(0));
    }

    #[test]
    fn toggle_line_comment_hash_languages_use_hash() {
        assert_eq!(commented("x = 1\n", "#"), "# x = 1\n");
    }

    #[test]
    fn toggle_block_comment_single_line_round_trip_keeps_indent() {
        let mut b = Buffer::from_str("    hello world\n", None);
        b.cursor = 4;
        assert!(b.toggle_block_comment("<!--", "-->"));
        assert_eq!(b.text(), "    <!-- hello world -->\n");
        assert_eq!(b.cursor, 9); // "<!-- " is 5 bytes, inserted at the cursor
        assert!(b.toggle_block_comment("<!--", "-->"));
        assert_eq!(b.text(), "    hello world\n");
        assert_eq!(b.cursor, 4);
    }

    #[test]
    fn toggle_block_comment_on_cursor_line_only() {
        let mut b = Buffer::from_str("one\ntwo\nthree\n", None);
        b.cursor = b.rope.line_to_byte(1);
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "one\n<!-- two -->\nthree\n");
    }

    #[test]
    fn toggle_block_comment_single_line_selection_wraps_that_line() {
        let mut b = Buffer::from_str("one\ntwo\nthree\n", None);
        b.selection = Some(0..3); // "one"
        b.cursor = 3;
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "<!-- one -->\ntwo\nthree\n");
    }

    #[test]
    fn toggle_block_comment_selection_ending_on_newline_is_still_one_line() {
        let mut b = Buffer::from_str("one\ntwo", None);
        b.selection = Some(0..4); // "one\n" — comment_line_span skips the next line
        b.cursor = 4;
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "<!-- one -->\ntwo");
    }

    #[test]
    fn toggle_block_comment_multiline_wraps_once() {
        let mut b = Buffer::from_str("one\ntwo\nthree\n", None);
        b.selection = Some(0..7); // "one\ntw" — lines 0..=1
        b.cursor = 7;
        b.selection_anchor = Some(0);
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "<!--\none\ntwo\n-->\nthree\n");
    }

    #[test]
    fn toggle_block_comment_multiline_round_trip() {
        let mut b = Buffer::from_str("one\ntwo\nthree\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        assert!(b.toggle_block_comment("<!--", "-->"));
        assert_eq!(b.text(), "<!--\none\ntwo\nthree\n-->\n");
        assert!(b.toggle_block_comment("<!--", "-->"));
        assert_eq!(b.text(), "one\ntwo\nthree\n");
    }

    #[test]
    fn toggle_block_comment_multiline_uses_minimum_indent() {
        let mut b = Buffer::from_str("    hello\n      world\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "    <!--\n    hello\n      world\n    -->\n");
    }

    #[test]
    fn toggle_block_comment_multiline_inner_blank_stays() {
        let mut b = Buffer::from_str("a\n\nb\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "<!--\na\n\nb\n-->\n");
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "a\n\nb\n");
    }

    #[test]
    fn toggle_block_comment_multiline_not_wrapped_wraps_again() {
        let mut b = Buffer::from_str("<!-- a -->\nb\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "<!--\n<!-- a -->\nb\n-->\n");
    }

    #[test]
    fn toggle_block_comment_empty_line_gains_tokens() {
        let mut b = Buffer::from_str("\n", None);
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "<!--  -->\n");
    }

    #[test]
    fn toggle_block_comment_removes_tokens_without_spaces() {
        let mut b = Buffer::from_str("<!--foo-->\n", None);
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "foo\n");
    }

    #[test]
    fn toggle_block_comment_cursor_in_indent_does_not_move() {
        let mut b = Buffer::from_str("    code\n", None);
        b.cursor = 2;
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "    <!-- code -->\n");
        assert_eq!(b.cursor, 2);
    }

    #[test]
    fn toggle_block_comment_is_one_undo_step() {
        let mut b = Buffer::from_str("a\nb\n", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.selection_anchor = Some(0);
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "<!--\na\nb\n-->\n");
        b.undo();
        assert_eq!(b.text(), "a\nb\n");
        assert_eq!(b.selection, Some(0..4));
        assert_eq!(b.selection_anchor, Some(0));
    }

    #[test]
    fn toggle_block_comment_rejects_empty_tokens() {
        let mut b = Buffer::from_str("hello\n", None);
        assert!(!b.toggle_block_comment("", "-->"));
        assert!(!b.toggle_block_comment("<!--", ""));
        assert_eq!(b.text(), "hello\n");
    }

    #[test]
    fn toggle_block_comment_last_line_without_newline() {
        let mut b = Buffer::from_str("one\ntwo", None);
        b.selection = Some(0..b.rope.len_bytes());
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "<!--\none\ntwo\n-->");
        b.toggle_block_comment("<!--", "-->");
        assert_eq!(b.text(), "one\ntwo");
    }

    #[test]
    fn undo_redo() {
        let mut b = Buffer::new();
        b.insert("hello");
        assert!(b.can_undo());
        b.undo();
        assert_eq!(b.text(), "");
        assert!(b.can_redo());
        b.redo();
        assert_eq!(b.text(), "hello");
    }

    #[test]
    fn replace_byte_range_is_one_undo_and_lands_after_the_new_word() {
        let mut buf = Buffer::from_str("say teh now", None);
        buf.replace_byte_range(4..7, "the");
        assert_eq!(buf.text(), "say the now");
        assert_eq!(buf.cursor, 7);
        buf.undo();
        assert_eq!(buf.text(), "say teh now");
        assert_eq!(buf.cursor, 0);
    }

    #[test]
    fn language_detection() {
        let cases = [
            ("foo.rs", Language::Rust),
            ("bar.py", Language::Python),
            ("types.pyi", Language::Python),
            ("notes.md", Language::Markdown),
            ("App.java", Language::Java),
            ("Main.cs", Language::CSharp),
            ("page.HTML", Language::Html),
            ("site.css", Language::Css),
            ("query.sql", Language::Sql),
            ("index.php", Language::Php),
            ("lib.pl", Language::Perl),
            ("main.go", Language::Go),
            ("app.rb", Language::Ruby),
            ("Gemfile", Language::Ruby),
            ("note.xml", Language::Xml),
            ("icon.svg", Language::Xml),
            ("cfg.ini", Language::Ini),
            ("app.jsonc", Language::Jsonc),
            ("main.zig", Language::Zig),
            ("header.h", Language::C),
            ("widget.m", Language::ObjectiveC),
            ("Makefile", Language::Makefile),
            ("lib.mk", Language::Makefile),
            (".bashrc", Language::Bash),
            ("script.fish", Language::Bash),
            ("notes.rst", Language::RestructuredText),
            ("data.conf", Language::Plain),
            ("suite.t", Language::Plain),
            ("mod.pyx", Language::Plain),
            ("readme", Language::Plain),
        ];
        for (path, lang) in cases {
            assert_eq!(Language::from_path(Path::new(path)), lang, "{path}");
        }
    }

    // Investigation tests for regressions (page, mouse-like set on blank/eof, insert row stability, undo after moves)
    #[test]
    fn page_and_set_visual_on_blank_and_eof() {
        let mut b = Buffer::from_str("line0\nline1\n", None); // 3 lines, last empty
        assert_eq!(b.line_count(), 3);
        // place like mouse on last (blank) line col 0
        b.set_cursor_from_visual(2, 0);
        assert_eq!(b.cursor_position(), (2, 0));
        // insert on blank should keep row
        b.insert_char('X');
        assert_eq!(b.cursor_position().0, 2); // no jump down
        assert!(b.text().contains("X"));

        // eof placement (click "below")
        b.set_cursor_from_visual(99, 0); // >= count should go to end of last
        let (r, c) = b.cursor_position();
        assert_eq!(r, 2);
        assert!(c >= 1); // after the X or at end

        // page down from top with vis=5 should change scroll and caret
        b.scroll_offset = (0, 0);
        b.cursor = 0;
        let _before_scroll = b.scroll_offset.0;
        let _before_row = b.cursor_position().0;
        b.page_down(5, false);
        let (row_after, _) = b.cursor_position();
        let after_scroll = b.scroll_offset.0;
        assert!(
            row_after > 0 || after_scroll > 0,
            "page should advance view or caret"
        );

        // Stronger multi-line case: many lines, small vis, page should move scroll by ~page
        let many = (0..30)
            .map(|i| format!("L{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let mut b2 = Buffer::from_str(&many, None);
        assert_eq!(b2.line_count(), 30);
        b2.scroll_offset = (0, 0);
        b2.cursor = 0;
        b2.page_down(10, false);
        assert!(
            b2.scroll_offset.0 >= 5,
            "page should have scrolled the view by a substantial amount"
        );
    }

    #[test]
    fn undo_after_various_non_edit_nav_and_insert_on_blank() {
        let mut b = Buffer::new();
        b.insert("hello\n\nworld");
        let snap = b.text();
        b.set_cursor_from_visual(1, 0); // blank line
        b.insert_char('X'); // edit on blank
        assert!(b.can_undo());
        b.undo();
        assert_eq!(b.text(), snap);
        assert!(b.can_redo());
    }

    // --- adjust_scroll edge clamping (Fix 1 & 2) ---
    // Cursor movement *inside* the viewport must NOT move the view; scrolling
    // only happens at the true top/bottom edges, using the real viewport height.
    fn make_long_buffer(n: usize) -> Buffer {
        let text = (0..n)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        Buffer::from_str(&text, None)
    }

    /// 80 logical lines, each 40 `x`s. At width 20 / tab 4 each line is 2 screen rows
    /// (no spaces → character-fallback wrap). 160 screen rows total.
    fn make_wrapping_buffer() -> Buffer {
        let text = (0..80)
            .map(|_| "x".repeat(40))
            .collect::<Vec<_>>()
            .join("\n");
        let mut b = Buffer::from_str(&text, None);
        b.visible_lines = 20;
        b.visible_width = 20;
        b.word_wrap = true;
        b.tab_size = 4;
        b
    }

    #[test]
    fn wrapped_in_view_mouse_place_does_not_move_scroll() {
        let mut b = make_wrapping_buffer();
        // Screen rows 0..159. Viewport 20 rows starting at screen row 40
        // (logical lines 20..29, each 2 rows).
        b.scroll_offset = (40, 0);
        let layout = crate::wrap::compute_layout(&b.rope, 0, b.line_count(), 20, 4);
        // Click in the middle of the viewport: screen row 50, col 0.
        let target = crate::wrap::byte_of_screen_row(&layout, &b.rope, 0, 50, 0);
        b.cursor = target;
        let scroll_before = b.scroll_offset.0;

        // Unwrapped ensure is the wrap-off helper; under wrap, mutation-time
        // scroll uses adjust_scroll which must keep screen-row units.
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0, scroll_before,
            "in-view wrap click must not change scroll"
        );
    }

    #[test]
    fn wrapped_ensure_leaves_in_view_cursor_unscrolled() {
        let mut b = make_wrapping_buffer();
        b.scroll_offset = (40, 0);
        let layout = crate::wrap::compute_layout(&b.rope, 0, b.line_count(), 20, 4);
        b.cursor = crate::wrap::byte_of_screen_row(&layout, &b.rope, 0, 50, 0);
        b.ensure_cursor_visible_wrapped(&layout, b.visible_lines);
        assert_eq!(b.scroll_offset.0, 40);
    }

    #[test]
    fn wrapped_insert_inside_viewport_does_not_jump_scroll() {
        let mut b = make_wrapping_buffer();
        b.scroll_offset = (40, 0);
        let layout = crate::wrap::compute_layout(&b.rope, 0, b.line_count(), 20, 4);
        b.cursor = crate::wrap::byte_of_screen_row(&layout, &b.rope, 0, 50, 0);
        let scroll_before = b.scroll_offset.0;
        b.insert_char('Z');
        assert_eq!(
            b.scroll_offset.0, scroll_before,
            "typing inside a wrapped viewport must not jump the view"
        );
    }

    #[test]
    fn wrapped_adjust_scroll_only_at_edges() {
        let mut b = make_wrapping_buffer();
        b.scroll_offset = (40, 0); // view spans screen rows 40..59
        let layout = crate::wrap::compute_layout(&b.rope, 0, b.line_count(), 20, 4);

        b.cursor = crate::wrap::byte_of_screen_row(&layout, &b.rope, 0, 50, 0);
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0, 40,
            "mid-window wrap cursor must not scroll"
        );

        b.cursor = crate::wrap::byte_of_screen_row(&layout, &b.rope, 0, 59, 0);
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0, 40,
            "cursor on last visible wrap row must not scroll"
        );

        b.cursor = crate::wrap::byte_of_screen_row(&layout, &b.rope, 0, 60, 0);
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0,
            60usize.saturating_sub(18),
            "cursor past wrap bottom should recenter to row - (v-2)"
        );

        b.cursor = crate::wrap::byte_of_screen_row(&layout, &b.rope, 0, 5, 0);
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0, 5,
            "cursor above wrap top should snap to that screen row"
        );
    }

    fn layout_width4(b: &Buffer) -> crate::wrap::WrapLayout {
        crate::wrap::compute_layout(&b.rope, 0, b.line_count(), 4, b.tab_size)
    }

    #[test]
    fn move_screen_rows_steps_through_wrapped_line() {
        let mut b = Buffer::from_str("abcdefghij\nzzzz\n", None);
        b.tab_size = 4;
        let layout = layout_width4(&b);
        assert_eq!(layout.breaks[0], vec![4, 8]);

        b.move_screen_rows(&layout, 1, false);
        assert_eq!(b.cursor, 4, "down stays on the continuation");
        b.move_screen_rows(&layout, 1, false);
        assert_eq!(b.cursor, 8);
        b.move_screen_rows(&layout, 1, false);
        assert_eq!(b.cursor, 11, "third down reaches zzzz");
        b.move_screen_rows(&layout, -1, false);
        assert_eq!(b.cursor, 8);
        b.move_screen_rows(&layout, -1, false);
        assert_eq!(b.cursor, 4);
        b.move_screen_rows(&layout, -1, false);
        assert_eq!(b.cursor, 0);
    }

    #[test]
    fn move_screen_rows_keeps_display_column() {
        let mut b = Buffer::from_str("abcdefghij\nzzzz\n", None);
        b.tab_size = 4;
        b.cursor = 2;
        let layout = layout_width4(&b);
        b.move_screen_rows(&layout, 1, false);
        assert_eq!(b.cursor, 6);
    }

    #[test]
    fn move_screen_rows_clamps_short_row_then_keeps_that_column() {
        // width 4: "abcd" / "efgh" / "i", then "abcd" / "efgh" / "ij".
        let mut b = Buffer::from_str("abcdefghi\nabcdefghij\n", None);
        b.tab_size = 4;
        b.cursor = 2;
        let layout = layout_width4(&b);
        b.move_screen_rows(&layout, 1, false);
        assert_eq!(b.cursor, 6);
        b.move_screen_rows(&layout, 1, false);
        // "i" is one column. Column 2 clamps to the newline (byte 9).
        assert_eq!(b.cursor, 9);
        b.move_screen_rows(&layout, 1, false);
        // Next row keeps the clamped column (1), so this is 'b', not 'c'.
        assert_eq!(b.cursor, 11);
    }

    #[test]
    fn move_screen_rows_clamps_onto_a_short_middle_row() {
        // width 5. Rows: "xxxx ", "y ", "zzzzz". From the third x (col 2),
        // Down must land on "y ", not on the z that starts the next row.
        let mut b = Buffer::from_str("xxxx y zzzzz\n", None);
        b.tab_size = 4;
        b.cursor = 2;
        let layout = crate::wrap::compute_layout(&b.rope, 0, b.line_count(), 5, 4);
        b.move_screen_rows(&layout, 1, false);
        assert_eq!(b.cursor, 6);
        assert_eq!(
            crate::wrap::screen_row_of_byte(&layout, &b.rope, 0, b.cursor),
            1
        );
    }

    #[test]
    fn move_screen_rows_at_ends_does_not_touch_selection() {
        let mut b = Buffer::from_str("abcdefghij\nzzzz\n", None);
        b.tab_size = 4;
        b.cursor = 2;
        b.move_cursor(-1, true);
        let cursor = b.cursor;
        let selection = b.selection.clone();
        let anchor = b.selection_anchor;
        let layout = layout_width4(&b);
        b.move_screen_rows(&layout, -1, false);
        assert_eq!(b.cursor, cursor);
        assert_eq!(b.selection, selection);
        assert_eq!(b.selection_anchor, anchor);

        // A trailing newline is its own empty screen row. That row is last.
        b.clear_selection();
        let last = layout.total_screen_rows - 1;
        b.cursor = crate::wrap::byte_of_screen_row(&layout, &b.rope, 0, last, 0);
        b.move_cursor(0, true);
        let cursor = b.cursor;
        let selection = b.selection.clone();
        let anchor = b.selection_anchor;
        b.move_screen_rows(&layout, 1, false);
        assert_eq!(b.cursor, cursor);
        assert_eq!(b.selection, selection);
        assert_eq!(b.selection_anchor, anchor);
    }

    #[test]
    fn move_screen_rows_shift_down_extends_selection() {
        let mut b = Buffer::from_str("abcdefghij\n", None);
        b.tab_size = 4;
        b.cursor = 1;
        let layout = layout_width4(&b);
        b.move_screen_rows(&layout, 1, true);
        assert_eq!(b.cursor, 5);
        assert_eq!(b.selection.clone(), Some(1..5));
        assert_eq!(b.selection_anchor, Some(1));
    }

    #[test]
    fn page_wrapped_lands_on_the_screen_row_a_page_down() {
        // 80 lines of 40 x's. Width 20 → two screen rows per line.
        let text = (0..80)
            .map(|_| "x".repeat(40))
            .collect::<Vec<_>>()
            .join("\n");
        let mut b = Buffer::from_str(&text, None);
        b.visible_lines = 20;
        b.visible_width = 20;
        b.word_wrap = true;
        b.tab_size = 4;
        b.cursor = 5;
        b.scroll_offset = (0, 0);
        let layout = crate::wrap::compute_layout(&b.rope, 0, b.line_count(), 20, 4);
        b.page_wrapped(&layout, true, 20, false);
        // page = 19, rel = 0, place = screen row 19, column 5.
        // Line 9's second row starts at byte 9*41 + 20.
        let row = crate::wrap::screen_row_of_byte(&layout, &b.rope, 0, b.cursor);
        assert_eq!(row, 19);
        assert_eq!(b.cursor, 9 * 41 + 20 + 5);
        assert_eq!(b.scroll_offset.0, 19);
        assert!(b.selection.is_none());

        b.page_wrapped(&layout, false, 20, false);
        let row = crate::wrap::screen_row_of_byte(&layout, &b.rope, 0, b.cursor);
        assert_eq!(row, 0);
        assert_eq!(b.cursor, 5);
        assert_eq!(b.scroll_offset.0, 0);
    }

    #[test]
    fn adjust_scroll_only_at_edges() {
        let mut b = make_long_buffer(200);
        b.visible_lines = 20; // viewport rows 0..19
        b.scroll_offset = (40, 0); // view spans rows 40..59

        // Cursor in the middle of the viewport → no scroll.
        b.cursor = b.rope.line_to_byte(50); // row 50
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0, 40,
            "mid-window cursor must not scroll the view"
        );

        // Cursor on the last visible row (row 59 = 40+20-1) → still in view, no scroll.
        b.cursor = b.rope.line_to_byte(59);
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0, 40,
            "cursor on bottom edge row must not scroll"
        );

        // Cursor one row past the bottom edge → recenter so cursor sits near bottom.
        b.cursor = b.rope.line_to_byte(60);
        b.adjust_scroll();
        assert_eq!(
            b.scroll_offset.0,
            60usize.saturating_sub(18),
            "cursor past bottom should recenter to row - (v-2)"
        );

        // Cursor above the top edge → bring to top.
        b.cursor = b.rope.line_to_byte(5);
        b.adjust_scroll();
        assert_eq!(b.scroll_offset.0, 5, "cursor above top should snap to top");
    }

    #[test]
    fn insert_on_blank_line_when_scrolled_keeps_view_and_row() {
        // Reproduces Bug 2: typing on a blank line in a scrolled long file must
        // land on that line, without the viewport (and thus perceived cursor)
        // jumping. The buffer content was always correct; the bug was the view.
        let mut b = Buffer::from_str("a\n\nb\n\nc\n\nd\n\ne", None);
        b.visible_lines = 3; // tiny viewport to force scrolling quickly
        b.scroll_offset = (6, 0);
        // Place cursor like a mouse click on the blank line at row 7 (which is row 6+1 in view).
        b.set_cursor_from_visual(7, 0);
        assert_eq!(b.cursor_position(), (7, 0));
        let scroll_before = b.scroll_offset.0;
        b.insert_char('X');
        assert_eq!(
            b.cursor_position().0,
            7,
            "row must stay 7 after typing on blank line"
        );
        assert_eq!(
            b.scroll_offset.0, scroll_before,
            "view must not jump when typing inside the viewport"
        );
        assert!(b.text().contains("X"), "character was actually inserted");
    }

    // --- generation invalidation key (Fix 3c) ---
    #[test]
    fn generation_bumps_on_edit_not_on_nav() {
        let mut b = Buffer::from_str("hello", None);
        let g0 = b.generation;

        // Navigation must NOT bump generation (no spurious cache invalidation).
        b.move_cursor(1, false);
        b.move_home(false);
        b.move_end(false);
        assert_eq!(b.generation, g0, "navigation must not bump generation");

        // Every text mutation bumps exactly once.
        b.insert_char('!');
        assert_eq!(b.generation, g0 + 1, "insert bumps generation");
        b.backspace();
        assert_eq!(b.generation, g0 + 2, "backspace bumps generation");
        b.newline_with_indent(4);
        assert_eq!(b.generation, g0 + 3, "newline bumps generation");
        b.indent(4);
        assert_eq!(b.generation, g0 + 4, "indent bumps generation");
        b.dedent(4);
        assert_eq!(b.generation, g0 + 5, "dedent bumps generation");

        // delete bumps only when it actually removes something.
        b.cursor = 0;
        let before = b.generation;
        b.delete();
        assert_eq!(b.generation, before + 1, "delete bumps generation");

        // undo/redo are mutations of content → bump.
        let before = b.generation;
        b.undo();
        assert_eq!(b.generation, before + 1, "undo bumps generation");
        let before = b.generation;
        b.redo();
        assert_eq!(b.generation, before + 1, "redo bumps generation");
    }

    // --- bidirectional selection (Fix: anchor drift) ---
    #[test]
    fn extend_selection_leftward_grows() {
        // Cursor at byte 5 (after "hello"). Shift+Left twice must select "llo"
        // then "llo"-+prev → "llo" grows left, NOT collapse to 1 char.
        let mut b = Buffer::from_str("hello world", None);
        b.cursor = 5; // h-e-l-l-o-| space...
        b.move_cursor(-1, true);
        assert_eq!(
            b.selection.as_ref().unwrap().clone(),
            4..5,
            "first shift-left selects one char to the left"
        );
        b.move_cursor(-1, true);
        assert_eq!(
            b.selection.as_ref().unwrap().clone(),
            3..5,
            "second shift-left must grow leftward (anchor held at 5)"
        );
        b.move_cursor(-1, true);
        assert_eq!(
            b.selection.as_ref().unwrap().clone(),
            2..5,
            "third shift-left keeps growing toward anchor"
        );
    }

    #[test]
    fn extend_right_then_left_shrinks_from_anchor() {
        let mut b = Buffer::from_str("abcdefgh", None);
        b.cursor = 3;
        // Extend right to 6: selection 3..6
        b.move_cursor(1, true);
        b.move_cursor(1, true);
        b.move_cursor(1, true);
        assert_eq!(b.selection.as_ref().unwrap().clone(), 3..6);
        // Now move back left: selection shrinks back toward anchor (3), not collapse.
        b.move_cursor(-1, true);
        assert_eq!(b.selection.as_ref().unwrap().clone(), 3..5);
        b.move_cursor(-1, true);
        assert_eq!(b.selection.as_ref().unwrap().clone(), 3..4);
        // Cursor back at the anchor → selection is empty (cursor == anchor).
        b.move_cursor(-1, true);
        assert_eq!(
            b.selection.as_ref().unwrap().clone(),
            3..3,
            "cursor at anchor yields an empty selection"
        );
        // One more left: cross the anchor → leftward extension.
        b.move_cursor(-1, true);
        assert_eq!(
            b.selection.as_ref().unwrap().clone(),
            2..3,
            "crossing anchor flips to leftward extension"
        );
    }

    #[test]
    fn non_extend_move_clears_anchor() {
        let mut b = Buffer::from_str("hello world", None);
        b.cursor = 5;
        b.move_cursor(-1, true); // selection 4..5, anchor 5
        assert!(b.selection_anchor.is_some());
        b.move_cursor(-1, false); // plain move clears everything
        assert!(b.selection.is_none());
        assert!(b.selection_anchor.is_none());
    }

    #[test]
    fn mouse_extend_both_directions() {
        let mut b = Buffer::from_str("hello world", None);
        b.cursor = 5;
        // Simulate drag to byte 8 (right of cursor)
        b.mouse_extend_to(8);
        assert_eq!(b.selection.as_ref().unwrap().clone(), 5..8);
        assert_eq!(b.cursor, 8);
        // Drag back past the anchor to byte 2 (left of anchor 5)
        b.mouse_extend_to(2);
        assert_eq!(b.selection.as_ref().unwrap().clone(), 2..5);
        assert_eq!(b.cursor, 2);
    }

    // --- horizontal auto-scroll (Fix: no way to scroll to off-right text) ---
    #[test]
    fn hscroll_follows_cursor_right_and_left() {
        // A long line; narrow viewport (width 10). Moving the cursor past the
        // right edge must shift the horizontal offset; moving it back must relax it.
        let mut b = Buffer::from_str(&"x".repeat(80), None);
        b.visible_width = 10;
        b.scroll_offset = (0, 0);
        // Move cursor to col 25 (well past width 10).
        b.cursor = 25;
        b.ensure_cursor_horizontally_visible();
        // Cursor should now be inside [hscroll, hscroll+10).
        let h = b.scroll_offset.1;
        assert!(
            h <= 25 && 25 < h + 10,
            "cursor col 25 must be inside window, got hscroll={h}"
        );
        // Move cursor back to col 0 → window snaps to 0.
        b.cursor = 0;
        b.ensure_cursor_horizontally_visible();
        assert_eq!(b.scroll_offset.1, 0, "cursor at col 0 snaps hscroll to 0");
    }

    #[test]
    fn max_line_width_ignores_newlines_and_picks_longest() {
        let b = Buffer::from_str("ab\nlonger-line\nxy\n", None);
        assert_eq!(b.compute_max_line_width(), 11); // "longer-line"
    }

    #[test]
    fn max_line_width_empty_is_zero() {
        let b = Buffer::new();
        assert_eq!(b.compute_max_line_width(), 0);
    }

    #[test]
    fn clamp_hscroll_leaves_caret_column() {
        let mut b = Buffer::from_str(&"x".repeat(80), None);
        b.visible_width = 10;
        b.refresh_max_line_width();
        b.scroll_offset.1 = 999;
        b.clamp_hscroll();
        // Extra column so an EOL caret (col == 80) stays in the text area.
        assert_eq!(b.scroll_offset.1, 71); // 80 - (10 - 1)
    }

    #[test]
    fn scroll_horizontal_steps_and_clamps() {
        let mut b = Buffer::from_str(&"x".repeat(20), None);
        b.visible_width = 10;
        b.refresh_max_line_width();
        b.scroll_horizontal(4);
        assert_eq!(b.scroll_offset.1, 4);
        b.scroll_horizontal(100);
        assert_eq!(b.scroll_offset.1, 11); // 20 - (10 - 1)
        b.scroll_horizontal(-100);
        assert_eq!(b.scroll_offset.1, 0);
    }

    #[test]
    fn end_on_longest_line_stays_in_text_viewport_after_clamp() {
        let mut b = Buffer::from_str(&"x".repeat(80), None);
        b.visible_width = 10;
        b.refresh_max_line_width();
        b.move_end(false);
        b.ensure_cursor_horizontally_visible();
        b.clamp_hscroll();
        let col = b.cursor_position().1;
        let h = b.scroll_offset.1;
        let w = b.visible_width;
        assert_eq!(col, 80, "End on an 80-char line sits at column 80");
        assert!(
            col >= h && col < h + w,
            "EOL caret col {col} must stay in [{h}, {h}+{w}), got hscroll={h}"
        );
    }

    #[test]
    fn replace_document_is_undoable_and_keeps_id() {
        let mut b = Buffer::from_str("hello", None);
        let id = b.id;
        b.cursor = 2; // on 'l'
        b.replace_document("hello world\n");
        assert_eq!(b.text(), "hello world\n");
        assert_eq!(b.id, id, "highlight-cache identity must not change");
        assert!(b.is_modified);
        assert!(b.can_undo());
        b.undo();
        assert_eq!(b.text(), "hello");
        assert_eq!(b.cursor, 2);
    }

    #[test]
    fn replace_document_maps_cursor_by_line_col() {
        let mut b = Buffer::from_str("abc\ndef\n", None);
        b.cursor = b.byte_at_visual(1, 2); // col 2 is 'f' (d=0,e=1,f=2)
        let (line, col) = b.cursor_position();
        assert_eq!((line, col), (1, 2));
        b.replace_document("ABC\nDE\n");
        let (nl, nc) = b.cursor_position();
        assert_eq!(nl, 1);
        assert_eq!(
            nc,
            2.min(b.line(1).trim_end_matches(['\n', '\r']).chars().count())
        );
    }

    #[test]
    fn replace_document_noop_when_identical() {
        let mut b = Buffer::from_str("same\n", None);
        b.replace_document("same\n");
        assert!(!b.can_undo());
        assert!(!b.is_modified);
    }

    #[test]
    fn replace_document_clears_selection() {
        let mut b = Buffer::from_str("abcdef", None);
        b.selection = Some(1..3);
        b.selection_anchor = Some(1);
        b.replace_document("abcdef\n");
        assert!(b.selection.is_none());
        assert!(b.selection_anchor.is_none());
    }

    #[test]
    fn from_str_with_path_captures_disk_stamp() {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-buf-stamp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, "hi\n").unwrap();
        let b = Buffer::from_str("hi\n", Some(p.clone()));
        assert_eq!(b.disk_stamp, crate::file_watch::FileStamp::of(&p));
        assert!(!b.auto_reload);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn from_str_without_path_has_no_stamp() {
        let b = Buffer::from_str("x", None);
        assert!(b.disk_stamp.is_none());
    }

    #[test]
    fn reload_from_replaces_text_clears_modified_keeps_id() {
        let mut b = Buffer::from_str("old\n", None);
        let id = b.id;
        b.cursor = b.rope.len_bytes();
        b.insert("!");
        assert!(b.is_modified);
        b.reload_from("new\n");
        assert_eq!(b.text(), "new\n");
        assert!(!b.is_modified);
        assert_eq!(b.id, id);
        b.undo();
        assert_eq!(b.text(), "old\n!");
    }
}
