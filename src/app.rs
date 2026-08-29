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
use std::path::{Path, PathBuf};

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
    // ----- Configure popup (tab size, toggles) -----
    pub show_configure: bool,
    /// Selected row index in the Configure popup (0..N_ROWS-1).
    pub configure_selected: usize,
    /// Row currently under the mouse (live hover). When set, steals the
    /// highlight from the keyboard selection so only one row is highlighted.
    pub configure_row_hover: Option<usize>,
    /// Hovered footer button in the Configure popup (0=Toggle, 1=Close).
    pub configure_button_hover: Option<usize>,
    // ----- Themes popup -----
    pub show_themes: bool,
    /// Which screen of the Themes popup is active: list or editor.
    pub themes_mode: crate::ui::popups::themes::ThemesMode,
    /// Selected row in the Themes list (built-ins then customs).
    pub themes_selected: usize,
    /// Themes list scroll offset (rows). The list can exceed the popup height
    /// once several custom themes are installed.
    pub themes_scroll: u16,
    /// Row currently under the mouse in the Themes list.
    pub themes_list_hover: Option<usize>,
    /// Hovered footer button in the Themes list.
    pub themes_button_hover: Option<usize>,
    /// Editor-mode state (name buffer, in-progress theme, focused field, role
    /// list scroll). Populated when the user picks New/Edit; `None` in list mode.
    pub themes_edit: Option<crate::ui::popups::themes::ThemeEditState>,
    // legacy (removed): show_settings/settings_selected/settings_theme_hover
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
    pub file_dialog_selected: usize, // 0 = Save/Open, 1 = Cancel
    pub confirm_selected: usize, // 0 = Confirm, 1 = Cancel
    /// Button hover states (per popup)
    pub file_dialog_button_hover: Option<usize>, // 0 = Save/Open, 1 = Cancel
    pub confirm_button_hover: Option<usize>, // 0 = Confirm, 1 = Cancel
    // ----- File browser popup state -----
    /// True while the directory-listing popup is open over the file dialog.
    pub show_file_browser: bool,
    /// Directory currently being listed.
    pub browser_cwd: std::path::PathBuf,
    /// Cached listing of `browser_cwd` (refreshed on dir change / hidden toggle).
    pub browser_entries: Vec<crate::ui::popups::file_browser::BrowserEntry>,
    /// Currently highlighted row in the listing.
    pub browser_selected: usize,
    /// Entry index currently under the mouse (live hover preview). When set,
    /// this row steals the highlight from the keyboard selection so only one
    /// row is highlighted at a time. Cleared when the mouse leaves the list.
    pub browser_list_hover: Option<usize>,
    /// Whether dotfile entries are shown.
    pub browser_show_hidden: bool,
    /// Last IO/permission error from `read_directory`, shown inline. Cleared
    /// on a successful refresh.
    pub browser_error: Option<String>,
    /// Which view the browser is showing: the live directory or the persisted
    /// recent-files list.
    pub browser_mode: crate::ui::popups::file_browser::BrowserMode,
    /// Hovered footer button index in the browser (0=Up,1=Recent/Browse,2=Hidden,3=Cancel).
    pub browser_button_hover: Option<usize>,
    /// Keyboard-selected footer button index in the browser.
    pub browser_button_selected: usize,
    /// Remembers whether the browser was launched from Save As (true) or Open.
    pub browser_origin_is_save: bool,
    /// Mouse position tracking
    pub last_mouse_pos: Option<(u16, u16)>,
    /// True while the left mouse button is held down in the editor area after
    /// an initial click (used to drive drag selection + edge auto-scroll).
    /// Cleared on MouseUp, on popup open, and when switching input mode.
    pub mouse_drag_active: bool,
    /// True while the left mouse button is held down over the editor scrollbar
    /// (click-to-jump / drag-to-scrub). Mutually exclusive with
    /// `mouse_drag_active`: a scrollbar press never starts a text selection.
    pub scrollbar_drag_active: bool,
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
        let theme = crate::themes::ThemeRegistry::load()
            .by_name(&config.theme)
            .cloned()
            .unwrap_or_default();
        let highlighter = SyntaxHighlighter::new();

        let mut buffers = Vec::new();
        if initial_files.is_empty() {
            buffers.push(Buffer::placeholder());
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
            show_configure: false,
            configure_selected: 0,
            configure_row_hover: None,
            configure_button_hover: None,
            show_themes: false,
            themes_mode: crate::ui::popups::themes::ThemesMode::default(),
            themes_selected: 0,
            themes_scroll: 0,
            themes_list_hover: None,
            themes_button_hover: None,
            themes_edit: None,
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
            show_file_browser: false,
            browser_cwd: PathBuf::new(),
            browser_entries: Vec::new(),
            browser_selected: 0,
            browser_list_hover: None,
            browser_show_hidden: false,
            browser_error: None,
            browser_mode: crate::ui::popups::file_browser::BrowserMode::default(),
            browser_button_hover: None,
            browser_button_selected: 0,
            browser_origin_is_save: false,
            last_mouse_pos: None,
            mouse_drag_active: false,
            scrollbar_drag_active: false,
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
            self.buffers[0] = Buffer::placeholder();
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
            buf.rope.byte_slice(start..end).to_string()
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
                fs.query = self.active_buffer().rope.byte_slice(start..end).to_string();
            }
            self.find_state = Some(fs);
            self.update_find_matches();
        }
    }

    /// Toggle word wrap on/off, persist the setting, and re-sync every buffer's
    /// viewport (reset hscroll, re-clamp scroll to the new range, ensure the
    /// cursor is visible). Called by both the Alt+Z shortcut and the Tools menu
    /// item so keyboard and mouse paths stay behaviorally identical
    /// (AGENTS Core Principle 2).
    pub fn toggle_word_wrap(&mut self) {
        let on = !self.config.word_wrap;
        self.config.set_word_wrap(on);
        let w = self.editor_visible_width.get().max(1);
        let vis = self.editor_visible_lines.get().max(1);
        let tab = self.config.tab_size;
        for b in self.buffers.iter_mut() {
            b.visible_width = w;
            if on {
                // Turning wrap ON: `scroll_offset.0` becomes a screen row.
                // Reset hscroll, clamp into the wrapped range, then keep the
                // cursor visible via the wrapped (screen-row) variant. The
                // unwrapped `ensure_cursor_visible` treats `scroll_offset.0`
                // as a logical-line index and would overwrite it with a
                // logical value, corrupting the screen-row invariant
                // (final-review Important-1).
                b.scroll_offset.1 = 0;
                let total = crate::wrap::total_screen_rows(&b.rope, b.visible_width, tab);
                let max = total.saturating_sub(vis);
                if b.scroll_offset.0 > max {
                    b.scroll_offset.0 = max;
                }
                let layout =
                    crate::wrap::compute_layout(&b.rope, 0, b.line_count(), b.visible_width, tab);
                b.ensure_cursor_visible_wrapped(&layout, vis);
            } else {
                // Turning wrap OFF: `scroll_offset.0` reverts to a logical
                // line index, so the unwrapped variant is correct.
                b.ensure_cursor_visible(vis);
            }
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
        let word_wrap = self.config.word_wrap;
        let tab_size = self.config.tab_size;
        let b = self.active_buffer_mut();
        let pos = mrange.start;
        let end = mrange.end.min(b.rope.len_bytes());
        b.cursor = pos;
        b.selection = Some(pos..end);
        if word_wrap {
            let layout = crate::wrap::compute_layout(
                &b.rope,
                0,
                b.line_count(),
                b.visible_width.max(1),
                tab_size,
            );
            b.ensure_cursor_visible_wrapped(&layout, vis);
            let cursor_row = crate::wrap::screen_row_of_byte(&layout, &b.rope, 0, b.cursor);
            if cursor_row < b.scroll_offset.0 || cursor_row >= b.scroll_offset.0 + vis {
                b.scroll_offset.0 = cursor_row.saturating_sub(vis / 2);
            }
            // No horizontal adjust under wrap.
        } else {
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
            let sub = buf.rope.byte_slice(s..e).to_string();
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

    // ----- Configure popup -----
    pub fn open_configure(&mut self) {
        self.show_configure = true;
        self.configure_selected = 0;
        self.configure_row_hover = None;
        self.configure_button_hover = None;
        self.close_menu();
    }

    pub fn close_configure(&mut self) {
        self.show_configure = false;
    }

    /// Number of editable rows in the Configure popup. Kept in sync with
    /// `popups::configure::render`. Exposed as a shared constant so the key/mouse
    /// handlers and the renderer never drift apart.
    pub fn configure_row_count() -> usize {
        crate::ui::popups::configure::ROW_COUNT
    }

    /// Apply a change to a Configure row and persist + live-apply it.
    /// `delta` is +1/-1 for steppers (tab size) or ignored for toggles.
    /// For toggles the value is flipped regardless of `delta`.
    pub fn configure_adjust(&mut self, row: usize, delta: i32) {
        match row {
            0 => {
                // Tab size stepper (clamped 1..=8 by Config).
                let new = (self.config.tab_size as i32 + delta).clamp(1, 8) as u8;
                self.config.set_tab_size(new);
            }
            1 => {
                // Word wrap toggle.
                let on = !self.config.word_wrap;
                self.toggle_word_wrap(); // handles viewport re-sync
                let _ = on;
            }
            2 => {
                self.config
                    .set_show_line_numbers(!self.config.show_line_numbers);
            }
            3 => {
                self.config.set_mouse_enabled(!self.config.mouse_enabled);
            }
            _ => {}
        }
    }

    /// Toggle the focused Configure row (bools flip; tab size steps up by one).
    pub fn configure_toggle(&mut self, row: usize) {
        match row {
            0 => self.configure_adjust(0, 1),
            _ => self.configure_adjust(row, 0),
        }
    }

    // ----- Themes popup -----
    pub fn open_themes(&mut self) {
        self.show_themes = true;
        self.themes_mode = crate::ui::popups::themes::ThemesMode::List;
        // Default-select the currently active theme so the list opens pointing
        // at what the user is seeing.
        let registry = crate::themes::ThemeRegistry::load();
        let active_name = &self.theme.name;
        self.themes_selected = registry
            .all()
            .iter()
            .position(|t| &t.name == active_name)
            .unwrap_or(0);
        self.themes_scroll = 0;
        self.themes_list_hover = None;
        self.themes_button_hover = None;
        self.themes_edit = None;
        self.close_menu();
    }

    pub fn close_themes(&mut self) {
        self.show_themes = false;
        self.themes_edit = None;
    }

    /// Apply a theme by registry index (built-ins then customs) live + persist.
    /// Out-of-range indices are ignored.
    pub fn apply_theme_by_index(&mut self, idx: usize) {
        let registry = crate::themes::ThemeRegistry::load();
        if let Some(theme) = registry.all().get(idx) {
            self.theme = (*theme).clone();
            self.config.set_theme(&self.theme.name);
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
                            self.buffers[0] = Buffer::placeholder();
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

    /// Open `path` into a tab.
    ///
    /// Order: read the file (error → `Err`, nothing changes); if a buffer
    /// already holds this file, focus it; else replace the first placeholder
    /// (`is_initial && path.is_none() && !is_modified`) in place; else append.
    /// On success, records the path in recents.
    pub fn open_file_in_tab(&mut self, path: &Path) -> Result<(), String> {
        let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

        if let Some(idx) = self.index_of_open_path(path) {
            self.active = idx;
            self.config.record_recent_file(path);
            self.cached_highlights = None;
            return Ok(());
        }

        let new_buf = Buffer::from_str(&contents, Some(path.to_path_buf()));
        if let Some(idx) = self.placeholder_index() {
            self.buffers[idx] = new_buf;
            self.active = idx;
        } else {
            self.buffers.push(new_buf);
            self.active = self.buffers.len() - 1;
        }
        self.config.record_recent_file(path);
        self.cached_highlights = None;
        Ok(())
    }

    pub(crate) fn placeholder_index(&self) -> Option<usize> {
        self.buffers
            .iter()
            .position(|b| b.is_initial && b.path.is_none() && !b.is_modified)
    }

    pub(crate) fn index_of_open_path(&self, path: &Path) -> Option<usize> {
        let want = Self::canonical_or_raw(path);
        self.buffers.iter().position(|b| {
            b.path
                .as_ref()
                .is_some_and(|p| Self::canonical_or_raw(p) == want)
        })
    }

    fn canonical_or_raw(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    /// Keyboard selection indices in the file dialog:
    /// 0 = Save/Open, 1 = Browse…, 2 = Cancel.
    pub fn close_file_dialog(&mut self, button: usize) {
        match button {
            0 => {} // Save/Open — handled below
            1 => {
                // Browse… — open the file browser over this dialog instead of
                // confirming. Dialog stays open underneath.
                self.open_file_browser();
                return;
            }
            _ => {
                // Cancel (2) or anything out of range.
                self.show_file_dialog = false;
                self.file_path_input.clear();
                self.file_dialog_message.clear();
                return;
            }
        }

        let path_str = self.file_path_input.trim().to_string();
        if path_str.is_empty() {
            self.show_file_dialog = false;
            self.file_path_input.clear();
            self.file_dialog_message.clear();
            return;
        }
        let path = PathBuf::from(&path_str);
        if self.file_dialog_is_save {
            if let Err(e) = std::fs::write(&path, self.active_buffer().text()) {
                self.file_dialog_message = format!("Save error: {}", e);
                return;
            }
            let buf = self.active_buffer_mut();
            buf.set_path(path.clone());
            buf.is_modified = false;
            self.config.record_recent_file(&path);
        } else {
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    let new_buf = Buffer::from_str(&contents, Some(path.clone()));
                    self.buffers.push(new_buf);
                    self.active = self.buffers.len() - 1;
                    self.config.record_recent_file(&path);
                }
                Err(e) => {
                    self.file_dialog_message = format!("Open error: {}", e);
                    return;
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

    // ===================== File browser =====================
    //
    // The browser is launched from the file dialog's `[ Browse… ]` button.
    // All filesystem IO is funnelled through `refresh_browser` so error
    // handling stays in one place. Selecting a file does NOT open/save it
    // directly — it populates `file_path_input` and returns the user to the
    // file dialog, preserving the typed-path flow (spec §"Confirm action").

    /// Open the browser over the current file dialog. Seeds the starting
    /// directory from the active buffer's path parent, falling back to the
    /// OS home dir, then the CWD.
    pub fn open_file_browser(&mut self) {
        self.browser_origin_is_save = self.file_dialog_is_save;
        self.browser_mode = crate::ui::popups::file_browser::BrowserMode::Dir;
        self.browser_show_hidden = false;
        self.browser_selected = 0;
        self.browser_list_hover = None;
        self.browser_error = None;
        self.browser_button_selected = 4; // 4 = "list area" sentinel (see main.rs keys)
        self.browser_button_hover = None;

        let start_dir = self
            .active_buffer()
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| {
                // A buffer path with no directory component (e.g. "Cargo.toml")
                // yields an empty parent — interpret that as the CWD (".").
                if p.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    p.to_path_buf()
                }
            })
            .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        // Canonicalize so `browser_cwd` is always absolute and free of `.`/`..`
        // segments. Without this, launching the editor with a relative file
        // path (e.g. `rust-edit src/main.rs`) seeds a relative cwd, and
        // `browser_go_up`'s `Path::parent()` call would then yield an empty
        // path → `read_directory("")` → ENOENT, bricking the browser.
        // Fall back to the seed unchanged only if canonicalization itself
        // fails (e.g. the directory vanished between open and browse).
        self.browser_cwd = std::fs::canonicalize(&start_dir).unwrap_or(start_dir);
        self.refresh_browser();
        self.show_file_browser = true;
    }

    /// Close the browser. When `confirmed` is true the currently-selected
    /// entry's path is written into `file_path_input` (file dialog reopens
    /// focused on the action button so the user can hit Save/Open directly).
    pub fn close_file_browser(&mut self, confirmed: bool) {
        if confirmed {
            if let Some(entry) = self.browser_entries.get(self.browser_selected) {
                if !entry.is_dir {
                    self.file_path_input = entry.path.to_string_lossy().to_string();
                }
            }
        }
        self.show_file_browser = false;
        self.browser_list_hover = None;
    }

    /// Re-read `browser_cwd` into `browser_entries` (Dir mode only). IO errors
    /// are captured into `browser_error` and the previous listing is cleared;
    /// the app never panics on an unreadable directory.
    pub fn refresh_browser(&mut self) {
        if self.browser_mode != crate::ui::popups::file_browser::BrowserMode::Dir {
            self.refresh_recent();
            return;
        }
        match crate::ui::popups::file_browser::read_directory(
            &self.browser_cwd,
            self.browser_show_hidden,
        ) {
            Ok(entries) => {
                self.browser_entries = entries;
                self.browser_error = None;
                if self.browser_selected >= self.browser_entries.len() {
                    self.browser_selected = self.browser_entries.len().saturating_sub(1);
                }
            }
            Err(e) => {
                self.browser_entries.clear();
                self.browser_error = Some(format!("{}: {}", self.browser_cwd.display(), e));
                self.browser_selected = 0;
            }
        }
    }

    /// Populate `browser_entries` from the persisted recent-files list,
    /// dropping any that no longer exist on disk so the UI never offers a
    /// dead path. Pure aside from the stat calls.
    pub fn refresh_recent(&mut self) {
        self.browser_entries = self
            .config
            .recent_files
            .iter()
            .filter_map(|s| {
                let p = PathBuf::from(s);
                let name = p.file_name()?.to_string_lossy().to_string();
                let is_dir = std::fs::metadata(&p).map(|m| m.is_dir()).unwrap_or(false);
                Some(crate::ui::popups::file_browser::BrowserEntry {
                    name,
                    is_dir,
                    path: p,
                })
            })
            .collect();
        self.browser_error = None;
        if self.browser_selected >= self.browser_entries.len() {
            self.browser_selected = self.browser_entries.len().saturating_sub(1);
        }
    }

    /// Activate (Enter / click) the selected row. In Dir mode: directories are
    /// entered, files confirm and return to the file dialog. In Recent mode:
    /// any entry confirms immediately (it's always a file path).
    pub fn browser_enter_selected(&mut self) {
        let Some(entry) = self.browser_entries.get(self.browser_selected).cloned() else {
            return;
        };
        match self.browser_mode {
            crate::ui::popups::file_browser::BrowserMode::Dir => {
                if entry.is_dir {
                    // The pinned `..` row's stored path is a literal `cwd/..`,
                    // which would make later `.parent()` calls bounce. Route it
                    // through `browser_go_up` instead so navigation stays on
                    // canonical absolute paths.
                    if entry.name == ".." {
                        self.browser_go_up();
                        return;
                    }
                    self.browser_cwd = entry.path.clone();
                    self.browser_selected = 0;
                    self.refresh_browser();
                } else {
                    self.close_file_browser(true);
                }
            }
            crate::ui::popups::file_browser::BrowserMode::Recent => {
                if entry.is_dir {
                    // Jump into Dir mode at this directory.
                    self.browser_mode = crate::ui::popups::file_browser::BrowserMode::Dir;
                    self.browser_cwd = entry.path;
                    self.browser_selected = 0;
                    self.refresh_browser();
                } else {
                    self.close_file_browser(true);
                }
            }
        }
    }

    /// Go up one directory (Dir mode only). Uses the canonical parent so
    /// repeated ups converge at the filesystem root rather than chasing
    /// literal `..` segments forever.
    pub fn browser_go_up(&mut self) {
        if self.browser_mode != crate::ui::popups::file_browser::BrowserMode::Dir {
            return;
        }
        let parent = match self.browser_cwd.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => return, // already at root, or cwd is empty/relative-rooted
        };
        if parent == self.browser_cwd {
            return; // already at root
        }
        self.browser_cwd = parent;
        self.browser_selected = 0;
        self.browser_list_hover = None;
        self.refresh_browser();
    }

    pub fn browser_toggle_hidden(&mut self) {
        self.browser_show_hidden = !self.browser_show_hidden;
        self.browser_list_hover = None;
        self.refresh_browser();
    }

    /// Switch between directory listing and recent-files view.
    pub fn browser_toggle_mode(&mut self) {
        self.browser_mode = match self.browser_mode {
            crate::ui::popups::file_browser::BrowserMode::Dir => {
                crate::ui::popups::file_browser::BrowserMode::Recent
            }
            crate::ui::popups::file_browser::BrowserMode::Recent => {
                crate::ui::popups::file_browser::BrowserMode::Dir
            }
        };
        self.browser_selected = 0;
        self.browser_list_hover = None;
        self.browser_error = None;
        self.refresh_browser();
    }

    /// Move the row cursor by `delta`, clamping at both ends. Kept in `App`
    /// (not the pure module) because it mutates UI state. Scroll is derived
    /// from the selection at render/click time, so it needs no clamping here.
    /// Clears the mouse hover so the keyboard selection is what shows.
    pub fn browser_move_cursor(&mut self, delta: i32) {
        if self.browser_entries.is_empty() {
            return;
        }
        let n = self.browser_entries.len() as i32;
        let mut next = self.browser_selected as i32 + delta;
        if next < 0 {
            next = 0;
        }
        if next >= n {
            next = n - 1;
        }
        self.browser_selected = next as usize;
        self.browser_list_hover = None;
    }

    /// Jump the row cursor to the first/last entry.
    pub fn browser_jump(&mut self, to_end: bool) {
        if self.browser_entries.is_empty() {
            return;
        }
        self.browser_selected = if to_end {
            self.browser_entries.len() - 1
        } else {
            0
        };
        self.browser_list_hover = None;
    }

    /// Page the row cursor by approximately one viewport height.
    pub fn browser_page(&mut self, up: bool, viewport_rows: usize) {
        let step = viewport_rows.max(1) as i32;
        self.browser_move_cursor(if up { -step } else { step });
    }

    /// Select an explicit row index (e.g. from a mouse click). Out-of-range
    /// indices are clamped.
    pub fn browser_select_index(&mut self, idx: usize) {
        if self.browser_entries.is_empty() {
            return;
        }
        self.browser_selected = idx.min(self.browser_entries.len() - 1);
    }

    /// Footer button labels for the current mode. Order: Up, Recent/Browse,
    /// Hidden, Cancel.
    pub fn browser_footer_labels(&self) -> [&'static str; 4] {
        let mode_btn = match self.browser_mode {
            crate::ui::popups::file_browser::BrowserMode::Dir => "[ Browse ]",
            crate::ui::popups::file_browser::BrowserMode::Recent => "[ Recent ]",
        };
        let hidden_btn = if self.browser_show_hidden {
            "[ Hidden: on ]"
        } else {
            "[ Hidden: off ]"
        };
        ["[ Up ]", mode_btn, hidden_btn, "[ Cancel ]"]
    }

    pub fn wrap_selection_or_cursor(&mut self, before: &str, after: &str) {
        let buf = self.active_buffer_mut();
        buf.push_undo(); // make MD formatting undoable (was direct rope bypass)
        if let Some(sel) = &buf.selection {
            let start = sel.start.min(buf.rope.len_bytes());
            let end = sel.end.min(buf.rope.len_bytes());
            let selected = buf.rope.byte_slice(start..end).to_string();
            let wrapped = format!("{}{}{}", before, selected, after);
            buf.remove_byte_range(start, end);
            buf.insert_at_byte(start, &wrapped);
            buf.cursor = start + wrapped.len();
            buf.selection = Some(start..start + wrapped.len());
            buf.is_modified = true;
        } else {
            buf.insert_at_byte(buf.cursor, before);
            buf.insert_at_byte(buf.cursor + before.len(), after);
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
    use std::fs;
    use std::path::Path;

    fn unique_temp_file(contents: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-open-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&p, contents).unwrap();
        p
    }

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

    // ----- File browser navigation regressions -----

    /// Helper: build an App whose active buffer carries the given path, with no
    /// real file IO needed for the buffer itself.
    fn app_with_buffer_path(path: &str) -> App {
        let mut app = App::new(&[]);
        app.active_buffer_mut().set_path(PathBuf::from(path));
        app
    }

    #[test]
    fn open_file_browser_canonicalizes_relative_buffer_path() {
        // Regression: launching with a relative file path used to seed a
        // relative `browser_cwd`, which broke `browser_go_up` (empty parent)
        // and left the browser stuck on an ENOENT error.
        // Use Cargo.toml at the crate root as the "buffer's file" so the
        // canonicalize call has a real, existing parent directory to resolve.
        let mut app = app_with_buffer_path("Cargo.toml");
        app.open_file_browser();

        assert!(
            app.browser_cwd.is_absolute(),
            "browser_cwd must be absolute after open_file_browser, got {:?}",
            app.browser_cwd
        );
        assert!(
            !app.browser_cwd.as_os_str().is_empty(),
            "browser_cwd must not be empty"
        );
        assert!(
            app.browser_error.is_none(),
            "initial listing must succeed, got error: {:?}",
            app.browser_error
        );
    }

    #[test]
    fn browser_go_up_never_yields_empty_cwd() {
        // Regression: `Path::new("src").parent()` returns `Some("")`, and
        // reading "" fails with ENOENT. After canonicalization + the
        // empty-parent guard, repeated ups must converge at the root without
        // ever producing an empty or relative cwd.
        let mut app = app_with_buffer_path("Cargo.toml");
        app.open_file_browser();
        assert!(app.browser_cwd.is_absolute());

        // Hammer "go up" far past any plausible depth — must stay valid.
        for _ in 0..64 {
            app.browser_go_up();
            assert!(
                !app.browser_cwd.as_os_str().is_empty(),
                "browser_cwd became empty after browser_go_up"
            );
            assert!(
                app.browser_cwd.is_absolute(),
                "browser_cwd must remain absolute, got {:?}",
                app.browser_cwd
            );
        }
        // We should have converged at the filesystem root.
        assert_eq!(app.browser_cwd.parent(), None);
    }

    #[test]
    fn empty_launch_creates_placeholder() {
        let app = App::new(&[]);
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        assert!(app.buffers[0].path.is_none());
        assert!(!app.buffers[0].is_modified);
    }

    #[test]
    fn new_tab_is_not_a_placeholder() {
        let mut app = App::new(&[]);
        app.new_tab();
        assert_eq!(app.buffers.len(), 2);
        assert!(
            app.buffers[0].is_initial,
            "original startup tab stays initial"
        );
        assert!(
            !app.buffers[1].is_initial,
            "[+] tab must never be marked initial"
        );
    }

    #[test]
    fn closing_last_tab_creates_a_fresh_placeholder() {
        let mut app = App::new(&[]);
        app.buffers[0].is_initial = false; // pretend it was a user tab
        app.close_current_tab();
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        assert!(app.buffers[0].path.is_none());
        assert!(!app.buffers[0].is_modified);
    }

    #[test]
    fn confirming_close_of_last_modified_tab_creates_placeholder() {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("x");
        app.show_confirm_dialog("Close tab with unsaved changes?", "close_tab");
        app.close_confirm(true);
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        assert_eq!(app.buffers[0].text(), "");
    }

    #[test]
    fn open_replaces_initial_placeholder() {
        let mut app = App::new(&[]);
        let path = unique_temp_file("hello from disk");
        app.open_file_in_tab(&path).expect("open");
        assert_eq!(
            app.buffers.len(),
            1,
            "placeholder must be replaced, not appended"
        );
        assert_eq!(app.active, 0);
        assert_eq!(app.buffers[0].text(), "hello from disk");
        assert_eq!(app.buffers[0].path.as_deref(), Some(path.as_path()));
        assert!(!app.buffers[0].is_initial);
        assert!(!app.buffers[0].is_modified);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_keeps_modified_placeholder_and_appends() {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("typed");
        let path = unique_temp_file("file");
        app.open_file_in_tab(&path).expect("open");
        assert_eq!(app.buffers.len(), 2);
        assert_eq!(app.buffers[0].text(), "typed");
        assert!(app.buffers[0].is_initial, "modified placeholder is kept");
        assert_eq!(app.buffers[1].path.as_deref(), Some(path.as_path()));
        assert_eq!(app.active, 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_read_error_changes_nothing() {
        let mut app = App::new(&[]);
        let err = app
            .open_file_in_tab(Path::new("/this/does/not/exist/rust-edit-nope.txt"))
            .unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        assert_eq!(app.active, 0);
    }

    #[test]
    fn undone_typing_in_placeholder_is_still_replaced() {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("hello");
        app.active_buffer_mut().undo();
        assert!(!app.buffers[0].is_modified);
        let path = unique_temp_file("replaced");
        app.open_file_in_tab(&path).expect("open");
        assert_eq!(app.buffers.len(), 1);
        assert_eq!(app.buffers[0].text(), "replaced");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn inactive_placeholder_is_still_replaced_and_activated() {
        let mut app = App::new(&[]);
        app.new_tab(); // active = 1, placeholder remains at 0
        assert_eq!(app.active, 1);
        let path = unique_temp_file("now-active");
        app.open_file_in_tab(&path).expect("open");
        assert_eq!(
            app.buffers.len(),
            2,
            "placeholder replaced in place; [+] kept"
        );
        assert_eq!(app.active, 0, "opened file becomes active");
        assert_eq!(app.buffers[0].path.as_deref(), Some(path.as_path()));
        assert!(app.buffers[1].path.is_none());
        assert!(!app.buffers[1].is_initial);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_file_in_tab_invalidates_highlight_cache() {
        let mut app = App::new(&[]);
        app.cached_highlights = Some(CachedHighlights {
            language: Language::Plain,
            generation: 0,
            start_line: 0,
            count: 1,
            spans: vec![vec![Span::raw("dummy")]],
        });
        let path = unique_temp_file("plain text");
        app.open_file_in_tab(&path).expect("open");
        assert!(
            app.cached_highlights.is_none(),
            "opening a Plain file must drop the generation-0 placeholder cache"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_same_file_twice_focuses_existing_tab() {
        let path = unique_temp_file("once");
        let mut app = App::new(&[]);
        app.open_file_in_tab(&path).unwrap();
        app.new_tab();
        assert_eq!(app.buffers.len(), 2);
        assert_eq!(app.active, 1);
        app.open_file_in_tab(&path).unwrap();
        assert_eq!(app.buffers.len(), 2, "must not duplicate");
        assert_eq!(app.active, 0, "existing tab focused");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_relative_and_absolute_path_are_the_same_tab() {
        let rel = PathBuf::from("Cargo.toml");
        let abs = std::fs::canonicalize(&rel).expect("Cargo.toml must exist");
        let mut app = App::new(&[]);
        app.open_file_in_tab(&rel).unwrap();
        app.open_file_in_tab(&abs).unwrap();
        assert_eq!(
            app.buffers.len(),
            1,
            "relative vs absolute must not duplicate"
        );
        assert_eq!(app.active, 0);
    }

    #[test]
    fn duplicate_check_falls_back_to_raw_path_when_canonicalize_fails() {
        // A buffer whose path no longer exists cannot be canonicalized.
        let mut app = App::new(&[]);
        let gone = PathBuf::from("/definitely/missing/rust-edit-gone-file.rs");
        app.buffers[0].set_path(gone.clone());
        app.buffers[0].is_initial = false;
        // Directly exercise the matcher (open_file_in_tab would fail at read — rule 1).
        assert_eq!(app.index_of_open_path(&gone), Some(0));
    }

    #[test]
    fn plus_created_empty_tab_is_never_replaced() {
        // Start with a real file so the only empty tab is the [+] one.
        let first = unique_temp_file("first");
        let mut app = App::new(&[first.to_string_lossy().to_string()]);
        assert_eq!(app.buffers.len(), 1);
        assert!(!app.buffers[0].is_initial);
        app.new_tab();
        assert!(!app.buffers[1].is_initial);
        let second = unique_temp_file("second");
        app.open_file_in_tab(&second).unwrap();
        assert_eq!(
            app.buffers.len(),
            3,
            "[+] empty tab must be kept; file appended"
        );
        assert!(app.buffers[1].path.is_none());
        assert!(!app.buffers[1].is_initial);
        assert_eq!(app.buffers[2].path.as_deref(), Some(second.as_path()));
        assert_eq!(app.active, 2);
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
    }

    #[test]
    fn open_appends_when_app_started_with_a_file() {
        let first = unique_temp_file("one");
        let mut app = App::new(&[first.to_string_lossy().to_string()]);
        let second = unique_temp_file("two");
        app.open_file_in_tab(&second).unwrap();
        assert_eq!(app.buffers.len(), 2);
        assert_eq!(app.active, 1);
        assert_eq!(app.buffers[0].path.as_deref(), Some(first.as_path()));
        assert_eq!(app.buffers[1].path.as_deref(), Some(second.as_path()));
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
    }

    #[test]
    fn placeholder_after_closing_last_tab_is_replaceable() {
        let first = unique_temp_file("closing");
        let mut app = App::new(&[first.to_string_lossy().to_string()]);
        app.close_current_tab(); // unmodified → reset to placeholder
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
        let second = unique_temp_file("after-close");
        app.open_file_in_tab(&second).unwrap();
        assert_eq!(app.buffers.len(), 1, "reset placeholder must be replaced");
        assert_eq!(app.buffers[0].path.as_deref(), Some(second.as_path()));
        assert_eq!(app.active, 0);
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
    }
}
