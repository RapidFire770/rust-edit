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
mod wrap;

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
    Frame, Terminal,
};
use std::io::stdout;
use std::time::Duration;

use crate::app::App;
use crate::buffer::Buffer;
use crate::ui::find::FindFocus;
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
        // Keep each buffer's notion of the viewport fresh so that the internal
        // adjust_scroll (run on edits/moves) clamps against the *real* height and
        // width instead of stale defaults. Fixes half-screen scroll jumping and
        // adds horizontal auto-scroll that follows the cursor on long lines.
        let vis = app.editor_visible_lines.get().max(1);
        let hwidth = app.editor_visible_width.get().max(1);
        for b in app.buffers.iter_mut() {
            b.visible_lines = vis;
            b.visible_width = hwidth;
        }

        terminal.hide_cursor().ok();
        terminal
            .draw(|f| ui(f, app))
            .map_err(|e| anyhow::anyhow!("draw error: {}", e))?;

        // Position the real terminal cursor (after draw) so it's always visible at the exact
        // position, including on whitespace, spaces, and end-of-line. Hide it for popups/modals.
        if app.show_help || app.show_word_count || app.show_settings || app.active_menu.is_some() {
            terminal.hide_cursor().ok();
        } else if app.config.word_wrap {
            let buf = app.active_buffer();
            let layout = crate::wrap::compute_layout(
                &buf.rope,
                0,
                buf.line_count(),
                buf.visible_width.max(1),
                app.config.tab_size,
            );
            let cursor_screen_row =
                crate::wrap::screen_row_of_byte(&layout, &buf.rope, 0, buf.cursor);
            let scroll_row = buf.scroll_offset.0;
            if cursor_screen_row >= scroll_row {
                let view_row = cursor_screen_row - scroll_row;
                let screen_y = crate::ui::layout::HEADER_ROWS + view_row as u16;
                // Column within the sub-row: distance from the sub-row's start byte to the cursor.
                let (line_idx, col_in_line) = buf.cursor_position();
                let line_first_byte = buf.rope.line_to_byte(line_idx);
                // Find the sub-row containing the cursor.
                let line_breaks = layout.breaks.get(line_idx).cloned().unwrap_or_default();
                let mut sub_row_start_byte = line_first_byte;
                for &brk in &line_breaks {
                    if buf.cursor < brk {
                        break;
                    }
                    sub_row_start_byte = brk;
                }
                let col_offset = buf
                    .rope
                    .byte_slice(sub_row_start_byte..buf.cursor)
                    .to_string()
                    .chars()
                    .count();
                let _ = col_in_line;
                let screen_x = Buffer::GUTTER_WIDTH as u16 + col_offset as u16;
                terminal.set_cursor_position((screen_x, screen_y)).ok();
                terminal.show_cursor().ok();
            } else {
                terminal.hide_cursor().ok();
            }
        } else {
            let buf = app.active_buffer();
            let (cur_row, cur_col) = buf.cursor_position();
            let scroll_row = buf.scroll_offset.0;
            let hscroll = buf.scroll_offset.1;
            if cur_row >= scroll_row {
                let view_row = cur_row - scroll_row;
                // Header now occupies HEADER_ROWS (menu + separator rule).
                let screen_y = crate::ui::layout::HEADER_ROWS + view_row as u16;
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
        //
        // Edge auto-scroll: when a mouse-drag selection is in progress and the
        // pointer sits at a viewport edge, crossterm emits no events while the
        // mouse is held still, so we switch to a short bounded poll. On timeout
        // we apply one scroll step + re-extend the selection, then loop. When
        // not dragging-at-edge we keep the original blocking semantics (idle CPU
        // stays at zero).
        if any_modal_open(app) {
            app.mouse_drag_active = false;
            app.scrollbar_drag_active = false;
        }
        let edge = if app.mouse_drag_active {
            drag_edge_delta(app)
        } else {
            None
        };
        let first = if let Some((drow, dcol)) = edge {
            if event::poll(Duration::from_millis(30))? {
                event::read()?
            } else {
                // Timed out: no input arrived, but we're dragging at an edge.
                // Advance the viewport one step and re-extend the selection.
                let vis = app.editor_visible_lines.get().max(1);
                if drow != 0 {
                    if app.config.word_wrap {
                        let buf = app.active_buffer();
                        let total = crate::wrap::total_screen_rows(
                            &buf.rope,
                            buf.visible_width,
                            app.config.tab_size,
                        );
                        app.active_buffer_mut().scroll_wrapped(drow, total);
                    } else {
                        app.active_buffer_mut().scroll(drow);
                        app.active_buffer_mut().clamp_scroll(vis);
                    }
                }
                if dcol != 0 && !app.config.word_wrap {
                    let cur = app.active_buffer_mut().scroll_offset.1;
                    app.active_buffer_mut().scroll_offset.1 = if dcol < 0 {
                        cur.saturating_sub(1)
                    } else {
                        cur.saturating_add(1)
                    };
                }
                extend_drag_to_mouse(app);
                continue;
            }
        } else {
            event::read()?
        };
        let mut events = vec![first];
        while event::poll(Duration::ZERO)? {
            events.push(event::read()?);
        }
        // Apply the accumulated wheel delta directly (one scroll op), then drop
        // all wheel events so only key/click/other events remain for the dispatch.
        let wheel_delta = events.iter().filter_map(wheel_lines).sum::<isize>();
        if wheel_delta != 0 {
            if app.show_file_browser {
                // Route wheel to the browser list, not the editor underneath.
                app.browser_move_cursor(wheel_delta as i32);
            } else if app.show_help {
                // Route wheel to help popup content scroll instead of buffer.
                let new_row = (app.help_content_scroll.0 as isize + wheel_delta).max(0) as u16;
                app.help_content_scroll.0 = new_row;
            } else if app.config.word_wrap {
                let buf = app.active_buffer();
                let total = crate::wrap::total_screen_rows(
                    &buf.rope,
                    buf.visible_width,
                    app.config.tab_size,
                );
                let b = app.active_buffer_mut();
                b.scroll_wrapped(wheel_delta, total);
            } else {
                let visible = app.editor_visible_lines.get().max(5);
                app.active_buffer_mut().scroll(wheel_delta);
                app.active_buffer_mut().clamp_scroll(visible);
            }
        }
        events.retain(|e| wheel_lines(e).is_none());

        for event in events {
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            // Ctrl+H toggles hidden files in the browser. Must
                            // live HERE, not in the browser key block below:
                            // this outer CONTROL block swallows every Ctrl+char
                            // via its `continue`, so a Ctrl+H arm further down
                            // would be unreachable (it was — see git history).
                            KeyCode::Char('h') if app.show_file_browser => {
                                app.browser_toggle_hidden();
                            }
                            KeyCode::Char('q') => {
                                app.request_quit();
                            }
                            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::ALT) => {
                                // Ctrl+Alt+S: Save As. Placed before plain Ctrl+S so the
                                // ALT variant doesn't fall through to save_current().
                                app.show_file_dialog(true);
                            }
                            KeyCode::Char('s') => {
                                app.save_current();
                            }
                            KeyCode::Char('o') => {
                                app.show_file_dialog(false);
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
                                // Single Find/Replace shortcut (Ctrl+F). The bar has
                                // separate Query/Replace fields reachable via Tab, so a
                                // second Ctrl+H shortcut is no longer needed.
                                let was_open = app.is_find_active();
                                app.toggle_find();
                                if !was_open {
                                    if let Some(fs) = &mut app.find_state {
                                        fs.focus = FindFocus::Query;
                                    }
                                }
                            }
                            KeyCode::Char('a') => {
                                if app.is_find_active() {
                                    app.replace_all();
                                } else {
                                    app.active_buffer_mut().select_all();
                                }
                            }
                            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                let _ = app.format_current_document();
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
                        // ----- File browser popup keys (takes precedence over
                        // everything else when open). -----
                        //
                        // Focus model: `browser_button_selected` holds either a
                        // button index (0..=3) or the sentinel `4` meaning the
                        // list area is focused. Tab cycles list -> 0 -> 1 -> 2
                        // -> 3 -> list. Enter activates whichever is focused.
                        //
                        // NOTE: Ctrl+H (toggle hidden) is handled up in the
                        // CONTROL block — adding it here would be dead code.
                        KeyCode::Up | KeyCode::Char('k') if app.show_file_browser => {
                            app.browser_move_cursor(-1);
                            app.browser_button_selected = 4; // typing nav refocuses list
                            continue;
                        }
                        KeyCode::Down | KeyCode::Char('j') if app.show_file_browser => {
                            app.browser_move_cursor(1);
                            app.browser_button_selected = 4;
                            continue;
                        }
                        KeyCode::Char('g') if app.show_file_browser => {
                            app.browser_jump(false);
                            continue;
                        }
                        KeyCode::Char('G') if app.show_file_browser => {
                            app.browser_jump(true);
                            continue;
                        }
                        KeyCode::PageUp if app.show_file_browser => {
                            let vh = app.editor_visible_lines.get().max(4);
                            app.browser_page(true, vh);
                            continue;
                        }
                        KeyCode::PageDown if app.show_file_browser => {
                            let vh = app.editor_visible_lines.get().max(4);
                            app.browser_page(false, vh);
                            continue;
                        }
                        KeyCode::Tab if app.show_file_browser => {
                            // list(4) -> 0 -> 1 -> 2 -> 3 -> list(4)
                            app.browser_button_selected = match app.browser_button_selected {
                                4 => 0,
                                n if n < 3 => n + 1,
                                _ => 4,
                            };
                            continue;
                        }
                        KeyCode::Enter if app.show_file_browser => {
                            match app.browser_button_selected {
                                0 => app.browser_go_up(),
                                1 => app.browser_toggle_mode(),
                                2 => app.browser_toggle_hidden(),
                                3 => app.close_file_browser(false),
                                _ => app.browser_enter_selected(),
                            }
                            continue;
                        }
                        // `l`/Right activate the focused thing (list entry or button).
                        KeyCode::Char('l') | KeyCode::Right if app.show_file_browser => {
                            match app.browser_button_selected {
                                0 => app.browser_go_up(),
                                1 => app.browser_toggle_mode(),
                                2 => app.browser_toggle_hidden(),
                                3 => app.close_file_browser(false),
                                _ => app.browser_enter_selected(),
                            }
                            continue;
                        }
                        // `h`/Left/Backspace/- go up one directory. (Plain `h`
                        // only — Ctrl+H is handled up in the CONTROL block.)
                        KeyCode::Backspace
                        | KeyCode::Char('-')
                        | KeyCode::Left
                        | KeyCode::Char('h')
                            if app.show_file_browser =>
                        {
                            app.browser_go_up();
                            continue;
                        }
                        KeyCode::Esc | KeyCode::Char('q') if app.show_file_browser => {
                            app.close_file_browser(false);
                            continue;
                        }
                        // While the browser is open, swallow every other key so
                        // unbound chars (e.g. 'x', 'f') don't leak into the
                        // underlying file dialog's path field.
                        _ if app.show_file_browser => {
                            continue;
                        }

                        // File dialog button navigation
                        KeyCode::Left if app.show_file_dialog => {
                            // Cycle 0 -> 2 -> 1 -> 0 (right-to-left visual order).
                            app.file_dialog_selected = match app.file_dialog_selected {
                                0 => 2,
                                2 => 1,
                                _ => 0,
                            };
                            continue;
                        }
                        KeyCode::Right if app.show_file_dialog => {
                            // Cycle 0 -> 1 -> 2 -> 0.
                            app.file_dialog_selected = match app.file_dialog_selected {
                                0 => 1,
                                1 => 2,
                                _ => 0,
                            };
                            continue;
                        }

                        // Confirm dialog button navigation
                        KeyCode::Left if app.show_confirm => {
                            app.confirm_selected = 0;
                            continue;
                        }
                        KeyCode::Right if app.show_confirm => {
                            app.confirm_selected = 1;
                            continue;
                        }

                        // Modal popups take focus precedence over everything else
                        // (find nav, editor typing). When a confirm or file dialog is
                        // open, ONLY its keys are accepted — this fixes the bug where
                        // the find bar's n/N/Y intercepted the confirm dialog.
                        KeyCode::Char('y' | 'Y') if app.show_confirm => {
                            let is_quit = app.confirm_action.as_deref() == Some("quit");
                            app.close_confirm(true);
                            if is_quit {
                                app.should_quit = true;
                                return Ok(());
                            }
                            continue;
                        }
                        KeyCode::Char('n' | 'N') if app.show_confirm => {
                            app.close_confirm(false);
                            continue;
                        }
                        KeyCode::Enter if app.show_confirm => {
                            let confirmed = app.confirm_selected == 0; // 0 = Confirm
                            let is_quit = app.confirm_action.as_deref() == Some("quit");
                            app.close_confirm(confirmed);
                            if confirmed && is_quit {
                                app.should_quit = true;
                            }
                            continue;
                        }
                        KeyCode::Esc if app.show_confirm => {
                            app.close_confirm(false);
                            continue;
                        }
                        KeyCode::Char(ch) if app.show_file_dialog => {
                            app.file_dialog_input_char(ch);
                            continue;
                        }
                        KeyCode::Backspace if app.show_file_dialog => {
                            app.file_dialog_backspace();
                            continue;
                        }
                        KeyCode::Enter if app.show_file_dialog => {
                            // 0 = Save/Open, 1 = Browse…, 2 = Cancel
                            app.close_file_dialog(app.file_dialog_selected);
                            continue;
                        }
                        KeyCode::Esc if app.show_file_dialog => {
                            app.close_file_dialog(2); // Cancel
                            continue;
                        }
                        // Help popup: when open, Esc closes and 1/2/3/Tab switch tabs.
                        KeyCode::Esc if app.show_help => {
                            app.show_help = false;
                            continue;
                        }
                        KeyCode::Tab if app.show_help => {
                            app.help_tab = app.help_tab.next();
                            continue;
                        }
                        KeyCode::Char('1') if app.show_help => {
                            app.help_tab = popups::help::HelpTab::Shortcuts;
                            continue;
                        }
                        KeyCode::Char('2') if app.show_help => {
                            app.help_tab = popups::help::HelpTab::Languages;
                            continue;
                        }
                        KeyCode::Char('3') if app.show_help => {
                            app.help_tab = popups::help::HelpTab::About;
                            continue;
                        }
                        KeyCode::F(1) => {
                            app.show_help = !app.show_help;
                        }
                        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::ALT) => {
                            app.toggle_word_wrap();
                        }
                        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::ALT) => {
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
                        // Horizontal scroll: Alt+H left, Alt+L right (mnemonic; Ctrl+H
                        // and Ctrl+L were taken/conflicting). Auto-hscroll also follows
                        // the cursor, but these give manual control over long lines.
                        KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::ALT) => {
                            app.active_buffer_mut().scroll_offset.1 =
                                app.active_buffer_mut().scroll_offset.1.saturating_sub(4);
                        }
                        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::ALT) => {
                            app.active_buffer_mut().scroll_offset.1 =
                                app.active_buffer_mut().scroll_offset.1.saturating_add(4);
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
                            // Tab cycles focus: Query → Replace → Command → Query.
                            // This is how the user leaves the text boxes so the
                            // navigation/replace keys (n/N/r/Enter/c/x/w) become live
                            // and n/N can be typed into the boxes again.
                            if let Some(fs) = &mut app.find_state {
                                fs.focus = fs.focus.next();
                            }
                        }
                        // While a text box has focus, every printable char (including
                        // n/N/r/c/x/w) edits that field; Backspace deletes from it.
                        KeyCode::Char(ch)
                            if app.is_find_active()
                                && app.find_state.as_ref().is_some_and(|fs| fs.editing_text()) =>
                        {
                            app.update_find_input(Some(ch), false);
                        }
                        KeyCode::Backspace
                            if app.is_find_active()
                                && app.find_state.as_ref().is_some_and(|fs| fs.editing_text()) =>
                        {
                            app.update_find_input(None, true);
                        }
                        // Command mode: navigation, replace, and option toggles.
                        // Other keys are ignored (editor stays blocked while bar open).
                        KeyCode::Char('n')
                            if app
                                .find_state
                                .as_ref()
                                .is_some_and(|fs| fs.focus == FindFocus::Command) =>
                        {
                            app.find_next();
                        }
                        KeyCode::Char('N')
                            if app
                                .find_state
                                .as_ref()
                                .is_some_and(|fs| fs.focus == FindFocus::Command) =>
                        {
                            app.find_prev();
                        }
                        KeyCode::Char('r')
                            if app
                                .find_state
                                .as_ref()
                                .is_some_and(|fs| fs.focus == FindFocus::Command) =>
                        {
                            app.replace_current();
                        }
                        KeyCode::Enter if app.is_find_active() => {
                            app.replace_current();
                        }
                        KeyCode::Char('c')
                            if app
                                .find_state
                                .as_ref()
                                .is_some_and(|fs| fs.focus == FindFocus::Command) =>
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
                                .is_some_and(|fs| fs.focus == FindFocus::Command) =>
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
                                .is_some_and(|fs| fs.focus == FindFocus::Command) =>
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
                                Some("Tools") => 5,
                                Some("Settings") => 2,
                                _ => 1,
                            };
                            app.menu_selected = (app.menu_selected + 1) % len;
                        }
                        KeyCode::Char('k') | KeyCode::Up if app.active_menu.is_some() => {
                            let len = match app.active_menu.as_deref() {
                                Some("File") => 6,
                                Some("Tools") => 5,
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
                            if app.config.word_wrap {
                                if let Some(layout) = active_wrap_layout(app) {
                                    app.active_buffer_mut()
                                        .ensure_cursor_visible_wrapped(&layout, vis);
                                }
                            } else {
                                app.active_buffer_mut().ensure_cursor_visible(vis);
                            }
                        }
                        KeyCode::Down => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            let vis = app.editor_visible_lines.get().max(1);
                            app.active_buffer_mut().move_lines_down(1, extend);
                            if app.config.word_wrap {
                                if let Some(layout) = active_wrap_layout(app) {
                                    app.active_buffer_mut()
                                        .ensure_cursor_visible_wrapped(&layout, vis);
                                }
                            } else {
                                app.active_buffer_mut().ensure_cursor_visible(vis);
                            }
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
                            if app.config.word_wrap {
                                if let Some(layout) = active_wrap_layout(app) {
                                    app.active_buffer_mut()
                                        .page_wrapped(&layout, false, visible, extend);
                                }
                            } else {
                                app.active_buffer_mut().page_up(visible, extend);
                                app.active_buffer_mut().ensure_cursor_visible(visible);
                            }
                        }
                        KeyCode::PageDown => {
                            let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                            let visible = app.editor_visible_lines.get().max(1);
                            if app.config.word_wrap {
                                if let Some(layout) = active_wrap_layout(app) {
                                    app.active_buffer_mut()
                                        .page_wrapped(&layout, true, visible, extend);
                                }
                            } else {
                                app.active_buffer_mut().page_down(visible, extend);
                                app.active_buffer_mut().ensure_cursor_visible(visible);
                            }
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

/// Update hover states for all active popups based on mouse position
fn update_hover_states(x: u16, y: u16, app: &mut App) {
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_area = Rect::new(0, 0, tw, th);

    if app.show_file_dialog {
        let popup = popups::file_dialog::file_dialog_rect(term_area);
        let btn_y = popups::file_dialog::file_dialog_button_row(popup) as usize;

        if y as usize == btn_y {
            let labels = popups::file_dialog::button_labels(app.file_dialog_is_save);
            let ranges = crate::ui::button::ButtonRow::button_x_ranges(popup.x + 1, &labels);
            app.file_dialog_button_hover = ranges.iter().position(|(s, e)| x >= *s && x < *e);
        } else {
            app.file_dialog_button_hover = None;
        }
    }

    // File browser footer button hover (only meaningful when browser is open).
    if app.show_file_browser {
        let popup = popups::file_browser::file_browser_rect(term_area);
        let btn_y = popups::file_browser::footer_button_row(popup) as usize;
        if y as usize == btn_y {
            let labels = app.browser_footer_labels();
            let ranges = crate::ui::button::ButtonRow::button_x_ranges(popup.x + 1, &labels);
            app.browser_button_hover = ranges.iter().position(|(s, e)| x >= *s && x < *e);
        } else {
            app.browser_button_hover = None;
        }
    }

    if app.show_confirm {
        let msg = app.confirm_message.clone();
        let popup = popups::confirm::confirm_rect(term_area, &msg);
        let btn_y = popups::confirm::confirm_button_row(popup) as usize;

        if y as usize == btn_y {
            let btn_x_start = popup.x + 2;
            let confirm_len = "[ Y ] Confirm".chars().count() as u16;

            if x >= btn_x_start && x < btn_x_start + confirm_len {
                app.confirm_button_hover = Some(0);
            } else if x >= btn_x_start + confirm_len + 3 && x < btn_x_start + confirm_len + 3 + 12 {
                app.confirm_button_hover = Some(1);
            } else {
                app.confirm_button_hover = None;
            }
        } else {
            app.confirm_button_hover = None;
        }
    }

    if app.show_settings {
        let w = 45u16;
        let h = 14u16;
        let px = (tw.saturating_sub(w)) / 2;
        let py = (th.saturating_sub(h)) / 2;

        if y >= py && y < py + h && x >= px && x < px + w {
            let rel_y = (y - py) as usize;
            if (3..6).contains(&rel_y) {
                app.settings_theme_hover = Some(rel_y - 3);
            } else {
                app.settings_theme_hover = None;
            }
        } else {
            app.settings_theme_hover = None;
        }
    }

    if let Some(menu_name) = app.active_menu.as_deref() {
        let dropdown = crate::ui::header::dropdown_rect(term_area, menu_name);

        if x >= dropdown.x
            && x < dropdown.x + dropdown.width
            && y >= dropdown.y
            && y < dropdown.y + dropdown.height
        {
            let inner_y = y - dropdown.y;
            let item_idx = (inner_y as usize).saturating_sub(1);

            let item_count = match menu_name {
                "File" => 6,
                "Tools" => 5,
                "Settings" => 2,
                _ => 0,
            };

            if item_idx < item_count {
                app.menu_selected = item_idx;
            }
        }
    }
}

/// Build the wrap layout for the active buffer's current viewport, or None
/// when word_wrap is off. Used by mouse/cursor handlers that need screen-row
/// translation outside the render path.
fn active_wrap_layout(app: &App) -> Option<crate::wrap::WrapLayout> {
    if !app.config.word_wrap {
        return None;
    }
    let buf = app.active_buffer();
    Some(crate::wrap::compute_layout(
        &buf.rope,
        0,
        buf.line_count(),
        buf.visible_width.max(1),
        app.config.tab_size,
    ))
}

fn handle_mouse(mouse: MouseEvent, app: &mut App) {
    let (x, y) = (mouse.column, mouse.row);
    app.last_mouse_pos = Some((x, y));

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            update_hover_states(x, y, app);
            let x = mouse.column as usize;
            let y = mouse.row as usize;

            let mut skip_editor = false;

            // === Dropdown menu clicks ===
            if let Some(ref menu_name) = app.active_menu {
                let (menu_x, num_items) = match menu_name.as_str() {
                    "File" => (0usize, 6),
                    "Tools" => (7, 5),
                    "Settings" => (15, 2),
                    _ => (0, 0),
                };
                let dy = crate::ui::layout::HEADER_ROWS as usize; // below header + rule
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

            // === Confirm popup: clickable [Y]/[N] buttons + outside-click cancel ===
            if app.show_confirm && !skip_editor {
                let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
                let term_area = Rect::new(0, 0, tw, th);
                let msg = app.confirm_message.clone();
                let popup = popups::confirm::confirm_rect(term_area, &msg);
                if x >= popup.x as usize
                    && x < (popup.x + popup.width) as usize
                    && y >= popup.y as usize
                    && y < (popup.y + popup.height) as usize
                {
                    let btn_y = popups::confirm::confirm_button_row(popup) as usize;
                    if y == btn_y {
                        if let Some(hovered) = app.confirm_button_hover {
                            let confirmed = hovered == 0;
                            let is_quit = app.confirm_action.as_deref() == Some("quit");
                            app.close_confirm(confirmed);
                            if confirmed && is_quit {
                                app.should_quit = true;
                            }
                        }
                    }
                    skip_editor = true;
                } else {
                    app.close_confirm(false);
                    skip_editor = true;
                }
            }

            // === File dialog: clickable Open/Save & Cancel buttons + outside close ===
            if app.show_file_browser && !skip_editor {
                let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
                let term_area = Rect::new(0, 0, tw, th);
                let popup = popups::file_browser::file_browser_rect(term_area);
                if x >= popup.x as usize
                    && x < (popup.x + popup.width) as usize
                    && y >= popup.y as usize
                    && y < (popup.y + popup.height) as usize
                {
                    let footer_y = popups::file_browser::footer_button_row(popup) as usize;
                    let list_area = popups::file_browser::list_area(popup);
                    if y == footer_y {
                        // Footer button click.
                        if let Some(idx) = app.browser_button_hover {
                            match idx {
                                0 => app.browser_go_up(),
                                1 => app.browser_toggle_mode(),
                                2 => app.browser_toggle_hidden(),
                                3 => app.close_file_browser(false),
                                _ => {}
                            }
                        }
                    } else if y >= list_area.0 && y < list_area.1 {
                        // List row click: select + activate (dir enters, file confirms).
                        // Scroll is derived from the selection via the SAME
                        // `compute_scroll` the renderer uses, so the clicked
                        // screen row maps to exactly the entry rendered there.
                        let rel = y - list_area.0;
                        let body_h = (list_area.1 - list_area.0).max(1);
                        let scroll =
                            popups::file_browser::compute_scroll(app.browser_selected, body_h);
                        let idx = rel + scroll;
                        if idx < app.browser_entries.len() {
                            app.browser_select_index(idx);
                            // Activate the clicked entry directly (single-click UX).
                            app.browser_enter_selected();
                        }
                        // Touch viewport height so subsequent PgUp/PgDn feels right.
                        app.editor_visible_lines.set(body_h);
                    }
                    skip_editor = true;
                } else {
                    app.close_file_browser(false);
                    skip_editor = true;
                }
            }

            if app.show_file_dialog && !skip_editor {
                let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
                let term_area = Rect::new(0, 0, tw, th);
                let popup = popups::file_dialog::file_dialog_rect(term_area);
                if x >= popup.x as usize
                    && x < (popup.x + popup.width) as usize
                    && y >= popup.y as usize
                    && y < (popup.y + popup.height) as usize
                {
                    let btn_y = popups::file_dialog::file_dialog_button_row(popup) as usize;
                    if y == btn_y {
                        if let Some(hovered) = app.file_dialog_button_hover {
                            // 0 = Save/Open, 1 = Browse…, 2 = Cancel
                            app.close_file_dialog(hovered);
                        }
                    }
                    skip_editor = true;
                } else {
                    app.close_file_dialog(2); // Cancel
                    skip_editor = true;
                }
            }

            // === Interactive popup clicks (e.g. Settings themes) + outside close for popups ===
            if app.show_settings && !skip_editor {
                // Compute the rect from the REAL terminal size (was hardcoded 120x20).
                let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
                let w = 45u16;
                let h = 14u16;
                let px = (tw.saturating_sub(w)) / 2;
                let py = (th.saturating_sub(h)) / 2;
                let in_popup = (y as u16 >= py && (y as u16) < py + h)
                    && (x as u16 >= px && (x as u16) < px + w);
                if in_popup {
                    let rel_y = (y as u16 - py) as usize;
                    // theme rows are at approx inner rel_y 3,4,5
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

            // === Help popup: clickable tabs + inside-click ignore + outside-click close ===
            if app.show_help && !skip_editor {
                let (h_tw, h_th) = crossterm::terminal::size().unwrap_or((80, 24));
                let term_area = Rect::new(0, 0, h_tw, h_th);
                let popup = popups::help::help_rect(term_area);
                if x >= popup.x as usize
                    && x < (popup.x + popup.width) as usize
                    && y >= popup.y as usize
                    && y < (popup.y + popup.height) as usize
                {
                    // Tab strip sits at inner row y = popup.y + 1.
                    let tab_row = (popup.y + 1) as usize;
                    if y == tab_row {
                        let tabs = popups::help::help_tab_rects();
                        for (_label, tab, x_start, x_end) in tabs {
                            let sx = (popup.x + *x_start) as usize;
                            let ex = (popup.x + *x_end) as usize;
                            if x >= sx && x < ex {
                                app.help_tab = *tab;
                                skip_editor = true;
                                break;
                            }
                        }
                        if !skip_editor {
                            // clicked inside popup but not on a tab -> ignore
                            skip_editor = true;
                        }
                    } else {
                        // clicked inside popup body -> ignore (don't close)
                        skip_editor = true;
                    }
                } else {
                    // clicked outside help popup -> close
                    app.show_help = false;
                    skip_editor = true;
                }
            }

            // Close other non-interactive popups on outside click
            if !skip_editor && app.show_word_count {
                app.show_word_count = false;
                skip_editor = false; // allow the click to still place cursor
            }

            if !skip_editor {
                let header_rows = crate::ui::layout::HEADER_ROWS as usize;
                // Scrollbar lives in the rightmost column of the editor render
                // area. Handle click-to-jump / drag-to-scrub here, ahead of
                // header/editor handling, so a scrollbar press never places the
                // cursor or starts a text selection. (find-bar row is excluded
                // by `scrollbar_target_for_row`'s vis-bounded y range.)
                let (tw, _th) = crossterm::terminal::size().unwrap_or((80, 24));
                if x + 1 == tw as usize {
                    if let Some(target) = scrollbar_target_for_row(app, y) {
                        let vis = app.editor_visible_lines.get().max(1);
                        let word_wrap = app.config.word_wrap;
                        let tab_size = app.config.tab_size;
                        let buf = app.active_buffer_mut();
                        buf.scroll_offset.0 = target;
                        if word_wrap {
                            let total = crate::wrap::total_screen_rows(
                                &buf.rope,
                                buf.visible_width,
                                tab_size,
                            );
                            buf.clamp_scroll_wrapped(total);
                        } else {
                            buf.clamp_scroll(vis);
                        }
                        app.scrollbar_drag_active = true;
                        app.mouse_drag_active = false;
                    }
                } else if y < header_rows {
                    // Header rows (menu/tabs + separator rule): tabs, menus or [+]
                    handle_header_click(x, app);
                } else {
                    // Editor area starts below the header + separator.
                    let editor_start_y = header_rows;
                    if y >= editor_start_y {
                        let editor_y = y - editor_start_y;
                        let vis = app.editor_visible_lines.get().max(1);
                        if editor_y < vis {
                            let gutter = Buffer::GUTTER_WIDTH;
                            if x >= gutter {
                                let col_in_row = x - gutter;
                                let layout = active_wrap_layout(app);
                                let buf = app.active_buffer_mut();
                                if let Some(layout) = layout {
                                    // Wrap on: editor_y is a screen row offset; add scroll.
                                    let screen_row = buf.scroll_offset.0 + editor_y;
                                    let target_byte = crate::wrap::byte_of_screen_row(
                                        &layout, &buf.rope, 0, screen_row, col_in_row,
                                    );
                                    buf.cursor = target_byte.min(buf.rope.len_bytes());
                                    buf.clear_selection();
                                } else {
                                    let hscroll = buf.scroll_offset.1;
                                    // visual_col must account for hscroll: screen col 0 in content == logical hscroll
                                    let visual_col = col_in_row + hscroll;
                                    let visual_line = buf.scroll_offset.0 + editor_y;
                                    buf.set_cursor_from_visual(visual_line, visual_col);
                                }
                                // Do not force recenter here (ensure only if offscreen); set_visual no longer adjusts.
                                buf.ensure_cursor_visible(vis);
                                // Begin a mouse-drag selection session; cleared on MouseUp
                                // or when a modal popup opens. Drives edge auto-scroll.
                                app.mouse_drag_active = true;
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
        MouseEventKind::Moved => {
            update_hover_states(x, y, app);
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Two drag modes (mutually exclusive): scrollbar scrub vs text
            // selection. The flag is set on MouseDown; route accordingly.
            if app.scrollbar_drag_active {
                if let Some(target) = scrollbar_target_for_row(app, y as usize) {
                    let vis = app.editor_visible_lines.get().max(1);
                    let word_wrap = app.config.word_wrap;
                    let tab_size = app.config.tab_size;
                    let buf = app.active_buffer_mut();
                    buf.scroll_offset.0 = target;
                    if word_wrap {
                        let total =
                            crate::wrap::total_screen_rows(&buf.rope, buf.visible_width, tab_size);
                        buf.clamp_scroll_wrapped(total);
                    } else {
                        buf.clamp_scroll(vis);
                    }
                }
            } else if app.mouse_drag_active {
                // Extend selection from a fixed anchor (set on the first drag move),
                // so dragging works in BOTH directions. The click handler seeds the
                // anchor by clearing it; extend_selection_to then captures the cursor
                // as the anchor on first use. visual_col must include hscroll to match
                // the click handler's column mapping.
                extend_drag_to_mouse(app);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            // End any in-progress drag selection session. Edge auto-scroll keys
            // off this flag, so it must be cleared as soon as the button releases.
            app.mouse_drag_active = false;
            app.scrollbar_drag_active = false;
        }
        // Horizontal mouse-wheel (Shift+wheel on most terminals) scrolls the
        // view sideways. Vertical wheel is coalesced in run_app, but horizontal
        // events pass through here since wheel_lines ignores them.
        MouseEventKind::ScrollLeft => {
            app.active_buffer_mut().scroll_offset.1 =
                app.active_buffer_mut().scroll_offset.1.saturating_sub(4);
        }
        MouseEventKind::ScrollRight => {
            app.active_buffer_mut().scroll_offset.1 =
                app.active_buffer_mut().scroll_offset.1.saturating_add(4);
        }
        _ => {}
    }
}

/// Map a screen row `y` to a target top-line scroll offset for the editor
/// scrollbar (proportional click-to-jump / drag-to-scrub). Returns `None` when
/// a modal is open or `y` is outside the scrollbar's vertical extent (which
/// excludes the find-bar row when the find bar is open). The mapping skips the
/// rendered `↑`/`↓` end cells and mirrors the `max = line_count - vis` clamp
/// used by `Buffer::scroll` / `clamp_scroll`, keeping it in sync with the thumb
/// calibration in `ui::editor::render`.
fn scrollbar_target_for_row(app: &App, y: usize) -> Option<usize> {
    if any_modal_open(app) {
        return None;
    }
    let header = crate::ui::layout::HEADER_ROWS as usize;
    let vis = app.editor_visible_lines.get().max(1);
    let sb_top = header;
    let sb_bottom = header + vis; // exclusive
    if !(sb_top..sb_bottom).contains(&y) {
        return None;
    }
    let total = if app.config.word_wrap {
        let buf = app.active_buffer();
        crate::wrap::total_screen_rows(&buf.rope, buf.visible_width, app.config.tab_size)
    } else {
        app.active_buffer().line_count()
    };
    let max = total.saturating_sub(vis);
    if max == 0 {
        return Some(0); // content fits viewport — no scroll range
    }
    // Exclude the ↑/↓ end cells so the clickable track matches the rendered one.
    let track = vis.saturating_sub(2).max(1);
    let rel = y.saturating_sub(sb_top + 1).min(track); // skip top ↑ cell
    let frac = rel as f64 / track as f64;
    Some((frac * max as f64).round() as usize)
}

/// Map `app.last_mouse_pos` to a target byte and extend the active drag
/// selection to it. Shared by the `Drag` event path and the edge-auto-scroll
/// timeout path so both produce identical selection behavior.
fn extend_drag_to_mouse(app: &mut App) {
    let (x, y) = match app.last_mouse_pos {
        Some(p) => p,
        None => return,
    };
    let (x, y) = (x as usize, y as usize);
    let header_rows = crate::ui::layout::HEADER_ROWS as usize;
    if y < header_rows {
        return;
    }
    let editor_y = y - header_rows;
    let gutter = Buffer::GUTTER_WIDTH;
    if x < gutter {
        return;
    }
    let col_in_row = x - gutter;
    let layout = active_wrap_layout(app);
    let buf = app.active_buffer_mut();
    let target_byte = if let Some(layout) = layout {
        let screen_row = buf.scroll_offset.0 + editor_y;
        crate::wrap::byte_of_screen_row(&layout, &buf.rope, 0, screen_row, col_in_row)
    } else {
        let hscroll = buf.scroll_offset.1;
        let visual_col = col_in_row + hscroll;
        let visual_line = buf.scroll_offset.0 + editor_y;
        buf.byte_at_visual(visual_line, visual_col)
    };
    buf.mouse_extend_to(target_byte);
}

/// If a drag is in progress and the mouse currently sits at the top/bottom or
/// left/right edge of the editor viewport, return the per-tick scroll delta
/// (rows, cols) that should be applied to keep scrolling past the edge.
/// Returns `None` when not at any edge.
fn drag_edge_delta(app: &App) -> Option<(isize, isize)> {
    let (x, y) = app.last_mouse_pos?;
    let (x, y) = (x as usize, y as usize);
    let header_rows = crate::ui::layout::HEADER_ROWS as usize;
    if y < header_rows {
        return None;
    }
    let editor_y = y - header_rows;
    let vis = app.editor_visible_lines.get().max(1);
    let gutter = Buffer::GUTTER_WIDTH;

    let mut drow: isize = 0;
    let mut dcol: isize = 0;
    if editor_y == 0 {
        drow = -1;
    } else if editor_y >= vis.saturating_sub(1) {
        drow = 1;
    }
    if !app.config.word_wrap {
        let visible_width = app.editor_visible_width.get().max(1);
        if x <= gutter {
            dcol = -1;
        } else if x >= gutter + visible_width.saturating_sub(1) {
            dcol = 1;
        }
    }

    if drow == 0 && dcol == 0 {
        None
    } else {
        Some((drow, dcol))
    }
}

/// True when any modal/popup is open and the editor (and its drag selection)
/// should be suspended.
fn any_modal_open(app: &App) -> bool {
    app.show_help
        || app.show_word_count
        || app.show_settings
        || app.show_file_dialog
        || app.show_file_browser
        || app.show_confirm
        || app.active_menu.is_some()
        || app.find_state.is_some()
}

fn handle_header_click(x: usize, app: &mut App) {
    // Menu button hit-ranges must mirror header.rs render exactly:
    //   " File "(0-5) │(6) " Tools "(7-13) │(14) " Settings "(15-24)  (gap 25-26)
    let menus = [("File", 0, 6), ("Tools", 7, 14), ("Settings", 15, 25)];
    for (name, start, end) in menus {
        if x >= start && x < end {
            app.open_menu(name);
            return;
        }
    }
    if app.active_menu.is_some() {
        app.close_menu();
    }

    // File tabs start after the menu block + a 2-col gap. Each tab renders as
    // "[ name• ] " — width in COLUMNS (chars().count(), not bytes) is name + 6
    // (brackets, two inner spaces, one trailing space). [+] is 3 cols.
    let mut pos = 27usize;
    for i in 0..app.buffers.len() {
        let buf = &app.buffers[i];
        let mut name = buf
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();
        if buf.is_modified {
            name.push('•');
        }
        let tab_width = name.chars().count() + 6; // "[ " + name + " ] "
        if x >= pos && x < pos + tab_width {
            app.active = i;
            app.close_menu();
            return;
        }
        pos += tab_width;
    }

    // [+] button (new tab)
    if x >= pos && x < pos + 3 {
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
                fs.focus = FindFocus::Query;
            }
        }
        ("Tools", 1) => {
            // Find & Replace (focus replace field)
            if app.find_state.is_none() {
                app.toggle_find();
            }
            if let Some(fs) = &mut app.find_state {
                fs.focus = FindFocus::Replace;
            }
        }
        ("Tools", 2) => app.compute_and_show_word_count(),
        ("Tools", 3) => {
            let _ = app.format_current_document();
        }
        ("Tools", 4) => app.toggle_word_wrap(),
        ("Settings", 0) => app.open_settings(),
        ("Settings", 1) => app.open_settings(), // themes
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = crate::ui::layout::main_layout(area);

    // Update editor viewport height for PgUp/PgDn (subtract find bar when present).
    let base_h = chunks[1].height as usize;
    let vis_h = if app.is_find_active() {
        base_h.saturating_sub(1)
    } else {
        base_h
    };
    let vis_h = vis_h.max(1);
    app.editor_visible_lines.set(vis_h);
    // Text-area width = editor area minus line-number gutter (6) minus scrollbar (1).
    let content_w = (chunks[1].width as usize)
        .saturating_sub(Buffer::GUTTER_WIDTH + 1)
        .max(1);
    app.editor_visible_width.set(content_w);

    // Refresh the cached syntax spans for the visible window. This is a cheap
    // key comparison on a steady frame and a full (single-HighlightLines) recompute
    // only on edit / scroll / language change. Done here so editor::render can read
    // the cache immutably.
    //
    // Under word wrap `scroll_offset.0` is a screen row, but
    // `refresh_highlight_cache` expects a logical-line index. Translate the
    // visible screen-row window back to a logical-line window first
    // (final-review Critical-1); otherwise `cache.start_line` exceeds the
    // logical-line indices of every visible line and `render_wrapped` falls
    // back to `Span::raw`, losing all syntax color.
    let scroll_row = app.active_buffer().scroll_offset.0;
    if app.config.word_wrap {
        let buf = app.active_buffer();
        let (start, count) = crate::wrap::visible_logical_window(
            &buf.rope,
            scroll_row,
            vis_h,
            buf.visible_width.max(1),
            app.config.tab_size,
        );
        app.refresh_highlight_cache(start, count);
    } else {
        app.refresh_highlight_cache(scroll_row, vis_h);
    }

    crate::ui::header::render(f, chunks[0], app);

    let edit_chunk = chunks[1];
    if app.is_find_active() {
        let (edit_area, find_area) = crate::ui::layout::editor_with_find_layout(edit_chunk);
        crate::ui::editor::render(f, edit_area, app);
        if let Some(fs) = &app.find_state {
            crate::ui::find::render_bar(f, find_area, fs);
        }
        crate::ui::status::render(f, chunks[2], app);
    } else {
        crate::ui::editor::render(f, edit_chunk, app);
        crate::ui::status::render(f, chunks[2], app);
    }

    if app.show_help {
        popups::help::render(
            f,
            area,
            app.current_theme(),
            app.help_tab,
            app.help_content_scroll,
        );
    }
    if app.show_word_count {
        popups::word_count::render(f, area, app.current_theme(), &app.word_count_text);
    }

    if let Some(ref menu) = app.active_menu {
        crate::ui::header::render_dropdown(f, menu, area, app.current_theme(), app.menu_selected);
    }

    if app.show_settings {
        popups::settings::render(
            f,
            area,
            app.current_theme(),
            app.settings_selected,
            app.config.word_wrap,
        );
    }

    if app.show_confirm {
        popups::confirm::render_confirm(f, area, app.current_theme(), &app.confirm_message, app);
    }

    if app.show_file_dialog {
        popups::file_dialog::render(
            f,
            area,
            app.current_theme(),
            app.file_dialog_is_save,
            &app.file_path_input,
            &app.file_dialog_message,
            app,
        );
    }

    // Browser renders on top of the file dialog when open.
    if app.show_file_browser {
        popups::file_browser::render(f, area, app.current_theme(), app);
    }
}
