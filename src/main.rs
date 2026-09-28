//! rust-edit — terminal text & code editor (Ratatui TUI)

#![allow(unreachable_patterns)]

mod app;
mod buffer;
mod config;
mod file_watch;
mod format;
mod fs_access;
mod highlight_cache;
mod syntax;
mod theme;
mod themes;
mod ui;
mod utils;
mod wrap;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
        Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
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
use std::time::{Duration, Instant};

use crate::app::{App, PopupFocus, ReloadChoice};
#[cfg(test)]
use crate::buffer::Buffer;
use crate::ui::find::FindFocus;
use crate::ui::popups;

/// What to do with CLI args before the terminal is initialized.
#[derive(Debug, PartialEq, Eq)]
enum Startup {
    /// Open these paths. Empty means the untitled placeholder.
    Run(Vec<String>),
    /// Print `rust-edit <version>` and exit.
    Version,
}

/// `--version` and `-V` print the version. `--` ends flag parsing so a file
/// named `--version` can still be opened. Every other argument is a path,
/// including names that start with `-`.
fn startup_from_args<I>(args: I) -> Startup
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut files = Vec::new();
    let mut literal = false;
    for arg in args {
        let arg = arg.as_ref();
        if literal {
            files.push(arg.to_string());
            continue;
        }
        match arg {
            "--" => literal = true,
            "--version" | "-V" => return Startup::Version,
            other => files.push(other.to_string()),
        }
    }
    Startup::Run(files)
}

fn main() -> Result<()> {
    let files = match startup_from_args(std::env::args().skip(1)) {
        Startup::Version => {
            println!("rust-edit {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Startup::Run(files) => files,
    };

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableFocusChange
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&files);
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableFocusChange
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

/// Blink interval for the software cursor blink.
const BLINK_INTERVAL: Duration = Duration::from_millis(533);
/// Long poll when the cursor is hidden.
const IDLE_POLL: Duration = Duration::from_secs(3600);
/// Max wait between disk-stamp checks when any buffer has a path.
const DISK_POLL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct HoverVisual {
    file_dialog_button_hover: Option<usize>,
    file_dialog_selected: usize,
    browser_button_hover: Option<usize>,
    browser_button_selected: usize,
    browser_list_hover: Option<usize>,
    confirm_button_hover: Option<usize>,
    reload_button_hover: Option<usize>,
    configure_row_hover: Option<usize>,
    configure_button_hover: Option<usize>,
    themes_list_hover: Option<usize>,
    themes_button_hover: Option<usize>,
    menu_selected: usize,
}

/// Snapshot popup hover/selection for idle-mouse redraw decisions.
fn hover_visual(app: &App) -> HoverVisual {
    HoverVisual {
        file_dialog_button_hover: app.file_dialog_button_hover,
        file_dialog_selected: app.file_dialog_selected,
        browser_button_hover: app.browser_button_hover,
        browser_button_selected: app.browser_button_selected,
        browser_list_hover: app.browser_list_hover,
        confirm_button_hover: app.confirm_button_hover,
        reload_button_hover: app.reload_button_hover,
        configure_row_hover: app.configure_row_hover,
        configure_button_hover: app.configure_button_hover,
        themes_list_hover: app.themes_list_hover,
        themes_button_hover: app.themes_button_hover,
        menu_selected: app.menu_selected,
    }
}

/// Remaining blink interval, or `IDLE_POLL` when hidden.
fn next_blink_timeout(last_blink: Instant, interval: Duration, should_blink: bool) -> Duration {
    if !should_blink {
        return IDLE_POLL;
    }
    let elapsed = last_blink.elapsed();
    if elapsed >= interval {
        Duration::ZERO
    } else {
        interval - elapsed
    }
}

/// True when the software cursor should toggle.
fn blink_is_due(last_blink: Instant, interval: Duration, should_blink: bool) -> bool {
    should_blink && last_blink.elapsed() >= interval
}

/// Where a vertical mouse-wheel burst should go. Help, the file browser,
/// and Themes scroll their own content; every other popup swallows the
/// wheel so it cannot leak through to the editor underneath.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WheelTarget {
    Editor,
    Help,
    FileBrowser,
    Swallow,
    TabStrip,
    Themes,
}

fn wheel_target(app: &App, wheel_col: u16, wheel_row: u16) -> WheelTarget {
    match app.popup_focus() {
        Some(PopupFocus::Help) => WheelTarget::Help,
        Some(PopupFocus::FileBrowser) => WheelTarget::FileBrowser,
        Some(PopupFocus::Themes) => WheelTarget::Themes,
        Some(_) => WheelTarget::Swallow,
        None => {
            if (wheel_row as usize) < crate::ui::layout::HEADER_ROWS as usize
                && (wheel_col as usize) >= crate::ui::header::MENU_BLOCK_COLS
            {
                let names = app.tab_names();
                let width = app.header_width.get().max(1);
                let layout = crate::ui::header::layout_tab_strip(&names, width, app.tab_scroll);
                if layout.overflowing {
                    return WheelTarget::TabStrip;
                }
            }
            WheelTarget::Editor
        }
    }
}

fn term_area() -> Rect {
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    Rect::new(0, 0, tw, th)
}

fn apply_help_scroll(app: &mut App, delta: i32) {
    let popup = popups::help::help_rect(term_area());
    let next = (app.help_content_scroll.0 as i32 + delta).max(0) as u16;
    app.help_content_scroll.0 = popups::help::clamp_help_scroll(next, app.help_tab, popup);
}

fn apply_help_scrollbar_at(app: &mut App, y: u16) {
    let popup = popups::help::help_rect(term_area());
    let sb = popups::help::help_scrollbar_rect(popup);
    let bar = popups::help::help_overflow_bar(app.help_tab, popup, app.help_content_scroll.0);
    if let Some(pos) = bar.target_for_row(y, sb) {
        app.help_content_scroll.0 =
            popups::help::clamp_help_scroll(pos as u16, app.help_tab, popup);
    }
}

fn apply_themes_role_scrollbar_at(app: &mut App, y: u16) {
    let popup = popups::themes::edit_rect(term_area());
    let Some(edit) = app.themes_edit.as_mut() else {
        return;
    };
    let bar = popups::themes::roles_overflow_bar(edit.role_scroll);
    if !bar.overflowing() {
        return;
    }
    let sb = popups::themes::roles_scrollbar_rect(popup);
    if let Some(pos) = bar.target_for_row(y, sb) {
        edit.role_scroll = bar.clamp_position(pos) as u16;
    }
}

fn apply_browser_scrollbar_at(app: &mut App, y: u16) {
    let popup = popups::file_browser::file_browser_rect(term_area());
    let reserve = app.browser_origin_is_save;
    let sb = popups::file_browser::list_scrollbar_rect(popup, reserve);
    let bar = popups::file_browser::list_overflow_bar(
        app.browser_entries.len(),
        app.browser_selected,
        popup,
        reserve,
    );
    if !bar.overflowing() {
        return;
    }
    if let Some(pos) = bar.target_for_row(y, sb) {
        // Properly map the scrollbar position to the selection index range
        let max_pos = bar.max_position();
        let total = app.browser_entries.len();
        let sel = if max_pos == 0 || total == 0 {
            0
        } else {
            // Scale from [0, max_pos] to [0, total-1]
            ((pos as f64 / max_pos as f64) * (total - 1) as f64).round() as usize
        };
        app.browser_select_index(sel);
        app.browser_list_hover = None;
        app.browser_button_selected = popups::file_browser::FOCUS_LIST;
    }
}

/// True for pointer motion that should not, by itself, force a redraw.
fn event_is_idle_mouse_move(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(m) if matches!(m.kind, MouseEventKind::Moved)
    )
}

/// True when this drained batch must produce a new frame.
/// `wheel_delta` is applied before dispatch and is a visual change even if
/// the wheel events were then dropped from `events`.
fn batch_needs_redraw(
    events: &[Event],
    wheel_delta: isize,
    hover_before: HoverVisual,
    hover_after: HoverVisual,
) -> bool {
    if wheel_delta != 0 {
        return true;
    }
    if hover_before != hover_after {
        return true;
    }
    events.iter().any(|e| !event_is_idle_mouse_move(e))
}

/// Compute the screen position of the cursor, or None if it's not in view.
/// Returns (x, y) in terminal coordinates.
fn compute_cursor_position(app: &App) -> Option<(u16, u16)> {
    // Hide cursor when any modal/popup is open
    if app.show_help
        || app.show_word_count
        || app.show_configure
        || app.show_themes
        || app.show_file_dialog
        || app.show_file_browser
        || app.show_confirm
        || app.show_reload
        || app.active_menu.is_some()
    {
        return None;
    }

    let vis = app.editor_visible_lines.get().max(1);

    if app.config.word_wrap {
        let buf = app.active_buffer();
        let layout = crate::wrap::compute_layout(
            &buf.rope,
            0,
            buf.line_count(),
            buf.visible_width.max(1),
            app.config.tab_size,
        );
        let cursor_screen_row = crate::wrap::screen_row_of_byte(&layout, &buf.rope, 0, buf.cursor);
        let scroll_row = buf.scroll_offset.0;
        if cursor_screen_row >= scroll_row {
            let view_row = cursor_screen_row - scroll_row;
            if view_row >= vis {
                return None;
            }
            let screen_y = crate::ui::layout::HEADER_ROWS + view_row as u16;
            // Column within the sub-row: distance from the sub-row's start byte to the cursor.
            let (line_idx, _col_in_line) = buf.cursor_position();
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
            let screen_x = app.gutter_width() as u16 + col_offset as u16;
            Some((screen_x, screen_y))
        } else {
            None
        }
    } else {
        let buf = app.active_buffer();
        let (cur_row, cur_col) = buf.cursor_position();
        let scroll_row = buf.scroll_offset.0;
        let hscroll = buf.scroll_offset.1;
        if cur_row >= scroll_row {
            let view_row = cur_row - scroll_row;
            if view_row >= vis {
                return None;
            }
            // Header now occupies HEADER_ROWS (menu + separator rule).
            let screen_y = crate::ui::layout::HEADER_ROWS + view_row as u16;
            // Hardware cursor must compensate hscroll: the drawn content starts at logical hscroll.
            let vis_width = buf.visible_width.max(1);
            let content_col = cur_col.saturating_sub(hscroll);
            // Hide rather than paint on the vertical scrollbar column.
            if content_col >= vis_width {
                return None;
            }
            let screen_x = app.gutter_width() as u16 + content_col as u16;
            Some((screen_x, screen_y))
        } else {
            None
        }
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    // Software cursor blink state
    let mut cursor_blink_on = true;
    let mut last_blink = Instant::now();

    'outer: loop {
        // Keep each buffer's notion of the viewport fresh so that the internal
        // adjust_scroll (run on edits/moves) clamps against the *real* height and
        // width instead of stale defaults. Fixes half-screen scroll jumping and
        // adds horizontal auto-scroll that follows the cursor on long lines.
        let vis = app.editor_visible_lines.get().max(1);
        let hwidth = app.editor_visible_width.get().max(1);
        let wrap = app.config.word_wrap;
        let tab = app.config.tab_size;
        for b in app.buffers.iter_mut() {
            b.visible_lines = vis;
            b.visible_width = hwidth;
            b.word_wrap = wrap;
            b.tab_size = tab;
        }

        terminal.hide_cursor().ok();
        terminal
            .draw(|f| ui(f, app))
            .map_err(|e| anyhow::anyhow!("draw error: {}", e))?;

        // Position the real terminal cursor (after draw) so it's always visible at the exact
        // position, including on whitespace, spaces, and end-of-line. Hide it for popups/modals.
        // The cursor blinks in software: only show it when the blink phase is "on".
        let cursor_pos = compute_cursor_position(app);
        if let Some((x, y)) = cursor_pos {
            if cursor_blink_on {
                terminal.set_cursor_position((x, y)).ok();
                terminal.show_cursor().ok();
            } else {
                terminal.hide_cursor().ok();
            }
        } else {
            terminal.hide_cursor().ok();
        }

        // Draw, then wait for a batch that actually changes the frame.
        // Idle mouse-moves stay in this wait loop so we never hide/draw/show
        // (which flashes the hardware caret). Blink due is checked first so
        // queued motion cannot starve the 533ms phase toggle. Only a cursor
        // byte change resets the blink timer; mouse-move does not.
        'wait: loop {
            if any_modal_open(app) {
                app.mouse_drag_active = false;
                app.scrollbar_drag_active = false;
                app.hscrollbar_drag_active = false;
            }
            let cursor_should_blink = compute_cursor_position(app).is_some();
            if blink_is_due(last_blink, BLINK_INTERVAL, cursor_should_blink) {
                cursor_blink_on = !cursor_blink_on;
                last_blink = Instant::now();
                break 'wait; // redraw so the new phase is visible
            }
            let edge = if app.mouse_drag_active {
                drag_edge_delta(app)
            } else {
                None
            };
            let mut poll_timeout = if edge.is_some() {
                Duration::from_millis(30)
            } else {
                next_blink_timeout(last_blink, BLINK_INTERVAL, cursor_should_blink)
            };
            if app.buffers.iter().any(|b| b.path.is_some()) {
                poll_timeout = poll_timeout.min(DISK_POLL);
            }

            let first = if event::poll(poll_timeout)? {
                event::read()?
            } else if let Some((drow, dcol)) = edge {
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
                    app.active_buffer_mut().scroll_horizontal(dcol);
                }
                extend_drag_to_mouse(app);
                let _ = app.check_external_changes();
                break 'wait;
            } else {
                // Timed out: blink (if needed) and poll disk stamps.
                if cursor_should_blink {
                    cursor_blink_on = !cursor_blink_on;
                    last_blink = Instant::now();
                }
                let _ = app.check_external_changes();
                break 'wait;
            };
            let mut events = vec![first];
            while event::poll(Duration::ZERO)? {
                events.push(event::read()?);
            }

            let hover_before = hover_visual(app);
            let cursor_pos_before = app.active_buffer().cursor;
            // Apply the accumulated wheel delta directly (one scroll op), then drop
            // all wheel events so only key/click/other events remain for the dispatch.
            let wheel_delta = events.iter().filter_map(wheel_lines).sum::<isize>();
            if wheel_delta != 0 {
                let (wheel_col, wheel_row) =
                    events.iter().rev().find_map(wheel_pos).unwrap_or((0, 0));
                match wheel_target(app, wheel_col, wheel_row) {
                    WheelTarget::FileBrowser => {
                        app.browser_move_cursor(wheel_delta as i32);
                    }
                    WheelTarget::Help => {
                        apply_help_scroll(app, wheel_delta as i32);
                    }
                    WheelTarget::Themes => {
                        if app.themes_mode == popups::themes::ThemesMode::Edit {
                            if let Some(edit) = app.themes_edit.as_mut() {
                                edit.scroll_roles(wheel_delta.signum() as i32);
                            }
                        } else {
                            themes_popup_handle_vert(app, wheel_delta.signum() as i32);
                        }
                    }
                    WheelTarget::Swallow => {}
                    WheelTarget::TabStrip => {
                        app.scroll_tab_strip(wheel_delta.signum() * (wheel_delta.abs() / 3).max(1));
                    }
                    WheelTarget::Editor => {
                        if app.config.word_wrap {
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
                }
            }
            events.retain(|e| wheel_lines(e).is_none());

            for event in events.clone() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        app.format_status = None;
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            if app.popup_is_open() {
                                match popup_ctrl_action(key.code, app) {
                                    Some(PopupCtrl::Quit) => app.request_quit(),
                                    Some(PopupCtrl::BrowserToggleHidden) => {
                                        app.browser_toggle_hidden()
                                    }
                                    None => {}
                                }
                                continue;
                            }
                            match key.code {
                                // Browser Ctrl+H is handled in popup_ctrl_action.
                                KeyCode::Char('q') => {
                                    app.request_quit();
                                }
                                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::ALT) => {
                                    // Ctrl+Alt+S: Save As. Placed before plain Ctrl+S so the
                                    // ALT variant doesn't fall through to save_current().
                                    app.show_file_dialog(true);
                                }
                                KeyCode::Char('s') => {
                                    let _ = app.save_current();
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
                                KeyCode::Char('f')
                                    if !key.modifiers.contains(KeyModifiers::SHIFT) =>
                                {
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
                                KeyCode::Char('f')
                                    if key.modifiers.contains(KeyModifiers::SHIFT) =>
                                {
                                    // Most terminals send Ctrl+Shift+F as plain Ctrl+F
                                    // (no Shift bit). Help points at the Tools menu.
                                    // This arm runs only when Shift is actually reported.
                                    app.format_current_document();
                                }
                                KeyCode::Char('z') => {
                                    // Ctrl+Y is the documented redo chord. Shift is set
                                    // only when the terminal reports it; a Ctrl+Z byte
                                    // arrives with Control alone.
                                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                                        app.redo();
                                    } else {
                                        app.undo();
                                    }
                                }
                                KeyCode::Char('y') => {
                                    app.redo();
                                }
                                // 0x1F. Crossterm reports it as Ctrl+7. That is the
                                // unshifted Ctrl+/ press (also Ctrl+_). Ctrl+? is
                                // 0x7F and stays Backspace.
                                KeyCode::Char('7') | KeyCode::Char('/') => {
                                    app.toggle_line_comment();
                                }
                                _ => {}
                            }
                            continue;
                        }

                        if let Some(focus) = app.popup_focus() {
                            match focus {
                                PopupFocus::Confirm => {
                                    if handle_confirm_key(key.code, app) {
                                        return Ok(());
                                    }
                                }
                                PopupFocus::Reload => handle_reload_key(key.code, app),
                                PopupFocus::FileBrowser => handle_file_browser_key(key.code, app),
                                PopupFocus::FileDialog => handle_file_dialog_key(key.code, app),
                                PopupFocus::Help => handle_help_key(key.code, app),
                                PopupFocus::Configure => handle_configure_key(key.code, app),
                                PopupFocus::Themes => handle_themes_key(key.code, app),
                                PopupFocus::WordCount => handle_word_count_key(key.code, app),
                            }
                            continue;
                        }

                        match key.code {
                            KeyCode::F(1) => {
                                app.open_help();
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
                                app.active_buffer_mut().scroll_horizontal(-4);
                            }
                            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::ALT) => {
                                app.active_buffer_mut().scroll_horizontal(4);
                            }
                            KeyCode::Char(d)
                                if ('1'..='9').contains(&d)
                                    && key.modifiers.contains(KeyModifiers::ALT) =>
                            {
                                let idx = (d as u8 - b'1') as usize;
                                if idx < app.buffers.len() {
                                    app.active = idx;
                                    app.ensure_active_tab_visible();
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
                                    && app
                                        .find_state
                                        .as_ref()
                                        .is_some_and(|fs| fs.editing_text()) =>
                            {
                                app.update_find_input(Some(ch), false);
                            }
                            KeyCode::Backspace
                                if app.is_find_active()
                                    && app
                                        .find_state
                                        .as_ref()
                                        .is_some_and(|fs| fs.editing_text()) =>
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
                            KeyCode::Esc if app.is_find_active() => {
                                app.clear_find();
                            }
                            // Dropdown menu keyboard navigation
                            KeyCode::Esc if app.active_menu.is_some() => {
                                app.close_menu();
                            }
                            KeyCode::Char('j') | KeyCode::Down if app.active_menu.is_some() => {
                                let len = crate::ui::header::menu_len(
                                    app.active_menu.as_deref().unwrap_or(""),
                                );
                                app.menu_selected = (app.menu_selected + 1) % len;
                            }
                            KeyCode::Char('k') | KeyCode::Up if app.active_menu.is_some() => {
                                let len = crate::ui::header::menu_len(
                                    app.active_menu.as_deref().unwrap_or(""),
                                );
                                app.menu_selected = if app.menu_selected == 0 {
                                    len - 1
                                } else {
                                    app.menu_selected - 1
                                };
                            }
                            KeyCode::Left | KeyCode::BackTab if app.active_menu.is_some() => {
                                app.cycle_open_menu(false);
                            }
                            KeyCode::Right | KeyCode::Tab if app.active_menu.is_some() => {
                                app.cycle_open_menu(true);
                            }
                            KeyCode::Enter if app.active_menu.is_some() => {
                                handle_menu_action(app);
                                app.close_menu();
                            }
                            // Find bar and dropdown menus are not centered popups
                            // (popup_focus is None). Swallow leftover keys so Space
                            // cannot type into the buffer behind them.
                            _ if any_modal_open(app) => {}
                            KeyCode::Char(ch) => {
                                app.active_buffer_mut().insert_char(ch);
                            }
                            KeyCode::Enter => {
                                let tab = app.config.tab_size;
                                app.active_buffer_mut().newline_with_indent(tab);
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
                                let tab = app.config.tab_size;
                                app.active_buffer_mut().insert_tab(tab);
                            }
                            KeyCode::BackTab => {
                                let tab = app.config.tab_size;
                                app.active_buffer_mut().dedent(tab);
                            }
                            KeyCode::Esc => {
                                // Drop the range and the anchor. Clearing only the
                                // range lets the next Shift-move revive the old span.
                                app.active_buffer_mut().clear_selection();
                            }
                            _ => {}
                        }
                    }
                    Event::Mouse(mouse) => handle_mouse(mouse, app),
                    Event::Resize(_, _) => {}
                    Event::FocusGained => {
                        let _ = app.check_external_changes();
                    }
                    _ => {}
                }
            }

            let cursor_pos_after = app.active_buffer().cursor;
            if cursor_pos_before != cursor_pos_after {
                cursor_blink_on = true;
                last_blink = Instant::now();
            }
            if app.should_quit {
                return Ok(());
            }
            let disk_changed = app.check_external_changes();
            if batch_needs_redraw(&events, wheel_delta, hover_before, hover_visual(app))
                || cursor_pos_before != cursor_pos_after
                || disk_changed
            {
                break 'wait; // real change → redraw
            }
            // else idle mouse-move: stay in 'wait. Do NOT hide/draw/show. Do NOT reset last_blink.
        }

        if app.should_quit {
            break 'outer;
        }
    }
    Ok(())
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

fn wheel_pos(event: &Event) -> Option<(u16, u16)> {
    if let Event::Mouse(m) = event {
        match m.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => Some((m.column, m.row)),
            _ => None,
        }
    } else {
        None
    }
}

/// Update hover states for the focused popup only. Buried popups must not
/// highlight through the top layer, so their hover fields are cleared first.
fn update_hover_states(x: u16, y: u16, app: &mut App) {
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_area = Rect::new(0, 0, tw, th);

    // Clear unfocused hover so a confirm/dialog underneath cannot light up.
    app.file_dialog_button_hover = None;
    app.browser_button_hover = None;
    app.browser_list_hover = None;
    app.confirm_button_hover = None;
    app.reload_button_hover = None;
    app.configure_row_hover = None;
    app.configure_button_hover = None;
    app.themes_list_hover = None;
    app.themes_button_hover = None;

    match app.popup_focus() {
        Some(PopupFocus::FileDialog) => {
            let popup = popups::file_dialog::file_dialog_rect(term_area);
            let btn_y = popups::file_dialog::file_dialog_button_row(popup) as usize;

            if y as usize == btn_y {
                let labels = popups::file_dialog::button_labels(app.file_dialog_is_save);
                let ranges = crate::ui::button::ButtonRow::button_x_ranges(popup.x + 1, &labels);
                let hit = ranges.iter().position(|(s, e)| x >= *s && x < *e);
                // `file_dialog_button_hover` is the transient hit-test used by the
                // click handler to know a button was actually clicked.
                app.file_dialog_button_hover = hit;
                // Hover steals the selection: the highlighted button follows the
                // mouse and STAYS put when the mouse leaves the button row, rather
                // than snapping back to the last keyboard-selected button. We do
                // NOT reset `file_dialog_selected` in the else-branch — that reset
                // is exactly the "jump back" behavior we're removing.
                if let Some(idx) = hit {
                    app.file_dialog_selected = idx;
                }
            }
        }
        Some(PopupFocus::FileBrowser) => {
            let popup = popups::file_browser::file_browser_rect(term_area);
            let btn_y = popups::file_browser::footer_button_row(popup) as usize;
            let name_y = popups::file_browser::filename_row(popup) as usize;
            // Horizontal bounds of the popup. The list/Name hit-tests below check
            // the mouse's row (y) but not its column (x); without this guard the
            // row highlight and Name-field focus chase the cursor even when it is
            // off to the side of the popup.
            let in_popup_x =
                x as usize >= popup.x as usize && (x as usize) < (popup.x + popup.width) as usize;
            if y as usize == btn_y {
                let labels = app.browser_footer_labels();
                let ranges = crate::ui::button::ButtonRow::button_x_ranges(popup.x + 1, &labels);
                let hit = ranges.iter().position(|(s, e)| x >= *s && x < *e);
                // Transient hit-test used by the click handler.
                app.browser_button_hover = hit;
                // Hover steals the footer-button selection so the highlight stays
                // on the last-hovered button when the mouse leaves the row, rather
                // than snapping back to the keyboard-focused button.
                if let Some(idx) = hit {
                    app.browser_button_selected = idx;
                }
            } else if app.browser_origin_is_save && in_popup_x && y as usize == name_y {
                app.browser_button_selected = popups::file_browser::FOCUS_FILENAME;
                app.browser_list_hover = None;
            }

            // List-row hover preview. Uses the same row->index math as the click
            // handler so the highlighted row is always the one that would activate
            // on a click. Entries past the last row (blank padding) yield None.
            let area = popups::file_browser::list_area(popup, app.browser_origin_is_save);
            let overflowing = popups::file_browser::list_overflow_bar(
                app.browser_entries.len(),
                app.browser_selected,
                popup,
                app.browser_origin_is_save,
            )
            .overflowing();
            let on_sb = popups::file_browser::list_click_is_scrollbar(
                x as usize,
                y as usize,
                popup,
                app.browser_origin_is_save,
                overflowing,
            );
            let list_hit = if !in_popup_x || on_sb {
                None
            } else {
                popups::file_browser::list_index_at(y as usize, area, app.browser_selected)
                    .filter(|&i| i < app.browser_entries.len())
            };
            app.browser_list_hover = list_hit;
            // Mouse owns focus: being over the list means list focus (sentinel 4),
            // so the previously-hovered footer button doesn't stay highlighted too.
            if list_hit.is_some() {
                app.browser_button_selected = popups::file_browser::FOCUS_LIST;
            }
        }
        Some(PopupFocus::Reload) => {
            let popup = popups::reload::reload_rect(term_area, &app.reload_message);
            let btn_y = popups::reload::button_row_y(popup) as usize;
            if y as usize == btn_y {
                let labels = popups::reload::button_labels();
                let ranges = crate::ui::button::ButtonRow::button_x_ranges(popup.x + 1, &labels);
                let hit = hit_button_row(x, &ranges);
                app.reload_button_hover = hit;
                if let Some(idx) = hit {
                    app.reload_selected = idx;
                }
            }
        }
        Some(PopupFocus::Confirm) => {
            let msg = app.confirm_message.clone();
            let popup = popups::confirm::confirm_rect(term_area, &msg);
            let btn_y = popups::confirm::confirm_button_row(popup) as usize;

            if y as usize == btn_y {
                let btn_x_start = popup.x + 2;
                let confirm_len = "[ Y ] Confirm".chars().count() as u16;

                if x >= btn_x_start && x < btn_x_start + confirm_len {
                    app.confirm_button_hover = Some(0);
                } else if x >= btn_x_start + confirm_len + 3
                    && x < btn_x_start + confirm_len + 3 + 12
                {
                    app.confirm_button_hover = Some(1);
                }
            }
        }
        Some(PopupFocus::Configure) => {
            let popup = popups::configure::configure_rect(term_area);
            let row_ys = popups::configure::row_ys(popup);
            if y >= popup.y
                && y < popup.y + popup.height
                && x >= popup.x
                && x < popup.x + popup.width
            {
                let mut on_row = None;
                for (i, &ry) in row_ys.iter().enumerate() {
                    if y == ry {
                        on_row = Some(i);
                    }
                }
                app.configure_row_hover = on_row;
                // Footer button hover.
                let fy = popups::configure::footer_row_y(popup);
                if y == fy {
                    let labels = popups::configure::footer_labels();
                    let ranges = crate::ui::button::ButtonRow::button_x_ranges(popup.x, &labels);
                    app.configure_button_hover = hit_button_row(x, &ranges);
                }
            }
        }
        Some(PopupFocus::Themes) => {
            themes_popup_handle_hover(app, (x as usize, y as usize), (tw, th));
        }
        Some(PopupFocus::Help | PopupFocus::WordCount) => {}
        None => {
            // Menus are not a PopupFocus; skip them entirely while a popup owns input.
            if let Some(menu_name) = app.active_menu.as_deref() {
                let dropdown = crate::ui::header::dropdown_rect(term_area, menu_name);

                if x >= dropdown.x
                    && x < dropdown.x + dropdown.width
                    && y >= dropdown.y
                    && y < dropdown.y + dropdown.height
                {
                    let inner_y = y - dropdown.y;
                    let item_idx = (inner_y as usize).saturating_sub(1);

                    let item_count = crate::ui::header::menu_len(menu_name);

                    if item_idx < item_count {
                        app.menu_selected = item_idx;
                    }
                }
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

fn handle_reload_click(x: usize, y: usize, app: &mut App) {
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_area = Rect::new(0, 0, tw, th);
    let popup = popups::reload::reload_rect(term_area, &app.reload_message);
    if x >= popup.x as usize
        && x < (popup.x + popup.width) as usize
        && y >= popup.y as usize
        && y < (popup.y + popup.height) as usize
    {
        let btn_y = popups::reload::button_row_y(popup) as usize;
        if y == btn_y {
            if let Some(idx) = app.reload_button_hover {
                let choice = match idx {
                    0 => ReloadChoice::Reload,
                    1 => ReloadChoice::Always,
                    _ => ReloadChoice::Keep,
                };
                app.close_reload(choice);
            }
        }
    } else {
        app.close_reload(ReloadChoice::Keep);
    }
}

fn handle_confirm_click(x: usize, y: usize, app: &mut App) {
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
    } else {
        app.close_confirm(false);
    }
}

fn handle_file_browser_click(x: usize, y: usize, app: &mut App) {
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_area = Rect::new(0, 0, tw, th);
    let popup = popups::file_browser::file_browser_rect(term_area);
    if x >= popup.x as usize
        && x < (popup.x + popup.width) as usize
        && y >= popup.y as usize
        && y < (popup.y + popup.height) as usize
    {
        let footer_y = popups::file_browser::footer_button_row(popup) as usize;
        let name_y = popups::file_browser::filename_row(popup) as usize;
        let list_area = popups::file_browser::list_area(popup, app.browser_origin_is_save);
        if y == footer_y {
            if let Some(idx) = app.browser_button_hover {
                app.browser_activate_footer(idx);
            }
        } else if app.browser_origin_is_save && y == name_y {
            app.browser_button_selected = popups::file_browser::FOCUS_FILENAME;
        } else if y >= list_area.0 && y < list_area.1 {
            let overflowing = popups::file_browser::list_overflow_bar(
                app.browser_entries.len(),
                app.browser_selected,
                popup,
                app.browser_origin_is_save,
            )
            .overflowing();
            if popups::file_browser::list_click_is_scrollbar(
                x,
                y,
                popup,
                app.browser_origin_is_save,
                overflowing,
            ) {
                apply_browser_scrollbar_at(app, y as u16);
                app.popup_scrollbar_drag = Some(PopupFocus::FileBrowser);
            } else {
                // List row click: select + activate (dir enters, file confirms).
                // Shares the row->index math with the hover preview so the clicked
                // row is always the highlighted one. Recent `[x]` is tested first
                // so a clear click never opens the file.
                let body_h = (list_area.1 - list_area.0).max(1);
                if let Some(idx) =
                    popups::file_browser::list_index_at(y, list_area, app.browser_selected)
                        .filter(|&i| i < app.browser_entries.len())
                {
                    if app.browser_mode == popups::file_browser::BrowserMode::Recent
                        && popups::file_browser::recent_row_click_is_clear(x, popup, overflowing)
                    {
                        app.remove_recent_at(idx);
                    } else {
                        app.browser_select_index(idx);
                        app.browser_enter_selected();
                    }
                }
                app.editor_visible_lines.set(body_h);
            }
        }
    } else {
        app.close_file_browser(false);
    }
}

fn handle_file_dialog_click(x: usize, y: usize, app: &mut App) {
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
    } else {
        app.close_file_dialog(2); // Cancel
    }
}

fn handle_configure_click(x: usize, y: usize, app: &mut App) {
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_area = Rect::new(0, 0, tw, th);
    let popup = popups::configure::configure_rect(term_area);
    let in_popup = (y as u16 >= popup.y && (y as u16) < popup.y + popup.height)
        && (x as u16 >= popup.x && (x as u16) < popup.x + popup.width);
    if in_popup {
        let row_ys = popups::configure::row_ys(popup);
        for (i, &ry) in row_ys.iter().enumerate() {
            if y as u16 == ry {
                if i == popups::configure::ROW_TAB_SIZE {
                    // Tab size: click "<" to decrease, ">" to increase.
                    let (dec_x, inc_x) = popups::configure::tab_size_arrow_xs(popup);
                    let delta = if x as u16 == dec_x {
                        -1
                    } else if x as u16 == inc_x {
                        1
                    } else {
                        0
                    };
                    if delta != 0 {
                        app.configure_adjust(i, delta);
                    }
                } else {
                    app.configure_toggle(i);
                }
                app.configure_selected = i;
            }
        }
        let fy = popups::configure::footer_row_y(popup);
        if y as u16 == fy {
            let labels = popups::configure::footer_labels();
            let ranges = crate::ui::button::ButtonRow::button_x_ranges(popup.x, &labels);
            if let Some(idx) = hit_button_row(x as u16, &ranges) {
                if idx == 0 {
                    app.configure_toggle(app.configure_selected);
                } else {
                    app.close_configure();
                }
            }
        }
    } else {
        app.close_configure();
    }
}

fn handle_help_click(x: usize, y: usize, app: &mut App) {
    let (h_tw, h_th) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_area = Rect::new(0, 0, h_tw, h_th);
    let popup = popups::help::help_rect(term_area);
    if x >= popup.x as usize
        && x < (popup.x + popup.width) as usize
        && y >= popup.y as usize
        && y < (popup.y + popup.height) as usize
    {
        if popups::help::help_click_is_scrollbar(
            x,
            y,
            popup,
            app.help_tab,
            app.help_content_scroll.0,
        ) {
            apply_help_scrollbar_at(app, y as u16);
            app.popup_scrollbar_drag = Some(PopupFocus::Help);
            return;
        }
        // Tab strip sits at inner row y = popup.y + 1.
        let tab_row = (popup.y + 1) as usize;
        if y == tab_row {
            let tabs = popups::help::help_tab_rects();
            for (_label, tab, x_start, x_end) in tabs {
                let sx = (popup.x + *x_start) as usize;
                let ex = (popup.x + *x_end) as usize;
                if x >= sx && x < ex {
                    app.help_tab = *tab;
                    app.help_content_scroll = (0, 0);
                    break;
                }
            }
        }
    } else {
        app.close_help();
    }
}

fn handle_mouse(mouse: MouseEvent, app: &mut App) {
    let (x, y) = (mouse.column, mouse.row);
    app.last_mouse_pos = Some((x, y));

    if matches!(mouse.kind, MouseEventKind::Down(_)) {
        app.format_status = None;
    }

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            update_hover_states(x, y, app);
            let x = mouse.column as usize;
            let y = mouse.row as usize;

            let mut skip_editor = false;

            // === Dropdown menu clicks ===
            // Leftover open menus are ignored for input once a popup has focus.
            // A menu action that *opens* a popup still runs here because
            // popup_is_open is false until after handle_menu_action.
            if !app.popup_is_open() {
                if let Some(ref menu_name) = app.active_menu {
                    let (menu_x, num_items) = (
                        crate::ui::header::menu_x(menu_name.as_str()) as usize,
                        crate::ui::header::menu_len(menu_name.as_str()),
                    );
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
            }

            // Only the focused popup receives the click. A menu action that
            // just opened a popup must not be treated as an outside-dismiss.
            if !skip_editor {
                if let Some(focus) = app.popup_focus() {
                    match focus {
                        PopupFocus::Confirm => handle_confirm_click(x, y, app),
                        PopupFocus::Reload => handle_reload_click(x, y, app),
                        PopupFocus::FileBrowser => handle_file_browser_click(x, y, app),
                        PopupFocus::FileDialog => handle_file_dialog_click(x, y, app),
                        PopupFocus::Configure => handle_configure_click(x, y, app),
                        PopupFocus::Themes => {
                            themes_popup_handle_click(app, (x, y), &mut skip_editor)
                        }
                        PopupFocus::Help => handle_help_click(x, y, app),
                        PopupFocus::WordCount => {
                            app.close_word_count();
                        }
                    }
                    skip_editor = true;
                }
            }

            if !skip_editor {
                if let Some(hbar) = editor_hscrollbar_rect(app) {
                    if y as u16 == hbar.y {
                        if let Some(target) = hscrollbar_target_for_col(app, x as u16) {
                            let buf = app.active_buffer_mut();
                            buf.scroll_offset.1 = target;
                            buf.clamp_hscroll();
                            app.hscrollbar_drag_active = true;
                            app.scrollbar_drag_active = false;
                            app.mouse_drag_active = false;
                            skip_editor = true;
                        }
                    }
                }
            }

            if !skip_editor {
                let header_rows = crate::ui::layout::HEADER_ROWS as usize;
                // Scrollbar lives in the rightmost column of the editor render
                // area. Handle click-to-jump / drag-to-scrub here, ahead of
                // header/editor handling, so a scrollbar press never places the
                // cursor or starts a text selection. (find-bar row is excluded
                // by `scrollbar_target_for_row`'s vis-bounded y range.)
                let (tw, _th) = crossterm::terminal::size().unwrap_or((80, 24));
                if is_editor_vscrollbar_col(x, y, tw as usize) {
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
                        app.hscrollbar_drag_active = false;
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
                            let gutter = app.gutter_width();
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
                                    // Screen-row ensure: in-view clicks are a no-op. The
                                    // unwrapped helper treats scroll_offset.0 as a logical
                                    // line and jumps the page under wrap.
                                    buf.ensure_cursor_visible_wrapped(&layout, vis);
                                } else {
                                    let hscroll = buf.scroll_offset.1;
                                    // visual_col must account for hscroll: screen col 0 in content == logical hscroll
                                    let visual_col = col_in_row + hscroll;
                                    let visual_line = buf.scroll_offset.0 + editor_y;
                                    buf.set_cursor_from_visual(visual_line, visual_col);
                                    // Do not force recenter here (ensure only if offscreen).
                                    buf.ensure_cursor_visible(vis);
                                }
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
            // Popup overflow-bar scrub is independent of the editor scrollbar
            // flag (that flag is cleared while a modal is open).
            match app.popup_scrollbar_drag {
                Some(PopupFocus::Help) => {
                    apply_help_scrollbar_at(app, y);
                    return;
                }
                Some(PopupFocus::FileBrowser) => {
                    apply_browser_scrollbar_at(app, y);
                    return;
                }
                Some(PopupFocus::Themes) => {
                    apply_themes_role_scrollbar_at(app, y);
                    return;
                }
                _ => {}
            }
            // Two drag modes (mutually exclusive): scrollbar scrub vs text
            // selection. The flag is set on MouseDown; route accordingly.
            if app.hscrollbar_drag_active {
                if let Some(target) = hscrollbar_target_for_col(app, x) {
                    let buf = app.active_buffer_mut();
                    buf.scroll_offset.1 = target;
                    buf.clamp_hscroll();
                }
                return;
            }
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
        // Middle-click on a file tab closes that tab (standard TUI behavior).
        // Reuses the header hit-test so the same tab geometry drives both the
        // left-click focus and the middle-click close. A modified tab still
        // routes through the unsaved-changes confirm dialog.
        MouseEventKind::Down(MouseButton::Middle) => {
            if !app.popup_is_open() {
                let width = app.header_width.get();
                let width = if width == 0 {
                    crossterm::terminal::size()
                        .map(|(w, _)| w as usize)
                        .unwrap_or(80)
                } else {
                    width
                };
                let names = app.tab_names();
                let layout = crate::ui::header::layout_tab_strip(&names, width, app.tab_scroll);
                if let crate::ui::header::HeaderHit::Tab(i) =
                    crate::ui::header::hit_test_header(x as usize, &layout)
                {
                    if i < app.buffers.len() {
                        app.close_tab(i);
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            // End any in-progress drag selection session. Edge auto-scroll keys
            // off this flag, so it must be cleared as soon as the button releases.
            app.mouse_drag_active = false;
            app.scrollbar_drag_active = false;
            app.hscrollbar_drag_active = false;
            app.popup_scrollbar_drag = None;
        }
        // Horizontal mouse-wheel (Shift+wheel on most terminals) scrolls the
        // view sideways. Vertical wheel is coalesced in run_app, but horizontal
        // events pass through here since wheel_lines ignores them. Swallow
        // while a popup owns focus so the editor underneath cannot pan.
        MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight if app.popup_is_open() => {}
        MouseEventKind::ScrollLeft => {
            if wheel_target(app, x, y) == WheelTarget::TabStrip {
                app.scroll_tab_strip(-1);
            } else {
                app.active_buffer_mut().scroll_horizontal(-4);
            }
        }
        MouseEventKind::ScrollRight => {
            if wheel_target(app, x, y) == WheelTarget::TabStrip {
                app.scroll_tab_strip(1);
            } else {
                app.active_buffer_mut().scroll_horizontal(4);
            }
        }
        _ => {}
    }
}

fn editor_hscrollbar_rect(app: &App) -> Option<Rect> {
    if any_modal_open(app) || !app.editor_hscrollbar_visible.get() {
        return None;
    }
    let vis = app.editor_visible_lines.get().max(1);
    let content_w = app.editor_visible_width.get().max(1);
    let bar_y = crate::ui::layout::HEADER_ROWS + vis as u16;
    // Gutter + text + the 1-col vertical bar, so `┘` is clickable too.
    let width = (app.gutter_width() + content_w + 1) as u16;
    Some(Rect::new(0, bar_y, width, 1))
}

fn hscrollbar_target_for_col(app: &App, x: u16) -> Option<usize> {
    let area = editor_hscrollbar_rect(app)?;
    let buf = app.active_buffer();
    let bar = crate::ui::scrollbar::OverflowBar {
        // +1 so max_position == max_hscroll (extra EOL caret column).
        total: buf.max_line_width.saturating_add(1),
        position: buf.scroll_offset.1,
        viewport: buf.visible_width.max(1),
    };
    bar.target_for_col(x, area)
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
    let gutter = app.gutter_width();
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
    let gutter = app.gutter_width();

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

/// Resolve which footer-button index (if any) a mouse column hits, given the
/// `(start,end)` x-ranges produced by [`ButtonRow::button_x_ranges`]. Shared by
/// the Configure and Themes click/hover handlers so both agree with the renderer.
fn hit_button_row(x: u16, ranges: &[(u16, u16)]) -> Option<usize> {
    for (i, (s, e)) in ranges.iter().enumerate() {
        if x >= *s && x < *e {
            return Some(i);
        }
    }
    None
}

// ===================== Themes popup input handlers =====================
//
// These helpers keep main.rs's event match arms short while giving the Themes
// popup the same keyboard + mouse parity as the other popups (AGENTS Core
// Principle 2). State mutations go through `App` methods where possible.

fn themes_popup_handle_vert(app: &mut App, delta: i32) {
    // List mode only: Edit wheel calls `scroll_roles` directly; Edit arrows go
    // through `handle_edit_key`.
    let registry = crate::themes::ThemeRegistry::load();
    let n = registry.all().len();
    if n == 0 {
        return;
    }
    let new = ((app.themes_selected as i32 + delta) % n as i32 + n as i32) % n as i32;
    app.themes_selected = new as usize;
}

fn themes_popup_handle_enter(app: &mut App) {
    // List mode only: Edit Enter is handled by `handle_edit_key`.
    app.apply_theme_by_index(app.themes_selected);
}

fn themes_popup_handle_esc(app: &mut App) {
    use popups::themes::ThemesMode;
    if app.themes_mode == ThemesMode::Edit {
        // Cancel editor → back to list.
        app.themes_edit = None;
        app.themes_mode = ThemesMode::List;
        app.popup_scrollbar_drag = None;
    } else {
        app.close_themes();
    }
}

/// Begin editing. When `copy_name` is true we're making a new theme from the
/// currently selected source (built-in or custom) with a blank name — the
/// user must type one before saving. Editing a built-in also forces a copy.
fn themes_popup_start_edit(app: &mut App, copy_name: bool) {
    use popups::themes::{ThemeEditState, ThemesMode};
    let registry = crate::themes::ThemeRegistry::load();
    let source = registry.all().get(app.themes_selected).cloned().cloned();
    let Some(source) = source else { return };
    let is_built_in = registry.is_built_in(&source.name);
    // Editing a built-in always becomes a Save-As (copy), so we never mutate
    // the hardcoded themes.
    let force_copy = is_built_in || copy_name;
    let name_override = if force_copy {
        Some(String::new())
    } else {
        None
    };
    let edit = ThemeEditState::new(source, name_override);
    app.themes_edit = Some(edit);
    app.themes_mode = ThemesMode::Edit;
}

/// Attempt to save the in-progress theme. On success, apply it and return to
/// list mode; on failure, surface the error inline (stored on the edit state).
fn themes_popup_try_save(app: &mut App) {
    use popups::themes::ThemesMode;
    let Some(edit) = app.themes_edit.as_ref() else {
        return;
    };
    let name = edit.name.trim().to_string();
    if name.is_empty() {
        // Can't save without a name — just stay in editor. The footer already
        // advertises the requirement.
        return;
    }
    let mut theme = edit.theme.clone();
    theme.name = name.clone();
    // Mutate the registry in place: build it once, save through it, then drop.
    let mut registry = crate::themes::ThemeRegistry::load();
    match registry.save_custom(theme.clone()) {
        Ok(_path) => {
            // Apply live + persist as the active theme.
            app.theme = theme.clone();
            app.config.set_theme(&theme.name);
            app.themes_edit = None;
            app.themes_mode = ThemesMode::List;
            app.popup_scrollbar_drag = None;
            // Re-select the saved theme in the list.
            let idx = registry.all().iter().position(|t| t.name == theme.name);
            if let Some(i) = idx {
                app.themes_selected = i;
            }
        }
        Err(e) => {
            // Surface the error in the editor's name field (best-effort).
            if let Some(edit) = app.themes_edit.as_mut() {
                edit.color_err = Some(format!("save failed: {e}"));
            }
        }
    }
}

/// Request deletion of the selected theme. Built-ins refuse; customs are
/// removed immediately (a confirm popup could gate this, but for now we keep
/// the flow short — undo is via re-saving the .toml).
fn themes_popup_request_delete(app: &mut App) {
    let registry = crate::themes::ThemeRegistry::load();
    let name_opt = registry
        .all()
        .get(app.themes_selected)
        .map(|t| t.name.clone());
    let Some(name) = name_opt else { return };
    if registry.is_built_in(&name) {
        return; // can't delete built-ins
    }
    let mut registry = crate::themes::ThemeRegistry::load();
    let _ = registry.delete_custom(&name);
    // Clamp selection into the new range.
    let n = registry.all().len();
    if app.themes_selected >= n && n > 0 {
        app.themes_selected = n - 1;
    }
}

/// Mouse hover for the Themes popup (both modes).
fn themes_popup_handle_hover(app: &mut App, pos: (usize, usize), term: (u16, u16)) {
    use popups::themes::ThemesMode;
    let (x, y) = pos;
    let term_area = Rect::new(0, 0, term.0, term.1);
    let (popup, footer_y, footer_labels) = match app.themes_mode {
        ThemesMode::List => {
            let p = popups::themes::list_rect(term_area);
            (
                p,
                popups::themes::list_footer_y(p),
                popups::themes::list_footer_labels().to_vec(),
            )
        }
        ThemesMode::Edit => {
            let p = popups::themes::edit_rect(term_area);
            (
                p,
                popups::themes::footer_row_y(p),
                popups::themes::edit_footer_labels().to_vec(),
            )
        }
    };
    let in_popup = (y as u16) >= popup.y
        && (y as u16) < popup.y + popup.height
        && (x as u16) >= popup.x
        && (x as u16) < popup.x + popup.width;

    if !in_popup {
        app.themes_list_hover = None;
        app.themes_button_hover = None;
        return;
    }

    // Footer button hover. List keeps its historical saturating_sub offset;
    // Edit hits `footer_row_y` exactly (Configure style).
    let on_footer = match app.themes_mode {
        ThemesMode::List => y as u16 == footer_y.saturating_sub(1),
        ThemesMode::Edit => y as u16 == footer_y,
    };
    if on_footer {
        let ranges = popups::themes::themes_button_x_ranges(popup.x + 1, &footer_labels);
        app.themes_button_hover = hit_button_row(x as u16, &ranges);
    } else {
        app.themes_button_hover = None;
    }

    // List-mode row hover.
    if app.themes_mode == ThemesMode::List {
        let registry = crate::themes::ThemeRegistry::load();
        let built_in_count = registry.built_ins().len();
        let total = registry.all().len();
        app.themes_list_hover =
            popups::themes::list_row_to_index(y as u16, popup, built_in_count, total);
    }
}

/// Mouse click for the Themes popup (both modes).
fn themes_popup_handle_click(app: &mut App, pos: (usize, usize), skip_editor: &mut bool) {
    use popups::themes::{EditFocus, ThemesMode};
    let (x, y) = pos;
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_area = Rect::new(0, 0, tw, th);
    let (popup, footer_y, footer_labels) = match app.themes_mode {
        ThemesMode::List => {
            let p = popups::themes::list_rect(term_area);
            (
                p,
                popups::themes::list_footer_y(p),
                popups::themes::list_footer_labels().to_vec(),
            )
        }
        ThemesMode::Edit => {
            let p = popups::themes::edit_rect(term_area);
            (
                p,
                popups::themes::footer_row_y(p),
                popups::themes::edit_footer_labels().to_vec(),
            )
        }
    };
    let in_popup = (y as u16) >= popup.y
        && (y as u16) < popup.y + popup.height
        && (x as u16) >= popup.x
        && (x as u16) < popup.x + popup.width;

    if !in_popup {
        // Click outside closes (list) or cancels (editor).
        themes_popup_handle_esc(app);
        *skip_editor = true;
        return;
    }
    *skip_editor = true;

    // Footer button click. List keeps saturating_sub; Edit hits footer_row_y.
    let on_footer = match app.themes_mode {
        ThemesMode::List => y as u16 == footer_y.saturating_sub(1),
        ThemesMode::Edit => y as u16 == footer_y,
    };
    if on_footer {
        let ranges = popups::themes::themes_button_x_ranges(popup.x + 1, &footer_labels);
        if let Some(idx) = hit_button_row(x as u16, &ranges) {
            match app.themes_mode {
                ThemesMode::List => match idx {
                    0 => app.apply_theme_by_index(app.themes_selected),
                    1 => themes_popup_start_edit(app, false),
                    2 => themes_popup_start_edit(app, true),
                    3 => themes_popup_request_delete(app),
                    _ => app.close_themes(),
                },
                ThemesMode::Edit => match idx {
                    0 => themes_popup_try_save(app),
                    _ => themes_popup_handle_esc(app),
                },
            }
            return;
        }
    }

    // List-mode: row click applies the theme.
    if app.themes_mode == ThemesMode::List {
        let registry = crate::themes::ThemeRegistry::load();
        let built_in_count = registry.built_ins().len();
        let total = registry.all().len();
        if let Some(idx) = popups::themes::list_row_to_index(y as u16, popup, built_in_count, total)
        {
            app.themes_selected = idx;
            app.apply_theme_by_index(app.themes_selected);
        }
        return;
    }

    if y as u16 == popups::themes::name_row_y(popup) {
        if let Some(edit) = app.themes_edit.as_mut() {
            edit.focus = EditFocus::Name;
        }
        return;
    }
    if y as u16 == popups::themes::hex_row_y(popup) {
        if let Some(edit) = app.themes_edit.as_mut() {
            edit.focus = EditFocus::Hex;
        }
        return;
    }

    let overflowing = app
        .themes_edit
        .as_ref()
        .map(|e| popups::themes::roles_overflow_bar(e.role_scroll).overflowing())
        .unwrap_or(false);
    if app.themes_mode == ThemesMode::Edit
        && popups::themes::roles_click_is_scrollbar(x, y, popup, overflowing)
    {
        apply_themes_role_scrollbar_at(app, y as u16);
        app.popup_scrollbar_drag = Some(PopupFocus::Themes);
        return;
    }

    let roles = popups::themes::roles_list_area(popup, overflowing);
    if (y as u16) >= roles.y
        && (y as u16) < roles.y.saturating_add(roles.height)
        && (x as u16) >= roles.x
        && (x as u16) < roles.x.saturating_add(roles.width)
    {
        let rel = (y as u16).saturating_sub(roles.y);
        if let Some(edit) = app.themes_edit.as_mut() {
            let idx = (edit.role_scroll as usize).saturating_add(rel as usize);
            if (rel as usize) < popups::themes::ROLE_VISIBLE as usize
                && idx < popups::themes::ROLE_FIELDS.len()
            {
                edit.role_selected = idx;
                use popups::themes::color_buf_for;
                use popups::themes::field_value;
                edit.color_buf = color_buf_for(&field_value(&edit.theme, edit.role_selected));
                edit.color_err = None;
                edit.focus = EditFocus::Role;
            }
        }
        return;
    }

    let pal = popups::themes::palette_area(popup);
    if let Some(i) = crate::ui::color_picker::hit_swatch(pal, x as u16, y as u16) {
        if let Some(edit) = app.themes_edit.as_mut() {
            edit.apply_swatch_at(i);
            edit.focus = EditFocus::Palette;
        }
        return;
    }

    for ch in 0u8..3 {
        if y as u16 != popups::themes::slider_row_y(popup, ch) {
            continue;
        }
        let bar = popups::themes::slider_bar_area(popup, ch);
        let (channel, focus) = match ch {
            0 => (crate::ui::color_picker::Channel::R, EditFocus::SliderR),
            1 => (crate::ui::color_picker::Channel::G, EditFocus::SliderG),
            _ => (crate::ui::color_picker::Channel::B, EditFocus::SliderB),
        };
        if let Some(edit) = app.themes_edit.as_mut() {
            edit.focus = focus;
            let col = x as u16;
            if col >= bar.x && col < bar.x.saturating_add(bar.width) {
                let v = crate::ui::color_picker::slider_value_at_x(
                    col.saturating_sub(bar.x),
                    crate::ui::color_picker::SLIDER_BAR_W,
                );
                edit.set_channel_value(channel, v);
            }
        }
        return;
    }
}

/// Ctrl shortcuts that may run while a centered popup has focus.
enum PopupCtrl {
    Quit,
    BrowserToggleHidden,
}

fn popup_ctrl_action(code: KeyCode, app: &App) -> Option<PopupCtrl> {
    match code {
        KeyCode::Char('q') => Some(PopupCtrl::Quit),
        KeyCode::Char('h') if app.popup_focus() == Some(PopupFocus::FileBrowser) => {
            Some(PopupCtrl::BrowserToggleHidden)
        }
        _ => None,
    }
}

fn handle_reload_key(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Left | KeyCode::BackTab => {
            app.reload_selected = if app.reload_selected == 0 {
                2
            } else {
                app.reload_selected - 1
            };
        }
        KeyCode::Right | KeyCode::Tab => {
            app.reload_selected = (app.reload_selected + 1) % 3;
        }
        KeyCode::Char('y' | 'Y' | 'r' | 'R') => {
            app.close_reload(ReloadChoice::Reload);
        }
        KeyCode::Char('a' | 'A') => {
            app.close_reload(ReloadChoice::Always);
        }
        KeyCode::Char('n' | 'N' | 'k' | 'K') | KeyCode::Esc => {
            app.close_reload(ReloadChoice::Keep);
        }
        KeyCode::Enter => {
            let choice = match app.reload_selected {
                0 => ReloadChoice::Reload,
                1 => ReloadChoice::Always,
                _ => ReloadChoice::Keep,
            };
            app.close_reload(choice);
        }
        _ => {}
    }
}

/// Returns true when the caller should leave `run_app` immediately (quit confirmed).
fn handle_confirm_key(code: KeyCode, app: &mut App) -> bool {
    match code {
        KeyCode::Left => {
            app.confirm_selected = 0;
            false
        }
        KeyCode::Right => {
            app.confirm_selected = 1;
            false
        }
        KeyCode::Char('y' | 'Y') => {
            let is_quit = app.confirm_action.as_deref() == Some("quit");
            app.close_confirm(true);
            if is_quit {
                app.should_quit = true;
                return true;
            }
            false
        }
        KeyCode::Char('n' | 'N') => {
            app.close_confirm(false);
            false
        }
        KeyCode::Enter => {
            let confirmed = app.confirm_selected == 0; // 0 = Confirm
            let is_quit = app.confirm_action.as_deref() == Some("quit");
            app.close_confirm(confirmed);
            if confirmed && is_quit {
                app.should_quit = true;
            }
            false
        }
        KeyCode::Esc => {
            app.close_confirm(false);
            false
        }
        _ => false,
    }
}

fn handle_file_browser_key(code: KeyCode, app: &mut App) {
    // Focus model: `browser_button_selected` holds a footer index (0..=3),
    // FOCUS_LIST (list), or FOCUS_FILENAME (Save As Name field).
    // Open Tab: list -> 0 -> 1 -> 2 -> 3 -> list
    // Save As Tab: list -> Name -> 0 -> 1 -> 2 -> 3 -> list
    //
    // NOTE: Ctrl+H (toggle hidden) is handled up in the CONTROL block —
    // adding it here would be dead code.
    use popups::file_browser::{FOCUS_FILENAME, FOCUS_LIST};

    let name_focused = app.browser_origin_is_save && app.browser_button_selected == FOCUS_FILENAME;
    if name_focused {
        match code {
            KeyCode::Char(ch) if !ch.is_control() => app.browser_filename_push(ch),
            KeyCode::Backspace => app.browser_filename_backspace(),
            KeyCode::Enter => app.browser_use_current_folder(),
            KeyCode::Tab => app.browser_button_selected = 0,
            KeyCode::Esc => app.close_file_browser(false),
            KeyCode::Up => app.browser_button_selected = FOCUS_LIST,
            KeyCode::Down => app.browser_button_selected = 0,
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.browser_move_cursor(-1);
            app.browser_button_selected = FOCUS_LIST;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.browser_move_cursor(1);
            app.browser_button_selected = FOCUS_LIST;
        }
        KeyCode::Char('g') => {
            app.browser_jump(false);
        }
        KeyCode::Char('G') => {
            app.browser_jump(true);
        }
        KeyCode::PageUp => {
            let vh = app.editor_visible_lines.get().max(4);
            app.browser_page(true, vh);
        }
        KeyCode::PageDown => {
            let vh = app.editor_visible_lines.get().max(4);
            app.browser_page(false, vh);
        }
        KeyCode::Tab => {
            app.browser_button_selected = if app.browser_origin_is_save {
                match app.browser_button_selected {
                    FOCUS_LIST => FOCUS_FILENAME,
                    FOCUS_FILENAME => 0,
                    n if n < 3 => n + 1,
                    _ => FOCUS_LIST,
                }
            } else {
                match app.browser_button_selected {
                    FOCUS_LIST => 0,
                    n if n < 3 => n + 1,
                    _ => FOCUS_LIST,
                }
            };
        }
        KeyCode::Enter => match app.browser_button_selected {
            0..=3 => app.browser_activate_footer(app.browser_button_selected),
            FOCUS_FILENAME => app.browser_use_current_folder(),
            _ => app.browser_enter_selected(),
        },
        // `l`/Right activate the focused thing (list entry or button).
        KeyCode::Char('l') | KeyCode::Right => match app.browser_button_selected {
            0..=3 => app.browser_activate_footer(app.browser_button_selected),
            FOCUS_FILENAME => {}
            _ => app.browser_enter_selected(),
        },
        KeyCode::Delete => {
            if app.browser_mode == popups::file_browser::BrowserMode::Recent
                && app.browser_button_selected == FOCUS_LIST
            {
                app.remove_recent_at(app.browser_selected);
            }
        }
        KeyCode::Backspace => {
            if app.browser_mode == popups::file_browser::BrowserMode::Recent
                && app.browser_button_selected == FOCUS_LIST
            {
                app.remove_recent_at(app.browser_selected);
            } else {
                app.browser_go_up();
            }
        }
        // `h`/Left/- go up one directory. (Plain `h` only —
        // Ctrl+H is handled up in the CONTROL block.)
        KeyCode::Char('-') | KeyCode::Left | KeyCode::Char('h') => {
            app.browser_go_up();
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.close_file_browser(false);
        }
        // Swallow every other key so unbound chars (e.g. 'x', 'f') don't
        // leak into the underlying file dialog's path field.
        _ => {}
    }
}

fn handle_file_dialog_key(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Left | KeyCode::BackTab => {
            // Cycle 0 -> 2 -> 1 -> 0 (right-to-left visual order).
            app.file_dialog_selected = match app.file_dialog_selected {
                0 => 2,
                2 => 1,
                _ => 0,
            };
        }
        KeyCode::Right | KeyCode::Tab => {
            // Cycle 0 -> 1 -> 2 -> 0.
            app.file_dialog_selected = match app.file_dialog_selected {
                0 => 1,
                1 => 2,
                _ => 0,
            };
        }
        KeyCode::Char(ch) => {
            app.file_dialog_input_char(ch);
        }
        KeyCode::Backspace => {
            app.file_dialog_backspace();
        }
        KeyCode::Enter => {
            // 0 = Save/Open, 1 = Browse…, 2 = Cancel
            app.close_file_dialog(app.file_dialog_selected);
        }
        KeyCode::Esc => {
            app.close_file_dialog(2); // Cancel
        }
        _ => {}
    }
}

fn handle_help_key(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Esc | KeyCode::F(1) => {
            app.close_help();
        }
        KeyCode::Tab => {
            app.help_tab = app.help_tab.next();
            app.help_content_scroll = (0, 0);
        }
        KeyCode::Char('1') => {
            app.help_tab = popups::help::HelpTab::Shortcuts;
            app.help_content_scroll = (0, 0);
        }
        KeyCode::Char('2') => {
            app.help_tab = popups::help::HelpTab::Languages;
            app.help_content_scroll = (0, 0);
        }
        KeyCode::Char('3') => {
            app.help_tab = popups::help::HelpTab::About;
            app.help_content_scroll = (0, 0);
        }
        KeyCode::Up | KeyCode::Char('k') => apply_help_scroll(app, -1),
        KeyCode::Down | KeyCode::Char('j') => apply_help_scroll(app, 1),
        KeyCode::PageUp => {
            let vis = popups::help::help_body_area(popups::help::help_rect(term_area())).height;
            apply_help_scroll(app, -(vis as i32).max(1));
        }
        KeyCode::PageDown => {
            let vis = popups::help::help_body_area(popups::help::help_rect(term_area())).height;
            apply_help_scroll(app, (vis as i32).max(1));
        }
        _ => {}
    }
}

fn handle_configure_key(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Esc => {
            app.close_configure();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let n = App::configure_row_count();
            app.configure_selected = (app.configure_selected + 1) % n;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let n = App::configure_row_count();
            app.configure_selected = if app.configure_selected == 0 {
                n - 1
            } else {
                app.configure_selected - 1
            };
        }
        KeyCode::Left => {
            app.configure_adjust(app.configure_selected, -1);
        }
        KeyCode::Right => {
            app.configure_adjust(app.configure_selected, 1);
        }
        KeyCode::Enter => {
            app.configure_toggle(app.configure_selected);
        }
        _ => {}
    }
}

fn handle_themes_key(code: KeyCode, app: &mut App) {
    use popups::themes::{ThemesEditCmd, ThemesMode};
    if app.themes_mode == ThemesMode::Edit {
        let cmd = if let Some(edit) = app.themes_edit.as_mut() {
            popups::themes::handle_edit_key(edit, code)
        } else {
            return;
        };
        match cmd {
            ThemesEditCmd::Save => themes_popup_try_save(app),
            ThemesEditCmd::Cancel => themes_popup_handle_esc(app),
            ThemesEditCmd::None => {}
        }
        return;
    }
    match code {
        KeyCode::Esc => {
            themes_popup_handle_esc(app);
        }
        KeyCode::Char('j') | KeyCode::Down => {
            themes_popup_handle_vert(app, 1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            themes_popup_handle_vert(app, -1);
        }
        KeyCode::Enter => {
            themes_popup_handle_enter(app);
        }
        KeyCode::Char('e') => {
            themes_popup_start_edit(app, /*copy_name=*/ false);
        }
        KeyCode::Char('n') => {
            themes_popup_start_edit(app, /*copy_name=*/ true);
        }
        KeyCode::Char('d') => {
            themes_popup_request_delete(app);
        }
        KeyCode::Char('s') => {
            themes_popup_try_save(app);
        }
        _ => {}
    }
}

fn handle_word_count_key(code: KeyCode, app: &mut App) {
    if matches!(code, KeyCode::Esc) {
        app.close_word_count();
    }
}

/// True when `(x, y)` is the editor vertical scrollbar column. Header row
/// last cells (`[+]`, `[>]`) must not be treated as the scrollbar.
fn is_editor_vscrollbar_col(x: usize, y: usize, term_width: usize) -> bool {
    y >= crate::ui::layout::HEADER_ROWS as usize && x + 1 == term_width
}

/// True when any modal/popup is open and the editor (and its drag selection)
/// should be suspended.
fn any_modal_open(app: &App) -> bool {
    app.show_help
        || app.show_word_count
        || app.show_configure
        || app.show_themes
        || app.show_file_dialog
        || app.show_file_browser
        || app.show_confirm
        || app.show_reload
        || app.active_menu.is_some()
        || app.find_state.is_some()
}

fn handle_header_click(x: usize, app: &mut App) {
    let width = app.header_width.get();
    let width = if width == 0 {
        crossterm::terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(80)
    } else {
        width
    };
    app.clamp_tab_strip(width);
    let names = app.tab_names();
    let layout = crate::ui::header::layout_tab_strip(&names, width, app.tab_scroll);
    match crate::ui::header::hit_test_header(x, &layout) {
        crate::ui::header::HeaderHit::Menu(name) => app.open_menu(name),
        crate::ui::header::HeaderHit::Tab(i) => {
            if i < app.buffers.len() {
                app.active = i;
                app.ensure_active_tab_visible();
            }
            app.close_menu();
        }
        crate::ui::header::HeaderHit::Plus => {
            app.new_tab();
            app.close_menu();
        }
        crate::ui::header::HeaderHit::ScrollLeft => {
            if layout.can_scroll_left {
                app.scroll_tab_strip(-1);
            }
            app.close_menu();
        }
        crate::ui::header::HeaderHit::ScrollRight => {
            if layout.can_scroll_right {
                app.scroll_tab_strip(1);
            }
            app.close_menu();
        }
        crate::ui::header::HeaderHit::Miss => {
            if app.active_menu.is_some() {
                app.close_menu();
            }
        }
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
            if app.find_state.is_none() {
                app.toggle_find();
            }
            if let Some(fs) = &mut app.find_state {
                fs.focus = FindFocus::Query;
            }
        }
        ("Tools", 1) => app.compute_and_show_word_count(),
        ("Tools", 2) => {
            app.format_current_document();
        }
        ("Tools", 3) => app.toggle_word_wrap(),
        ("Tools", 4) => app.toggle_line_comment(),
        ("Settings", 0) => app.open_configure(),
        ("Settings", 1) => app.open_themes(),
        ("Settings", 2) => app.open_help(), // Help (F1)
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
    // Text-area width = editor area minus live gutter minus scrollbar (1).
    let content_w = (chunks[1].width as usize)
        .saturating_sub(app.gutter_width() + 1)
        .max(1);
    // `vis_h` here is the full editor content+vbar height (find bar already
    // subtracted). Reserve one row for the h-bar when wrap is off and the
    // longest line exceeds the text width, then sync viewport to the shrunk
    // height so wrap math / cache / paint agree.
    let wrap = app.config.word_wrap;
    let max_w = {
        let b = app.active_buffer_mut();
        b.refresh_max_line_width();
        b.max_line_width
    };
    let show_hbar = crate::ui::scrollbar::needs_horizontal_scrollbar(wrap, max_w, content_w, vis_h);
    app.editor_hscrollbar_visible.set(show_hbar);
    let vis_h = if show_hbar {
        vis_h.saturating_sub(1).max(1)
    } else {
        vis_h
    };
    app.apply_editor_viewport(vis_h, content_w);

    app.clamp_tab_strip(chunks[0].width as usize);
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
        popups::word_count::render(
            f,
            area,
            app.current_theme(),
            &app.info_popup_title,
            &app.word_count_text,
        );
    }

    if app.show_configure {
        popups::configure::render(f, area, app.current_theme(), app);
    }

    if app.show_themes {
        popups::themes::render(f, area, app.current_theme(), app);
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

    if let Some(ref menu) = app.active_menu {
        crate::ui::header::render_dropdown(f, menu, area, app.current_theme(), app.menu_selected);
    }

    if app.show_reload {
        popups::reload::render(f, area, app.current_theme(), app);
    }

    // Quit confirm always paints last so it stacks above every other popup.
    if app.show_confirm {
        popups::confirm::render_confirm(f, area, app.current_theme(), &app.confirm_message, app);
    }
}

#[cfg(test)]
mod cursor_blink_tests {
    use super::*;
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    };
    use std::time::{Duration, Instant};

    fn moved(column: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn left_down(column: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn key_char(c: char) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        })
    }

    #[test]
    fn blink_timeout_is_remaining_interval() {
        let last = Instant::now() - Duration::from_millis(100);
        let t = next_blink_timeout(last, Duration::from_millis(533), true);
        assert!(t <= Duration::from_millis(433));
        assert!(t > Duration::from_millis(300));
    }

    #[test]
    fn blink_timeout_is_zero_when_due() {
        let last = Instant::now() - Duration::from_millis(800);
        assert_eq!(
            next_blink_timeout(last, Duration::from_millis(533), true),
            Duration::ZERO
        );
        assert!(blink_is_due(last, Duration::from_millis(533), true));
    }

    #[test]
    fn blink_timeout_is_long_when_cursor_hidden() {
        let t = next_blink_timeout(Instant::now(), Duration::from_millis(533), false);
        assert!(t >= Duration::from_secs(60));
        assert!(!blink_is_due(
            Instant::now(),
            Duration::from_millis(533),
            false
        ));
    }

    fn empty_hover() -> HoverVisual {
        HoverVisual::default()
    }

    #[test]
    fn idle_mouse_moves_do_not_need_redraw() {
        let events = vec![moved(10, 10), moved(11, 10), moved(12, 11)];
        assert!(!batch_needs_redraw(
            &events,
            0,
            empty_hover(),
            empty_hover()
        ));
    }

    #[test]
    fn mouse_move_needs_redraw_when_hover_changes() {
        let before = HoverVisual {
            configure_button_hover: None,
            ..HoverVisual::default()
        };
        let after = HoverVisual {
            configure_button_hover: Some(1),
            ..HoverVisual::default()
        };
        assert!(batch_needs_redraw(&[moved(4, 20)], 0, before, after));
    }

    #[test]
    fn mouse_move_needs_redraw_when_menu_selection_changes() {
        let before = HoverVisual {
            menu_selected: 0,
            ..HoverVisual::default()
        };
        let after = HoverVisual {
            menu_selected: 2,
            ..HoverVisual::default()
        };
        assert!(batch_needs_redraw(&[moved(4, 20)], 0, before, after));
    }

    #[test]
    fn click_or_key_or_wheel_always_needs_redraw() {
        assert!(batch_needs_redraw(
            &[left_down(8, 8)],
            0,
            empty_hover(),
            empty_hover()
        ));
        assert!(batch_needs_redraw(
            &[key_char('a')],
            0,
            empty_hover(),
            empty_hover()
        ));
        assert!(batch_needs_redraw(&[], 3, empty_hover(), empty_hover()));
    }

    #[test]
    fn resize_needs_redraw() {
        assert!(batch_needs_redraw(
            &[Event::Resize(80, 24)],
            0,
            empty_hover(),
            empty_hover()
        ));
    }
}

#[cfg(test)]
mod plus_button_hit_test_tests {
    use super::*;

    /// The `[+]` new-tab button's hit region must track the packed tab-strip
    /// layout. n=1..=3 still fit on width 80; n=4 overflows and `[+]` sits
    /// after `[>]` rather than past the last packed tab.
    #[test]
    fn plus_button_stays_aligned_as_tabs_accumulate() {
        for n in 1..=4usize {
            // App::new starts with one placeholder buffer; add tabs to reach N.
            let mut app = App::new(&[]);
            while app.buffers.len() < n {
                app.new_tab();
            }
            assert_eq!(app.buffers.len(), n, "setup: expected {n} buffers");

            app.clamp_tab_strip(80);
            let names = app.tab_names();
            let layout = crate::ui::header::layout_tab_strip(&names, 80, app.tab_scroll);
            let click_x = layout.plus_x + 1;

            let before = app.buffers.len();
            handle_header_click(click_x, &mut app);
            assert_eq!(
                app.buffers.len(),
                before + 1,
                "with {n} tab(s), a click at column {click_x} (middle of [+]) must create a new tab"
            );
        }
    }

    #[test]
    fn plus_stays_clickable_when_tabs_overflow() {
        let mut app = App::new(&[]);
        while app.buffers.len() < 8 {
            app.new_tab();
        }
        app.clamp_tab_strip(80);
        let names = app.tab_names();
        let layout = crate::ui::header::layout_tab_strip(&names, 80, app.tab_scroll);
        let before = app.buffers.len();
        handle_header_click(layout.plus_x + 1, &mut app);
        assert_eq!(app.buffers.len(), before + 1);
    }

    #[test]
    fn plus_last_cell_stays_clickable_when_tabs_overflow() {
        let mut app = App::new(&[]);
        while app.buffers.len() < 8 {
            app.new_tab();
        }
        app.clamp_tab_strip(80);
        let names = app.tab_names();
        let layout = crate::ui::header::layout_tab_strip(&names, 80, app.tab_scroll);
        let before = app.buffers.len();
        handle_header_click(layout.plus_x + crate::ui::header::PLUS_COLS - 1, &mut app);
        assert_eq!(app.buffers.len(), before + 1);
    }

    #[test]
    fn header_last_col_is_not_the_editor_scrollbar() {
        assert!(!is_editor_vscrollbar_col(79, 0, 80));
        assert!(is_editor_vscrollbar_col(79, 1, 80));
    }

    #[test]
    fn click_right_arrow_advances_tab_scroll() {
        let mut app = App::new(&[]);
        while app.buffers.len() < 8 {
            app.new_tab();
        }
        app.active = 0;
        app.clamp_tab_strip(80);
        app.tab_scroll = 0;
        app.clamp_tab_strip(80);
        let names = app.tab_names();
        let layout = crate::ui::header::layout_tab_strip(&names, 80, app.tab_scroll);
        let start = app.tab_scroll;
        handle_header_click(layout.right_arrow_x.expect("overflow"), &mut app);
        assert!(app.tab_scroll > start);
    }

    #[test]
    fn click_visible_tab_activates_it() {
        let mut app = App::new(&[]);
        app.new_tab();
        app.clamp_tab_strip(80);
        let names = app.tab_names();
        let layout = crate::ui::header::layout_tab_strip(&names, 80, app.tab_scroll);
        let tab1 = layout.tabs.iter().find(|t| t.index == 1).unwrap();
        handle_header_click(tab1.x + 1, &mut app);
        assert_eq!(app.active, 1);
    }

    #[test]
    fn middle_click_on_non_active_tab_closes_it_and_shifts_active() {
        let mut app = App::new(&[]);
        app.new_tab();
        app.new_tab();
        assert_eq!(app.buffers.len(), 3);
        // active is 2; middle-click tab index 1 (which sits before the active one).
        app.clamp_tab_strip(80);
        let names = app.tab_names();
        let layout = crate::ui::header::layout_tab_strip(&names, 80, app.tab_scroll);
        let tab1 = layout.tabs.iter().find(|t| t.index == 1).unwrap();
        let before = app.buffers.len();
        handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Middle),
                column: (tab1.x + 1) as u16,
                row: crate::ui::layout::HEADER_ROWS,
                modifiers: KeyModifiers::NONE,
            },
            &mut app,
        );
        assert_eq!(app.buffers.len(), before - 1);
        assert_eq!(app.active, 1);
    }

    #[test]
    fn middle_click_does_not_activate_the_tab() {
        let mut app = App::new(&[]);
        app.new_tab();
        app.new_tab();
        assert_eq!(app.buffers.len(), 3);
        // Force active to the first tab so the clicked tab is definitely not
        // active (a left-click would have switched to it).
        app.active = 0;
        app.clamp_tab_strip(80);
        let names = app.tab_names();
        let layout = crate::ui::header::layout_tab_strip(&names, 80, app.tab_scroll);
        let last = layout.tabs.iter().max_by_key(|t| t.index).unwrap();
        let active_before = app.active;
        handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Middle),
                column: (last.x + 1) as u16,
                row: crate::ui::layout::HEADER_ROWS,
                modifiers: KeyModifiers::NONE,
            },
            &mut app,
        );
        // Middle-click only closes; it must not switch the active tab to the
        // clicked one (unlike a left-click).
        assert_eq!(app.active, active_before);
        assert_eq!(app.buffers.len(), 2);
    }
}

#[cfg(test)]
mod popup_focus_key_tests {
    use super::*;
    use crate::app::PopupFocus;
    use crossterm::event::KeyCode;

    #[test]
    fn ctrl_q_allowed_when_configure_is_open() {
        let mut app = App::new(&[]);
        app.open_configure();
        assert!(matches!(
            popup_ctrl_action(KeyCode::Char('q'), &app),
            Some(PopupCtrl::Quit)
        ));
        assert!(popup_ctrl_action(KeyCode::Char('o'), &app).is_none());
        assert!(popup_ctrl_action(KeyCode::Char('f'), &app).is_none());
        assert!(popup_ctrl_action(KeyCode::Char('n'), &app).is_none());
        assert!(popup_ctrl_action(KeyCode::Char('w'), &app).is_none());
    }

    #[test]
    fn ctrl_h_allowed_only_in_file_browser() {
        let mut app = App::new(&[]);
        app.open_configure();
        assert!(popup_ctrl_action(KeyCode::Char('h'), &app).is_none());
        app.close_configure();
        app.show_file_dialog(false);
        app.open_file_browser();
        assert!(matches!(
            popup_ctrl_action(KeyCode::Char('h'), &app),
            Some(PopupCtrl::BrowserToggleHidden)
        ));
        assert_eq!(app.popup_focus(), Some(PopupFocus::FileBrowser));
    }

    fn confirm_quit_app() -> App {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("unsaved");
        app.request_quit();
        assert!(app.show_confirm);
        assert_eq!(app.confirm_action.as_deref(), Some("quit"));
        app
    }

    #[test]
    fn handle_confirm_enter_on_quit_sets_should_quit_and_returns_false() {
        let mut app = confirm_quit_app();
        assert_eq!(app.confirm_selected, 0);
        let leave_run_app = handle_confirm_key(KeyCode::Enter, &mut app);
        assert!(app.should_quit);
        assert!(!leave_run_app);
    }

    #[test]
    fn handle_confirm_y_on_quit_sets_should_quit_and_returns_true() {
        let mut app = confirm_quit_app();
        let leave_run_app = handle_confirm_key(KeyCode::Char('y'), &mut app);
        assert!(app.should_quit);
        assert!(leave_run_app);
    }

    #[test]
    fn handle_confirm_n_on_quit_cancels() {
        let mut app = confirm_quit_app();
        let leave_run_app = handle_confirm_key(KeyCode::Char('n'), &mut app);
        assert!(!app.should_quit);
        assert!(!app.show_confirm);
        assert!(!leave_run_app);
    }

    #[test]
    fn handle_help_f1_and_esc_close_help() {
        let mut app = App::new(&[]);
        app.open_help();
        assert!(app.show_help);
        handle_help_key(KeyCode::F(1), &mut app);
        assert!(!app.show_help);

        app.open_help();
        assert!(app.show_help);
        handle_help_key(KeyCode::Esc, &mut app);
        assert!(!app.show_help);
    }

    #[test]
    fn handle_help_tab_resets_scroll() {
        let mut app = App::new(&[]);
        app.open_help();
        app.help_content_scroll.0 = 40;
        handle_help_key(KeyCode::Tab, &mut app);
        assert_eq!(app.help_content_scroll.0, 0);
        assert_eq!(app.help_tab, popups::help::HelpTab::Languages);
    }

    #[test]
    fn handle_help_down_increases_scroll_on_shortcuts() {
        let mut app = App::new(&[]);
        app.open_help();
        handle_help_key(KeyCode::Down, &mut app);
        assert!(app.help_content_scroll.0 >= 1);
    }

    #[test]
    fn handle_file_browser_scrollbar_click_selects_without_entering() {
        use crate::ui::popups::file_browser::BrowserEntry;
        use std::path::PathBuf;

        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.open_file_browser();
        let cwd = app.browser_cwd.clone();
        app.browser_entries = (0..80)
            .map(|i| BrowserEntry {
                name: format!("f{i}"),
                is_dir: false,
                path: PathBuf::from(format!("/tmp/f{i}")),
            })
            .collect();
        app.browser_selected = 0;
        let popup = popups::file_browser::file_browser_rect(term_area());
        let sb = popups::file_browser::list_scrollbar_rect(popup, false);
        assert!(popups::file_browser::list_overflow_bar(
            app.browser_entries.len(),
            0,
            popup,
            false
        )
        .overflowing());
        handle_file_browser_click(sb.x as usize, sb.y as usize + 4, &mut app);
        assert_eq!(
            app.browser_cwd, cwd,
            "scrollbar click must not enter a file"
        );
        assert!(
            app.show_file_browser,
            "scrollbar click must not close the browser"
        );
        assert_eq!(app.popup_scrollbar_drag, Some(PopupFocus::FileBrowser));
    }

    #[test]
    fn browser_scrollbar_position_maps_consistently() {
        use crate::ui::popups::file_browser::BrowserEntry;
        use std::path::PathBuf;

        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.open_file_browser();
        // 100 files, viewport ~15 rows
        app.browser_entries = (0..100)
            .map(|i| BrowserEntry {
                name: format!("f{i}"),
                is_dir: false,
                path: PathBuf::from(format!("/tmp/f{i}")),
            })
            .collect();
        app.browser_selected = 0;
        let popup = popups::file_browser::file_browser_rect(term_area());
        let sb = popups::file_browser::list_scrollbar_rect(popup, false);
        let total = app.browser_entries.len();

        // Test that dragging to the top of the scrollbar selects an early file
        handle_file_browser_click(sb.x as usize, sb.y as usize + 1, &mut app);
        let top_sel = app.browser_selected;
        assert!(
            top_sel < total / 4,
            "top of scrollbar should select early file"
        );

        // Test that dragging to the middle of the scrollbar selects a middle file
        let mid_y = sb.y + sb.height / 2;
        handle_file_browser_click(sb.x as usize, mid_y as usize, &mut app);
        let mid_sel = app.browser_selected;
        assert!(
            mid_sel > total / 4 && mid_sel < (total * 3) / 4,
            "middle of scrollbar should select middle file"
        );

        // Test that dragging to the bottom of the scrollbar selects a late file
        let bottom_y = sb.y + sb.height - 2;
        handle_file_browser_click(sb.x as usize, bottom_y as usize, &mut app);
        let bottom_sel = app.browser_selected;
        assert!(
            bottom_sel > (total * 3) / 4,
            "bottom of scrollbar should select late file"
        );

        // Verify the mapping is monotonic (top < middle < bottom)
        assert!(
            top_sel < mid_sel && mid_sel < bottom_sel,
            "scrollbar mapping should be monotonic: top ({}) < middle ({}) < bottom ({})",
            top_sel,
            mid_sel,
            bottom_sel
        );
    }

    #[test]
    fn handle_help_tab_click_resets_scroll() {
        let mut app = App::new(&[]);
        app.open_help();
        app.help_content_scroll.0 = 40;
        let popup = popups::help::help_rect(term_area());
        // "2 Languages" is popup-local x 15..26 on the tab row (popup.y + 1).
        handle_help_click((popup.x + 16) as usize, (popup.y + 1) as usize, &mut app);
        assert_eq!(app.help_tab, popups::help::HelpTab::Languages);
        assert_eq!(app.help_content_scroll.0, 0);
    }

    #[test]
    fn handle_file_dialog_tab_cycles_footer_buttons() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        assert_eq!(app.file_dialog_selected, 0);

        handle_file_dialog_key(KeyCode::Tab, &mut app);
        assert_eq!(app.file_dialog_selected, 1);
        handle_file_dialog_key(KeyCode::Tab, &mut app);
        assert_eq!(app.file_dialog_selected, 2);
        handle_file_dialog_key(KeyCode::Tab, &mut app);
        assert_eq!(app.file_dialog_selected, 0);

        handle_file_dialog_key(KeyCode::BackTab, &mut app);
        assert_eq!(app.file_dialog_selected, 2);
        handle_file_dialog_key(KeyCode::Left, &mut app);
        assert_eq!(app.file_dialog_selected, 1);
        handle_file_dialog_key(KeyCode::Right, &mut app);
        assert_eq!(app.file_dialog_selected, 2);
    }

    #[test]
    fn handle_file_browser_tab_from_list_focuses_first_button() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.open_file_browser();
        app.browser_button_selected = popups::file_browser::FOCUS_LIST;
        handle_file_browser_key(KeyCode::Tab, &mut app);
        assert_eq!(app.browser_button_selected, 0);
    }

    #[test]
    fn handle_file_browser_tab_from_list_in_save_focuses_name_field() {
        let mut app = App::new(&[]);
        app.show_file_dialog(true);
        app.open_file_browser();
        app.browser_button_selected = popups::file_browser::FOCUS_LIST;
        handle_file_browser_key(KeyCode::Tab, &mut app);
        assert_eq!(
            app.browser_button_selected,
            popups::file_browser::FOCUS_FILENAME
        );
        // Clear the pre-filled filename and type a new one
        app.browser_filename.clear();
        handle_file_browser_key(KeyCode::Char('n'), &mut app);
        handle_file_browser_key(KeyCode::Char('o'), &mut app);
        handle_file_browser_key(KeyCode::Char('t'), &mut app);
        handle_file_browser_key(KeyCode::Char('e'), &mut app);
        assert_eq!(app.browser_filename, "note");
    }

    fn recent_temp_file(tag: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-recent-key-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, tag).unwrap();
        p
    }

    #[test]
    fn handle_file_browser_delete_removes_selected_recent() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.open_file_browser();
        app.browser_mode = popups::file_browser::BrowserMode::Recent;
        app.browser_button_selected = popups::file_browser::FOCUS_LIST;
        let a = recent_temp_file("del-a");
        let b = recent_temp_file("del-b");
        app.config.recent_files = vec![
            a.to_string_lossy().into_owned(),
            b.to_string_lossy().into_owned(),
        ];
        app.refresh_recent();
        app.browser_selected = 0;
        handle_file_browser_key(KeyCode::Delete, &mut app);
        assert_eq!(
            app.config.recent_files,
            vec![b.to_string_lossy().into_owned()]
        );
        assert!(app.show_file_browser);
    }

    #[test]
    fn handle_file_browser_backspace_removes_recent_when_list_focused() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.open_file_browser();
        app.browser_mode = popups::file_browser::BrowserMode::Recent;
        app.browser_button_selected = popups::file_browser::FOCUS_LIST;
        let a = recent_temp_file("bs-a");
        app.config.recent_files = vec![a.to_string_lossy().into_owned()];
        app.refresh_recent();
        handle_file_browser_key(KeyCode::Backspace, &mut app);
        assert!(app.config.recent_files.is_empty());
    }

    #[test]
    fn handle_file_browser_backspace_still_goes_up_in_dir_mode() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.open_file_browser();
        app.browser_mode = popups::file_browser::BrowserMode::Dir;
        app.browser_cwd = std::path::PathBuf::from("/tmp/outdir");
        handle_file_browser_key(KeyCode::Backspace, &mut app);
        assert_eq!(app.browser_cwd, std::path::PathBuf::from("/tmp"));
    }

    #[test]
    fn handle_file_browser_delete_ignored_when_footer_focused() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.open_file_browser();
        app.browser_mode = popups::file_browser::BrowserMode::Recent;
        app.browser_button_selected = 1; // mode toggle
        let a = recent_temp_file("footer-del");
        app.config.recent_files = vec![a.to_string_lossy().into_owned()];
        app.refresh_recent();
        handle_file_browser_key(KeyCode::Delete, &mut app);
        assert_eq!(app.config.recent_files.len(), 1);
    }

    #[test]
    fn handle_file_browser_click_on_recent_x_removes_without_opening() {
        let mut app = App::new(&[]);
        app.show_file_dialog(false);
        app.open_file_browser();
        app.browser_mode = popups::file_browser::BrowserMode::Recent;
        let a = recent_temp_file("click-a");
        let b = recent_temp_file("click-b");
        app.config.recent_files = vec![
            a.to_string_lossy().into_owned(),
            b.to_string_lossy().into_owned(),
        ];
        app.refresh_recent();

        let popup = popups::file_browser::file_browser_rect(term_area());
        let overflowing = popups::file_browser::list_overflow_bar(
            app.browser_entries.len(),
            app.browser_selected,
            popup,
            false,
        )
        .overflowing();
        let (x0, _) = popups::file_browser::recent_clear_x_range(popup, overflowing);
        let (list_y, _) = popups::file_browser::list_area(popup, false);
        handle_file_browser_click(x0 as usize, list_y, &mut app);
        assert_eq!(
            app.config.recent_files,
            vec![b.to_string_lossy().into_owned()]
        );
        assert!(app.show_file_browser, "clear must not confirm/open");
        assert!(app.show_file_dialog);
    }

    #[test]
    fn handle_themes_key_edit_mode_types_list_shortcuts_into_name() {
        use popups::themes::{EditFocus, ThemesMode};

        let mut app = App::new(&[]);
        app.open_themes();
        assert_eq!(app.themes_mode, ThemesMode::List);

        // List-mode `e` still enters Edit.
        handle_themes_key(KeyCode::Char('e'), &mut app);
        assert_eq!(app.themes_mode, ThemesMode::Edit);
        let edit = app.themes_edit.as_ref().expect("edit state after list e");
        assert_eq!(edit.focus, EditFocus::Name);
        let start_name = edit.name.clone();

        handle_themes_key(KeyCode::Char('e'), &mut app);
        handle_themes_key(KeyCode::Char('n'), &mut app);
        handle_themes_key(KeyCode::Char('d'), &mut app);

        assert_eq!(
            app.themes_mode,
            ThemesMode::Edit,
            "e/n/d must not restart/new/delete while editing"
        );
        let edit = app.themes_edit.as_ref().expect("edit state after typing");
        assert_eq!(edit.focus, EditFocus::Name);
        assert_eq!(edit.name, format!("{start_name}end"));
    }

    #[test]
    fn handle_themes_edit_scrollbar_click_scrolls_without_changing_role() {
        use popups::themes::ThemesMode;

        let mut app = App::new(&[]);
        app.open_themes();
        handle_themes_key(KeyCode::Char('e'), &mut app);
        assert_eq!(app.themes_mode, ThemesMode::Edit);
        let selected_before = app.themes_edit.as_ref().expect("edit state").role_selected;
        let popup = popups::themes::edit_rect(term_area());
        let sb = popups::themes::roles_scrollbar_rect(popup);
        let bar = popups::themes::roles_overflow_bar(0);
        assert!(bar.overflowing());
        let mut skip = false;
        themes_popup_handle_click(&mut app, (sb.x as usize, sb.y as usize + 4), &mut skip);
        assert!(skip, "themes click swallows editor input");
        let edit = app.themes_edit.as_ref().expect("edit still open");
        assert_eq!(
            edit.role_selected, selected_before,
            "scrollbar click must not change the selected role"
        );
        assert_eq!(app.popup_scrollbar_drag, Some(PopupFocus::Themes));
        assert!(
            edit.role_scroll > 0,
            "mid-track click should advance role_scroll, got {}",
            edit.role_scroll
        );
    }

    #[test]
    fn handle_themes_edit_role_label_click_still_selects() {
        let mut app = App::new(&[]);
        app.open_themes();
        handle_themes_key(KeyCode::Char('e'), &mut app);
        let popup = popups::themes::edit_rect(term_area());
        let list = popups::themes::roles_list_area(popup, true);
        let mut skip = false;
        // Second visible row → role index 1 (header_fg) at scroll 0.
        themes_popup_handle_click(&mut app, (list.x as usize, list.y as usize + 1), &mut skip);
        let edit = app.themes_edit.as_ref().expect("edit still open");
        assert_eq!(edit.role_selected, 1);
        assert_eq!(edit.focus, popups::themes::EditFocus::Role);
        assert_eq!(app.popup_scrollbar_drag, None);
    }

    #[test]
    fn leaving_themes_edit_clears_popup_scrollbar_drag() {
        use popups::themes::ThemesMode;

        let mut app = App::new(&[]);
        app.open_themes();
        handle_themes_key(KeyCode::Char('e'), &mut app);
        let popup = popups::themes::edit_rect(term_area());
        let sb = popups::themes::roles_scrollbar_rect(popup);
        let mut skip = false;
        themes_popup_handle_click(&mut app, (sb.x as usize, sb.y as usize), &mut skip);
        assert_eq!(app.popup_scrollbar_drag, Some(PopupFocus::Themes));

        handle_themes_key(KeyCode::Esc, &mut app);
        assert_eq!(app.themes_mode, ThemesMode::List);
        assert_eq!(
            app.popup_scrollbar_drag, None,
            "canceling Edit must drop the role-list scrub"
        );

        handle_themes_key(KeyCode::Char('e'), &mut app);
        themes_popup_handle_click(&mut app, (sb.x as usize, sb.y as usize), &mut skip);
        assert_eq!(app.popup_scrollbar_drag, Some(PopupFocus::Themes));
        app.close_themes();
        assert_eq!(
            app.popup_scrollbar_drag, None,
            "closing Themes must drop the role-list scrub"
        );
    }

    fn reload_prompt_app() -> (App, std::path::PathBuf) {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-reload-key-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, "one\n").unwrap();
        let mut app = App::new(&[]);
        app.config.auto_reload_all = false;
        app.open_file_in_tab(&p).unwrap();
        std::fs::write(&p, "two!\n").unwrap();
        app.check_external_changes();
        assert!(app.show_reload);
        (app, p)
    }

    #[test]
    fn handle_reload_y_reloads_and_esc_keeps() {
        let (mut app, path) = reload_prompt_app();
        assert_eq!(app.popup_focus(), Some(PopupFocus::Reload));
        handle_reload_key(KeyCode::Char('y'), &mut app);
        assert!(!app.show_reload);
        assert_eq!(app.active_buffer().text(), "two!\n");
        let _ = std::fs::remove_file(&path);

        let (mut app, path) = reload_prompt_app();
        handle_reload_key(KeyCode::Esc, &mut app);
        assert!(!app.show_reload);
        assert_eq!(app.active_buffer().text(), "one\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn confirm_still_wins_over_reload() {
        let (mut app, path) = reload_prompt_app();
        app.show_confirm_dialog("Quit with unsaved changes?", "quit");
        assert_eq!(app.popup_focus(), Some(PopupFocus::Confirm));
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod wheel_target_tests {
    use super::*;

    #[test]
    fn wheel_target_is_focused_popup_not_editor() {
        let mut app = App::new(&[]);
        assert_eq!(wheel_target(&app, 0, 10), WheelTarget::Editor);
        app.open_themes();
        assert_eq!(wheel_target(&app, 0, 10), WheelTarget::Themes);
        app.close_themes();
        app.open_configure();
        assert_eq!(wheel_target(&app, 0, 10), WheelTarget::Swallow);
        app.close_configure();
        app.open_help();
        assert_eq!(wheel_target(&app, 0, 10), WheelTarget::Help);
        app.close_help();
        app.show_file_dialog(false);
        app.open_file_browser();
        assert_eq!(wheel_target(&app, 0, 10), WheelTarget::FileBrowser);
        app.show_confirm = true;
        assert_eq!(wheel_target(&app, 0, 10), WheelTarget::Swallow);
    }

    #[test]
    fn wheel_over_overflowing_tab_strip_is_tab_strip() {
        let mut app = App::new(&[]);
        while app.buffers.len() < 8 {
            app.new_tab();
        }
        app.clamp_tab_strip(80);
        let names = app.tab_names();
        let layout = crate::ui::header::layout_tab_strip(&names, 80, app.tab_scroll);
        assert!(layout.overflowing);
        assert_eq!(wheel_target(&app, 40, 0), WheelTarget::TabStrip);
        assert_eq!(wheel_target(&app, 40, 10), WheelTarget::Editor);
    }
}

#[cfg(test)]
mod popup_mouse_focus_tests {
    use super::*;
    use crate::app::PopupFocus;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    fn left_down(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn update_hover_clears_buried_popup_fields() {
        let mut app = App::new(&[]);
        app.open_configure();
        app.configure_row_hover = Some(0);
        app.configure_button_hover = Some(1);
        app.show_confirm = true;
        app.confirm_button_hover = Some(0);

        update_hover_states(0, 0, &mut app);

        assert_eq!(app.popup_focus(), Some(PopupFocus::Confirm));
        assert_eq!(app.configure_row_hover, None);
        assert_eq!(app.configure_button_hover, None);
    }

    #[test]
    fn word_count_click_consumes_and_does_not_move_editor_cursor() {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("hello world");
        let cursor_before = app.active_buffer().cursor;
        app.editor_visible_lines.set(40);
        app.compute_and_show_word_count();
        assert_eq!(app.popup_focus(), Some(PopupFocus::WordCount));

        handle_mouse(left_down(8, crate::ui::layout::HEADER_ROWS), &mut app);

        assert!(!app.show_word_count);
        assert_eq!(app.active_buffer().cursor, cursor_before);
        assert!(!app.mouse_drag_active);
    }

    #[test]
    fn update_hover_sets_reload_button_and_clears_buried_configure() {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-reload-hover-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, "one\n").unwrap();
        let mut app = App::new(&[]);
        app.config.auto_reload_all = false;
        app.open_file_in_tab(&p).unwrap();
        std::fs::write(&p, "two!\n").unwrap();
        app.check_external_changes();
        app.open_configure(); // should no-op while reload owns focus
        app.configure_row_hover = Some(0);
        assert_eq!(app.popup_focus(), Some(PopupFocus::Reload));

        let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
        let term_area = Rect::new(0, 0, tw, th);
        let popup = popups::reload::reload_rect(term_area, &app.reload_message);
        let labels = popups::reload::button_labels();
        let ranges = crate::ui::button::ButtonRow::button_x_ranges(popup.x + 1, &labels);
        let (x0, _) = ranges[0];
        let y = popups::reload::button_row_y(popup);
        update_hover_states(x0, y, &mut app);
        assert_eq!(app.reload_button_hover, Some(0));
        assert_eq!(app.configure_row_hover, None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn leftover_menu_item_click_ignored_when_popup_focused() {
        let mut app = App::new(&[]);
        app.active_buffer_mut().insert("unsaved");
        app.active_menu = Some("File".into());
        app.menu_selected = 0;
        app.request_quit();
        assert_eq!(app.popup_focus(), Some(PopupFocus::Confirm));
        assert_eq!(app.buffers.len(), 1);

        // File > New is item 0 at (menu_x=0, y just below the header).
        handle_mouse(left_down(1, crate::ui::layout::HEADER_ROWS + 1), &mut app);

        // Dropdown handling was skipped, so File > New did not run and the
        // leftover menu was not closed by handle_menu_action.
        assert_eq!(app.buffers.len(), 1);
        assert_eq!(app.active_menu.as_deref(), Some("File"));
    }

    #[test]
    fn tools_menu_find_opens_query_and_index_1_is_word_count() {
        let mut app = App::new(&[]);
        app.active_menu = Some("Tools".into());
        app.menu_selected = 0;
        handle_menu_action(&mut app);
        assert!(app.find_state.is_some());
        assert_eq!(
            app.find_state.as_ref().unwrap().focus,
            crate::ui::find::FindFocus::Query
        );

        let mut app = App::new(&[]);
        app.active_menu = Some("Tools".into());
        app.menu_selected = 1;
        handle_menu_action(&mut app);
        assert!(app.show_word_count);
        assert!(app.find_state.is_none());
    }

    #[test]
    fn configure_tab_size_mouse_arrows_step_up_and_down() {
        let mut app = App::new(&[]);
        app.open_configure();
        // Pin in-memory only: App::new() loads the user's XDG config, which may
        // not be the crate default of 4. Clicks persist via set_tab_size; restore
        // the original value when the test finishes.
        let saved_tab = app.config.tab_size;
        app.config.tab_size = 4;
        assert_eq!(app.config.tab_size, 4);

        // Default 80x24 terminal (crossterm::terminal::size() falls back to this
        // in tests), matching handle_configure_click's own lookup.
        let term_area = Rect::new(0, 0, 80, 24);
        let popup = popups::configure::configure_rect(term_area);
        let (dec_x, inc_x) = popups::configure::tab_size_arrow_xs(popup);
        let row_y = popups::configure::row_ys(popup)[popups::configure::ROW_TAB_SIZE];

        // Click ">" -> increase.
        handle_mouse(left_down(inc_x, row_y), &mut app);
        assert_eq!(app.config.tab_size, 5);

        // Click "<" -> decrease.
        handle_mouse(left_down(dec_x, row_y), &mut app);
        assert_eq!(app.config.tab_size, 4);

        // Click the number (between the arrows) -> no change.
        handle_mouse(left_down((dec_x + inc_x) / 2, row_y), &mut app);
        assert_eq!(app.config.tab_size, 4);

        // Clamp low: click "<" until it bottoms out at 1.
        for _ in 0..10 {
            handle_mouse(left_down(dec_x, row_y), &mut app);
        }
        assert_eq!(app.config.tab_size, 1);

        // Clamp high: click ">" until it tops out at 8.
        for _ in 0..10 {
            handle_mouse(left_down(inc_x, row_y), &mut app);
        }
        assert_eq!(app.config.tab_size, 8);

        app.config.set_tab_size(saved_tab);
    }
}

#[cfg(test)]
mod editor_mouse_click_scroll_tests {
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::backend::TestBackend;

    fn left_down(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn editor_cell(buf: &ratatui::buffer::Buffer, x: u16, y: u16) -> String {
        buf[(x, y)].symbol().to_string()
    }

    fn wrapping_app() -> App {
        let text = (0..80)
            .map(|_| "x".repeat(40))
            .collect::<Vec<_>>()
            .join("\n");
        let mut app = App::new(&[]);
        app.config.word_wrap = true;
        app.config.tab_size = 4;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(&text, None);
        app.editor_visible_lines.set(20);
        app.editor_visible_width.set(20);
        let b = app.active_buffer_mut();
        b.visible_lines = 20;
        b.visible_width = 20;
        b.word_wrap = true;
        b.tab_size = 4;
        b.scroll_offset = (40, 0);
        app
    }

    #[test]
    fn wrap_click_in_viewport_does_not_move_scroll() {
        let mut app = wrapping_app();
        let scroll_before = app.active_buffer().scroll_offset.0;
        // Content col 0 is terminal x = GUTTER_WIDTH. Editor y 10 → screen row 50.
        let y = crate::ui::layout::HEADER_ROWS + 10;
        handle_mouse(
            left_down(crate::buffer::Buffer::GUTTER_WIDTH as u16, y),
            &mut app,
        );
        assert_eq!(
            app.active_buffer().scroll_offset.0,
            scroll_before,
            "click inside a wrapped viewport must not jump the page"
        );
        let layout = crate::wrap::compute_layout(
            &app.active_buffer().rope,
            0,
            app.active_buffer().line_count(),
            20,
            4,
        );
        let cursor_row = crate::wrap::screen_row_of_byte(
            &layout,
            &app.active_buffer().rope,
            0,
            app.active_buffer().cursor,
        );
        assert_eq!(cursor_row, 50, "caret must land on the clicked screen row");
    }

    #[test]
    fn unwrap_click_in_viewport_does_not_move_scroll() {
        let text = (0..200)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut app = App::new(&[]);
        app.config.word_wrap = false;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(&text, None);
        app.editor_visible_lines.set(20);
        let b = app.active_buffer_mut();
        b.visible_lines = 20;
        b.word_wrap = false;
        b.scroll_offset = (40, 0);
        let y = crate::ui::layout::HEADER_ROWS + 10;
        handle_mouse(
            left_down(crate::buffer::Buffer::GUTTER_WIDTH as u16, y),
            &mut app,
        );
        assert_eq!(app.active_buffer().scroll_offset.0, 40);
        assert_eq!(app.active_buffer().cursor_position().0, 50);
    }

    #[test]
    fn hardware_cursor_hidden_when_view_row_past_viewport() {
        let mut app = App::new(&[]);
        app.config.word_wrap = false;
        app.editor_visible_lines.set(20);
        let b = app.active_buffer_mut();
        b.visible_lines = 20;
        b.word_wrap = false;
        // Empty/untitled buffer: cursor in view → Some.
        assert!(compute_cursor_position(&app).is_some());
        // Cursor below the viewport: view_row 25 >= vis 20 → None (not status bar).
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(
            &(0..30)
                .map(|i| format!("L{i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            None,
        );
        app.active_buffer_mut().cursor = app.active_buffer().rope.line_to_byte(25);
        app.active_buffer_mut().scroll_offset = (0, 0);
        app.editor_visible_lines.set(20);
        assert_eq!(
            compute_cursor_position(&app),
            None,
            "caret below the editor must hide, not paint on the status bar"
        );
    }

    #[test]
    fn hardware_cursor_hidden_when_wrap_view_row_past_viewport() {
        let mut app = wrapping_app();
        app.active_buffer_mut().scroll_offset = (0, 0);
        let layout = crate::wrap::compute_layout(
            &app.active_buffer().rope,
            0,
            app.active_buffer().line_count(),
            20,
            4,
        );
        // Screen row 25 with vis 20 → view_row 25 >= vis → None.
        let target = crate::wrap::byte_of_screen_row(&layout, &app.active_buffer().rope, 0, 25, 0);
        app.active_buffer_mut().cursor = target;
        assert_eq!(
            compute_cursor_position(&app),
            None,
            "wrapped caret below the editor must hide, not paint on the status bar"
        );
    }

    #[test]
    fn hardware_cursor_y_is_inside_editor_for_last_visible_row() {
        let mut app = App::new(&[]);
        app.config.word_wrap = false;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(
            &(0..30)
                .map(|i| format!("L{i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            None,
        );
        app.editor_visible_lines.set(20);
        let b = app.active_buffer_mut();
        b.scroll_offset = (10, 0);
        b.cursor = b.rope.line_to_byte(29); // last visible row: view_row = 19
        let (_x, y) = compute_cursor_position(&app).expect("last visible row is on-screen");
        let last_editor_y = crate::ui::layout::HEADER_ROWS + 19;
        assert_eq!(y, last_editor_y);
        // Status bar is HEADER_ROWS + vis; cursor must be strictly above it.
        assert!(y < crate::ui::layout::HEADER_ROWS + 20);
    }

    fn long_line_app() -> App {
        let mut app = App::new(&[]);
        app.config.word_wrap = false;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(&"x".repeat(80), None);
        app.editor_visible_lines.set(20);
        // Viewport must be narrower than the 80-char line (overflow) but wide
        // enough that the click at column 20 lands on the track
        // (`GUTTER_WIDTH + visible_width`).
        app.editor_visible_width.set(40);
        app.editor_hscrollbar_visible.set(true);
        let b = app.active_buffer_mut();
        b.visible_lines = 20;
        b.visible_width = 40;
        b.word_wrap = false;
        b.refresh_max_line_width();
        b.scroll_offset = (0, 0);
        app
    }

    #[test]
    fn hscrollbar_click_pans_and_does_not_move_cursor() {
        let mut app = long_line_app();
        let cursor_before = app.active_buffer().cursor;
        // Bar row is HEADER + vis. Tests set vis=20 as *content* height.
        let y = crate::ui::layout::HEADER_ROWS + 20;
        handle_mouse(left_down(20, y), &mut app);
        assert!(app.hscrollbar_drag_active);
        assert!(!app.mouse_drag_active);
        assert_eq!(app.active_buffer().cursor, cursor_before);
        assert!(app.active_buffer().scroll_offset.1 > 0);
    }

    #[test]
    fn hscrollbar_hidden_under_wrap() {
        let mut app = long_line_app();
        app.config.word_wrap = true;
        app.editor_hscrollbar_visible.set(false);
        let y = crate::ui::layout::HEADER_ROWS + 20;
        handle_mouse(left_down(20, y), &mut app);
        assert!(!app.hscrollbar_drag_active);
        assert_eq!(app.active_buffer().scroll_offset.1, 0);
    }

    #[test]
    fn hardware_cursor_x_stays_in_text_area_at_eol_when_hscroll_clamped() {
        let mut app = App::new(&[]);
        app.config.word_wrap = false;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(&"x".repeat(80), None);
        app.editor_visible_lines.set(20);
        app.editor_visible_width.set(10);
        let b = app.active_buffer_mut();
        b.visible_lines = 20;
        b.visible_width = 10;
        b.word_wrap = false;
        b.refresh_max_line_width();
        b.cursor = b.rope.len_bytes();
        b.ensure_cursor_horizontally_visible();
        b.clamp_hscroll();
        let (screen_x, _y) =
            compute_cursor_position(&app).expect("EOL caret should stay in the text area");
        let vis_w = app.editor_visible_width.get();
        assert!(
            screen_x < crate::buffer::Buffer::GUTTER_WIDTH as u16 + vis_w as u16,
            "hardware cursor must stay left of the v-bar; got screen_x={screen_x}"
        );
    }

    #[test]
    fn hscrollbar_right_end_reaches_max_hscroll_and_keeps_eol_caret() {
        let mut app = long_line_app();
        let rect = editor_hscrollbar_rect(&app).expect("hbar visible");
        let end_x = rect.x + rect.width.saturating_sub(1);
        let y = crate::ui::layout::HEADER_ROWS + app.editor_visible_lines.get() as u16;
        handle_mouse(left_down(end_x, y), &mut app);
        app.active_buffer_mut().refresh_max_line_width();
        assert_eq!(
            app.active_buffer().scroll_offset.1,
            app.active_buffer().max_hscroll(),
            "→ cap must reach max_hscroll so the thumb matches End"
        );

        // Cursor at EOF, then the same bar-end jump, must keep the hardware caret.
        let mut app = long_line_app();
        {
            let buf = app.active_buffer_mut();
            buf.cursor = buf.rope.len_bytes();
            buf.ensure_cursor_horizontally_visible();
            buf.clamp_hscroll();
        }
        let rect = editor_hscrollbar_rect(&app).expect("hbar visible");
        let end_x = rect.x + rect.width.saturating_sub(1);
        let target = hscrollbar_target_for_col(&app, end_x).expect("end cell maps");
        {
            let buf = app.active_buffer_mut();
            buf.scroll_offset.1 = target;
            buf.clamp_hscroll();
        }
        let (screen_x, _y) =
            compute_cursor_position(&app).expect("EOL caret still visible at bar end");
        let vis_w = app.active_buffer().visible_width.max(1);
        assert!(
            screen_x < crate::buffer::Buffer::GUTTER_WIDTH as u16 + vis_w as u16,
            "hardware cursor must stay left of the v-bar; got screen_x={screen_x}"
        );
    }

    #[test]
    fn hscrollbar_row_is_above_status_with_line_glyphs_to_the_right_edge() {
        // 1 header + 10 editor + 1 status. Long line forces the h-bar, which
        // must occupy the last editor row (not the status row) and meet the
        // vertical bar's column with `┘`.
        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(&[]);
        app.config.word_wrap = false;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(&"x".repeat(200), None);
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        let status_y = 11u16;
        let hbar_y = 10u16;

        for x in 0..80u16 {
            let ch = buf[(x, status_y)].symbol();
            assert!(
                !["▄", "▁", "█", "─", "━", "┘"].contains(&ch),
                "status row must not carry h-bar glyphs, found {ch:?} at x={x}"
            );
        }

        let mut saw_track = false;
        for x in 0..80u16 {
            let ch = buf[(x, hbar_y)].symbol();
            assert_ne!(ch, "█", "full-cell block on h-bar at x={x}");
            assert_ne!(ch, "▄", "lower-half block on h-bar at x={x}");
            assert_ne!(ch, "▁", "lower-eighth block on h-bar at x={x}");
            if ch == "─" || ch == "━" || ch == "←" || ch == "┘" {
                saw_track = true;
            }
        }
        assert!(saw_track, "h-bar row should contain line glyphs");
        assert_eq!(
            buf[(79, hbar_y)].symbol(),
            "┘",
            "right edge must join the vertical bar column"
        );
        assert!(
            app.editor_hscrollbar_visible.get(),
            "long unwrapped line must show the h-bar"
        );
    }

    #[test]
    fn hidden_line_numbers_paint_text_at_column_zero() {
        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(&[]);
        app.config.show_line_numbers = false;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str("HELLO", None);
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        let y = crate::ui::layout::HEADER_ROWS;
        assert_eq!(
            editor_cell(buf, 0, y),
            "H",
            "first text column must occupy x=0 when the gutter is hidden"
        );
        assert_eq!(app.editor_visible_width.get(), 79); // 80 - vbar 1, no gutter
    }

    #[test]
    fn shown_line_numbers_keep_six_column_gutter() {
        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(&[]);
        app.config.show_line_numbers = true;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str("HELLO", None);
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        let y = crate::ui::layout::HEADER_ROWS;
        assert_eq!(editor_cell(buf, 3, y), "1");
        assert_eq!(editor_cell(buf, 4, y), "|");
        assert_eq!(editor_cell(buf, 6, y), "H");
        assert_eq!(app.editor_visible_width.get(), 73); // 80 - gutter 6 - vbar 1
    }

    #[test]
    fn hardware_cursor_x_tracks_live_gutter_width() {
        let mut app = App::new(&[]);
        app.config.word_wrap = false;
        app.config.show_line_numbers = true;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str("HELLO", None);
        app.editor_visible_lines.set(20);
        app.editor_visible_width.set(40);
        let (x_on, _) = compute_cursor_position(&app).expect("caret in view");
        assert_eq!(x_on, Buffer::GUTTER_WIDTH as u16);

        app.config.show_line_numbers = false;
        let (x_off, _) = compute_cursor_position(&app).expect("caret in view");
        assert_eq!(x_off, 0);
    }

    #[test]
    fn click_at_column_zero_places_caret_when_gutter_hidden() {
        let mut app = App::new(&[]);
        app.config.show_line_numbers = false;
        app.config.word_wrap = false;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str("HELLO", None);
        app.active_buffer_mut().cursor = 5; // not already at col 0
        app.editor_visible_lines.set(20);
        let y = crate::ui::layout::HEADER_ROWS;
        handle_mouse(left_down(0, y), &mut app);
        assert_eq!(
            app.active_buffer().cursor_position(),
            (0, 0),
            "x=0 is the first text column when line numbers are hidden"
        );
    }

    #[test]
    fn wrap_continuation_has_no_indent_when_gutter_hidden() {
        // Long unwrapped-by-words line of A's. With no gutter the first wrapped
        // continuation row must start at x=0, not x=6.
        let backend = TestBackend::new(40, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(&[]);
        app.config.show_line_numbers = false;
        app.config.word_wrap = true;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(&"A".repeat(80), None);
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        let y0 = crate::ui::layout::HEADER_ROWS;
        let y1 = y0 + 1;
        assert_eq!(editor_cell(buf, 0, y0), "A");
        assert_eq!(
            editor_cell(buf, 0, y1),
            "A",
            "wrapped continuation must also start at x=0 when the gutter is hidden"
        );
        assert_eq!(
            app.editor_visible_width.get(),
            39,
            "40-col terminal minus vbar, no gutter"
        );
    }

    #[test]
    fn wrap_continuation_indents_when_gutter_shown() {
        let backend = TestBackend::new(40, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(&[]);
        app.config.show_line_numbers = true;
        app.config.word_wrap = true;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str(&"A".repeat(80), None);
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        let y0 = crate::ui::layout::HEADER_ROWS;
        let y1 = y0 + 1;
        assert_eq!(editor_cell(buf, 4, y0), "|");
        assert_eq!(editor_cell(buf, 6, y0), "A");
        assert_eq!(editor_cell(buf, 0, y1), " ");
        assert_eq!(editor_cell(buf, 5, y1), " ");
        assert_eq!(
            editor_cell(buf, 6, y1),
            "A",
            "wrap continuation must indent to the text column when line numbers are shown"
        );
    }

    #[test]
    fn click_in_gutter_does_not_move_caret_when_numbers_shown() {
        let mut app = App::new(&[]);
        app.config.show_line_numbers = true;
        app.config.word_wrap = false;
        *app.active_buffer_mut() = crate::buffer::Buffer::from_str("HELLO", None);
        app.active_buffer_mut().cursor = 5;
        app.editor_visible_lines.set(20);
        let y = crate::ui::layout::HEADER_ROWS;
        handle_mouse(left_down(0, y), &mut app);
        assert_eq!(app.active_buffer().cursor_position(), (0, 5));
        handle_mouse(left_down(5, y), &mut app);
        assert_eq!(app.active_buffer().cursor_position(), (0, 5));
        handle_mouse(left_down(Buffer::GUTTER_WIDTH as u16, y), &mut app);
        assert_eq!(
            app.active_buffer().cursor_position(),
            (0, 0),
            "first text column (x=gutter) must place the caret"
        );
    }
}

#[cfg(test)]
mod startup_tests {
    use super::{startup_from_args, Startup};

    #[test]
    fn version_flags_are_not_filenames() {
        assert_eq!(startup_from_args(["--version"]), Startup::Version);
        assert_eq!(startup_from_args(["-V"]), Startup::Version);
    }

    #[test]
    fn version_flag_wins_when_mixed_with_paths() {
        assert_eq!(
            startup_from_args(["src/main.rs", "--version"]),
            Startup::Version
        );
    }

    #[test]
    fn double_dash_keeps_a_version_looking_filename() {
        assert_eq!(
            startup_from_args(["--", "--version"]),
            Startup::Run(vec!["--version".to_string()])
        );
    }

    #[test]
    fn ordinary_arguments_stay_filenames() {
        assert_eq!(
            startup_from_args(["src/main.rs", "-notes.txt"]),
            Startup::Run(vec!["src/main.rs".to_string(), "-notes.txt".to_string()])
        );
    }
}
