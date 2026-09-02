// File-system browser popup for the Open/Save As dialogs.
//
// Pure presentation + value types. All mutable state lives on `App`
// (consistent with the other popups — see AGENTS §3 "Module rules").
//
// Listing logic (`sort_entries`, `filter_hidden`, `read_directory`) is
// side-effect-free and unit-tested in isolation below.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::path::{Path, PathBuf};

use crate::theme::Theme;
use crate::ui::button::ButtonRow;
use crate::ui::scrollbar::{self, OverflowBar};

/// Number of lines rendered above the list body (hint + blank separator).
/// `list_area` and `render` both derive their layout from this so the click
/// hit-test and the rendered rows can never drift.
pub const HEADER_LINES: usize = 2;

/// `browser_button_selected` sentinel: the file list is focused.
pub const FOCUS_LIST: usize = 4;
/// `browser_button_selected` sentinel: the Save As Name field is focused.
pub const FOCUS_FILENAME: usize = 5;

/// Page-aligned scroll offset that keeps `sel` visible in a viewport of
/// `body_h` rows. Shared by `render` and the mouse click handler so both agree
/// on which absolute row corresponds to which entry index.
pub fn compute_scroll(sel: usize, body_h: usize) -> usize {
    let bh = body_h.max(1);
    (sel / bh) * bh
}

/// A single row in the browser listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserEntry {
    pub name: String,
    pub is_dir: bool,
    pub path: PathBuf,
}

impl BrowserEntry {
    /// The pinned "go up one directory" entry. `None` when the cwd is a root
    /// with no parent (e.g. `/` on Unix).
    pub fn parent_entry(cwd: &Path) -> Option<Self> {
        cwd.parent().map(|_| {
            let path = cwd.join("..");
            Self {
                name: "..".to_string(),
                is_dir: true,
                path,
            }
        })
    }
}

/// Browser view mode: directory listing or flat recent-files list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BrowserMode {
    #[default]
    Dir,
    Recent,
}

/// Sort entries: directories first, then files; case-insensitive alpha within
/// each group. Stable for equal keys. Pure — does not touch the filesystem.
pub fn sort_entries(mut entries: Vec<BrowserEntry>) -> Vec<BrowserEntry> {
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}

/// Drop dotfile entries unless `show_hidden` is true. Pure.
///
/// NOTE: `read_directory` already skips dotfiles at the FS layer when
/// `show_hidden` is false; this function exists for unit testing the filter
/// rule in isolation and is not called from the read path.
pub fn filter_hidden(entries: Vec<BrowserEntry>, show_hidden: bool) -> Vec<BrowserEntry> {
    if show_hidden {
        return entries;
    }
    entries
        .into_iter()
        .filter(|e| !e.name.starts_with('.'))
        .collect()
}

/// Read a directory and return a sorted, optionally-filtered listing.
///
/// The pinned `..` entry (when the cwd has a parent) is always first and is
/// never filtered out by `filter_hidden` — the user always needs a way up.
///
/// Symlinks are followed via `metadata()` so a symlink to a directory is
/// enterable. Broken symlinks fall back to `symlink_metadata` and are shown as
/// files (best-effort — they are not selectable as directories).
pub fn read_directory(cwd: &Path, show_hidden: bool) -> std::io::Result<Vec<BrowserEntry>> {
    let mut entries: Vec<BrowserEntry> = Vec::new();

    for entry in std::fs::read_dir(cwd)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy().to_string();

        // Skip dotfiles early unless requested (saves a stat call).
        if !show_hidden && name.starts_with('.') {
            continue;
        }

        // Follow symlinks: `DirEntry::metadata` does NOT traverse them on
        // Unix, so a symlink-to-dir would otherwise be reported as a file.
        // Fall back to the entry's own file_type if the target is unreachable
        // (broken symlink) so the row still renders.
        let is_dir = std::fs::metadata(entry.path())
            .map(|m| m.is_dir())
            .unwrap_or_else(|_| entry.file_type().map(|t| t.is_dir()).unwrap_or(false));

        entries.push(BrowserEntry {
            name,
            is_dir,
            path: entry.path(),
        });
    }

    let mut sorted = sort_entries(entries);

    // Pin `..` at the top so it is always reachable regardless of hidden filter.
    if let Some(parent) = BrowserEntry::parent_entry(cwd) {
        sorted.insert(0, parent);
    }

    Ok(sorted)
}

/// Build a save target: `dir` + filename taken from `current_input`'s
/// last component, or `fallback_filename` if the input is empty / has none.
/// Pure — does not touch the filesystem.
pub fn compose_save_path(dir: &Path, current_input: &str, fallback_filename: &str) -> PathBuf {
    let trimmed = current_input.trim();
    let name = Path::new(trimmed)
        .file_name()
        .filter(|n| !n.is_empty())
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from(fallback_filename));
    dir.join(name)
}

/// True when `name` is a single path component we can join onto a save dir.
/// Rejects empty, `.` / `..`, and anything with a path separator.
pub fn is_valid_save_filename(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && n != "." && n != ".." && !n.contains('/') && !n.contains('\\')
}

/// Compute the centered popup rect for the browser, mirroring the geometry
/// helpers used by the other popups. Kept in one place so the render and the
/// mouse handler agree.
pub fn file_browser_rect(area: Rect) -> Rect {
    let w = (area.width * 4 / 5).clamp(50, 96);
    let h = (area.height * 7 / 10).clamp(16, 40);
    let x = area.width.saturating_sub(w) / 2;
    let y = area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

/// The footer button row sits on the last inner line of the popup — i.e. the
/// line directly above the bottom border. The paragraph's inner area starts at
/// `popup.y + 1`; the render pads lines up to index `popup.height - 3` and
/// pushes the footer as the last line at that same index, so its screen y is
/// `popup.y + 1 + (popup.height - 3)` = `popup.y + popup.height - 2`.
pub fn footer_button_row(popup: Rect) -> u16 {
    popup.y + popup.height.saturating_sub(2)
}

/// Screen y of the Save As `Name:` field — the inner line directly above
/// the footer. Open-mode browsers do not render this row.
pub fn filename_row(popup: Rect) -> u16 {
    footer_button_row(popup).saturating_sub(1)
}

/// The list body occupies inner rows `(1 + HEADER_LINES) .. footer`.
/// When `reserve_filename_row` is true the last inner line above the footer
/// is the Name field, so the list ends one row earlier.
/// Returns `(start_y, end_y)` in absolute screen coordinates; `end_y` is
/// exclusive.
pub fn list_area(popup: Rect, reserve_filename_row: bool) -> (usize, usize) {
    let start = (popup.y + 1 + HEADER_LINES as u16) as usize;
    let footer = (popup.y + popup.height.saturating_sub(2)) as usize;
    let end = if reserve_filename_row {
        footer.saturating_sub(1)
    } else {
        footer
    };
    (start, end)
}

/// Map a screen row `y` to the entry index rendered there, or `None` if `y`
/// falls outside the list body. Uses the same `compute_scroll` the renderer
/// uses, so the result always matches the row the user sees. The caller is
/// responsible for filtering the result against `browser_entries.len()` — a
/// row inside the body but past the last entry (blank padding) returns
/// `Some(idx)` with `idx >= len` and must be treated as "no entry".
pub fn list_index_at(y: usize, area: (usize, usize), sel: usize) -> Option<usize> {
    if y < area.0 || y >= area.1 {
        return None;
    }
    let body_h = (area.1 - area.0).max(1);
    let scroll = compute_scroll(sel, body_h);
    Some((y - area.0) + scroll)
}

/// Rightmost inner column of the list body — the overflow scrollbar track.
pub fn list_scrollbar_rect(popup: Rect, reserve_filename_row: bool) -> Rect {
    let (start, end) = list_area(popup, reserve_filename_row);
    let h = end.saturating_sub(start) as u16;
    Rect::new(popup.x + popup.width.saturating_sub(2), start as u16, 1, h)
}

/// True when `(x, y)` is on the list-body overflow track. Always false when
/// the listing fits the viewport so the last filename column stays clickable.
pub fn list_click_is_scrollbar(
    x: usize,
    y: usize,
    popup: Rect,
    reserve_filename_row: bool,
    overflowing: bool,
) -> bool {
    if !overflowing {
        return false;
    }
    let sb = list_scrollbar_rect(popup, reserve_filename_row);
    sb.width > 0
        && x >= sb.x as usize
        && x < (sb.x + sb.width) as usize
        && y >= sb.y as usize
        && y < (sb.y + sb.height) as usize
}

pub fn list_overflow_bar(
    entry_count: usize,
    sel: usize,
    popup: Rect,
    reserve_filename_row: bool,
) -> OverflowBar {
    let (start, end) = list_area(popup, reserve_filename_row);
    let body_h = end.saturating_sub(start).max(1);
    OverflowBar {
        total: entry_count,
        position: compute_scroll(sel, body_h),
        viewport: body_h,
    }
}

/// Render the browser popup. Pure presentation — all state is read from `app`.
pub fn render(f: &mut Frame, area: Rect, theme: &Theme, app: &crate::App) {
    let popup = file_browser_rect(area);

    let title = match app.browser_mode {
        BrowserMode::Dir if app.browser_origin_is_save => {
            format!(" Save to — {} ", app.browser_cwd.display())
        }
        BrowserMode::Dir => format!(" Browse — {} ", app.browser_cwd.display()),
        BrowserMode::Recent => " Recent Files ".to_string(),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let dir_style = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let file_style = Style::default().fg(Color::from(&theme.editor_fg));
    let selection_bg = Style::default()
        .fg(Color::from(&theme.editor_bg))
        .bg(Color::from(&theme.accent));

    let mut lines: Vec<Line> = Vec::new();

    // Header hint line.
    let hint = match app.browser_mode {
        BrowserMode::Dir if app.browser_origin_is_save => {
            "Enter dir: Enter/→/l   Tab: Name field / buttons   Esc: back"
        }
        BrowserMode::Dir => {
            "Enter dir: Enter/→/l   Up: Backspace/←/h/-   \
                             Toggle hidden: Ctrl+H   Tab: footer buttons"
        }
        BrowserMode::Recent => {
            "Open: Enter/l   Switch to browser: Tab to [Browse]   \
                                Esc: back to dialog"
        }
    };
    lines.push(Line::from(Span::styled(hint, dim)));
    lines.push(Line::from(""));

    let reserve_name = app.browser_origin_is_save;
    let (list_start, list_end) = list_area(popup, reserve_name);
    let body_h = list_end.saturating_sub(list_start).max(1);

    if let Some(err) = &app.browser_error {
        lines.push(Line::from(Span::styled(
            format!(" Error: {} ", err),
            Style::default()
                .fg(Color::from(&theme.editor_bg))
                .bg(Color::Red),
        )));
    } else if app.browser_entries.is_empty() {
        let msg = match app.browser_mode {
            BrowserMode::Dir => "(empty directory)",
            BrowserMode::Recent => "(no recent files yet — open or save a file first)",
        };
        lines.push(Line::from(Span::styled(msg, dim)));
    } else {
        // Scroll is derived from the selection via the shared `compute_scroll`
        // helper — the click handler uses the same function, so a screen row
        // always maps back to the same entry the user sees highlighted.
        let sel = app.browser_selected;
        let scroll = compute_scroll(sel, body_h);
        let scroll = scroll.min(app.browser_entries.len().saturating_sub(1));

        let show_from = scroll;
        let show_to = (show_from + body_h).min(app.browser_entries.len());
        let overflowing =
            list_overflow_bar(app.browser_entries.len(), sel, popup, reserve_name).overflowing();
        let inner_w = popup.width.saturating_sub(2) as usize;
        let name_budget = if overflowing {
            inner_w.saturating_sub(3) // marker + space + scrollbar column
        } else {
            inner_w.saturating_sub(2)
        };

        for i in show_from..show_to {
            let entry = &app.browser_entries[i];
            let marker = if entry.is_dir { "/" } else { " " };
            let mut name = format!("{}{}", entry.name, if entry.is_dir { "/" } else { "" });
            if name.chars().count() > name_budget {
                name = name.chars().take(name_budget).collect();
            }
            let base = if entry.is_dir { dir_style } else { file_style };
            // Hover steals the highlight: when the mouse is over a row, that
            // row is the only one highlighted. With no hover, the keyboard
            // selection (`browser_selected`, when the list is focused) shows.
            let is_hover = app.browser_list_hover == Some(i);
            let is_sel = i == sel
                && app.browser_button_selected == FOCUS_LIST
                && app.browser_list_hover.is_none();
            let style = if is_hover || is_sel {
                selection_bg
            } else {
                base
            };
            lines.push(Line::from(vec![
                Span::styled(marker, dim),
                Span::raw(" "),
                Span::styled(name, style),
            ]));
        }
    }

    // Pad so the Name field (Save As) and footer sit on the last inner lines.
    let inner_h = popup.height.saturating_sub(2) as usize; // minus borders
    let name_pad = if reserve_name { 2 } else { 1 };
    while lines.len() < inner_h.saturating_sub(name_pad) {
        lines.push(Line::from(""));
    }

    if reserve_name {
        let focused = app.browser_button_selected == FOCUS_FILENAME;
        let name_style = if focused { selection_bg } else { dim };
        let caret = if focused { "█" } else { "" };
        lines.push(Line::from(vec![
            Span::styled(" Name: ", dim),
            Span::styled(format!("{}{}", app.browser_filename, caret), name_style),
        ]));
    }

    // Footer buttons.
    let labels = app.browser_footer_labels();
    let is_primary = if app.browser_origin_is_save {
        [true, false, false, false] // Use folder
    } else {
        [false, true, false, false] // Recent/Browse toggle
    };
    let mut row = ButtonRow::new(&labels, &is_primary);
    // 0..=3 are buttons; FOCUS_LIST/FOCUS_FILENAME → no button selected.
    if app.browser_button_selected < 4 {
        row.select(Some(app.browser_button_selected));
    }
    if let Some(h) = app.browser_button_hover {
        row.set_hover(Some(h));
    }
    let _ = accent; // suppress unused warning if dim path took everything
    let footer_line = row.render(theme);
    lines.push(footer_line);

    let para = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(Color::from(&theme.editor_fg)));

    f.render_widget(Clear, popup);
    f.render_widget(para, popup);

    let bar = list_overflow_bar(
        app.browser_entries.len(),
        app.browser_selected,
        popup,
        reserve_name,
    );
    scrollbar::render(f, list_scrollbar_rect(popup, reserve_name), bar, theme);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn entry(name: &str, is_dir: bool) -> BrowserEntry {
        BrowserEntry {
            name: name.to_string(),
            is_dir,
            path: PathBuf::from(format!("/tmp/{}", name)),
        }
    }

    #[test]
    fn sort_entries_directories_first_then_alpha_case_insensitive() {
        let input = vec![
            entry("zebra.txt", false),
            entry("Apple.txt", false),
            entry("src", true),
            entry("Beta", true),
            entry("banana.rs", false),
        ];
        let out = sort_entries(input);

        let names: Vec<&str> = out.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Beta", "src", "Apple.txt", "banana.rs", "zebra.txt"]
        );
    }

    #[test]
    fn filter_hidden_hides_dotfiles_by_default() {
        let input = vec![
            entry(".secret", false),
            entry("visible.txt", false),
            entry(".config", true),
            entry("src", true),
        ];
        let out = filter_hidden(input, false);
        let names: Vec<&str> = out.iter().map(|e| e.name.as_str()).collect();
        // Dotfile dirs are also hidden by default (matches typical file managers).
        assert_eq!(names, vec!["visible.txt", "src"]);
    }

    #[test]
    fn filter_hidden_keeps_everything_when_enabled() {
        let input = vec![entry(".secret", false), entry("visible.txt", false)];
        let out = filter_hidden(input, true);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn parent_entry_is_some_when_cwd_has_parent() {
        let cwd = Path::new("/home/josh");
        let p = BrowserEntry::parent_entry(cwd).unwrap();
        assert_eq!(p.name, "..");
        assert!(p.is_dir);
        assert_eq!(p.path, PathBuf::from("/home/josh/.."));
    }

    #[test]
    fn parent_entry_is_none_at_root() {
        assert!(BrowserEntry::parent_entry(Path::new("/")).is_none());
    }

    #[test]
    fn read_directory_lists_and_sorts_with_pinned_parent() {
        let tmp = tempfile_dir();
        fs::create_dir_all(tmp.join("subdir")).unwrap();
        fs::write(tmp.join("zebra.txt"), "").unwrap();
        fs::write(tmp.join("alpha.md"), "").unwrap();
        fs::write(tmp.join(".hidden"), "").unwrap();

        let entries = read_directory(&tmp, false).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        // `..` first (pinned), then dirs, then files alpha; dotfile hidden.
        assert_eq!(names[0], "..");
        assert!(names.contains(&"subdir"));
        assert!(names.contains(&"alpha.md"));
        assert!(names.contains(&"zebra.txt"));
        assert!(!names.contains(&".hidden"));
        // subdir before files
        assert!(
            names.iter().position(|n| *n == "subdir").unwrap()
                < names.iter().position(|n| *n == "alpha.md").unwrap()
        );

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_directory_shows_dotfiles_when_requested() {
        let tmp = tempfile_dir();
        fs::write(tmp.join(".visible_now"), "").unwrap();
        fs::write(tmp.join("plain"), "").unwrap();

        let entries = read_directory(&tmp, true).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&".visible_now"));
        assert!(names.contains(&"plain"));

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_directory_errors_on_file_not_dir() {
        let tmp = tempfile_dir();
        let file_path = tmp.join("not_a_dir");
        fs::write(&file_path, "").unwrap();
        let result = read_directory(&file_path, false);
        assert!(result.is_err());
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_directory_follows_symlink_to_dir() {
        let tmp = tempfile_dir();
        let real = tmp.join("real_dir");
        let link = tmp.join("link_dir");
        fs::create_dir_all(&real).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();

        #[cfg(unix)]
        {
            let entries = read_directory(&tmp, true).unwrap();
            let link_entry = entries.iter().find(|e| e.name == "link_dir").unwrap();
            assert!(link_entry.is_dir, "symlink to dir should be enterable");
        }

        fs::remove_dir_all(&tmp).ok();
    }

    /// Helper: create a unique temp dir without pulling in the `tempfile` crate.
    fn tempfile_dir() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn footer_button_row_is_last_inner_line_above_bottom_border() {
        // Regression for the off-by-one that made hover register one row
        // below the visible footer buttons.
        let popup = Rect::new(10, 5, 80, 20);
        // Inner area: rows 6..=23 (borders at 5 and 24). Last inner row = 23.
        // Bottom border = 24. Footer must sit at 23, NOT 24.
        assert_eq!(footer_button_row(popup), 5 + 20 - 2);
        assert_eq!(footer_button_row(popup), 23);
        assert!(footer_button_row(popup) < popup.y + popup.height - 1);
    }

    #[test]
    fn header_lines_constant_matches_rendered_header() {
        // hint line + blank separator = 2 lines above the body. If this
        // changes, `render` must push the same number of lines before the
        // first entry or clicks will drift again.
        assert_eq!(HEADER_LINES, 2);
    }

    #[test]
    fn list_area_first_row_matches_first_rendered_entry() {
        // Regression for the off-by-one where clicking the first entry
        // selected the second. The first entry renders at inner row
        // HEADER_LINES (0-indexed) inside the bordered block, i.e. at screen
        // y = popup.y + 1 + HEADER_LINES. list_area.0 must equal that.
        let popup = Rect::new(10, 5, 80, 20);
        let (start, end) = list_area(popup, false);
        assert_eq!(start, popup.y as usize + 1 + HEADER_LINES);
        assert_eq!(start, 5 + 1 + 2); // = 8
                                      // end is the exclusive upper bound; footer begins at end.
        assert_eq!(end, (popup.y + popup.height - 2) as usize);
        assert_eq!(end, footer_button_row(popup) as usize);
    }

    #[test]
    fn list_area_with_filename_row_ends_one_above_footer() {
        let popup = Rect::new(10, 5, 80, 20);
        let (start, end) = list_area(popup, true);
        assert_eq!(start, popup.y as usize + 1 + HEADER_LINES);
        assert_eq!(end, footer_button_row(popup) as usize - 1);
        assert_eq!(filename_row(popup), footer_button_row(popup) - 1);
        assert_eq!(filename_row(popup) as usize, end);
    }

    #[test]
    fn is_valid_save_filename_rejects_empty_dots_and_separators() {
        assert!(!is_valid_save_filename(""));
        assert!(!is_valid_save_filename("  "));
        assert!(!is_valid_save_filename("."));
        assert!(!is_valid_save_filename(".."));
        assert!(!is_valid_save_filename("a/b"));
        assert!(!is_valid_save_filename("a\\b"));
        assert!(is_valid_save_filename("notes.md"));
        assert!(is_valid_save_filename("untitled"));
    }

    #[test]
    fn compose_save_path_uses_basename_of_typed_path() {
        let out = compose_save_path(
            Path::new("/tmp/dest"),
            "/home/josh/project/main.rs",
            "untitled",
        );
        assert_eq!(out, PathBuf::from("/tmp/dest/main.rs"));
    }

    #[test]
    fn compose_save_path_uses_plain_filename() {
        let out = compose_save_path(Path::new("/tmp/dest"), "notes.md", "untitled");
        assert_eq!(out, PathBuf::from("/tmp/dest/notes.md"));
    }

    #[test]
    fn compose_save_path_falls_back_when_input_empty() {
        let out = compose_save_path(Path::new("/tmp/dest"), "  ", "untitled");
        assert_eq!(out, PathBuf::from("/tmp/dest/untitled"));
    }

    #[test]
    fn compose_save_path_falls_back_when_input_has_no_filename() {
        // Path::new("/").file_name() is None
        let out = compose_save_path(Path::new("/tmp/dest"), "/", "untitled");
        assert_eq!(out, PathBuf::from("/tmp/dest/untitled"));
    }

    #[test]
    fn compute_scroll_keeps_selection_in_first_page_when_small() {
        assert_eq!(compute_scroll(0, 10), 0);
        assert_eq!(compute_scroll(3, 10), 0);
        assert_eq!(compute_scroll(9, 10), 0);
    }

    #[test]
    fn compute_scroll_advances_page_when_selection_exceeds_viewport() {
        // Page-aligned: sel 10 in a 10-row viewport starts the second page.
        assert_eq!(compute_scroll(10, 10), 10);
        assert_eq!(compute_scroll(15, 10), 10);
        assert_eq!(compute_scroll(25, 10), 20);
    }

    #[test]
    fn compute_scroll_handles_zero_or_unit_viewport() {
        // Must never divide by zero. With body_h clamped to 1, every row is
        // its own "page", so scroll simply tracks sel.
        assert_eq!(compute_scroll(0, 0), 0);
        assert_eq!(compute_scroll(5, 0), 5);
        assert_eq!(compute_scroll(3, 1), 3);
    }

    #[test]
    fn list_index_at_returns_none_outside_list_area() {
        let area = (10usize, 20usize);
        assert_eq!(list_index_at(9, area, 0), None); // above
        assert_eq!(list_index_at(20, area, 0), None); // at footer (exclusive end)
        assert_eq!(list_index_at(21, area, 0), None); // below
    }

    #[test]
    fn list_index_at_first_row_is_index_zero_when_unscrolled() {
        let area = (10usize, 20usize);
        // sel in the first page → scroll 0 → top row is index 0.
        assert_eq!(list_index_at(10, area, 0), Some(0));
        assert_eq!(list_index_at(10, area, 5), Some(0));
        assert_eq!(list_index_at(15, area, 5), Some(5)); // middle row
        assert_eq!(list_index_at(19, area, 5), Some(9)); // last body row
    }

    #[test]
    fn list_index_at_accounts_for_scroll_past_first_page() {
        let area = (10usize, 20usize); // body_h = 10
                                       // sel 15 → compute_scroll(15,10)=10 → top row maps to index 10.
        assert_eq!(list_index_at(10, area, 15), Some(10));
        assert_eq!(list_index_at(15, area, 15), Some(15)); // row under sel
        assert_eq!(list_index_at(19, area, 15), Some(19)); // last body row
    }

    #[test]
    fn list_scrollbar_rect_is_rightmost_inner_column_of_body() {
        let popup = file_browser_rect(Rect::new(0, 0, 80, 24));
        let (start, end) = list_area(popup, false);
        let sb = list_scrollbar_rect(popup, false);
        assert_eq!(sb.x, popup.x + popup.width - 2);
        assert_eq!(sb.width, 1);
        assert_eq!(sb.y as usize, start);
        assert_eq!(sb.height as usize, end - start);
        assert!(list_click_is_scrollbar(
            sb.x as usize,
            sb.y as usize,
            popup,
            false,
            true
        ));
        assert!(!list_click_is_scrollbar(
            sb.x as usize - 1,
            sb.y as usize,
            popup,
            false,
            true
        ));
        assert!(
            !list_click_is_scrollbar(sb.x as usize, sb.y as usize, popup, false, false),
            "hidden bar must not steal the last filename column"
        );
    }

    #[test]
    fn overflowing_when_entries_exceed_body() {
        let popup = file_browser_rect(Rect::new(0, 0, 80, 24));
        let (a, b) = list_area(popup, false);
        let body_h = b - a;
        assert!(list_overflow_bar(body_h + 5, 0, popup, false).overflowing());
        assert!(!list_overflow_bar(body_h, 0, popup, false).overflowing());
        assert!(!list_overflow_bar(0, 0, popup, false).overflowing());
    }
}
