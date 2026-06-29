### Fixes & Bugs:
- [ ] Smart indentation does not function as intended. Right now when hitting enter to move down to a new line the cusor can jump down multiple lines at once or push the next line down while keeping the fist letter of the text that was on that line in place while pushing the rest of the text down where it should be.
- [x] Scroll bar does not match the size of the content in the editor.

---

### Features:

- [x] Ctrl+A to select all text.
- [x] Clicking and dragging to select text and getting to the bottom or top edge of the editor window should start to scroll the view the same as it does when using shift+down or shift+up to select text with the keyboard.
- [ ] Spellcheck for normal text files and markdown
- [ ] Implement a word wrap feature for lines that are longer than the visible width of the editor screen. Word wrap on or off with option in the tools menu and a keyboard shortcut. Word wrapped lines should not have a line number or count as a line in the line counter.
- [ ] Fully implemented settings with themes, tab spacing, help and other future configurable options. Themes will be its own separate popup window from the other settings popup
- [ ] Open file and Save As should have the option to allow you to open a file browser popup to browse the file system and choose a file to open or place to the save file.
- [ ] Create an installer script for the program.
- [ ] Implement more color theme presets and an option for the user to make custom color themes. (Dark, Light, High Contrast, Solarized-inspired, Monochrome, Terminal Green)
- [ ] Tool: function list, when in a code file you can open this side panel to show defined functions in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config and coding languages support can be added to the syntect crate we are using. It uses sublime
