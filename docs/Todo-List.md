# Fixes & Bugs:

### Minor:

- [x] Find and Find and replace in the tools menu does not need to be shown as separate items since they both open the same function/tool.
- [x] Tab button should also cycle through the open/save, browse, cancel options in the open and save as popups like you can with the arrow keys.
- [x] When a file, tool, or settings drop down is open you should be able to use the left and right arrow keys to move between the drop down menus or cycle through them with the tab button.
- [x] In the themes menus edit theme section we need a small scroll bar or other indicator for the list of items/roles that the user can select and modify the colours of to show that the list is scrollable. Also the preview of the color edits on the right is missing a lot of items, the user can only see what a change may look like after they hit save and leave the edit screen. This makes editing a theme a bit annoying since you have to go in and out of the edit theme popup many times to get the look your trying to achieve, if we can find a solution to display more of the ui elements in the edit preview it would be extremely helpful.

### Major:

- [ ] None

---

# Features:

- [ ] See if more config file types (e.g. ini, json, jsonc, xml, etc.) and language support (e.g. zig, java, HTML, CSS, C#, SQL, PHP, Perl, Go, Ruby) can be added to the syntect crate we are using. It uses sublime text syntax definitions according to the crate documentation. Make sure format document function works for supported languages and the comment types are supported with our comment shortcut key and menu toggle.
- [ ] Spellcheck for normal text files and markdown
- [ ] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window.
- [ ] Possibly have it work on Mac and Windows as well.
