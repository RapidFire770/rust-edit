## Rust-Edit

A feature rich text and code editor for use in a Linux terminal

**Main Programming Language:** 

- Rust

Possible crates to use:

- [Ratatui](https://ratatui.rs/)

**UI:**

- Top menu bar with tabs for open files. Initial options will be File, Tools, Settings, Open Files Tabs, Plus Button Tab \[+\] to open a new empty file
- The menu buttons will drop down when clicked on and clicking the options should popup a window over the editor window for the user to interact with.
- Menu bar to be colored red by default, user can change color themes in the settings.
- Editing window under the menu bar, black ground with white text by default
- Line numbers down left side of editing window

**Initial Features:**

- Syntax highlighting for multiple popular programming languages and config file types.
- When hitting enter to go to next line in code it will place the cursor at the properly spaced position for that next code line.
- Possible auto format of code to fix any spacing issues.
- Rich text editing features for text files. Things like fonts, font sizes, bold, underlining, italics, etc.…(all major text editing options) / These will appear on a bar bellow the file and tabs menu bar when on a normal text document file.
- Printer support for the user to print out their files.
- Full markdown file support with a preview mode for the rendered file.
- Initial Tools - Find, Find and Replace, Word Count
- Initial Settings - Color Themes, Tab spacing
- Ability to use installed coding language servers.
- Can open and save programs directly in the program from the file menu or from feeding it from the command line like “rust-edit source.rs”
- Can be navigated with either the mouse or keyboard. Each feature / menu / tool should have corresponding keyboard shortcuts for navigation.