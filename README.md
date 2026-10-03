<div align="center">
<picture>
<source media="(prefers-color-scheme: dark)" srcset="docs/logo.svg">
<source media="(prefers-color-scheme: light)" srcset="docs/logo.svg">
<img alt="rust-edit" src="docs/logo.svg" width="120">
</picture>

<h1>rust-edit</h1>
<p>Edit text and code in a Linux terminal, with tabs, menus, and a mouse.</p>
</div>

<div align="center">

![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)![Version](https://img.shields.io/badge/version-0.1.0-blue)![Platform](https://img.shields.io/badge/platform-Linux-blue?logo=linux&logoColor=white)

</div>

<div align="center">
<a href="#quick-start">Quick Start</a> ·
<a href="#install">Install</a> ·
<a href="#everyday-keys">Everyday keys</a> ·
<a href="https://github.com/RapidFire770/rust-edit/issues/new">Report a bug</a>
</div>

---

## Why rust-edit?

If you edit files in a Linux terminal and want menus, tabs, and a mouse alongside the keyboard, rust-edit is for you.

It keeps the habits of a desktop editor inside the terminal: several files open as tabs, a click to place the cursor, a drag to select text, and Ctrl+S to save.

## Features

- **The mouse and the keyboard both finish the job.** Click tabs, menus, scroll bars, and popup buttons, or use the matching keys.
- **Desktop editing habits.** Tabs, a menu bar, undo and redo, and the system clipboard.
- **A home-folder install and an application menu entry.** The menu item is named Rust-Edit and opens in your desktop's default terminal. An update asks before it replaces the copy you already have.
- **Themes you can switch or design.** Dark Red, Light, High Contrast, Solarized, Monochrome, and Terminal Green apply as soon as you choose them. You can also build, edit, and delete your own themes.
- **Find and replace you can type into.** Match case, a regular expression, or a whole word. You type the search, then Tab to the keys that jump between matches.
- **A prompt when a file changes on disk.** You can reload it, keep your copy, or reload that file quietly for the rest of the session. With **Auto-reload all** turned on, unmodified files reload on their own. rust-edit asks before it replaces a file that has unsaved edits.
- **Format the current file from the Tools menu.** rust-edit runs `rustfmt`, `black`, `prettier`, `clang-format`, `gofmt`, `zig fmt`, `perltidy`, `stylua`, or `xmllint` when that program is already installed. Otherwise it expands tabs to your tab size, strips trailing whitespace, and ends the file with a newline.
- **Color for many languages and config files.** Rust, Python, Go, Java, C, C++, C#, JavaScript, TypeScript, HTML, CSS, JSON, JSONC, TOML, YAML, INI, XML, and shell are among them. Help lists every extension. Other files stay plain text.

## When to use it

Open rust-edit when a file is a path in your shell, or choose Rust-Edit in the application menu. Press F1 any time for the full list of shortcuts, including the file browser and the theme editor.

This release edits local files in one window on Linux. Code completion, split panes, git tools, plugins, and restoring an earlier session are left for a later release. macOS and Windows are left for a later release.

## Quick Start

Download the latest `rust-edit-x.x.x-x86_64-unknown-linux-gnu.tar.gz` from [Releases][releases-url], then:

```bash
tar -xzf rust-edit-x.x.x-x86_64-unknown-linux-gnu.tar.gz
cd rust-edit-x.x.x-x86_64-unknown-linux-gnu
./install.sh
rust-edit
```

## Install

`./install.sh` installs rust-edit for your user. The release archive already contains the program. Run it from that unpacked folder. A copy that has been moved to the trash refuses to install.

| Piece                  | Where it goes                                              |
|------------------------|------------------------------------------------------------|
| Program                | `~/.local/bin/rust-edit`                                   |
| Uninstaller            | `~/.local/bin/rust-edit-uninstall`                         |
| Application menu entry | Rust-Edit, opened in your default terminal                 |
| Menu icon              | `~/.local/share/icons/hicolor/scalable/apps/rust-edit.svg` |

Run `./install.sh` again to update. When a copy is already installed, the script compares versions and asks before replacing it. `./install.sh --yes` answers yes to that prompt, including a reinstall or a downgrade.

The menu entry and icon normally live under `~/.local/share`. When `XDG_DATA_HOME` is set, they go there instead.

When the shell cannot find `rust-edit`, add `~/.local/bin` to your PATH. Put this line in `~/.profile`, then open a new terminal:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

sudo does not look in `~/.local/bin`. The installer asks whether to link `~/.local/bin/rust-edit` into `/usr/local/bin`, which is on sudo's command path. Saying yes asks for your password once. Saying no still leaves the home-folder install in place. The link points at the copy in your home folder, and sudo runs that file as root. `rust-edit-uninstall` removes that link when it points at this copy.

```bash
sudo rust-edit /etc/hosts
```

When the menu entry is missing after install, log out and back in.

Remove the program from any directory with `rust-edit-uninstall`. From an unpacked archive, `./install.sh --uninstall` removes the same files: the program, the menu entry, and the icon. Settings in `~/.config/rust-edit` stay in place.

## Usage

Start on an empty tab, or name the files to open:

```bash
rust-edit
rust-edit notes.md src/main.rs
rust-edit --version
```

`--version` and `-V` print the version and exit. `--` ends option parsing, so a file that is itself named `--version` still opens:

```bash
rust-edit -- --version
```

Every other argument is a path to open.

### System files

A file you can read but not write opens with a read-only notice. The status bar says Read-only. Ctrl+S explains why the save failed and leaves the modified mark on the tab. Your edits stay in the tab. Save As (Ctrl+Alt+S) can store a copy in a folder you can write.

`sudo rust-edit` only works after install links the program into `/usr/local/bin`. sudo does not search `~/.local/bin`. If that link is missing, pass the real program path:

```bash
sudo ~/.local/bin/rust-edit /etc/hosts
```

The notice prints that command with the path of the binary you are running.

### Menus

The menu bar is **File**, **Tools**, and **Settings**. Alt+F, Alt+T, and Alt+S open them. A click opens them too. Left, Right, and Tab move between those three menus while one is open.

- **File** creates a tab, opens and saves, closes a tab, and quits. Closing or quitting with unsaved edits asks you to confirm.
- **Open** and **Save As** start as a path dialog. **Browse…** opens the folder listing. On Open, that listing can switch to recent files; each recent row shows `name @ directory`, and a long directory is truncated from the left. Click `[x]`, or press Delete or Backspace, to drop one recent; **Clear** empties the list. Paths that no longer exist are removed when you open the list. On Save As, type a file name and choose **Use folder** to save into the folder you are viewing.
- **Tools → Find & Replace** opens the find bar. Tab moves from the query, to the replacement, to the command keys. On the command keys, `n` and `N` move between matches, and `c`, `x`, and `w` toggle case, regular expression, and whole word. Whole word applies to a plain search. A regular expression is the pattern you type.
- Enter and `r` replace the current match and insert the replacement as written. While the bar is open, Ctrl+A replaces every match. An empty replacement leaves the text alone. Replace All on a regular expression expands `$` groups such as `$1`, and it matches case exactly even when the case toggle is off.
- **Tools → Format Document** formats the current file. The status bar names the formatter that ran (`rustfmt`, `black`, `prettier`, `clang-format`, `gofmt`, `zig`, `perltidy`, `stylua`, `xmllint`, or editor preferences). PHP, Ruby, SQL, INI, TOML, shell, and the other languages without one of those tools use the editor-preferences cleanup. When an external formatter runs and fails, the file is left as it was and a message explains why.
- **Tools → Word Count** (Alt+W) reports words, characters, and lines for the file. A selection adds the same word and character counts for that span.
- **Settings → Configure** sets tab size, word wrap, line numbers, spellcheck, and whether unmodified files reload on their own.
- **Tools → Toggle Spellcheck** is the same switch. While it is on, misspelled words keep their syntax color and gain a red underline. In code, that mark is limited to comments and strings. Markdown, plain text, and reStructuredText check the writing and skip fenced code. Put the cursor on a marked word and press F7, or choose **Tools → Spelling (F7)**. Pick a suggestion with the keyboard or a click. **Add word** saves it in `dictionary.txt` next to `config.toml`. **Ignore** hides that spelling until you quit. Spellcheck uses the SCOWL American English word list.
- **Settings → Themes** applies a built-in theme or edits a custom one. The editor redraws with the new colors immediately.
- **Settings → Help** is the same window as F1: Shortcuts, Languages, and About.

Enter keeps the indentation of the current line, and adds one more level when that line ends in `{`, `[`, `(`, or `:`. Tab inserts spaces at the cursor. The number of spaces is your tab size. A selection indents with Tab and dedents with Shift+Tab.

### Files changed on disk

When another program writes a file you have open, rust-edit offers **Reload**, **Always**, and **Keep**. **Always** reloads that file quietly until you quit. **Auto-reload all** in Configure does that for every unmodified file. A file with unsaved edits still asks. If the file is deleted on disk, your text stays open and the status bar reports the deletion once.

## Everyday keys

F1 lists every shortcut. These are the ones you need on the first day.

| Action                       | Key                                     |
|------------------------------|-----------------------------------------|
| New tab                      | Ctrl+N                                  |
| Open                         | Ctrl+O                                  |
| Save                         | Ctrl+S                                  |
| Save As                      | Ctrl+Alt+S                              |
| Close tab                    | Ctrl+W                                  |
| Quit                         | Ctrl+Q                                  |
| Undo / redo                  | Ctrl+Z / Ctrl+Y                         |
| Cut / copy / paste           | Ctrl+X / Ctrl+C / Ctrl+V                |
| Select all                   | Ctrl+A (while Find is closed)           |
| Clear the selection          | Esc                                     |
| Find                         | Ctrl+F, then Tab to move between fields |
| Help                         | F1                                      |
| Word count                   | Alt+W                                   |
| Word wrap                    | Alt+Z                                   |
| Spelling suggestions         | F7                                      |
| Toggle comment               | Ctrl+/                                  |
| Switch to tab 1–9            | Alt+1 … Alt+9                           |
| Open File / Tools / Settings | Alt+F / Alt+T / Alt+S                   |

Ctrl+/ comments or uncomments the current line, or the selected lines.

- `//` for Rust, C, C++, JavaScript, TypeScript, Java, C#, Go, PHP, Scala, Groovy, D, Objective-C, Objective-C++, Zig, and JSONC
- `#` for Python, TOML, YAML, shell, Ruby, Perl, R, Tcl, Java Properties, and Makefiles
- `--` for SQL, Lua, and Haskell
- `;` for INI, Clojure, and Lisp. An INI line that starts with `#` is not treated as a comment, so the toggle inserts `;` in front of it
- `%` for Erlang, LaTeX, and TeX
- `REM` for Batch, in any letter case. A longer word such as `REMote` is not a comment
- Markdown, HTML, and XML wrap one line as `<!-- text -->`, and wrap a multi-line selection as a `<!--` line above and a `-->` line below
- CSS uses `/* */` the same way. OCaml uses `(* *)`

JSON, Diff, reStructuredText, and plain text have no comment syntax. The status bar says so.

Shift with the arrow keys, Home, End, Page Up, or Page Down extends the selection. Page Up and Page Down move by a page. Alt+H and Alt+L scroll sideways.

## Mouse

Click a tab to switch to it. Middle-click a tab to close it. Click `[+]` for a new tab. When the tab strip overflows, click `[<]` and `[>]`, or roll the wheel over the tabs.

Click in the text to place the cursor, and drag to select. The wheel scrolls vertically. Shift and the wheel scroll horizontally. Drag the scroll bars to move through a long file or a long line. The horizontal bar shows up when word wrap is off and a line is wider than the window.

Popup buttons accept a click, including save confirmations, the file browser, Configure, and theme colors.

## Settings

rust-edit starts from these defaults. Changing a setting, or opening a file, writes `~/.config/rust-edit/config.toml`. When `XDG_CONFIG_HOME` is set, that file is `$XDG_CONFIG_HOME/rust-edit/config.toml`.

| Setting         | Default               | In the file            |
|-----------------|-----------------------|------------------------|
| Tab size        | 4 spaces, from 1 to 8 | `tab_size`             |
| Word wrap       | Off                   | `word_wrap`            |
| Line numbers    | On                    | `show_line_numbers`    |
| Auto-reload all | Off                   | `auto_reload_all`      |
| Spellcheck      | Off                   | `spellcheck`           |
| Theme           | Dark Red              | `theme` = `"dark-red"` |

Theme names in that file are `dark-red`, `light`, `high-contrast`, `solarized`, `monochrome`, and `terminal-green`, plus the name of any theme you create. The editor also keeps up to 10 recent file paths in the same file. Uninstall leaves this directory alone.

## Languages

Highlighting follows the file extension. A few names are recognized with no extension: `Makefile`, `GNUmakefile`, `Gemfile`, `Rakefile`, `Vagrantfile`, `config.ru`, and the shell startup files `.bashrc`, `.bash_profile`, `.bash_login`, `.bash_logout`, `.profile`, `.zshrc`, and `.zprofile`.

| Language         | Extensions                                              |
|------------------|---------------------------------------------------------|
| Bash / shell     | `.sh`, `.bash`, `.zsh`, `.fish`                         |
| Batch            | `.bat`, `.cmd`                                          |
| C                | `.c`, `.h`                                              |
| C#               | `.cs`, `.csx`                                           |
| C++              | `.cpp`, `.cc`, `.cxx`, `.cp`, `.hpp`, `.hh`, `.hxx`, `.inl`, `.ipp` |
| Clojure          | `.clj`, `.cljs`, `.cljc`                                |
| CSS              | `.css`                                                  |
| D                | `.d`, `.di`                                             |
| Diff             | `.diff`, `.patch`                                       |
| Erlang           | `.erl`, `.hrl`                                          |
| Go               | `.go`                                                   |
| Groovy           | `.groovy`, `.gvy`, `.gradle`                            |
| Haskell          | `.hs`                                                   |
| HTML             | `.html`, `.htm`, `.shtml`, `.xhtml`                     |
| INI              | `.ini`, `.cfg`                                          |
| Java             | `.java`                                                 |
| Java Properties  | `.properties`                                           |
| JavaScript       | `.js`, `.mjs`, `.jsx`                                   |
| JSON             | `.json`                                                 |
| JSONC            | `.jsonc`                                                |
| LaTeX            | `.tex`, `.ltx`                                          |
| Lisp             | `.lisp`, `.cl`, `.el`, `.scm`, `.ss`                    |
| Lua              | `.lua`                                                  |
| Makefile         | `.mk`, `.mak`, and the filename `Makefile`              |
| Markdown         | `.md`, `.markdown`                                      |
| Objective-C      | `.m`                                                    |
| Objective-C++    | `.mm`                                                   |
| OCaml            | `.ml`, `.mli`                                           |
| Perl             | `.pl`, `.pm`, `.pod`                                    |
| PHP              | `.php`, `.phtml`, `.php3`, `.php4`, `.php5`, `.php7`, `.phps`, `.phpt` |
| Python           | `.py`, `.pyw`, `.pyi`, `.py3`                           |
| R                | `.r`                                                    |
| reStructuredText | `.rst`, `.rest`                                         |
| Ruby             | `.rb`, `.rake`, `.gemspec`, plus `Gemfile` and `Rakefile` |
| Rust             | `.rs`                                                   |
| Scala            | `.scala`, `.sbt`                                        |
| SQL              | `.sql`, `.ddl`, `.dml`                                  |
| Tcl              | `.tcl`                                                  |
| TeX              | `.sty`, `.cls`                                          |
| TOML             | `.toml`                                                 |
| TypeScript       | `.ts`, `.tsx`                                           |
| XML              | `.xml`, `.xsd`, `.xslt`, `.svg`, `.rss`, `.opml`        |
| YAML             | `.yaml`, `.yml`                                         |
| Zig              | `.zig`                                                  |

Markdown is highlighted as Markdown source. `.h` is C. `.conf`, `.t`, and `.pyx` stay plain text.

**Format Document** calls a formatter only when it is installed:

| Files                                                         | Program                                                              |
|---------------------------------------------------------------|----------------------------------------------------------------------|
| Rust                                                          | `rustfmt`                                                            |
| Python                                                        | `black`                                                              |
| JavaScript, TypeScript, JSON, JSONC, YAML, Markdown, HTML, CSS | `prettier`                                                         |
| C, C++, Java, C#, Objective-C, Objective-C++                  | `clang-format`                                                       |
| Go                                                            | `gofmt`                                                              |
| Zig                                                           | `zig fmt --stdin`                                                    |
| Perl                                                          | `perltidy`                                                           |
| Lua                                                           | `stylua`                                                             |
| XML                                                           | `xmllint`                                                            |
| Other highlighted files, including PHP, Ruby, SQL, INI, TOML, and shell | editor-preferences cleanup: tabs, trailing whitespace, final newline |

## Prerequisites

| Requirement | What you need                                    |
|-------------|--------------------------------------------------|
| System      | 64-bit Linux (`x86_64`)                          |
| C library   | glibc 2.39 or newer                              |
| Kernel      | Linux 6.1 or newer                               |
| Terminal    | UTF-8, plus mouse support if you want to click   |
| Color       | True color, so theme colors match their palettes |

Ubuntu 24.04, Debian 13, Fedora 40 and newer, and Arch meet the library and kernel requirement. Debian 12 and Fedora 38 are older than this build.

rust-edit is aimed at common Linux terminals, including kitty, WezTerm, Alacritty, foot, and GNOME Terminal.

## Contributing

Bug reports and changes are welcome on [GitHub][repo-url]. For a display problem, name your terminal and include the output of `rust-edit --version`.

## License

MIT. See [LICENSE](LICENSE).

---

Crafted with [Readme Craft](https://github.com/motiful/readme-craft)

<!-- Reference-style links -->

[releases-url]: https://github.com/RapidFire770/rust-edit/releases
[repo-url]: https://github.com/RapidFire770/rust-edit
