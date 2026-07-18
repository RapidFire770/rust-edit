### Fixes & Bugs:
- [ ] Themes popup is to small, bottom row buttons are cut off.
- [ ] Themes popup buttons only highlight when mouse is below the button.
- [ ] Configure popup tab size can be increased with the mouse but not decreased.
- [ ] Edit / new theme not working. Cannot select any colors to change or type color values.
- [ ] Browser Window when mouse is outside the popup it still scrolls the files list.

---

### Features:

- [ ] Spellcheck for normal text files and markdown
- [ ] ctrl+/ to comment lines in support programming languages
- [x] Fully implemented settings with themes, tab spacing, help and other future configurable options. Themes will be its own separate popup window from the other settings. Configurable options will be its own option in the menu and the popup will be called "Configure" it will house the default settings like tab spacing, word wrap on or off and other configurable options that the user can change from within this popup.
- [ ] Create an installer script for the program.
- [x] Implement more color theme presets and an option for the user to make custom color themes. (Dark, Light, High Contrast, Solarized-inspired, Monochrome, Terminal Green)
- [ ] Tool: function list, when in a code file you can open this side panel to show defined functions in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config and coding languages support can be added to the syntect crate we are using. It uses sublime
