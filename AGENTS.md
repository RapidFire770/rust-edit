# AGENTS.md — Coding Agent Guidelines for Rust-Edit

**Project:** Rust-Edit — Feature-rich terminal text & code editor (TUI)  
**Primary Framework:** Ratatui v0.30+  
**Status:** v0.1.1 released - v0.1.2 in development
**Last Updated:** October 18, 2026

This file defines how coding agents (Grok Build, Cursor, Claude, human contributors, etc.) should work on the project. It exists to keep changes consistent, high-quality, and aligned with the terminal-first philosophy.

**IMPORTANT:**

- Always check available skills and invoke them when relevant to the user's request.
- If adding a new crate explain to the user why it is needed or should be used compared to what it will take to do the work without it and give them the option to say yes or no to its inclusion.
- Delete dead or obsolete code, unreachable code, or functions that are never called.
- Code must look hand-written for this repository. Changes that read like generic AI-generated code will be rejected automatically: unnecessary helper layers, vague names, boilerplate comments, defensive branches without a real failure mode, broad rewrites, or code that ignores the local style.
- For small one file changes, they can be preformed on the main git branch.
- For medium to large multi file changes and refactors use writing-plans, executing-plans and subagent-driven-development skills.
- Always use a Code reviewer subagent to review the work afterward. Create a commit once work has been validated.
- Verify that any changes do not break existing functionality.
- Keep this AGENTS.md file updated with information about the project that other AI agents would need to know to keep the project consistent from session to session. (Project layout/structure, technology stack, tool chain, coding standards & style, testing, pitfalls and conflicts to avoid, features, etc.)
- When a bug is fixed or feature is implemented check it off the users `docs/Todo-List.md` and include it in the commit for for the final work.
- AGENTS.md takes priorty over the original design spec (`docs/Rust-Edit-Design-Specification-2026-06-24.md`) as it has become outdated.

---

## 1. Core Principles (Non-Negotiable)

1. **Terminal reality first**
   - Every feature must work well in common Linux terminal emulators (kitty, wezterm, alacritty, foot, gnome-terminal, etc.).
   - No reliance on non-portable extensions (Kitty graphics protocol for "font sizes", sixel for rich text, etc.) unless explicitly approved and feature-gated.
   - When in doubt: choose the simpler, more compatible path and document the limitation.
2. **Keyboard and mouse are first-class peers**
   - Every action must be reachable both ways: full keyboard shortcuts *and* full mouse interaction (click tabs, menus, cursor placement, text selection, scrolling, and clickable buttons in every popup).
   - Neither input method is "secondary" or "optional." When adding a feature, wire up both the keybinding and the mouse path.
3. **Incremental & shippable > perfect**
   - Phase 1 is complete. Phase 2 features (full LSP, split panes, git integration, plugins, session restore — see Design Spec §5.3) require explicit approval before starting.
   - Every change should leave the app in a runnable, testable state.
   - If a major refactor would be the best choice for the future of the application give the user the option to approve or deny refactoring vs the small change.
4. **Explain tradeoffs**
   - When you have a choice (crate X vs Y, approach A vs B), briefly state the pros/cons in your reasoning or commit message. Cite terminal constraints, performance, complexity, or maintenance burden.
5. **Respect the spec — with documented deviations**
   - The Design Spec is the historical source of intent; this AGENTS.md reflects the *current* agreed scope. Where they differ, AGENTS.md wins:
     - **Markdown rendered preview and formatting chords were removed.** Markdown files keep source syntax highlighting. `Ctrl+I` is Tab on a terminal and inserts `config.tab_size` spaces at the caret. The editor does not apply italic styling to buffer text. Re-adding a preview or a formatting toolbar requires a new design decision.
     - **Help lists only chords terminals deliver.** Word count is `Alt+W`. Format Document is the Tools menu item (`Ctrl+Shift+F` arrives as `Ctrl+F` on a standard terminal; a Shift-aware handler remains for terminals that report Shift, and help does not advertise it). Help opens with `F1`; `?` inserts a question mark. Find/Replace is `Ctrl+F` (Tab moves to the replace field). The Tools menu item is "Find & Replace (Ctrl+F)" and opens that bar on the query field. `Ctrl+H` toggles hidden files in the file browser only.
     - **Single Find/Replace shortcut (**`Ctrl+F`**)** — the bar has query/replace fields reachable via Tab, so a separate `Ctrl+H` was dropped.
     - **Text fields paint a caret cell.** The hardware cursor is the editor caret. `compute_cursor_position` hides it while a popup or menu is open, so single-line fields paint their own cell from `text_field::caret_cell_style` (`Theme::cursor_style` with `REVERSED` removed, because reverse swaps the cell into the popup background) in `src/ui/text_field.rs`. The cell is one terminal column. It is the character under the caret, or a space when the caret is at the end. An unfocused field keeps that cell as a plain space so the line does not jump. The caret does not blink. Find query and replace show it only in that field (`FindFocus::Command` shows neither). The Open/Save As path always shows it, because keystrokes always edit the path while that dialog is the top popup. Save As Name shows it only when `FOCUS_FILENAME`. Themes Name and Hex show it only for `EditFocus::Name` and `EditFocus::Hex`, inside a 16-column bracket. The dictionary word box uses the same cell at its real caret and hides it while Add/Edit buttons are focused. A long value scrolls by character so the caret stays inside the field. Configure, spelling, help, confirm, reload, word count, and the project-folder picker have no text field.
     - **Edit sits between File and Tools.** Alt+E opens and toggles it, the same as Alt+F, Alt+T, and Alt+S. A click on the label does too. The items are Undo, Redo, Cut, Copy, Paste, and Select All, and each calls the same path as Ctrl+Z, Ctrl+Y, Ctrl+X, Ctrl+C, Ctrl+V, and Ctrl+A. Cut with no selection copies the current line, including its newline, and leaves the buffer alone. Select All from the menu selects the whole buffer even while Find is open; Ctrl+A in that bar still replaces every match. `MENU_BLOCK_COLS` is 34. `TAB_INNER_COLS` is 9 so three tabs and `[+]` fit on 80 columns (34 + 3*14 + 3 = 79).
     - **Tab at caret inserts spaces** — Design Spec §7 said Tab/Shift+Tab indent/dedent the line. Caret Tab now inserts `config.tab_size` spaces at the cursor (always the full width, not a remainder to the next tab stop). Non-empty selections still indent; Shift+Tab still dedents. Smart-indent extra width on Enter also uses `tab_size` (no more hardcoded 4).
     - **Ctrl+/ toggles a comment.** The unshifted `/` `?` key sends `0x1F`. Crossterm reports that as `KeyCode::Char('7')` with Control. The key arm also accepts `Char('/')` with Control. Ctrl+? is `0x7F` and is Backspace; do not bind it. Help says `Ctrl+/`. The Tools menu item is "Toggle Comment (Ctrl+/)". Line tokens live on `Language::line_comment_prefix`: `//` (Rust, C, C++, JavaScript, TypeScript, Java, C#, Go, PHP, Scala, Groovy, D, Objective-C, Objective-C++, Zig, JSONC, KickAss), `#` (Python, TOML, YAML, shell, Ruby, Perl, R, Tcl, Java Properties, Makefile), `--` (SQL, Lua, Haskell), `;` (INI, Clojure, Lisp), `%` (Erlang, LaTeX, TeX), `REM` (Batch, and BASIC after a leading line number: `10 PRINT` becomes `10 REM PRINT`; `REMOTE` is not a comment). INI also highlights `#`, but the toggle writes `;`. Languages with both a line comment and `/* */` use the line token only. KickAss `/* */` is highlighted and is not what Ctrl+/ inserts. Block pairs live on `Language::block_comment_pair` when there is no line token: Markdown, HTML, and XML use `<!--` / `-->`; CSS uses `/*` / `*/`; OCaml uses `(*` / `*)`. A one-line span wraps inline (`<!-- text -->`); a multi-line span inserts the open token on its own line before the first line and the close token on its own line after the last, indented to the span’s leading whitespace. Plain text, JSON, Diff, and reStructuredText set the status notice `No comment for this file`.
     - **v1 packaging is a home-folder tarball.** Design Spec Sprint 3 named `cargo install` and an optional `.deb`. The release path is `scripts/package.sh`, which packs the host release binary, `docs/logo.svg`, `README.md`, `LICENSE`, and `scripts/install.sh` into `dist/rust-edit-<version>-<host>.tar.gz`. `dist/` is generated and gitignored. The bundle binary is the maintainer's glibc build (glibc >= 2.39, Linux kernel >= 6.1). The binary's ELF version need includes a non-weak `GLIBC_2.39` (`pidfd_spawnp`), so Debian 12 and Fedora 38 cannot run that build. macOS and Windows stay out of scope.
     - **README.md is the end-user page.** It covers install, daily use, shortcuts, settings, and requirements. It does not link this file, the design specification, or contributor workflows. Keep contributor instructions here.
     - **Clipboard copy uses the OS clipboard, and a terminal paste is one undo.** `arboard` is built with `wayland-data-control`. On Wayland, Ctrl+C, Ctrl+X, and Ctrl+V talk to that clipboard, which is the one Foot and other programs read. arboard falls back to X11 when the protocol is missing. `App` keeps the `Clipboard` for the session because the copying process serves the data. Startup enables bracketed paste and shutdown disables it. `Event::Paste` goes through `apply_bracketed_paste`: the editor `insert`s the whole string (`\r\n` and a lone `\r` become `\n`) as one undo step. A focused single-line field (find query or replace, file-dialog path, Save As name, dictionary word, theme name or hex) receives that text with control characters removed. Find text focus still receives the paste when a menu is also open, matching typed characters. Find command mode, a menu with no text field, and a non-text popup swallow the paste. Shift+drag is Foot's own selection while mouse capture is on; Ctrl+Shift+C and Super+C copy that selection and do not read the editor selection.
     - **Function List and Variables List are session-only side panels.** Tools opens them. There is no chord to open a list. Alt+Left and Alt+Right focus the open list on that side. They do not open or close a list. The focused header uses the accent background, the same as the active tab. Choosing the Tools item again focuses an open list that does not have the keyboard, and closes it when it does. Function List prefers the left; Variables List prefers the right. They share a side only by swapping. A side holds one panel. `App.editor_origin_x` is the editor's screen column (gutter, text, vertical bar). Mouse hits, the hardware cursor, and both scrollbars add it. `ui/side_panel.rs` is the only paint and hit-test geometry. `PANEL_COLS` is 24 and `MIN_EDITOR_COLS` is 24, so both default symbol lists fit on 80 columns. `ui()` drops a panel only when `MIN_PANEL_COLS` (12) will not fit beside a 24-column editor. Opening a list still uses the saved widths, and two symbol lists then drop the right one. Enter jumps to the name byte and returns focus to the editor. Esc leaves the panel open. The wheel and that list's scrollbar move `PanelNav.scroll` and set `free_scroll`. The next paint clamps the offset and does not call `reveal` while the flag is set, so a long project tree or symbol list can reach both ends. Up, Down, `j`, and `k` clear the flag and reveal the highlight. Focusing a symbol list snaps to the caret's symbol and reveals it. An unfocused symbol list updates that highlight only when the caret's symbol index changes, and that update clears `free_scroll`. Switching the active buffer clears `free_scroll` for that list. A click on the panel header calls `focus_side`, so a symbol list snaps back to the caret's symbol. Enter clears `free_scroll` and reveals the highlight before it activates that row. `x` and `[x]` close the focused panel. `[` and `]` step that panel by one column. The floor is 12. Growth stops so the editor stays at least 24 columns and the other panel keeps its drawn width. Dragging the inner `│` uses the same limit. Each kind keeps its session width on `App.panel_widths`, including across a Left/Right move, and the width is not saved. Plain text and other unscanned languages set `No symbols for this file` and do not open. Scanners live in `src/symbols.rs` (Rust), `src/symbol_families.rs` (Python, Go, JavaScript, TypeScript), and `src/symbol_c_family.rs` (Java, C#, C, C++). They skip comments and strings. They do not list parameters, destructuring, or a C function whose return type is on the previous line. Methods read `Type::name` in Rust and `Type.name` elsewhere. The design spec's "no sidebar" line does not apply to these two panels or to the project tree. The project browser is a third panel. It starts at 36 columns (`PROJECT_PANEL_COLS`) and uses the same session width. It opens on the left from File → Open Project Folder / Project Browser, remembers the folder for the session, and persists up to 10 folders in `recent_projects`. It does not restore itself after quit. Opening still uses the saved width, so a default project tree still will not open beside a symbol list on an 80-column window. A list that is already open stays when the window shrinks and draws narrower until 12 columns will not fit.
     - **Spellcheck is English, off by default, and scoped.** `Config.spellcheck` persists. Configure and Tools → Toggle Spellcheck flip it. F7 and Tools → Spelling open the suggestion popup (Enter or a click replaces; `a` adds the word; `i` ignores it until quit; Esc closes). Underlines use theme role `spell_error` (`Modifier::UNDERLINED` plus underline color) and keep the syntax foreground. Code languages check `comment` and `string` scopes only. JSONC checks comments. JSON and Diff are skipped. Plain, Markdown, and reStructuredText check prose and skip `markup.raw`, `meta.embedded`, and `source.` spans. Tokens shorter than 2 letters, tokens with a digit, and all-ASCII capitals are skipped. The Hunspell pair is `assets/dictionaries/en_US.aff` / `.dic` (SCOWL, MIT AND BSD; license text in `en.LICENSE`), loaded by `spellbook` 0.4.2. Personal words are `$XDG_CONFIG_HOME/rust-edit/dictionary.txt`. Tools → Dictionary lists them (spellcheck may be off; opening the list does not load Hunspell). `[ Add ]` and `[ Edit ]` (`a` / `e`, or Enter on a row) open a text box on top of the list. Save accepts one checker token (`don't`, `well-known`) and refuses an empty field, more than one word, a digit, a one-letter token, or all-ASCII capitals. Delete, Backspace, and `[x]` remove a word immediately and rewrite the file, keeping `#` comments and blank lines. A failed write leaves the list and the in-memory set unchanged. There is no Dictionary chord. Session ignores stay in memory and are not listed. Suggestions run only when the popup opens. The visible window is checked from the highlight checkpoint, not the whole file. The word being typed waits 750ms or until the caret leaves it; other marks stay immediate; F7 still reads the full cache.
   - Printing/PDF export is future-only.
   - If the spec is unclear, ask before coding.

---

## 2. Tech Stack & Pinned Dependencies

Use these exact versions (or compatible patch releases) unless a compelling reason exists:

```toml
[dependencies]
ratatui = "0.30.2"
crossterm = "0.29"
ropey = "1.6.1"
syntect = "5.3.0"
clap = { version = "4.6.6", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
toml = "1.1.4"
directories = "6.0.0"
arboard = { version = "3.6.1", features = ["wayland-data-control"] }
anyhow = "1.0"
regex = "1.13.1"
```

> `pulldown-cmark` was removed when the markdown preview feature was deleted.

**Optional / evaluation only (do not add to Cargo.toml without approval):**

- `ratatui-code-editor = "0.0.4"` — promising but very new (0.0.x). Test thoroughly in isolation before considering integration.

**Dev dependencies (recommended):**

```toml
[dev-dependencies]
pretty_assertions = "1.4"
# insta = "1"          # only if we decide on snapshot testing for pure logic
```

Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` on every change.

### Toolchain / cargo path (important for agents)

**For use in most environments** : Cargo is located at `/usr/bin/cargo`.

**For use in Zcode Appimage** : The system `cargo` proxy is broken in this environment (it fails with
`error: unknown proxy name: 'zcode'`). Use the real toolchain binary instead:

```bash
CARGO="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo"
"$CARGO" --version     # sanity check
```

Discover the path with `ls ~/.rustup/toolchains/*/bin/cargo`. It is also recorded
in `docs/zcode cargo toolchain location.md`. All `cargo` commands in this file assume
you've set this `CARGO` variable (or that the proxy is fixed).

Cloud Agents already have rustup stable on `PATH` (`/usr/local/cargo/bin/cargo`, rustup home `/usr/local/rustup`). There is no `~/.rustup` tree, and the Zcode proxy note does not apply. `xterm` is installed. Start it with `stty -ixon` so Ctrl+S reaches the editor instead of XOFF.

---

## 3. Recommended Project Structure

```
rust-edit/
├── Cargo.toml
├── build.rs                   # links assets/syntaxes onto syntect's default set
├── README.md
├── AGENTS.md                  # this file
├── docs/
│   ├── Rust-Edit-Design-Specification-2026-06-24.md
│   ├── ui-design-sample-2.jpg
│   └── Todo-List.md
├── src/
│   ├── main.rs
│   ├── app.rs                 # Top-level App state, event loop, high-level commands, highlight cache
│   ├── buffer.rs              # Buffer struct + Ropey wrapper + editing ops + cursor/selection/scroll + line/block comment toggle
│   ├── config.rs              # Config loading/saving (toml + XDG)
│   ├── theme.rs               # Theme struct + color roles + built-in themes + serde
│   ├── themes.rs								# Registry of built-in + user-installed custom themes.
│   ├── highlight_cache.rs     # Parse-state checkpoints + overscan span window
│   ├── format.rs              # Format Document: rustfmt/black/prettier/clang-format/gofmt/zig/perltidy/stylua/xmllint + builtin pass
│   ├── file_watch.rs          # FileStamp mtime+len + DiskEvent classify for external reloads
│   ├── fs_access.rs           # Write probe + read-only / save-denied notice text
│   ├── syntax.rs              # Syntect wrapper, highlight_range / from_state resume
│   ├── spellcheck.rs          # English spellcheck: spellbook + SCOWL, scoped regions, suggestion cache
│   ├── project_tree.rs        # Project folder tree: expand, flatten, hidden names, symlink jail
│   ├── symbols.rs             # Rust symbol scan + Symbol type; collect() dispatches families
│   ├── symbol_families.rs     # Python, Go, JavaScript/TypeScript definition scans
│   ├── symbol_c_family.rs     # Java, C#, C, C++ definition scans
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── layout.rs          # Ratatui Layout helpers + HEADER_ROWS constant
│   │   ├── header.rs          # Menu bar + file tabs + overflow arrows + [+] + dropdown
│   │   ├── spans.rs           # Batched paint_line_spans (selection / hscroll / wrap clip)
│   │   ├── editor.rs          # Main editing view, line numbers, cursor, selection, hscroll
│   │   ├── scrollbar.rs       # Shared vertical and horizontal overflow scrollbar (editor + popups)
│   │   ├── side_panel.rs      # Function List / Variables List geometry, paint, and hit-test
│   │   ├── project_panel.rs   # Project tree row text and paint; geometry stays in side_panel
│   │   ├── status.rs          # Status bar
│   │   ├── find.rs            # Find/Replace bar (FindState + FindFocus 3-state model)
│   │   ├── text_field.rs      # One-cell caret for single-line popup and find fields
│   │   ├── button.rs          # Shared popup button chrome (hover / selected)
│   │   ├── color_picker.rs    # Premade palette + RGB slider math + swatch hit-test
│   │   └── popups/
│   │       ├── mod.rs
│   │       ├── confirm.rs     # Unsaved changes (clickable [Y]/[N] buttons)
│   │       ├── file_dialog.rs # Open / Save As (scalable, scrollable path input)
│   │       ├── file_browser.rs # Open / Save As directory listing (Browse…)
│   │       ├── project_folder.rs # Recent project folders and Browse for Open Project Folder
│   │       ├── help.rs        # Tabbed help (Shortcuts / Languages / About)
│   │       ├── configure.rs   # Settings form (tab size, wrap, line numbers, auto-reload all)
│   │       ├── dictionary.rs  # Personal spellcheck words: list, add, edit, remove
│   │       ├── themes.rs      # Theme list + custom editor (palette / RGB / hex)
│   │       ├── reload.rs      # External-change prompt ([ Reload ] [ Always ] [ Keep ])
│   │       └── word_count.rs
│   └── utils/
│       └── clipboard.rs       # arboard thin wrapper
├── scripts/
│   ├── install.sh             # ~/.local install, menu entry, version prompt
│   └── package.sh             # release tarball into dist/
├── assets/
│   └── syntaxes/              # TOML, TypeScript, Zig, INI, JSONC, BASIC, KickAss grammars + NOTICE / INI-LICENSE
└── tests/                     # Integration or property tests (optional early)
```

**Module rules:**

- Keep `ui/` focused on presentation only. Business logic (buffer mutations, find logic, formatting) lives in `buffer.rs`, `app.rs`, or dedicated modules.
- One popup = one file in `popups/`.
- No god objects. App should delegate to Buffer, Theme, SyntaxHighlighter, etc.

**Key types / established patterns (read before editing these areas):**

- `Buffer` fields: `id` (unique `AtomicU64` identity for the highlight cache key), `cursor` (byte offset), `selection` (normalized `Range`) + `selection_anchor` (fixed anchor for bidirectional extend), `scroll_offset` (row, col), `visible_lines`/`visible_width`/`word_wrap`/`tab_size` (synced each frame from the run loop), `generation` (invalidation key for the highlight cache), `is_initial` (true only for the auto-created untitled placeholder at startup or after closing the last tab; `[+]` tabs stay false), `disk_stamp` (`FileStamp` mtime+len of the last accepted on-disk identity), session `auto_reload` (set by the reload popup's `[ Always ]`; not persisted), `disk_gone_notified` (one-shot deleted-on-disk status). Auto-scroll lives in `adjust_scroll` (vertical+horizontal edge clamping; wrap-aware when `word_wrap` is set so `scroll_offset.0` stays a screen row) and `ensure_cursor_*` (use `ensure_cursor_visible_wrapped` for mouse/find under wrap). Under word wrap, Up, Down, Page Up, and Page Down move by screen row and keep the display column inside that row (`Buffer::move_screen_rows`, `page_wrapped`). Viewport `visible_lines`/`visible_width` are applied from the current frame in `App::apply_editor_viewport` (called from `ui()`) **before** `refresh_highlight_cache`; wrap scroll is clamped there on every draw, not only on `toggle_word_wrap`. Unwrapped overflow paints a bottom bar: `ui()` sets `editor_hscrollbar_visible` and shrinks `vis_h`; `Buffer.max_line_width` (generation-cached char width, no newlines) is the overflow signal; `hscrollbar_drag_active` is exclusive with text-drag and the vertical bar. File open goes through `App::open_file_in_tab` (duplicate-focus, then in-place placeholder replace, else append). `App.tab_scroll` is the first visible file-tab index; `header_width` is a Cell set each frame; `ui()` calls `clamp_tab_strip` (does not auto-reveal); `ensure_active_tab_visible` runs when `active` changes; `ui::header::layout_tab_strip` / `hit_test_header` are the single paint/hit-test source. File tabs are fixed `ui::header::TAB_COLS` (inner `TAB_INNER_COLS` = 9 plus `[ ] `chrome, 14 columns). `MENU_BLOCK_COLS` is 34, so three tabs and `[+]` fit on 80 columns (34 + 3*14 + 3 = 79). Long names ellipsize with `…`; modified `•` is kept; short names are centered (extra pad on the right when odd). `TabGeom.clipped` remains viewport-only.
- Side panels: `left_panel` / `right_panel` are `Option<PanelKind>` (`Functions`, `Variables`, or `Project`). One kind, one slot. Symbol lists start at 24 columns and the project tree at 36 (`App.panel_widths`, session only). Opening uses those saved widths, so a default project panel and one symbol list need 84 columns and a narrower window keeps the project panel and drops the other side. A list that is already open is dropped only when 12 columns will not fit beside a 24-column editor. `[` and `]` resize the focused panel. Dragging its inner `│` does the same. A side holds one panel. Open state is not written to config. `editor_origin_x`, `left_panel_area`, and `right_panel_area` are set every frame in `ui()`. Hardware cursor X, editor clicks, drag, and both editor scrollbars use that origin. The vertical bar is the last column of the editor rect, not the terminal, when a right panel is open. The find bar and status bar stay full width. Symbol text comes from `symbols::collect`, cached on `App` by buffer id, generation, and language. Alt+Left and Alt+Right focus the open list on that side. The focused header uses the accent background.
- Line-number gutter: `Buffer::GUTTER_WIDTH` is 6 ASCII columns (`{:>4}| `) when `config.show_line_numbers` is true. `App::gutter_width()` returns that or `0`. Paint (`ui/editor.rs` `gutter_prefix`), `ui()` `content_w`, hardware cursor X, mouse hit-testing, drag-edge, and `editor_hscrollbar_rect` must all use the live width. Hidden numbers do not leave a blank prefix. Wrap continuation rows indent 6 spaces only when the gutter is shown.
- `FindState.focus: FindFocus` — a 3-state model (Query → Replace → Command) so letters like `n`/`N` type normally into the boxes and navigate only in Command mode.
- An open dropdown is cycled with Left/Right/Tab via `App::cycle_open_menu` through File, Edit, Tools, and Settings; `open_menu` remains a toggle.
- Extra syntaxes: `build.rs` loads `SyntaxSet::load_defaults_newlines()`, adds every `assets/syntaxes/*.sublime-syntax` with newlines mode, and writes an uncompressed packdump. Runtime loads that dump. syntect 5.3's default set has no TOML, TypeScript, Zig, INI, or JSONC grammar, and it does not implement Sublime `extends`, so TypeScript embeds `scope:source.js` without an extra keyword list (`with_prototype` also matches inside strings and comments), and JSONC is a small self-contained grammar. `assets/syntaxes/BASIC.sublime-syntax` and `KickAss.sublime-syntax` are editor-written. `.bas` is Commodore/VS64 BASIC. `.asm` is KickAssembler. The status bar uses `Language::label()` (JSON, C#, Objective-C++), not the Debug name. Detection stays on `Language::from_path` (do not use syntect's extension finder: `.h` is claimed by C, C++, and Objective-C). `.h` stays C. `.conf`, `.t`, and `.pyx` stay plain. `.s`, `.inc`, and `.a65` stay plain. Filenames matched before the extension: `Makefile` / `GNUmakefile`, `Gemfile` / `Rakefile` / `Vagrantfile` / `config.ru`, and `.bashrc` / `.bash_profile` / `.bash_login` / `.bash_logout` / `.profile` / `.zshrc` / `.zprofile`.
- Syntax highlighting: `HighlightCache` (`src/highlight_cache.rs`) stores syntect `ParseState`/`HighlightState` checkpoints every 64 lines plus an overscanned span window and a frontier. `App::refresh_highlight_cache` is a cache fill, not a full viewport reparse. Scrolling inside the padded window is a hit. Mid-file multi-line constructs resume from the last checkpoint — never `HighlightLines::new` at the viewport. Cache key is `(language, generation, Buffer.id)` so two gen-0 tabs cannot alias. Under wrap, `wrap::visible_logical_window` translates the screen-row scroll into a logical-line window (clamping past-end scroll to the last page). Syntect rayon-style parallel highlighting is not used for a single buffer (grammars are sequential). A background highlight thread is deferred until large-file *jumps* still hitch after checkpoints exist. `editor.rs` reads `line_spans` and paints via `ui::spans::paint_line_spans` (batched tokens, not per-character Spans). `render_wrapped` uses `wrap::first_logical_line_for_screen_row` so cache fill and paint cannot disagree on the first visible logical line.
- Event coalescing: `run_app` drains queued events and sums consecutive mouse-wheel deltas into one scroll → one redraw per burst.
- Popup focus: at most one centered popup owns input (`App::popup_focus`). Other popups cannot open. Global shortcuts are swallowed except Ctrl+Q. Quit confirm paints last and stacks on top of the current popup without dismissing it. The external-reload prompt (`PopupFocus::Reload`) stacks above other popups and below quit confirm; `check_external_changes` queues the prompt while `show_confirm` is up and presents it after `close_confirm`. File browser may stack only on the file dialog (Browse…). The dictionary word editor is drawn on top of the Dictionary list and owns input until Save, Cancel, Esc, or a click outside that box; Esc there returns to the list. Save As browse has a Name field plus `[ Use folder ]` (joins cwd + the typed filename; never invents `untitled`). Open browse still only confirms files. Mouse and wheel follow `popup_focus`, not independent `show_*` flags.
- Recent files: Open browse only. `Config::remove_recent_file` / `clear_recent_files` persist immediately (skipped under `cfg(test)`). `refresh_recent` calls `prune_missing_recent_files` so gone paths and directories drop from both the listing and config (other IO errors keep the entry). Recent footer is `[ Clear ] [ Browse ] [ Hidden ] [ Cancel ]` (Dir keeps `[ Up ]` and the dual Browse/Recent toggle). Each Recent row paints a trailing `[x]`; `file_browser::recent_clear_x_range` is the single paint/hit-test source. Delete/Backspace remove the selected recent when the list is focused. No confirm dialog.
- External file changes: `src/file_watch.rs` classifies `mtime`+`len`. `run_app` caps poll at 500ms when any buffer has a path and also checks on `FocusGained`. No `notify` crate. Dirty buffers never auto-overwrite. `Config.auto_reload_all` is the global Configure toggle; per-file Always is session-only on `Buffer`. Own saves refresh `disk_stamp`. Deleted files keep the buffer and set a one-shot status notice.
- Save failures: `App::save_current` reports the `std::io::Error` through the info popup (`PopupFocus::WordCount`, title `Cannot Save`). Permission denied also sets `Buffer.read_only`. The status hint is `Read-only` while that flag is set and `format_status` is empty. Writability is `fs_access::can_write_regular_file` (open for write, no truncate). Do not use `Permissions::readonly()`. A CLI open that fails with `PermissionDenied` does not create an empty tab under that path. Untitled Ctrl+S opens Save As. The installer may symlink `~/.local/bin/rust-edit` into `/usr/local/bin` so `sudo rust-edit` is on sudo's secure_path (`/usr/local/sbin:/usr/local/bin:/usr/bin` on the maintainer machine). The home install still succeeds if that link is skipped.
- Info popup / format notice: `show_word_count` + `word_count_text` + `info_popup_title`. Word Count sets the title to `"Word Count"`; `App::show_info_popup(title, body)` reuses the same popup (`PopupFocus::WordCount`) for format errors. It is a no-op if another popup is already focused. `format_status: Option<String>` is a transient status-bar notice that replaces the `Ctrl+Q quit | F1 help` hint; `run_app` clears it on Key Press and Mouse Down before dispatching.
- Themes popup: List vs Edit. Edit has Name, 17 color roles including `spell_error` (name is NOT a role), premade swatches, RGB sliders, hex. Role list uses the shared overflow bar (`roles_scrollbar_rect` / `roles_overflow_bar`); click/drag on the track scrolls and does not change `role_selected`. The right pane is a mini chrome replica (header, tabs, gutter, selection, caret, find accent, status, and a spell underline) painted from `edit.theme` so every role is visible before Save; popup chrome stays on the saved app theme. `handle_edit_key` owns Edit keystrokes; list shortcuts `e/n/d` must not run in Edit. Geometry helpers in `themes.rs` are the single paint/hit-test source (like Configure).
- Installer: `rust-edit --version` and `rust-edit -V` print `rust-edit <CARGO_PKG_VERSION>` and exit before raw mode. `--` ends flag parsing so the remaining arguments stay filenames; every other argument is still a path. `scripts/install.sh` probes that line with `timeout` + `setsid` (no controlling terminal) so an older binary cannot put the caller's terminal into raw mode. Install paths are `$HOME/.local/bin/rust-edit`, `$HOME/.local/bin/rust-edit-uninstall` (execs a copy of `install.sh --uninstall` kept at `$XDG_DATA_HOME/doc/rust-edit/install.sh`), `$XDG_DATA_HOME/applications/rust-edit.desktop` (`Terminal=true`, absolute `Exec`, `%F` unquoted), and `$XDG_DATA_HOME/icons/hicolor/scalable/apps/rust-edit.svg` copied from `logo.svg`. Do not write a user `index.theme`. Uninstall removes those files and does not delete `~/.config/rust-edit`. `install.sh` refuses to run from a Trash path, because a shell that was already in a deleted bundle keeps executing that old copy.

---

## 4. Coding Standards & Style

### Rust

- Edition 2021.
- `cargo fmt` (default rustfmt config is fine; add `rustfmt.toml` only if we need specific rules later).
- `cargo clippy --all-targets -- -D warnings` must pass. Allow only with `#[allow(...)]` + comment explaining why.
- Error handling:
  - User-facing errors → `anyhow::Result` + context + nice popup message.
  - Internal recoverable → `Result<T, MyError>` with `thiserror` (only if we grow a small lib).
  - Never `unwrap()` or `expect()` in production paths without a very good reason.
- Async: Only introduce `tokio` when we actually need it (e.g., future LSP client). Keep the main event loop synchronous with `crossterm::event::read()` for now.
- Logging: Use `log` + `env_logger` (or `tracing`) for debug info. Never print directly to stdout/stderr except for final binary output.

### UI / Ratatui Specific

- All colors and styles come from the current `Theme` (UI chrome included). Never hard-code `Color::Red` etc. outside the default theme definition. Exception: theme-editor swatches and slider bars paint `Color::from(&ColorDef)` literals.
- Popups: Use a centered `Block` + `Clear` widget pattern. Dismiss on `Esc`, click outside (if mouse), or explicit button.
- Layout: Prefer `Layout` constraints (`Length`, `Min`, `Percentage`) over manual math.
- Text rendering in editor: Build `Vec<Line<'static>>` or use a custom widget that applies pre-computed syntax `Span`s. Do not re-highlight the entire buffer on every frame.
- Scroll & viewport: Track `scroll_offset` (row, col) per buffer. Only render visible lines.

### Theming & Config

- Theme changes must apply live (no restart) where possible.
- All user-configurable values live in `config.toml` under clear sections (`[ui]`, `[editor]`, `[theme]`).
- Provide sensible defaults; never require a config file on first run.

---

## 5. Workflow & Change Process

### Before Writing Code

1. Read the relevant section of the Design Specification.
2. Check the priority (MVP vs Phase 1 vs Future). If it's Future, stop and confirm with the user.
3. Think about terminal constraints and the "keyboard-first" rule.
4. If adding a new dependency, justify it (why not std or existing crate? size? maintenance? terminal impact?).

### Making Changes

- Small, focused commits or PRs. One logical change per PR when possible.
- For UI changes: also update or add to the manual test checklist (see §7).
- When touching the editor core (cursor, rope ops, selection, indent): add or update unit tests in `buffer.rs` or `tests/`.
- For new popups or complex UI: implement the logic first (stateless or with clear state machine), then the rendering.

### Commit / PR Messages

Good example:

```
feat(find): add incremental search with match highlighting

- Opens bottom bar on Ctrl+F
- Highlights all matches in visible area using theme accent
- Basic regex + case toggle (Phase 1 scope only)
- No Replace yet (see Design Spec §5.2)

Tested in: kitty, wezterm (mouse + keyboard)
Tradeoff: chose simple regex crate over full fancy engine for MVP size
```

Bad: "added find stuff", "fix bug", "wip".

### When to Ask Questions

Ask early if:

- The spec is ambiguous on behavior or priority.
- A terminal limitation makes the requested feature painful.
- You see a significantly better architectural approach (with clear pros/cons).
- Adding a new external dependency or major crate.

Do **not** ask for every tiny decision. Propose and note the reasoning.

---

## 6. Specific Implementation Patterns

### Adding a New Popup (example flow)

1. Create `src/ui/popups/my_popup.rs`.
2. Define a state struct (e.g., `MyPopupState { input: String, ... }`).
3. In `app.rs` or a UI coordinator, handle `Event` for that popup (key handling, mouse).
4. Render with a helper that draws a centered bordered block + inner widgets.
5. Wire dismissal (Esc → close, Enter on action → execute + close).
6. Add keyboard shortcut or menu entry that opens it.
7. Update help text / keybinding list.

### Smart Indent on Enter

- In `buffer.rs`: on newline, copy leading whitespace from previous line.
- Then apply simple language-aware rules (increase after `{`, `:` for Python, etc.) using `config.tab_size` spaces.
- Use the detected `Language` enum. Keep rules small and testable.
- Do **not** implement full tree-sitter indent queries in MVP unless the `ratatui-code-editor` route is taken.

### Syntax Highlighting

- Cache highlighted `Vec<Span>` for the current visible window only.
- Recompute on edit or scroll into new lines (dirty-line tracking is a plus).
- Fall back gracefully if syntect fails for a language.

### Theme Switching

- `Theme` is plain data (colors as `ratatui::style::Color` or hex strings + conversion).
- Changing theme in Settings immediately rebuilds the current UI draw (no restart).
- Provide 4–5 built-in themes matching the spec (Dark Red default, Light, High Contrast, etc.).

---

## 7. Testing & Verification

### Automated

- `cargo test` for all pure logic (buffer operations, config serde roundtrip, syntax language detection, find/replace engine, etc.).
- `cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`.

### Manual (required for any UI change)

Test checklist (run in at least 2 different terminals, e.g. kitty + alacritty):

- [ ] App starts and shows header + empty buffer matching the red/dark aesthetic.
- [ ] Open file from CLI and from menu; tabs appear and switch correctly.
- [ ] Typing, cursor movement (arrows, Home/End, word jumps), selection, copy/paste all work.
- [ ] Line numbers visible and correct.
- [ ] Syntax highlighting appears for supported languages (test .rs, .py, .md, .json, .java, .html, .css, .toml, .jsonc, .zig at minimum).
- [ ] A `.bas` file highlights `PRINT` and `REM`, and Ctrl+/ on `10 PRINT "HI"` becomes `10 REM PRINT "HI"`. A `.asm` file highlights `lda` and `//` comments. `.s` stays plain.
- [ ] Smart indent on Enter behaves reasonably for code.
- [ ] Find bar opens, incremental highlight works, basic replace functions.
- [ ] Theme switch in Settings applies immediately and looks good in both dark/light.
- [ ] Markdown file shows source syntax highlighting (no formatting chords; `Ctrl+I` is Tab).
- [ ] Unsaved changes confirmation appears on close/quit; Y/N works via keyboard and clickable buttons.
- [ ] Mouse works for tab switching, cursor placement, scrolling, and clicking popup buttons.
- [ ] No obvious layout breakage on resize (small vs wide terminal).
- [ ] Help (`F1`) shows accurate keybindings. `?` types a question mark.
- [ ] Large file (e.g. 50k+ lines) remains responsive (test scrolling & editing).
- [ ] Horizontal scrollbar appears on a long unwrapped line; click/drag pans; disappears under wrap / when lines fit.
- [ ] Application menu entry Rust-Edit shows the logo and opens the editor in the desktop's default terminal.
- [ ] Open browse Recent: `[x]` and Delete remove one row; `[ Clear ]` empties the list; missing files disappear from the list and from config.toml.
- [ ] Markdown Ctrl+/ wraps one line as `<!-- text -->`; a multi-line selection becomes a single `<!--` / `-->` block; a second press unwraps; JSON still shows `No comment for this file`.
- [ ] Ctrl+/ on a Java line inserts `//` and a second press removes it. CSS wraps as `/* text */`. HTML wraps as `<!-- text -->`.
- [ ] Themes Edit: role list shows an overflow bar (wheel + click/drag); the right preview shows header, tabs, gutter, selection, caret, find accent, and status from the draft theme; Save still required to apply to the real UI.
- [ ] Open/Save As, Save As Name, Themes Name, Themes Hex, Find query, Find replace, and Dictionary Add/Edit show a block caret in the focused field. Tab or arrows move the caret to the next field and leave a blank cell behind. Command mode in Find shows no field caret. The editor caret returns when the popup closes.
- [ ] Spellcheck off by default. Turn it on from Configure or Tools. A Markdown note underlines a bad word and skips a fenced code block. A Rust file underlines a bad word in a comment or string and leaves identifiers alone. F7 lists suggestions; Enter and a click replace the word. Add word clears the underline after a restart. Ignore clears it until quit. Toggle Spellcheck removes the underlines. Themes Edit shows `spell_error` as an underline in the preview.
- [ ] Tools → Dictionary lists custom words with spellcheck on or off. Add and Edit open a text box on top; Save stores one word. Delete, Backspace, or `[x]` removes it and the underline returns. Esc on the text box returns to the list.
- [ ] Type a bad word and the underline waits until a pause or until you leave the word; a finished typo elsewhere on the line stays underlined; F7 during the pause still lists suggestions.
- [ ] Word wrap on: Up and Down step onto each wrapped row of a long line and keep the column. Shift+Up and Shift+Down select across those rows. Page Up and Page Down do the same by a page. Word wrap off: Up and Down still move one logical line.
- [ ] Edit menu sits between File and Tools. Alt+E and a click open it. Undo, Redo, Cut, Copy, Paste, and Select All match the chords. Left/Right/Tab cycle File, Edit, Tools, Settings.
- [ ] Ctrl+C copies text that another program can paste. Ctrl+Shift+V or Super+V pastes several lines into the buffer, and Ctrl+Z removes that whole paste. Ctrl+V of an in-editor copy still inserts it in one step.
- [ ] Project Browser, Function List, and Variables List: with more rows than the panel, the wheel and the scrollbar reach the last row and then the first row. Up and Down still move the highlight. After Esc, wheeling a symbol list keeps that position until the caret moves onto another symbol.
- [ ] On an 80-column terminal, Tools → Function List and Tools → Variables List open on opposite sides. Enter and a click jump to the name. Left/Right swap them. A plain-text file shows `No symbols for this file`. A narrower window drops the right list first. The mouse and the keyboard both reach open, jump, move, close, and refocus. Esc, then Alt+Left or Alt+Right, returns the keyboard to that side. The focused header uses the accent background.
- [ ] File → Open Project Folder lists recent folders and Browse. Choosing one opens a 36-column tree on the left. Enter or a click expands a folder or opens a file. `x` hides the tree and Project Browser shows it again without asking. Left/Right moves it. On 80 columns the other side closes. A wider window can keep a symbol list on the other side. Quitting does not reopen the tree. Recent folders are still in the picker.
- [ ] Focus a side panel. `[` and `]` change its width by one column and stop at a readable minimum and when the editor would get too small. Drag the inner edge to the same widths. `x` still closes it. Esc still leaves it open. Left and Right still move it, and the width moves with it. Quit and the next open starts at 24 or 36 again.

Document any terminal-specific quirks you discover.

### Performance

- Editing and scrolling a 10k+ line file should feel instant.
- Highlighting should not cause visible lag on normal typing.
- If you introduce slowness, profile and explain before merging.

---

## 8. What NOT to Do Yet (needs approval)

- Do not add session restore, split panes, another sidebar besides the symbol lists and the project tree, git integration, or a plugin system. The project tree is the one project-folder panel. It uses the same side slots as Function List and Variables List and must not share a side with them. Do not restore open tabs or which panels were open.
- Do not re-add a markdown rendered preview or Ctrl+B / Ctrl+I / Ctrl+backtick formatting inserters without a fresh design decision (both were removed — see §1). Ctrl+I is Tab.
- Do not add "font size" or arbitrary rich-text WYSIWYG editing.
- Do not add built-in printing / PDF generation unless the user explicitly moves it into scope.
- Do not introduce heavy GUI crates (egui, iced, etc.) or webview.
- Do not change the core data model (Rope + per-buffer state) without strong justification.
- Do not hard-code colors or layout numbers outside the Theme / Layout system.
- Avoid "clever" global singletons or excessive use of `Rc<RefCell<>>`. Prefer ownership and message passing where possible.
- Do not add a feature with keyboard input but no mouse path (or vice versa) — see Core Principle 2.

---

## 9. Communication Style Expected from the Agent

- Direct and concise.
- Always cite the Design Specification section when relevant.
- When proposing an alternative: "Option A (current spec) vs Option B (my suggestion). Tradeoffs: X, Y, Z. Recommendation: B because..."
- For UI work: describe what the user will see and how to test it.
- If something in the spec seems suboptimal for implementation or UX, say so with evidence (complexity, performance, terminal limits) rather than silently working around it.

---

## 10. Getting Started Checklist (for a fresh clone)

> The system `cargo` proxy may be broken — see §2 "Toolchain / cargo path".
> Set `CARGO="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo"`
> and use `"$CARGO"` in place of `cargo` below if needed.

```bash
git clone ... && cd rust-edit
cargo build --release
./target/release/rust-edit                    # empty buffer
./target/release/rust-edit src/main.rs        # open a file
```

Then explore the code, run `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`,
and review `docs/Todo-List.md` for open Phase 2 ideas.

---

**Remember:** The goal is a delightful, fast, keyboard-centric terminal editor that respects the constraints of the medium. Ship working slices often, document tradeoffs, and stay aligned with the Design Specification.

If this file conflicts with new decisions, update it and note the change.

---

*This AGENTS.md is intended to be placed in the root of the rust-edit repository.*
