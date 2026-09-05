// Top-level App state, event loop, high-level commands (to be implemented in Sprint 1)
// See Design Spec §6 data model sketch and AGENTS.md recommended structure.

#![allow(dead_code)]

use crate::buffer::Buffer;
use crate::config::Config;
use crate::syntax::SyntaxHighlighter;
use crate::theme::Theme;
use crate::ui::find::{FindFocus, FindState};
use crate::utils::clipboard::ClipboardWrapper;
use std::path::{Path, PathBuf};

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
    /// Hovered footer button index in the browser (matches `browser_footer_labels()`).
    pub browser_button_hover: Option<usize>,
    /// Keyboard-selected footer button index in the browser.
    pub browser_button_selected: usize,
    /// Remembers whether the browser was launched from Save As (true) or Open.
    pub browser_origin_is_save: bool,
    /// Save As only: filename typed in the browser Name field (not a path).
    pub browser_filename: String,
    /// Last directory used for Save As, used to pre-fill the path for subsequent saves.
    pub last_save_directory: Option<std::path::PathBuf>,
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
    /// Popup whose overflow scrollbar is being dragged. Separate from
    /// `scrollbar_drag_active` because the event loop clears the editor flag
    /// whenever a modal is open.
    pub popup_scrollbar_drag: Option<PopupFocus>,
    /// Set to true to exit the main loop cleanly (used by menu Quit etc.)
    pub should_quit: bool,
    /// Last known height (in lines) of the editor viewport area. Used for PgUp/PgDn and better scrolling.
    pub editor_visible_lines: std::cell::Cell<usize>,
    /// Last known width (in columns) of the editor text area (excludes gutter +
    /// scrollbar). Used by horizontal auto-scroll to follow the cursor.
    pub editor_visible_width: std::cell::Cell<usize>,
    /// Whether `ui()` reserved a row for the editor horizontal scrollbar this
    /// frame. `editor::render` reads this instead of recomputing from
    /// `buf.visible_lines` (already post-subtract).
    pub editor_hscrollbar_visible: std::cell::Cell<bool>,
    /// Parse-state checkpoint cache for the active buffer. `refresh_highlight_cache`
    /// ensures an overscanned window; the editor reads spans via `line_spans`.
    pub highlight_cache: crate::highlight_cache::HighlightCache,
}

/// Which centered popup currently owns input. Highest z-order wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupFocus {
    Confirm,
    FileBrowser,
    FileDialog,
    Themes,
    Configure,
    Help,
    WordCount,
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
            browser_filename: String::new(),
            last_save_directory: std::env::current_dir().ok(),
            last_mouse_pos: None,
            mouse_drag_active: false,
            scrollbar_drag_active: false,
            popup_scrollbar_drag: None,
            should_quit: false,
            editor_visible_lines: std::cell::Cell::new(20),
            editor_visible_width: std::cell::Cell::new(80),
            editor_hscrollbar_visible: std::cell::Cell::new(false),
            highlight_cache: crate::highlight_cache::HighlightCache::new(),
        }
    }

    pub fn active_buffer(&self) -> &Buffer {
        &self.buffers[self.active]
    }

    pub fn active_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active]
    }

    /// Apply the current editor viewport to every buffer, clamp scroll, and
    /// fill the highlight cache for the active tab.
    ///
    /// Must run inside `ui()` *after* the layout size is known and *before*
    /// `editor::render`, so wrap math, scroll, and cache window all use the
    /// same width/height as the frame being painted (resize crash).
    pub fn apply_editor_viewport(&mut self, vis_h: usize, content_w: usize) {
        let vis_h = vis_h.max(1);
        let content_w = content_w.max(1);
        self.editor_visible_lines.set(vis_h);
        self.editor_visible_width.set(content_w);
        let wrap = self.config.word_wrap;
        let tab = self.config.tab_size;
        for b in self.buffers.iter_mut() {
            b.visible_lines = vis_h;
            b.visible_width = content_w;
            b.word_wrap = wrap;
            b.tab_size = tab;
            b.refresh_max_line_width();
            if wrap {
                b.scroll_offset.1 = 0;
            } else {
                b.clamp_hscroll();
            }
            if wrap {
                let total = crate::wrap::total_screen_rows(&b.rope, content_w, tab);
                b.clamp_scroll_wrapped(total);
            } else {
                b.clamp_scroll(vis_h);
            }
        }
        let scroll_row = self.active_buffer().scroll_offset.0;
        if wrap {
            let buf = self.active_buffer();
            let (start, count) = crate::wrap::visible_logical_window(
                &buf.rope,
                scroll_row,
                vis_h,
                buf.visible_width.max(1),
                tab,
            );
            self.refresh_highlight_cache(start, count);
        } else {
            self.refresh_highlight_cache(scroll_row, vis_h);
        }
    }

    /// Ensure the highlight cache covers the active buffer's visible window.
    ///
    /// Delegates to `HighlightCache::ensure_window`, which is a cheap no-op on
    /// a hit and otherwise resumes from the nearest parse checkpoint instead of
    /// rebuilding `HighlightLines` from line 0 on every scroll.
    pub fn refresh_highlight_cache(&mut self, start_line: usize, count: usize) {
        let App {
            highlighter,
            highlight_cache,
            buffers,
            active,
            ..
        } = self;
        let buf = &buffers[*active];
        let n_lines = buf.line_count();
        let language = buf.language;
        let generation = buf.generation;
        let source = buf.id;
        highlight_cache.ensure_window(
            highlighter,
            |i| buffers[*active].line(i),
            n_lines,
            start_line,
            count,
            language,
            generation,
            source,
        );
    }

    /// Read-only access to the highlight cache for the active buffer.
    pub fn cached_visible_spans(&self) -> &crate::highlight_cache::HighlightCache {
        &self.highlight_cache
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
                // Remember the directory for future Save As operations
                if let Some(parent) = path.parent() {
                    self.last_save_directory = Some(parent.to_path_buf());
                }
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
        // An empty replacement would just delete the matched word, which is
        // almost always a mistake. Treat it as a no-op instead.
        if repl.is_empty() {
            return;
        }

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
        // An empty replacement would delete every match. Treat it as a no-op
        // to avoid an accidental delete-all.
        if fs.replace.is_empty() {
            return;
        }
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
            buf.bump_generation();
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
        if self.popup_is_open() {
            return;
        }
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
        if self.popup_is_open() {
            return;
        }
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
        if self.popup_is_open() {
            return;
        }
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
        if self.popup_is_open() {
            return;
        }
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

    pub fn popup_focus(&self) -> Option<PopupFocus> {
        if self.show_confirm {
            Some(PopupFocus::Confirm)
        } else if self.show_file_browser {
            Some(PopupFocus::FileBrowser)
        } else if self.show_file_dialog {
            Some(PopupFocus::FileDialog)
        } else if self.show_themes {
            Some(PopupFocus::Themes)
        } else if self.show_configure {
            Some(PopupFocus::Configure)
        } else if self.show_help {
            Some(PopupFocus::Help)
        } else if self.show_word_count {
            Some(PopupFocus::WordCount)
        } else {
            None
        }
    }

    pub fn popup_is_open(&self) -> bool {
        self.popup_focus().is_some()
    }

    pub fn open_help(&mut self) {
        if self.popup_is_open() {
            return;
        }
        self.show_help = true;
        self.help_content_scroll = (0, 0);
        self.close_menu();
    }

    pub fn close_help(&mut self) {
        self.show_help = false;
        self.popup_scrollbar_drag = None;
    }

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
        if self.popup_is_open() {
            return;
        }
        self.file_dialog_selected = 0;
        self.show_file_dialog = true;
        self.file_dialog_is_save = is_save;
        self.file_path_input.clear();
        if is_save {
            // For Save As, pre-fill with current file path if one exists
            if let Some(ref path) = self.active_buffer().path {
                self.file_path_input = path.to_string_lossy().to_string();
            } else if let Some(ref dir) = self.last_save_directory {
                // No current path - use last save directory with untitled filename
                self.file_path_input = dir.join("untitled.txt").to_string_lossy().to_string();
            } else {
                self.file_path_input = "untitled.txt".to_string();
            }
        }
        // For Open, leave path empty so user selects a new file
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
            self.highlight_cache.invalidate();
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
        self.highlight_cache.invalidate();
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
            // Remember the directory for future Save As operations
            if let Some(parent) = path.parent() {
                self.last_save_directory = Some(parent.to_path_buf());
            }
        } else {
            match self.open_file_in_tab(&path) {
                Ok(()) => {}
                Err(e) => {
                    self.file_dialog_message = format!("Open error: {e}");
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
        if !self.show_file_dialog || self.show_confirm {
            return;
        }
        self.browser_origin_is_save = self.file_dialog_is_save;
        self.browser_mode = crate::ui::popups::file_browser::BrowserMode::Dir;
        self.browser_show_hidden = false;
        self.browser_selected = 0;
        self.browser_list_hover = None;
        self.browser_error = None;
        self.browser_button_selected = crate::ui::popups::file_browser::FOCUS_LIST;
        self.browser_button_hover = None;
        self.browser_filename = Path::new(self.file_path_input.trim())
            .file_name()
            .filter(|n| !n.is_empty())
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|s| crate::ui::popups::file_browser::is_valid_save_filename(s))
            .or_else(|| {
                self.active_buffer()
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .filter(|s| crate::ui::popups::file_browser::is_valid_save_filename(s))
            })
            .unwrap_or_default();

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
    /// Directories are never confirmed here — use `browser_use_current_folder`
    /// for the Save As "use this directory" path.
    pub fn close_file_browser(&mut self, confirmed: bool) {
        if confirmed {
            if let Some(entry) = self.browser_entries.get(self.browser_selected) {
                if !entry.is_dir {
                    self.file_path_input = entry.path.to_string_lossy().to_string();
                    self.file_dialog_selected = 0;
                }
            }
        }
        self.show_file_browser = false;
        self.browser_list_hover = None;
        self.popup_scrollbar_drag = None;
    }

    /// Confirm the current directory as the Save As destination, joined with
    /// the Name field. No-op if launched from Open. Empty / invalid names stay
    /// in the browser and focus the Name field — never invent "untitled".
    pub fn browser_use_current_folder(&mut self) {
        if !self.browser_origin_is_save {
            return;
        }
        let name = self.browser_filename.trim();
        if !crate::ui::popups::file_browser::is_valid_save_filename(name) {
            self.browser_error = Some("Enter a filename in the Name field".to_string());
            self.browser_button_selected = crate::ui::popups::file_browser::FOCUS_FILENAME;
            return;
        }
        self.file_path_input = self.browser_cwd.join(name).to_string_lossy().into_owned();
        self.show_file_browser = false;
        self.browser_list_hover = None;
        self.popup_scrollbar_drag = None;
        self.file_dialog_selected = 0;
    }

    pub fn browser_filename_push(&mut self, ch: char) {
        if ch.is_control() || ch == '/' || ch == '\\' {
            return;
        }
        self.browser_filename.push(ch);
        self.browser_error = None;
    }

    pub fn browser_filename_backspace(&mut self) {
        self.browser_filename.pop();
        self.browser_error = None;
    }

    /// Activate a footer button by index. Indices match `browser_footer_labels()`.
    pub fn browser_activate_footer(&mut self, idx: usize) {
        if self.browser_origin_is_save {
            match idx {
                0 => self.browser_use_current_folder(),
                1 => self.browser_go_up(),
                2 => self.browser_toggle_hidden(),
                3 => self.close_file_browser(false),
                _ => {}
            }
        } else {
            match idx {
                0 => self.browser_go_up(),
                1 => self.browser_toggle_mode(),
                2 => self.browser_toggle_hidden(),
                3 => self.close_file_browser(false),
                _ => {}
            }
        }
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

    /// Footer button labels. Open: Up / Recent-toggle / Hidden / Cancel.
    /// Save As: Use folder / Up / Hidden / Cancel (no Recent — folder pick is
    /// the extra action, and five buttons overflow the 50-col min width).
    pub fn browser_footer_labels(&self) -> [&'static str; 4] {
        let hidden_btn = if self.browser_show_hidden {
            "[ Hidden: on ]"
        } else {
            "[ Hidden: off ]"
        };
        if self.browser_origin_is_save {
            ["[ Use folder ]", "[ Up ]", hidden_btn, "[ Cancel ]"]
        } else {
            let mode_btn = match self.browser_mode {
                crate::ui::popups::file_browser::BrowserMode::Dir => "[ **Browse** / Recent ]",
                crate::ui::popups::file_browser::BrowserMode::Recent => "[ Browse / **Recent** ]",
            };
            ["[ Up ]", mode_btn, hidden_btn, "[ Cancel ]"]
        }
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
        buf.bump_generation();
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

    // ----- Empty-replace no-op regressions -----

    /// Build an App with one buffer holding `text` and a find state carrying
    /// `query`/`replace`, with matches already computed. Lets us exercise the
    /// replace paths without a live event loop.
    fn app_with_find(text: &str, query: &str, replace: &str) -> App {
        let mut app = App::new(&[]);
        app.active_buffer_mut().rope = ropey::Rope::from_str(text);
        let mut fs = FindState::new();
        fs.query = query.to_string();
        fs.replace = replace.to_string();
        app.find_state = Some(fs);
        app.update_find_matches();
        app
    }

    #[test]
    fn replace_current_empty_replace_is_noop() {
        // Regression: an empty Replace field used to delete the matched word.
        let mut app = app_with_find("Test Testing", "Test", "");
        let before = app.active_buffer().text();
        app.replace_current();
        assert_eq!(
            app.active_buffer().text(),
            before,
            "empty replace must not alter the buffer"
        );
    }

    #[test]
    fn replace_all_empty_replace_is_noop() {
        // Regression: empty Replace + replace-all used to delete every match.
        let mut app = app_with_find("Test Testing", "Test", "");
        let before = app.active_buffer().text();
        app.replace_all();
        assert_eq!(
            app.active_buffer().text(),
            before,
            "empty replace-all must not alter the buffer"
        );
    }

    #[test]
    fn replace_current_nonempty_replace_still_works() {
        // Sanity: a non-empty replacement still performs the replace.
        let mut app = app_with_find("Test Testing", "Test", "Hello");
        app.replace_current();
        // First match ("Test") is replaced; "Testing" is untouched (substring).
        assert_eq!(app.active_buffer().text(), "Hello Testing");
    }

    #[test]
    fn replace_all_bumps_generation_and_invalidates_highlights() {
        // Regression: replace-all used to assign a new rope without bumping
        // generation, so HighlightCache treated the fill as a hit and kept
        // painting pre-replace Span content.
        let mut app = app_with_find("foo foo", "foo", "bar");
        let gen_before = app.active_buffer().generation;
        app.refresh_highlight_cache(0, 5);
        let old = app.highlight_cache.line_spans(0).map(|s| {
            s.iter()
                .map(|sp| sp.content.to_string())
                .collect::<String>()
        });
        app.replace_all();
        assert_ne!(app.active_buffer().generation, gen_before);
        app.refresh_highlight_cache(0, 5);
        let new = app.highlight_cache.line_spans(0).map(|s| {
            s.iter()
                .map(|sp| sp.content.to_string())
                .collect::<String>()
        });
        assert!(
            new.as_ref().is_some_and(|s| s.contains("bar")),
            "highlight cache must show replaced text, got {new:?}"
        );
        assert!(
            old.as_ref().is_some_and(|s| s.contains("foo")),
            "pre-replace spans must still contain the original query, got {old:?}"
        );
    }

    #[test]
    fn markdown_wrap_bumps_generation() {
        // Regression: wrap used insert_at_byte/remove_byte_range which do not
        // bump generation (indent/newline bump once at the end instead).
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("hi");
        let g = app.active_buffer().generation;
        app.active_buffer_mut().selection = Some(0..2);
        app.wrap_selection_or_cursor("**", "**");
        assert_ne!(app.active_buffer().generation, g);
        assert_eq!(app.active_buffer().text(), "**hi**");
    }

    #[test]
    fn highlight_cache_covers_wrapped_lines_after_width_change() {
        // Repro: wrap on, long lines, scrolled, then the editor gets narrower
        // (terminal resize). After applying the new viewport, every logical
        // line the wrapped renderer would visit must be in the cache.
        let text = (0..80)
            .map(|_| "x".repeat(80))
            .collect::<Vec<_>>()
            .join("\n");
        let mut app = App::new(&[]);
        app.config.word_wrap = true;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(&text, None);
        app.editor_visible_lines.set(20);
        app.editor_visible_width.set(80);
        let b = app.active_buffer_mut();
        b.visible_lines = 20;
        b.visible_width = 80;
        b.word_wrap = true;
        b.tab_size = 4;
        b.scroll_offset = (100, 0);

        // Previous frame filled the cache at width 80 (logical start ≈ 100,
        // overscan → cache starts at line 36).
        app.refresh_highlight_cache(100, 20);
        assert!(
            app.highlight_cache.line_spans(35).is_none(),
            "precondition: wide fill must miss line 35 (overscan edge)"
        );

        // Resize: apply current frame size.
        app.apply_editor_viewport(20, 20);

        let buf = app.active_buffer();
        let vis = buf.visible_lines.max(1);
        let width = buf.visible_width.max(1);
        let (start, count) = crate::wrap::visible_logical_window(
            &buf.rope,
            buf.scroll_offset.0,
            vis,
            width,
            buf.tab_size,
        );
        let layout =
            crate::wrap::compute_layout(&buf.rope, 0, buf.line_count(), width, buf.tab_size);
        assert_eq!(
            start,
            crate::wrap::first_logical_line_for_screen_row(&layout, buf.scroll_offset.0),
            "cache window must start at the same logical line render_wrapped would paint"
        );
        for i in start..start + count {
            if i >= buf.line_count() {
                break;
            }
            assert!(
                app.highlight_cache.line_spans(i).is_some(),
                "after resize, cache must cover visible logical line {i} (window {start}+{count})"
            );
        }
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
        app.show_file_dialog = true;
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
        app.show_file_dialog = true;
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
        app.refresh_highlight_cache(0, 1);
        assert!(app.highlight_cache.line_spans(0).is_some());
        let path = unique_temp_file("plain text");
        app.open_file_in_tab(&path).expect("open");
        assert!(
            app.highlight_cache.line_spans(0).is_none(),
            "opening a file must invalidate the previous buffer's highlight cache"
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

    #[test]
    fn file_dialog_open_replaces_placeholder() {
        let mut app = App::new(&[]);
        let path = unique_temp_file("from-dialog");
        app.show_file_dialog(false);
        app.file_path_input = path.to_string_lossy().to_string();
        app.close_file_dialog(0); // Open
        assert!(!app.show_file_dialog, "success must dismiss the dialog");
        assert_eq!(app.buffers.len(), 1, "dialog must use open_file_in_tab");
        assert_eq!(app.buffers[0].text(), "from-dialog");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn file_dialog_open_error_keeps_dialog_and_buffers() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.file_path_input = "/this/does/not/exist/rust-edit-nope.txt".into();
        app.close_file_dialog(0);
        assert!(app.show_file_dialog, "error must keep the dialog open");
        assert!(
            app.file_dialog_message.starts_with("Open error:"),
            "got {:?}",
            app.file_dialog_message
        );
        assert_eq!(app.buffers.len(), 1);
        assert!(app.buffers[0].is_initial);
    }

    #[test]
    fn popup_focus_prefers_confirm_over_other_popups() {
        let mut app = App::new(&[]);
        app.show_help = true;
        app.show_file_dialog = true;
        app.show_file_browser = true;
        assert_eq!(app.popup_focus(), Some(PopupFocus::FileBrowser));
        app.show_confirm_dialog("Quit with unsaved changes?", "quit");
        assert_eq!(app.popup_focus(), Some(PopupFocus::Confirm));
    }

    #[test]
    fn popup_focus_file_browser_over_file_dialog() {
        let mut app = App::new(&[]);
        app.show_file_dialog = true;
        assert_eq!(app.popup_focus(), Some(PopupFocus::FileDialog));
        app.show_file_browser = true;
        assert_eq!(app.popup_focus(), Some(PopupFocus::FileBrowser));
    }

    #[test]
    fn opening_a_second_popup_is_a_noop() {
        let mut app = App::new(&[]);
        app.open_configure();
        app.open_themes();
        app.show_file_dialog(false);
        app.compute_and_show_word_count();
        app.open_help();
        assert!(app.show_configure);
        assert!(!app.show_themes);
        assert!(!app.show_file_dialog);
        assert!(!app.show_word_count);
        assert!(!app.show_help);
        assert_eq!(app.popup_focus(), Some(PopupFocus::Configure));
    }

    #[test]
    fn quit_confirm_opens_on_top_without_closing_underlying_popup() {
        let mut app = App::new(&[]);
        app.open_configure();
        app.active_buffer_mut().insert("unsaved");
        app.request_quit();
        assert!(app.show_configure);
        assert!(app.show_confirm);
        assert_eq!(app.confirm_action.as_deref(), Some("quit"));
        assert_eq!(app.popup_focus(), Some(PopupFocus::Confirm));
        app.close_confirm(false);
        assert!(!app.show_confirm);
        assert!(app.show_configure);
        assert_eq!(app.popup_focus(), Some(PopupFocus::Configure));
    }

    #[test]
    fn file_browser_may_open_over_file_dialog_only() {
        let mut app = App::new(&[]);
        app.open_configure();
        app.open_file_browser();
        assert!(
            !app.show_file_browser,
            "browser must not stack on configure"
        );
        app.close_configure();
        app.show_file_dialog(false);
        app.open_file_browser();
        assert!(app.show_file_dialog);
        assert!(app.show_file_browser);
        assert_eq!(app.popup_focus(), Some(PopupFocus::FileBrowser));
    }

    #[test]
    fn open_menu_is_blocked_while_a_popup_is_focused() {
        let mut app = App::new(&[]);
        app.open_configure();
        app.open_menu("File");
        assert!(app.active_menu.is_none());
    }

    #[test]
    fn save_browser_use_folder_fills_cwd_plus_basename() {
        let mut app = App::new(&[]);
        app.show_file_dialog = true;
        app.file_dialog_is_save = true;
        app.browser_origin_is_save = true;
        app.browser_cwd = PathBuf::from("/tmp/outdir");
        app.browser_filename = "widget.rs".into();
        app.show_file_browser = true;

        app.browser_use_current_folder();

        assert!(!app.show_file_browser);
        assert_eq!(app.file_path_input, "/tmp/outdir/widget.rs");
        assert_eq!(app.file_dialog_selected, 0);
    }

    #[test]
    fn save_browser_use_folder_empty_name_stays_open_without_untitled() {
        let mut app = App::new(&[]);
        app.show_file_dialog = true;
        app.file_dialog_is_save = true;
        app.file_path_input.clear();
        app.browser_origin_is_save = true;
        app.browser_cwd = PathBuf::from("/tmp/outdir");
        app.browser_filename.clear();
        app.show_file_browser = true;

        app.browser_use_current_folder();

        assert!(
            app.show_file_browser,
            "empty Name field must not confirm the folder"
        );
        assert!(
            !app.file_path_input.contains("untitled"),
            "must not invent untitled; got {:?}",
            app.file_path_input
        );
        assert_eq!(
            app.browser_button_selected,
            crate::ui::popups::file_browser::FOCUS_FILENAME
        );
        assert!(app
            .browser_error
            .as_deref()
            .is_some_and(|e| e.contains("filename")));
    }

    #[test]
    fn save_browser_open_seeds_filename_from_path_input() {
        let mut app = App::new(&[]);
        app.show_file_dialog(true);
        app.file_path_input = "/old/path/widget.rs".into();
        app.open_file_browser();
        assert_eq!(app.browser_filename, "widget.rs");
    }

    #[test]
    fn save_browser_open_seeds_empty_filename_for_untitled_buffer() {
        let mut app = App::new(&[]);
        app.show_file_dialog(true);
        assert!(app.active_buffer().path.is_none());
        // file_path_input is now pre-filled with last_save_directory + untitled.txt
        // for Save As dialogs, and browser_filename is seeded from it
        app.open_file_browser();
        assert_eq!(
            app.browser_filename, "untitled.txt",
            "new buffers should seed untitled.txt; got {:?}",
            app.browser_filename
        );
    }

    #[test]
    fn open_browser_confirm_still_ignores_directories() {
        let mut app = App::new(&[]);
        app.show_file_dialog = true;
        app.file_dialog_is_save = false;
        app.file_path_input.clear();
        app.browser_origin_is_save = false;
        app.browser_selected = 0;
        app.browser_entries = vec![crate::ui::popups::file_browser::BrowserEntry {
            name: "src".into(),
            is_dir: true,
            path: PathBuf::from("/tmp/src"),
        }];
        app.show_file_browser = true;

        app.close_file_browser(true);

        assert!(
            app.file_path_input.is_empty(),
            "Open must not confirm a directory"
        );
        assert!(!app.show_file_browser);
    }

    #[test]
    fn open_browser_footer_unchanged() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = false;
        // Dir mode: index 1 is the toggle *to* Recent, labelled "[ **Browse** / Recent ]"
        // (shows both options with active view bolded).
        assert_eq!(
            app.browser_footer_labels(),
            [
                "[ Up ]",
                "[ **Browse** / Recent ]",
                "[ Hidden: off ]",
                "[ Cancel ]"
            ]
        );
    }

    #[test]
    fn save_browser_footer_is_use_folder_up_hidden_cancel() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = true;
        assert_eq!(
            app.browser_footer_labels(),
            ["[ Use folder ]", "[ Up ]", "[ Hidden: off ]", "[ Cancel ]"]
        );
    }

    #[test]
    fn browser_use_current_folder_is_noop_for_open() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = false;
        app.file_path_input = "keep-me".into();
        app.browser_cwd = PathBuf::from("/tmp");
        app.show_file_browser = true;
        app.browser_use_current_folder();
        assert!(app.show_file_browser);
        assert_eq!(app.file_path_input, "keep-me");
    }

    #[test]
    fn save_browser_activate_footer_zero_uses_folder() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = true;
        app.browser_filename = "notes.md".into();
        app.browser_cwd = PathBuf::from("/tmp/outdir");
        app.show_file_browser = true;
        app.browser_activate_footer(0);
        assert!(!app.show_file_browser);
        assert_eq!(app.file_path_input, "/tmp/outdir/notes.md");
    }

    #[test]
    fn open_browser_activate_footer_zero_goes_up() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = false;
        app.browser_cwd = PathBuf::from("/tmp/outdir");
        app.browser_activate_footer(0);
        assert_eq!(app.browser_cwd, PathBuf::from("/tmp"));
    }

    #[test]
    fn open_browser_activate_footer_one_toggles_recent() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = false;
        assert_eq!(
            app.browser_mode,
            crate::ui::popups::file_browser::BrowserMode::Dir
        );
        app.browser_activate_footer(1);
        assert_eq!(
            app.browser_mode,
            crate::ui::popups::file_browser::BrowserMode::Recent
        );
    }

    #[test]
    fn open_browser_activate_footer_three_cancels() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = false;
        app.show_file_browser = true;
        app.file_path_input = "keep".into();
        app.browser_activate_footer(3);
        assert!(!app.show_file_browser);
        assert_eq!(app.file_path_input, "keep");
    }

    #[test]
    fn save_browser_activate_footer_one_goes_up() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = true;
        app.browser_cwd = PathBuf::from("/tmp/outdir");
        app.browser_activate_footer(1);
        assert_eq!(app.browser_cwd, PathBuf::from("/tmp"));
    }

    #[test]
    fn save_browser_activate_footer_three_cancels() {
        let mut app = App::new(&[]);
        app.browser_origin_is_save = true;
        app.show_file_browser = true;
        app.file_path_input = "keep".into();
        app.browser_activate_footer(3);
        assert!(!app.show_file_browser);
        assert_eq!(app.file_path_input, "keep");
    }

    #[test]
    fn close_file_browser_file_confirm_focuses_action_button() {
        let mut app = App::new(&[]);
        app.file_dialog_selected = 2;
        app.browser_entries = vec![crate::ui::popups::file_browser::BrowserEntry {
            name: "a.rs".into(),
            is_dir: false,
            path: PathBuf::from("/tmp/a.rs"),
        }];
        app.browser_selected = 0;
        app.show_file_browser = true;
        app.close_file_browser(true);
        assert_eq!(app.file_path_input, "/tmp/a.rs");
        assert_eq!(app.file_dialog_selected, 0);
    }
}
