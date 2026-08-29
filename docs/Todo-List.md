### Fixes & Bugs:
- [x] The text cursor does not blink properly, it only blinks/flashes slightly when the mouse moves around the window.
- [x] Mouse movment is slightly interupting the cursor blink causing an oddly blink/flashing effect on the cursor.
- [ ] When opening a file and the default untitled document is displayed and not modified, it should be closed/replaced on the tab bar with the opened file.
- [ ] When a popup modal is visible such as the Settings Configure popup, hitting the space bar is moving the cursor position in the background, the column indicator is updating and when leaving the popup the cursor position on screen has moved.
- [ ] Not all popup modals are hiding the cursor in the background when they are visible.
- [ ] The setttings configure popup window toggle and close buttons highlighting does not function as intended. When hovering over the close button it highlights properly and unhighlights when the mouse moves off. The toggle button stays constantly highlighted red no matter if the mouse is over it or not.
- [ ] Themes popup is to small, bottom row buttons are cut off.
- [ ] Themes popup buttons only highlight when mouse is below the button.
- [ ] Configure popup tab size can be increased with the mouse but not decreased.
- [ ] Edit / new theme not working. Cannot select any colors to change or type color values.
- [ ] Browser Window when mouse is outside the popup it still scrolls the files list.
- [ ] Scrollbar to display in the help menu to indicate to the user that the content is scrollable.

---

### Features:

- [ ] When word wrap isn't on and a line is longer than the visible width of the window a horizontal scrollbar should be displayed along the bottom of the window to allow the user to scroll the line horizontally.
- [ ] Spellcheck for normal text files and markdown
- [ ] ctrl+/ to comment lines in support programming languages
- [x] Fully implemented settings with themes, tab spacing, help and other future configurable options. Themes will be its own separate popup window from the other settings. Configurable options will be its own option in the menu and the popup will be called "Configure" it will house the default settings like tab spacing, word wrap on or off and other configurable options that the user can change from within this popup.
- [ ] Create an installer script for the program.
- [x] Implement more color theme presets and an option for the user to make custom color themes. (Dark, Light, High Contrast, Solarized-inspired, Monochrome, Terminal Green)
- [ ] Tool: function list, when in a code file you can open this side panel to show defined functions in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config and coding languages support can be added to the syntect crate we are using. It uses sublime
