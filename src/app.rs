// Top-level App state, event loop, high-level commands (to be implemented in Sprint 1)
// See Design Spec §6 data model sketch and AGENTS.md recommended structure.

#![allow(dead_code)]

use crate::buffer::{Buffer, Language};
use crate::config::Config;
use crate::syntax::SyntaxHighlighter;
use crate::theme::Theme;
use crate::ui::find::{FindFocus, FindState};
use crate::utils::clipboard::ClipboardWrapper;
use ratatui::text::Span;
use std::path::PathBuf;

/// Cached syntax-highlight spans for the currently visible editor window.
///
/// Recomputed only when one of its keys changes — the buffer's `generation`
/// (bumped on every edit/undo/redo), the active `language` (tab switch or
/// re-detect), or the `start_line` of the visible region (scroll). Otherwise the
/// spans are reused frame-to-frame, so rendering no longer re-highlights every
/// visible line on every keystroke/scroll (AGENTS §6 "Syntax Highlighting").
#[derive(Clone)]
pub struct CachedHighlights {
    pub language: Language,
    pub generation: u64,
    pub start_line: usize,
    pub count: usize,
    pub spans: Vec<Vec<Span<'static>>>,
}

#[derive(Default)] // Debug removed because SyntaxHighlighter doesn't impl it
pub struct App {
    pub buffers: Vec<Buffer>,
    pub active: usize,
    pub config: Config,
    pub theme: Theme,
    pub highlighter: SyntaxHighlighter,
    pub show_help: bool,
    pub clipboard: ClipboardWrapper,
    pub find_state: Option<FindState>,
    pub show_word_count: bool,
    pub word_count_text: String,
    /// Active tab in the help popup.
    pub help_tab: crate::ui::popups::help::HelpTab,
    /// Scroll offset (row, col) for the help popup content. Used to scroll the
    /// shortcuts page when content exceeds the popup's inner height.
    pub help_content_scroll: (u16, u16),
    /// Currently open menu for dropdown ("File", "Tools", "Settings")
    pub active_menu: Option<String>,
    pub show_settings: bool,
    pub settings_selected: usize,
    /// Selected index in currently open dropdown menu (if any)
    pub menu_selected: usize,
    // Popup states for Sprint 2
    pub show_confirm: bool,
    pub confirm_message: String,
    pub confirm_action: Option<String>, // "close_tab", "quit" etc
    pub show_file_dialog: bool,
    pub file_dialog_is_save: bool,
    pub file_path_input: String,
    pub file_dialog_message: String,
    /// Keyboard selection state for buttons
    pub file_dialog_selected: usize,  // 0 = Save/Open, 1 = Cancel
    pub confirm_selected: usize,     // 0 = Confirm, 1 = Cancel
    /// Button hover states (per popup)
    pub file_dialog_button_hover: Option<usize>,  // 0 = Save/Open, 1 = Cancel
    pub confirm_button_hover: Option<usize>,       // 0 = Confirm, 1 = Cancel
    pub settings_theme_hover: Option<usize>,       // Which theme row hovered
    /// Mouse position tracking
    pub last_mouse_pos: Option<(u16, u16)>,
    /// Set to true to exit the main loop cleanly (used by menu Quit etc.)
    pub should_quit: bool,
    /// Last known height (in lines) of the editor viewport area. Used for PgUp/PgDn and better scrolling.
    pub editor_visible_lines: std::cell::Cell<usize>,
    /// Last known width (in columns) of the editor text area (excludes gutter +
    /// scrollbar). Used by horizontal auto-scroll to follow the cursor.
    pub editor_visible_width: std::cell::Cell<usize>,
    /// Cached syntax spans for the active buffer's visible window. Refreshed each
    /// frame by `refresh_highlight_cache` only when content/scroll/language changed.
    pub cached_highlights: Option<CachedHighlights>,
}

impl App {
    pub fn new(initial_files: &[String]) -> Self {
        let config = Config::load();
        let theme = match config.theme.as_str() {
            "light" => Theme::light(),
            "high-contrast" | "high_contrast" => Theme::high_contrast(),
            _ => Theme::dark_red(),
        };
        let highlighter = SyntaxHighlighter::new();

        let mut buffers = Vec::new();
        if initial_files.is_empty() {
            buffers.push(Buffer::new());
        } else {
            for f in initial_files {
                let path = PathBuf::from(f);
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    buffers.push(Buffer::from_str(&contents, Some(path)));
                } else {
                    let mut b = Buffer::new();
                    b.set_path(path);
                    buffers.push(b);
                }
            }
        }

        Self {
            buffers,
            active: 0,
            config,
            theme,
            highlighter,
            show_help: false,
            clipboard: ClipboardWrapper::new(),
            find_state: None,
            show_word_count: false,
            word_count_text: String::new(),
            help_tab: crate::ui::popups::help::HelpTab::default(),
            help_content_scroll: (0, 0),
            active_menu: None,
            show_settings: false,
            settings_selected: 0,
            menu_selected: 0,
            show_confirm: false,
            confirm_message: String::new(),
            confirm_action: None,
            show_file_dialog: false,
            file_dialog_is_save: false,
            file_path_input: String::new(),
            file_dialog_message: String::new(),
            file_dialog_selected: 0,
            confirm_selected: 0,
            file_dialog_button_hover: None,
            confirm_button_hover: None,
            settings_theme_hover: None,
            last_mouse_pos: None,
            should_quit: false,
            editor_visible_lines: std::cell::Cell::new(20),
            editor_visible_width: std::cell::Cell::new(80),
            cached_highlights: None,
        }
    }

    pub fn active_buffer(&self) -> &Buffer {
        &self.buffers[self.active]
    }

    pub fn active_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active]
    }

    /// Ensure `cached_highlights` matches the active buffer's visible window.
    ///
    /// Recomputes (and rebuilds one shared `HighlightLines` for the whole window)
    /// only when the cache key is stale: the buffer's text `generation`, its
    /// `language`, or the visible region's `start_line`/`count`. On a steady
    /// render (no edit, no scroll, no resize) this is a cheap key comparison and
    /// the editor reuses the existing spans — eliminating the per-frame re-highlight
    /// that caused lag on large files.
    pub fn refresh_highlight_cache(&mut self, start_line: usize, count: usize) {
        let buf = self.active_buffer();
        let language = buf.language;
        let generation = buf.generation;

        let needs_recompute = match &self.cached_highlights {
            None => true,
            Some(c) => {
                c.language != language
                    || c.generation != generation
                    || c.start_line != start_line
                    || c.count != count
            }
        };

        if needs_recompute {
            // Collect the visible lines (clamped to the buffer) without joining them
            // into a single String — highlight_window feeds them one at a time to a
            // single HighlightLines, preserving multi-line syntax state.
            let n_lines = buf.line_count();
            let end = (start_line + count).min(n_lines);
            let lines: Vec<String> = if start_line < n_lines {
                (start_line..end).map(|i| buf.line(i)).collect()
            } else {
                Vec::new()
            };
            let spans = self
                .highlighter
                .highlight_window(lines.iter().map(String::as_str), language);

            self.cached_highlights = Some(CachedHighlights {
                language,
                generation,
                start_line,
                count,
                spans,
            });
        }
    }

    /// Read-only access to the cached visible-window spans (if any).
    pub fn cached_visible_spans(&self) -> Option<&CachedHighlights> {
        self.cached_highlights.as_ref()
    }

    pub fn current_theme(&self) -> &Theme {
        &self.theme
    }

    /// Switch tab (wrap around)
    pub fn switch_tab(&mut self, delta: isize) {
        if self.buffers.is_empty() {
            return;
        }
        let len = self.buffers.len() as isize;
        let new = (self.active as isize + delta).rem_euclid(len) as usize;
        self.active = new;
    }

    pub fn close_current_tab(&mut self) {
        if self.active_buffer().is_modified {
            self.show_confirm_dialog("Close tab with unsaved changes?", "close_tab");
            return;
        }
        if self.buffers.len() <= 1 {
            self.buffers[0] = Buffer::new();
            self.active = 0;
            return;
        }
        self.buffers.remove(self.active);
        if self.active >= self.buffers.len() {
            self.active = self.buffers.len() - 1;
        }
    }

    pub fn new_tab(&mut self) {
        self.buffers.push(Buffer::new());
        self.active = self.buffers.len() - 1;
    }

    /// Basic save (overwrite). Returns success.
    pub fn save_current(&mut self) -> bool {
        let buf = self.active_buffer_mut();
        if let Some(ref path) = buf.path.clone() {
            if std::fs::write(path, buf.text()).is_ok() {
                buf.is_modified = false;
                return true;
            }
        }
        false
    }

    pub fn copy_selection_or_line(&mut self) -> bool {
        let buf = self.active_buffer();
        let text = if let Some(sel) = &buf.selection {
            let start = sel.start.min(buf.rope.len_bytes());
            let end = sel.end.min(buf.rope.len_bytes());
            buf.rope.slice(start..end).to_string()
        } else {
            // copy current line as convenience
            let (row, _) = buf.cursor_position();
            buf.line(row)
        };
        self.clipboard.copy(&text)
    }

    pub fn paste(&mut self) {
        if let Some(text) = self.clipboard.paste() {
            self.active_buffer_mut().insert(&text);
        }
    }

    // --- Find & Replace (Sprint 2) ---

    pub fn toggle_find(&mut self) {
        if self.find_state.is_some() {
            self.find_state = None;
        } else {
            let mut fs = FindState::new();
            fs.focus = FindFocus::Query;
            // seed with current selection or word if any (basic)
            if let Some(sel) = &self.active_buffer().selection {
                let start = sel.start.min(self.active_buffer().rope.len_bytes());
                let end = sel.end.min(self.active_buffer().rope.len_bytes());
                fs.query = self.active_buffer().rope.slice(start..end).to_string();
            }
            self.find_state = Some(fs);
            self.update_find_matches();
        }
    }

    pub fn update_find_input(&mut self, append: Option<char>, backspace: bool) {
        if let Some(fs) = &mut self.find_state {
            let editing_replace = fs.focus == FindFocus::Replace;
            let target = if editing_replace {
                &mut fs.replace
            } else {
                &mut fs.query
            };
            if backspace {
                target.pop();
            } else if let Some(ch) = append {
                target.push(ch);
            }
            if !editing_replace {
                // Only re-search and reset current when the query itself changes.
                // Typing in replace box should not reset navigation position.
                self.update_find_matches();
            }
        }
    }

    pub fn update_find_matches(&mut self) {
        let text = if let Some(_fs) = &self.find_state {
            self.active_buffer().text()
        } else {
            return;
        };
        if let Some(fs) = &mut self.find_state {
            fs.matches = find_matches_in_text(
                &text,
                &fs.query,
                fs.case_sensitive,
                fs.use_regex,
                fs.whole_word,
            );
            fs.current_match = 0;
        }
    }

    pub fn find_next(&mut self) {
        let (has_matches, new_idx) = if let Some(fs) = &mut self.find_state {
            if fs.matches.is_empty() {
                (false, 0)
            } else {
                fs.current_match = (fs.current_match + 1) % fs.matches.len();
                (true, fs.current_match)
            }
        } else {
            (false, 0)
        };
        if has_matches {
            self.jump_to_match_index(new_idx);
        }
    }

    pub fn find_prev(&mut self) {
        let (has_matches, new_idx) = if let Some(fs) = &mut self.find_state {
            if fs.matches.is_empty() {
                (false, 0)
            } else {
                fs.current_match = if fs.current_match == 0 {
                    fs.matches.len() - 1
                } else {
                    fs.current_match - 1
                };
                (true, fs.current_match)
            }
        } else {
            (false, 0)
        };
        if has_matches {
            self.jump_to_match_index(new_idx);
        }
    }

    fn jump_to_match_index(&mut self, idx: usize) {
        let mrange = if let Some(fs) = &self.find_state {
            if let Some(r) = fs.matches.get(idx) {
                r.clone()
            } else {
                return;
            }
        } else {
            return;
        };
        let vis = self.editor_visible_lines.get().max(5);
        let b = self.active_buffer_mut();
        let pos = mrange.start;
        let end = mrange.end.min(b.rope.len_bytes());
        b.cursor = pos;
        b.selection = Some(pos..end);
        // Use minimal ensure (from slice 1) so nav doesn't jump view unnecessarily.
        b.ensure_cursor_visible(vis);
        // If still far (e.g. initial), modest recenter with real vis.
        let (row, _) = b.cursor_position();
        if row < b.scroll_offset.0 || row >= b.scroll_offset.0 + vis {
            b.scroll_offset.0 = row.saturating_sub(vis / 2);
        }
        // Bring the match column into view horizontally (matches off the right
        // edge used to land the cursor invisibly).
        b.ensure_cursor_horizontally_visible();
    }

    pub fn replace_current(&mut self) {
        let repl = match &self.find_state {
            Some(fs) => fs.replace.clone(),
            None => return,
        };

        // Determine target range from current selection (preferred) or matches list.
        // Read using shared borrows first.
        let active_sel = self.active_buffer().selection.clone();
        let match_ranges = self.find_state.as_ref().map(|fs| fs.matches.clone());
        let current_idx = self
            .find_state
            .as_ref()
            .map(|fs| fs.current_match)
            .unwrap_or(0);

        let (pos, end) = if let Some(sel) = &active_sel {
            let s = sel.start.min(self.active_buffer().rope.len_bytes());
            let e = sel.end.min(self.active_buffer().rope.len_bytes());
            if e > s {
                (s, e)
            } else if let Some(ranges) = &match_ranges {
                if let Some(r) = ranges.get(current_idx) {
                    let p = r.start.min(self.active_buffer().rope.len_bytes());
                    let ee = r.end.min(self.active_buffer().rope.len_bytes());
                    (p, ee)
                } else {
                    return;
                }
            } else {
                return;
            }
        } else if let Some(ranges) = &match_ranges {
            if let Some(r) = ranges.get(current_idx) {
                let p = r.start.min(self.active_buffer().rope.len_bytes());
                let ee = r.end.min(self.active_buffer().rope.len_bytes());
                (p, ee)
            } else {
                return;
            }
        } else {
            return;
        };

        let b = self.active_buffer_mut();
        if end > pos {
            b.selection = Some(pos..end);
        } else {
            b.selection = None;
        }
        b.insert(&repl); // uses push_undo + selection-replace (or insert) + adjust + modified
        self.update_find_matches();
        // Advance to next match after replace for convenience
        self.find_next();
    }

    pub fn replace_all(&mut self) {
        let fs = match &self.find_state {
            Some(fs) if !fs.query.is_empty() => fs.clone(),
            _ => return,
        };
        let buf = self.active_buffer_mut();
        let text = buf.text();
        let path = buf.path.clone();

        let new_text = if fs.use_regex {
            if let Ok(re) = regex::Regex::new(&fs.query) {
                re.replace_all(&text, fs.replace.as_str()).to_string()
            } else {
                text.clone()
            }
        } else {
            // Use precomputed matches (respecting case/whole_word) and replace backwards
            let mut matches = fs.matches.clone();
            if matches.is_empty() {
                matches =
                    find_matches_in_text(&text, &fs.query, fs.case_sensitive, false, fs.whole_word);
            }
            let repl = fs.replace.clone();
            let mut rope = ropey::Rope::from_str(&text);
            for r in matches.iter().rev() {
                let p = r.start;
                let e = r.end;
                if e <= rope.len_bytes() && e > p {
                    rope.remove(p..e);
                    rope.insert(p, &repl);
                }
            }
            rope.to_string()
        };

        if new_text != text {
            buf.push_undo();
            buf.rope = ropey::Rope::from_str(&new_text);
            buf.path = path;
            buf.cursor = buf.cursor.min(buf.rope.len_bytes());
            buf.selection = None;
            buf.is_modified = true;
        }
        self.update_find_matches();
    }

    pub fn clear_find(&mut self) {
        self.find_state = None;
    }

    pub fn is_find_active(&self) -> bool {
        self.find_state.is_some()
    }

    pub fn compute_and_show_word_count(&mut self) {
        let buf = self.active_buffer();
        let text = buf.text();
        let words = text.split_whitespace().count();
        let chars = text.chars().count();
        let lines = buf.line_count();
        let mut info = format!("Words: {}\nCharacters: {}\nLines: {}", words, chars, lines);
        if let Some(sel) = &buf.selection {
            let s = sel.start.min(buf.rope.len_bytes());
            let e = sel.end.min(buf.rope.len_bytes());
            let sub = buf.rope.slice(s..e).to_string();
            let sw = sub.split_whitespace().count();
            let sc = sub.chars().count();
            info.push_str(&format!(
                "\n\nSelection:\n  Words: {}\n  Characters: {}",
                sw, sc
            ));
        }
        self.word_count_text = info;
        self.show_word_count = true;
    }

    pub fn close_word_count(&mut self) {
        self.show_word_count = false;
    }

    pub fn open_menu(&mut self, name: &str) {
        if self.active_menu.as_deref() == Some(name) {
            self.active_menu = None;
            self.menu_selected = 0;
        } else {
            self.active_menu = Some(name.to_string());
            self.menu_selected = 0;
        }
    }

    pub fn close_menu(&mut self) {
        self.active_menu = None;
        self.menu_selected = 0;
    }

    pub fn open_settings(&mut self) {
        self.show_settings = true;
        self.settings_selected = 0;
        self.close_menu();
    }

    pub fn close_settings(&mut self) {
        self.show_settings = false;
    }

    /// Cycle or set theme live from settings
    pub fn apply_settings_theme(&mut self, idx: usize) {
        let themes = crate::theme::Theme::built_ins();
        if idx < themes.len() {
            self.theme = themes[idx].clone();
            // update config for persist
            self.config.theme = self.theme.name.clone();
            self.config.save();
        }
    }

    /// Format the current document using external formatter if available.
    /// Returns true on success.
    pub fn format_current_document(&mut self) -> bool {
        let buf = self.active_buffer_mut();
        let lang = buf.language;
        let cmd = match lang {
            crate::buffer::Language::Rust => "rustfmt",
            crate::buffer::Language::Python => "black",
            crate::buffer::Language::Javascript | crate::buffer::Language::Typescript => "prettier",
            crate::buffer::Language::Json
            | crate::buffer::Language::Toml
            | crate::buffer::Language::Yaml => "prettier",
            _ => return false,
        };

        let input = buf.text();
        let output = std::process::Command::new(cmd)
            .arg("-") // stdin
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(input.as_bytes());
                }
                child.wait_with_output()
            });

        match output {
            Ok(out) if out.status.success() => {
                let formatted = String::from_utf8_lossy(&out.stdout).to_string();
                if !formatted.trim().is_empty() && formatted != input {
                    // replace content (simple, could be smarter with rope)
                    *buf = crate::buffer::Buffer::from_str(&formatted, buf.path.clone());
                    buf.is_modified = true;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    // --- Sprint 2 popup helpers ---

    pub fn show_confirm_dialog(&mut self, message: &str, action: &str) {
        self.confirm_selected = 0;
        self.confirm_message = message.to_string();
        self.confirm_action = Some(action.to_string());
        self.show_confirm = true;
    }

    pub fn close_confirm(&mut self, confirmed: bool) {
        if confirmed {
            if let Some(ref action) = self.confirm_action {
                match action.as_str() {
                    "close_tab" => {
                        if self.buffers.len() > 1 {
                            self.buffers.remove(self.active);
                            if self.active >= self.buffers.len() {
                                self.active = self.buffers.len() - 1;
                            }
                        } else {
                            self.buffers[0] = Buffer::new();
                            self.active = 0;
                        }
                    }
                    "quit" => {
                        // handled in main event loop
                    }
                    _ => {}
                }
            }
        }
        self.show_confirm = false;
        self.confirm_message.clear();
        self.confirm_action = None;
    }

    pub fn show_file_dialog(&mut self, is_save: bool) {
        self.file_dialog_selected = 0;
        self.show_file_dialog = true;
        self.file_dialog_is_save = is_save;
        self.file_path_input.clear();
        if let Some(ref path) = self.active_buffer().path {
            self.file_path_input = path.to_string_lossy().to_string();
        }
        self.file_dialog_message = if is_save {
            "Enter path to save as".to_string()
        } else {
            "Enter path to open".to_string()
        };
    }

    pub fn close_file_dialog(&mut self, confirmed: bool) {
        if confirmed {
            let path_str = self.file_path_input.trim();
            if !path_str.is_empty() {
                let path = PathBuf::from(path_str);
                if self.file_dialog_is_save {
                    if let Err(e) = std::fs::write(&path, self.active_buffer().text()) {
                        self.file_dialog_message = format!("Save error: {}", e);
                        return;
                    }
                    let buf = self.active_buffer_mut();
                    buf.set_path(path);
                    buf.is_modified = false;
                } else {
                    match std::fs::read_to_string(&path) {
                        Ok(contents) => {
                            let new_buf = Buffer::from_str(&contents, Some(path));
                            self.buffers.push(new_buf);
                            self.active = self.buffers.len() - 1;
                        }
                        Err(e) => {
                            self.file_dialog_message = format!("Open error: {}", e);
                            return;
                        }
                    }
                }
            }
        }
        self.show_file_dialog = false;
        self.file_path_input.clear();
        self.file_dialog_message.clear();
    }

    pub fn file_dialog_input_char(&mut self, ch: char) {
        if self.show_file_dialog {
            self.file_path_input.push(ch);
        }
    }

    pub fn file_dialog_backspace(&mut self) {
        if self.show_file_dialog {
            self.file_path_input.pop();
        }
    }

    pub fn wrap_selection_or_cursor(&mut self, before: &str, after: &str) {
        let buf = self.active_buffer_mut();
        buf.push_undo(); // make MD formatting undoable (was direct rope bypass)
        if let Some(sel) = &buf.selection {
            let start = sel.start.min(buf.rope.len_bytes());
            let end = sel.end.min(buf.rope.len_bytes());
            let selected = buf.rope.slice(start..end).to_string();
            let wrapped = format!("{}{}{}", before, selected, after);
            buf.rope.remove(start..end);
            buf.rope.insert(start, &wrapped);
            buf.cursor = start + wrapped.len();
            buf.selection = Some(start..start + wrapped.len());
            buf.is_modified = true;
        } else {
            buf.rope.insert(buf.cursor, before);
            buf.rope.insert(buf.cursor + before.len(), after);
            buf.cursor += before.len();
            buf.is_modified = true;
        }
    }

    /// Shared logic for Quit (menu or Ctrl+Q): show confirm if unsaved, else set flag.
    pub fn request_quit(&mut self) {
        let has_unsaved = self.buffers.iter().any(|b| b.is_modified);
        if has_unsaved {
            self.show_confirm_dialog("Quit with unsaved changes?", "quit");
        } else {
            self.should_quit = true;
        }
    }

    /// Undo last edit (wired to Ctrl+Z / Ctrl+Shift+Z). Also refresh find matches if active.
    pub fn undo(&mut self) {
        self.active_buffer_mut().undo();
        if self.is_find_active() {
            self.update_find_matches();
        }
    }

    /// Redo last undone edit (wired to Ctrl+Y / Ctrl+Shift+Z). Also refresh find matches if active.
    pub fn redo(&mut self) {
        self.active_buffer_mut().redo();
        if self.is_find_active() {
            self.update_find_matches();
        }
    }
}

/// Find matches supporting case, regex, whole word (Sprint 2 full).
fn find_matches_in_text(
    text: &str,
    query: &str,
    case_sensitive: bool,
    use_regex: bool,
    whole_word: bool,
) -> Vec<std::ops::Range<usize>> {
    if query.is_empty() {
        return vec![];
    }
    let mut matches = vec![];

    if use_regex {
        // Honor the case toggle in regex mode by prefixing the inline flag.
        // (whole-word is intentionally ignored in regex mode — the user can
        // write \b themselves; the toggle is documented as a plain-mode option.)
        let pattern = if case_sensitive {
            query.to_string()
        } else {
            format!("(?i){}", query)
        };
        if let Ok(re) = regex::Regex::new(&pattern) {
            for m in re.find_iter(text) {
                matches.push(m.start()..m.end());
            }
        }
        return matches;
    }

    // Plain substring search. To return BYTE offsets that always map back to the
    // original text (critical for selection/replace), search the original text
    // and compare case manually when case-insensitive. Lowercasing the haystack
    // separately can change byte lengths for non-ASCII and corrupt offsets.
    let q_chars: Vec<char> = query.chars().collect();
    let hay_chars: Vec<char> = text.chars().collect();
    let n = hay_chars.len();
    let qlen = q_chars.len();
    if qlen == 0 || qlen > n {
        return vec![];
    }

    let eq = |a: char, b: char| -> bool {
        if case_sensitive {
            a == b
        } else {
            a.to_lowercase().eq(b.to_lowercase())
        }
    };

    // Walk by char index, recording byte offsets alongside.
    // Precompute the byte offset at each char boundary once.
    let mut byte_at: Vec<usize> = Vec::with_capacity(n + 1);
    let mut b = 0usize;
    for c in text.chars() {
        byte_at.push(b);
        b += c.len_utf8();
    }
    byte_at.push(b); // one-past-end byte offset

    let mut i = 0usize;
    while i + qlen <= n {
        let mut ok = true;
        for k in 0..qlen {
            if !eq(hay_chars[i + k], q_chars[k]) {
                ok = false;
                break;
            }
        }
        if ok {
            let m_start = byte_at[i];
            let m_end = byte_at[i + qlen];
            if whole_word {
                let before_ok = i == 0 || !hay_chars[i - 1].is_alphanumeric();
                let after_ok = (i + qlen >= n) || !hay_chars[i + qlen].is_alphanumeric();
                if before_ok && after_ok {
                    matches.push(m_start..m_end);
                }
            } else {
                matches.push(m_start..m_end);
            }
        }
        i += 1;
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_case_sensitive_default() {
        // Default (case-insensitive) matches both cases.
        let m = find_matches_in_text("Hello hello HELLO", "hello", false, false, false);
        assert_eq!(m.len(), 3, "case-insensitive should match all three");
        // First match is at byte 0.
        assert_eq!(m[0], 0..5);
    }

    #[test]
    fn search_case_sensitive_on() {
        let m = find_matches_in_text("Hello hello", "Hello", true, false, false);
        assert_eq!(m.len(), 1, "case-sensitive matches only exact case");
        assert_eq!(m[0], 0..5);
    }

    #[test]
    fn search_whole_word_excludes_substring() {
        // 'foo' inside 'foobar' must be excluded when whole_word is on.
        let m = find_matches_in_text("foo foobar foo", "foo", false, false, true);
        assert_eq!(m.len(), 2, "whole-word excludes embedded matches");
        assert_eq!(m[0], 0..3);
        assert_eq!(m[1], 11..14);
    }

    #[test]
    fn search_whole_word_off_includes_substring() {
        let m = find_matches_in_text("foobar", "foo", false, false, false);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn search_regex_basic() {
        let m = find_matches_in_text("a1 b2 c3", r"\d", false, true, false);
        assert_eq!(m.len(), 3, "regex \\d matches all digits");
    }

    #[test]
    fn search_regex_case_insensitive() {
        // Regex mode honoring the case toggle via (?i).
        let m = find_matches_in_text("Cat cat CAT", "cat", false, true, false);
        assert_eq!(m.len(), 3, "case-insensitive regex matches all");
    }

    #[test]
    fn search_byte_offsets_valid_for_multibyte() {
        // Ensure returned byte offsets correctly index the original text even when
        // earlier characters are multi-byte (the old lowercase-haystack bug).
        let text = "ééHello"; // é is 2 bytes
        let m = find_matches_in_text(text, "hello", false, false, false);
        assert_eq!(m.len(), 1);
        let r = &m[0];
        assert_eq!(
            &text[r.start..r.end],
            "Hello",
            "byte range must slice the original"
        );
    }
}
