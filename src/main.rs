//! rust-edit — terminal text & code editor (Ratatui TUI)
//! Phase 1 implementation per Design Spec + AGENTS.md

#![allow(unreachable_patterns)]

mod app;
mod buffer;
mod config;
mod syntax;
mod theme;
mod ui;
mod utils;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::Rect,
    widgets::Clear,
    Frame, Terminal,
};
use std::io::stdout;
use std::time::Duration;

use crate::app::App;
use crate::buffer::Buffer;
use crate::ui::popups;

fn main() -> Result<()> {
    // CLI support: rust-edit [file ...]
    let files: Vec<String> = std::env::args().skip(1).collect();

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&files);
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        // Keep each buffer's notion of the viewport height fresh so that the
        // internal adjust_scroll (run on edits/moves) clamps against the *real*
        // height instead of a stale default. Fixes half-screen scroll jumping.
        let vis = app.editor_visible_lines.get().max(1);
        for b in app.buffers.iter_mut() {
            b.visible_lines = vis;
        }

        terminal.hide_cursor().ok();
        terminal
            .draw(|f| ui(f, app))
            .map_err(|e| anyhow::anyhow!("draw error: {}", e))?;

        // Position the real terminal cursor (after draw) so it's always visible at the exact
        // position, including on whitespace, spaces, and end-of-line. Hide it for popups/modals.
        if app.show_help || app.show_word_count || app.show_settings || app.active_menu.is_some() {
            terminal.hide_cursor().ok();
        } else {
            let buf = app.active_buffer();
            let (cur_row, cur_col) = buf.cursor_position();
            let scroll_row = buf.scroll_offset.0;
            let hscroll = buf.scroll_offset.1;
            if cur_row >= scroll_row {
                let view_row = cur_row - scroll_row;
                let screen_y = 1u16 + view_row as u16; // after header
                                                       // Hardware cursor must compensate hscroll: the drawn content starts at logical hscroll.
                let content_col = cur_col.saturating_sub(hscroll);
                let screen_x = Buffer::GUTTER_WIDTH as u16 + content_col as u16;
                terminal.set_cursor_position((screen_x, screen_y)).ok();
                terminal.show_cursor().ok();
            } else {
                terminal.hide_cursor().ok();
            }
        }

        // Block for the first event, then drain anything already queued without
        // blocking. Consecutive mouse-wheel events are collapsed into a single
        // net scroll applied once, so a scroll burst produces one redraw instead
        // of one per wheel notch — removing the seconds-of-lag backlog and CPU
        // spike on large files. Key/mouse-click events are never dropped: they
        // are applied in arrival order.
        let first = event::read()?;
        let mut events = vec![first];
        while event::poll(Duration::ZERO)? {
            events.push(event::read()?);
        }
        // Apply the accumulated wheel delta directly (one scroll op), then drop
        // all wheel events so only key/click/other events remain for the dispatch.
        let wheel_delta = events.iter().filter_map(wheel_lines).sum::<isize>();
        if wheel_delta != 0 {
            let visible = app.editor_visible_lines.get().max(5);
            app.active_buffer_mut().scroll(wheel_delta);
            app.active_buffer_mut().clamp_scroll(visible);
        }
        events.retain(|e| wheel_lines(e).is_none());

        for event in events {
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('q') => {
                                app.request_quit();
                            }
                            KeyCode::Char('s') => {
                                app.save_current();
                            }
                            KeyCode::Char('o') => {
                                app.show_file_dialog(false);
                            }
                            KeyCode::Char('S') => {
                                app.show_file_dialog(true);
                            }
                            KeyCode::Char('n') => app.new_tab(),
                            KeyCode::Char('w') => app.close_current_tab(),
                            KeyCode::Char('c') => {
                                app.copy_selection_or_line();
                            }
                            KeyCode::Char('v') => {
                                app.paste();
                            }
                            KeyCode::Char('x') => {
                                app.copy_selection_or_line();
                                if app.active_buffer().selection.is_some() {
                                    app.active_buffer_mut().delete();
                                }
                            }
                            KeyCode::Char('f') if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                                let was_open = app.is_find_active();
                                app.toggle_find();
                                if !was_open {
                                    if let Some(fs) = &mut app.find_state {
                                        fs.editing_replace = false;
                                    }
                                }
                            }
                            KeyCode::Char('h') => {
                                let was_open = app.is_find_active();
                                app.toggle_find();
                                if !was_open {
                                    if let Some(fs) = &mut app.find_state {
                                        fs.editing_replace = true;
                                    }
                                }
                            }
                            KeyCode::Char('a') => {
                                if app.is_find_active() {
                                    app.replace_all();
                                }
                            }
                            KeyCode::Char('p') => {
                                app.toggle_markdown_preview();
                            }
                            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                let _ = app.format_current_document();
                            }
                            // horizontal scroll
                            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.active_buffer_mut().scroll_offset.1 =
                                    app.active_buffer_mut().scroll_offset.1.saturating_add(4);
                            }
                            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.active_buffer_mut().scroll_offset.1 =
                                    app.active_buffer_mut().scroll_offset.1.saturating_sub(4);
                            }
                            KeyCode::Char('z') => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.redo();
                                } else {
                                    app.undo();
                                }
                            }
                            KeyCode::Char('y') => {
                                app.redo();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('?') => {
                            app.show_help = !app.show_help;
                        }
                        KeyCode::Char('W') => {
                            app.compute_and_show_word_count();
                        }
                        // Alt+letter for menus (opens dropdown)
                        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
                            app.open_menu("File");
                        }
                        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::ALT) => {
                            app.open_menu("Tools");
                        }
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::ALT) => {
                            app.open_menu("Settings");
                        }
                        KeyCode::Char(d)
                            if ('1'..='9').contains(&d)
                                && key.modifiers.contains(KeyModifiers::ALT) =>
                        {
                            let idx = (d as u8 - b'1') as usize;
                            if idx < app.buffers.len() {
                                app.active = idx;
                            }
                        }
                        KeyCode::Tab if app.is_find_active() => {
                            if let Some(fs) = &mut app.find_state {
                                fs.editing_replace = !fs.editing_replace;
                            }
                        }
                        KeyCode::F(3) if app.is_find_active() => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.find_prev();
                            } else {
                                app.find_next();
                            }
                        }
                        // n/N always navigate matches when find bar open (from either box).
                        // This prioritizes nav discoverability; note that literal 'n'/'N' cannot be typed
                        // into the query/repl fields while the bar is active (classic tradeoff; use F3 or seed via selection).
                        KeyCode::Char('n') if app.is_find_active() => {
                            app.find_next();
                        }
                        KeyCode::Char('N') if app.is_find_active() => {
                            app.find_prev();
                        }
                        KeyCode::Char('r')
                            if app
                                .find_state
                                .as_ref()
                                .is_some_and(|fs| !fs.editing_replace) =>
                        {
                            app.replace_current();
                        }
                        KeyCode::Enter if app.is_find_active() => {
                            app.replace_current();
                        }
                        KeyCode::Char(ch) if app.is_find_active() => {
                            // route typing to focused field (query or replace)
                            app.update_find_input(Some(ch), false);
                        }
                        KeyCode::Backspace if app.is_find_active() => {
                            app.update_find_input(None, true);
                        }
                        KeyCode::Char('c')
                            if app
                                .find_state
                                .as_ref()
                                .is_some_and(|fs| !fs.editing_replace) =>
                        {
                            if let Some(fs) = &mut app.find_state {
                                fs.case_sensitive = !fs.case_sensitive;
                                app.update_find_matches();
                            }
                        }
                        KeyCode::Char('x')
                            if app
                                .find_state
                                .as_ref()
                                .is_some_and(|fs| !fs.editing_replace) =>
                        {
                            if let Some(fs) = &mut app.find_state {
                                fs.use_regex = !fs.use_regex;
                                app.update_find_matches();
                            }
                        }
                        KeyCode::Char('w')
                            if app
                                .find_state
                                .as_ref()
                                .is_some_and(|fs| !fs.editing_replace) =>
                        {
                            if let Some(fs) = &mut app.find_state {
                                fs.whole_word = !fs.whole_word;
                                app.update_find_matches();
                            }
                        }
                        // MD formatting toolbar (Sprint 2)
                        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if app.active_buffer().language == crate::buffer::Language::Markdown {
                                app.wrap_selection_or_cursor("**", "**");
                            }
                        }
                        KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if app.active_buffer().language == crate::buffer::Language::Markdown {
                                app.wrap_selection_or_cursor("*", "*");
                            }
                        }
                        KeyCode::Char('`') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if app.active_buffer().language == crate::buffer::Language::Markdown {
                                app.wrap_selection_or_cursor("`", "`");
                            }
                        }
                        KeyCode::Esc if app.is_find_active() => {
                            app.clear_find();
                        }
                        KeyCode::Esc if app.show_word_count => {
                            app.close_word_count();
                        }
                        KeyCode::Esc if app.show_settings => {
                            app.close_settings();
                        }
                        KeyCode::Esc if app.show_confirm => {
                            app.close_confirm(false);
                        }
                        KeyCode::Esc if app.show_file_dialog => {
                            app.close_file_dialog(false);
                        }
                        KeyCode::Char('y' | 'Y') if app.show_confirm => {
                            let is_quit = app.confirm_action.as_deref() == Some("quit");
                            app.close_confirm(true);
                            if is_quit {
                                app.should_quit = true;
                                return Ok(());
                            }
                        }
                        KeyCode::Char('n' | 'N') if app.show_confirm => {
                            app.close_confirm(false);
                        }
                        KeyCode::Char(ch) if app.show_file_dialog => {
                            app.file_dialog_input_char(ch);
                        }
                        KeyCode::Backspace if app.show_file_dialog => {
                            app.file_dialog_backspace();
                        }
                        KeyCode::Enter if app.show_file_dialog => {
                            app.close_file_dialog(true);
                        }
                        KeyCode::Enter if app.show_confirm => {
                            app.close_confirm(true);
                        }
                        KeyCode::Char('j') | KeyCode::Down if app.show_settings => {
                            let n = crate::theme::Theme::built_ins().len();
                            app.settings_selected = (app.settings_selected + 1) % n;
                        }
                        KeyCode::Char('k') | KeyCode::Up if app.show_settings => {
                            let n = crate::theme::Theme::built_ins().len();
                            app.settings_selected = if app.settings_selected == 0 {
                                n - 1
                            } else {
                                app.settings_selected - 1
                            };
                        }
                        KeyCode::Enter if app.show_settings => {
                            app.apply_settings_theme(app.settings_selected);
                        }
                        // Dropdown menu keyboard navigation
                        KeyCode::Esc if app.active_menu.is_some() => {
                            app.close_menu();
                        }
                        KeyCode::Char('j') | KeyCode::Down if app.active_menu.is_some() => {
                            let len = match app.active_menu.as_deref() {
                                Some("File") => 6,
                                Some("Tools") => 4,
                                Some("Settings") => 2,
                                _ => 1,
                            };
                            app.menu_selected = (app.menu_selected + 1) % len;
                        }
                        KeyCode::Char('k') | KeyCode::Up if app.active_menu.is_some() => {
                            let len = match app.active_menu.as_deref() {
                                Some("File") => 6,
                                Some("Tools") => 4,
                                Some("Settings") => 2,
                                _ => 1,
                            };
                            app.menu_selected = if app.menu_selected == 0 {
                                len - 1
                            } else {
                                app.menu_selected - 1
                            };
                        }
                        KeyCode::Enter if app.active_menu.is_some() => {
                            handle_menu_action(app);
                            app.close_menu();
                        }
                        KeyCode::Char(ch) => {
                            app.active_buffer_mut().insert_char(ch);
                        }
                        KeyCode::Enter => {
                            app.active_buffer_mut().newline_with_indent();
                        }
                        KeyCode::Backspace => {
                            app.active_buffer_mut().backspace();
                        }
                        KeyCode::Delete => {
                            app.active_buffer_mut().delete();
                        }
                        KeyCode::Left => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            app.active_buffer_mut().move_cursor(-1, extend);
                        }
                        KeyCode::Right => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            app.active_buffer_mut().move_cursor(1, extend);
                        }
                        KeyCode::Up => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            let vis = app.editor_visible_lines.get().max(1);
                            app.active_buffer_mut().move_lines_up(1, extend);
                            app.active_buffer_mut().ensure_cursor_visible(vis);
                        }
                        KeyCode::Down => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            let vis = app.editor_visible_lines.get().max(1);
                            app.active_buffer_mut().move_lines_down(1, extend);
                            app.active_buffer_mut().ensure_cursor_visible(vis);
                        }
                        KeyCode::Home => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            app.active_buffer_mut().move_home(extend);
                        }
                        KeyCode::End => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            app.active_buffer_mut().move_end(extend);
                        }
                        KeyCode::PageUp => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            let visible = app.editor_visible_lines.get().max(1);
                            app.active_buffer_mut().page_up(visible, extend);
                            app.active_buffer_mut().ensure_cursor_visible(visible);
                        }
                        KeyCode::PageDown => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            let visible = app.editor_visible_lines.get().max(1);
                            app.active_buffer_mut().page_down(visible, extend);
                            app.active_buffer_mut().ensure_cursor_visible(visible);
                        }
                        KeyCode::Tab => {
                            app.active_buffer_mut().indent();
                        }
                        KeyCode::Esc => {
                            app.active_buffer_mut().selection = None;
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => handle_mouse(mouse, app),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

/// Signed scroll contribution of an event in lines (down positive), or None if
/// it isn't a wheel event. Used to coalesce a burst of wheel events into one
/// net delta before a single redraw.
fn wheel_lines(event: &Event) -> Option<isize> {
    if let Event::Mouse(m) = event {
        match m.kind {
            MouseEventKind::ScrollUp => Some(-3),
            MouseEventKind::ScrollDown => Some(3),
            _ => None,
        }
    } else {
        None
    }
}

fn handle_mouse(mouse: MouseEvent, app: &mut App) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let x = mouse.column as usize;
            let y = mouse.row as usize;

            let mut skip_editor = false;

            // === Dropdown menu clicks ===
            if let Some(ref menu_name) = app.active_menu {
                let (menu_x, num_items) = match menu_name.as_str() {
                    "File" => (1usize, 6),
                    "Tools" => (8, 4),
                    "Settings" => (15, 2),
                    _ => (0, 0),
                };
                let dy = 1usize;
                let dropdown_h = num_items + 2;
                let in_dropdown_y = y >= dy && y < dy + dropdown_h;
                let in_dropdown_x = x >= menu_x && x < menu_x + 30;
                if in_dropdown_y && in_dropdown_x {
                    let item_row = y.saturating_sub(dy).saturating_sub(1);
                    if item_row < num_items {
                        app.menu_selected = item_row;
                        handle_menu_action(app);
                        app.close_menu();
                        skip_editor = true;
                    } else {
                        app.close_menu();
                        skip_editor = true;
                    }
                } else {
                    // click outside the open dropdown -> close it
                    app.close_menu();
                    // allow fallthrough for editor click etc.
                }
            }

            // === Interactive popup clicks (e.g. Settings themes) + outside close for popups ===
            if app.show_settings && !skip_editor {
                // Duplicate rect calc (same as render_settings_popup)
                let w = 45u16;
                let h = 14u16;
                let term_w = 120u16; // reasonable default
                let px = (term_w.saturating_sub(w)) / 2;
                let py = (20u16.saturating_sub(h)) / 2; // rough center
                let in_popup = (y as u16 >= py && (y as u16) < py + h)
                    && (x as u16 >= px && (x as u16) < px + w);
                if in_popup {
                    let rel_y = (y as u16 - py) as usize;
                    // themes at approx rel_y 3,4,5
                    if (3..6).contains(&rel_y) {
                        let idx = rel_y - 3;
                        app.settings_selected = idx;
                        app.apply_settings_theme(idx);
                        skip_editor = true;
                    } else {
                        skip_editor = true; // clicked inside but non-item
                    }
                } else {
                    app.close_settings();
                    skip_editor = true;
                }
            }

            // Close other non-interactive popups on outside click
            if !skip_editor && (app.show_help || app.show_word_count) {
                // simple: any click not in editor? but to avoid complexity close on any click outside header
                if y > 0 {
                    if app.show_help {
                        app.show_help = false;
                    }
                    if app.show_word_count {
                        app.show_word_count = false;
                    }
                    skip_editor = false; // allow the click to still place cursor
                }
            }

            if !skip_editor {
                if y == 0 {
                    // Header row: tabs, menus or [+]
                    handle_header_click(x, app);
                } else {
                    // Editor area (header takes y=0)
                    let editor_start_y = 1usize; // after header
                    if y >= editor_start_y {
                        let editor_y = y - editor_start_y;
                        let vis = app.editor_visible_lines.get().max(1);
                        if editor_y < vis {
                            let gutter = Buffer::GUTTER_WIDTH;
                            if x >= gutter {
                                let hscroll = app.active_buffer().scroll_offset.1;
                                // visual_col must account for hscroll: screen col 0 in content == logical hscroll
                                let visual_col = (x - gutter) + hscroll;
                                let visual_line = app.active_buffer().scroll_offset.0 + editor_y;
                                let buf = app.active_buffer_mut();
                                buf.set_cursor_from_visual(visual_line, visual_col);
                                // Do not force recenter here (ensure only if offscreen); set_visual no longer adjusts.
                                buf.ensure_cursor_visible(vis);
                            }
                        }
                        // clicks in find bar / padded / status area are ignored for cursor placement
                    }
                }
            }
        }
        // Mouse-wheel scrolling is coalesced and applied directly in run_app
        // (one net delta per burst → one redraw), so wheel events never reach
        // handle_mouse. These arms are intentionally absent there.
        MouseEventKind::Drag(MouseButton::Left) => {
            // Basic drag to extend selection
            let x = mouse.column as usize;
            let y = mouse.row as usize;
            if y > 0 {
                let editor_y = y - 1;
                let gutter = Buffer::GUTTER_WIDTH;
                if x >= gutter {
                    let visual_col = x - gutter;
                    let visual_line = app.active_buffer().scroll_offset.0 + editor_y;
                    let buf = app.active_buffer_mut();
                    // compute target pos and extend selection from current or start
                    let target_byte = buf.byte_at_visual(visual_line, visual_col);
                    if let Some(sel) = &mut buf.selection {
                        if buf.cursor < sel.start {
                            sel.start = target_byte;
                        } else {
                            sel.end = target_byte;
                        }
                    } else {
                        let start = buf.cursor.min(target_byte);
                        let end = buf.cursor.max(target_byte);
                        buf.selection = Some(start..end);
                    }
                    buf.cursor = target_byte;
                }
            }
        }
        _ => {}
    }
}

fn handle_header_click(x: usize, app: &mut App) {
    // Menus: " File  Tools  Settings  "
    // Approximate char positions for click detection
    let menus = [("File", 0, 6), ("Tools", 7, 13), ("Settings", 14, 23)];

    for (name, start, end) in menus {
        if x >= start && x < end {
            app.open_menu(name);
            return;
        }
    }

    // If a menu is open and click outside menus, close it (but still check tabs)
    if app.active_menu.is_some() {
        app.close_menu();
    }

    // Tabs start roughly after menus
    let mut pos = 24usize; // after "Settings  "
    for i in 0..app.buffers.len() {
        let buf = &app.buffers[i];
        let mut name = buf
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[untitled]")
            .to_string();
        if buf.is_modified {
            name.push('•');
        }
        let tab_width = name.len() + 2; // " name "
        if x >= pos && x < pos + tab_width {
            app.active = i;
            app.close_menu();
            return;
        }
        pos += tab_width;
    }

    // [+] button
    if x >= pos && x < pos + 5 {
        app.new_tab();
        app.close_menu();
    }
}

fn handle_menu_action(app: &mut App) {
    let menu = match &app.active_menu {
        Some(m) => m.as_str(),
        None => return,
    };
    let idx = app.menu_selected;
    match (menu, idx) {
        ("File", 0) => app.new_tab(),               // New
        ("File", 1) => app.show_file_dialog(false), // Open
        ("File", 2) => {
            let _ = app.save_current();
        }
        ("File", 3) => app.show_file_dialog(true), // Save As
        ("File", 4) => app.close_current_tab(),
        ("File", 5) => app.request_quit(), // Quit
        ("Tools", 0) => {
            // Find (focus query)
            if app.find_state.is_none() {
                app.toggle_find();
            }
            if let Some(fs) = &mut app.find_state {
                fs.editing_replace = false;
            }
        }
        ("Tools", 1) => {
            // Replace (focus replace field)
            if app.find_state.is_none() {
                app.toggle_find();
            }
            if let Some(fs) = &mut app.find_state {
                fs.editing_replace = true;
            }
        }
        ("Tools", 2) => app.compute_and_show_word_count(),
        ("Tools", 3) => {
            let _ = app.format_current_document();
        }
        ("Settings", 0) => app.open_settings(),
        ("Settings", 1) => app.open_settings(), // themes
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = crate::ui::layout::main_layout(area);

    // Update editor viewport height for PgUp/PgDn (subtract find bar when present; approx for md split).
    let base_h = chunks[1].height as usize;
    let vis_h = if app.is_find_active() {
        base_h.saturating_sub(1)
    } else {
        base_h
    };
    let vis_h = if app.markdown_preview
        && app.active_buffer().language == crate::buffer::Language::Markdown
    {
        (vis_h / 2).max(1)
    } else {
        vis_h.max(1)
    };
    app.editor_visible_lines.set(vis_h);

    // Refresh the cached syntax spans for the visible window. This is a cheap
    // key comparison on a steady frame and a full (single-HighlightLines) recompute
    // only on edit / scroll / language change. Done here so editor::render can read
    // the cache immutably. Skipped for the markdown preview path (no syntax spans).
    if !(app.markdown_preview && app.active_buffer().language == crate::buffer::Language::Markdown)
    {
        app.refresh_highlight_cache(app.active_buffer().scroll_offset.0, vis_h);
    }

    crate::ui::header::render(f, chunks[0], app);

    let edit_chunk = chunks[1];
    if app.is_find_active() {
        let (edit_area, find_area) = crate::ui::layout::editor_with_find_layout(edit_chunk);
        if app.markdown_preview && app.active_buffer().language == crate::buffer::Language::Markdown
        {
            let (src, prev) = crate::ui::layout::md_split_preview(edit_area);
            // render source in left, preview in right (simplified, no gutter on preview)
            crate::ui::editor::render(f, src, app);
            // simple preview render on right (re-use preview logic)
            let md_lines = crate::ui::markdown::render_preview(
                &app.active_buffer().text(),
                app.current_theme(),
            );
            let para = ratatui::widgets::Paragraph::new(md_lines)
                .style(app.current_theme().editor_bg_style());
            f.render_widget(para, prev);
        } else {
            crate::ui::editor::render(f, edit_area, app);
        }
        if let Some(fs) = &app.find_state {
            crate::ui::find::render_bar(f, find_area, fs);
        }
        crate::ui::status::render(f, chunks[2], app);
    } else if app.markdown_preview
        && app.active_buffer().language == crate::buffer::Language::Markdown
    {
        let (src, prev) = crate::ui::layout::md_split_preview(edit_chunk);
        crate::ui::editor::render(f, src, app);
        let md_lines =
            crate::ui::markdown::render_preview(&app.active_buffer().text(), app.current_theme());
        let para =
            ratatui::widgets::Paragraph::new(md_lines).style(app.current_theme().editor_bg_style());
        f.render_widget(para, prev);
        crate::ui::status::render(f, chunks[2], app);
    } else {
        crate::ui::editor::render(f, edit_chunk, app);
        crate::ui::status::render(f, chunks[2], app);
    }

    if app.show_help {
        render_help_overlay(f, area);
    }
    if app.show_word_count {
        popups::word_count::render(f, area, app.current_theme(), &app.word_count_text);
    }

    if let Some(ref menu) = app.active_menu {
        crate::ui::header::render_dropdown(f, menu, area, app.current_theme(), app.menu_selected);
    }

    if app.show_settings {
        popups::settings::render(f, area, app.current_theme(), app.settings_selected);
    }

    if app.show_confirm {
        popups::confirm::render_confirm(f, area, app.current_theme(), &app.confirm_message);
    }

    if app.show_file_dialog {
        popups::file_dialog::render(
            f,
            area,
            app.current_theme(),
            app.file_dialog_is_save,
            &app.file_path_input,
            &app.file_dialog_message,
        );
    }
}

fn render_help_overlay(f: &mut Frame, area: Rect) {
    // TODO: move to ui/popups/help.rs once popup infra is ready
    let help_text = vec![
        ratatui::text::Line::from(" rust-edit — Phase 1 Help "),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(" Ctrl+S  Save     Ctrl+N New tab    Ctrl+W Close tab"),
        ratatui::text::Line::from(" Ctrl+Q  Quit     Arrows + Shift select    PgUp/PgDn page"),
        ratatui::text::Line::from(" Ctrl+Z Undo   Ctrl+Y Redo     (Shift+Z also redo)"),
        ratatui::text::Line::from(" Enter   Smart indent     Tab / Shift+Tab  indent/dedent"),
        ratatui::text::Line::from(" Home/End     Backspace/Delete"),
        ratatui::text::Line::from(
            " Ctrl+F find / Ctrl+H replace    n/N or F3/Shift+F3 nav  r/Enter rep  ^A all  Tab focus",
        ),
        ratatui::text::Line::from(" Ctrl+B/I/` for MD bold/italic/code"),
        ratatui::text::Line::from(" Shift+W word count    Ctrl+Shift+F format"),
        ratatui::text::Line::from(" Mouse: tabs / [+] / editor / dropdowns / popups / scroll"),
        ratatui::text::Line::from(" Alt+F/T/S menus    Alt+1-9 tabs    ? help"),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(" (Full keybindings & mouse in later slices)"),
        ratatui::text::Line::from(" Press ? or Esc to close"),
    ];

    let block = ratatui::widgets::Block::default()
        .title("Help")
        .borders(ratatui::widgets::Borders::ALL);
    let para = ratatui::widgets::Paragraph::new(help_text).block(block);

    let popup_width = 50u16;
    let popup_height = 12u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(
        x,
        y,
        popup_width.min(area.width),
        popup_height.min(area.height),
    );

    f.render_widget(Clear, popup_area);
    f.render_widget(para, popup_area);
}
