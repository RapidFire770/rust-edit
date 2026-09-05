# Fixes & Bugs:

### Minor:

- [ ] Edit / new theme not working. Cannot select any colors to change or type color values.
  - The theme editor should have some kind of graphical color picker or at minimum a list of premade colors (red, green, blue, yellow, etc.) instead of just a value.
- [x] In the file browser window when the mouse is outside of the popup window it still scrolls the files list.
- [x] A Scroll
- [x] Save As and Open file popups file paths are crossing over with each other on reopening the popups. These file paths should not cross from one popup to another and I would expect that re-opening these popups that the file path would have been empty for the user to select a new file to open or save.bar should display in the popups to indicate to the user that the content is scrollable when it overflows the popup window. Currently noticable as an issue in help and file browsing popups.
- [x] Cannot click and drag the scrollbar with the mouse in the file browser popups.
- [ ] Save as and Open file popups file paths are crossing over with each other on reopening the popups. These file paths should not cross from one popup to another and I would expect that re-opening these popups that the file path would have been empty for the user to select a new file to open or save.
- [ ] In the open popups browse files feature, the option to view recent files is not clear. Currently is initially says Browse with no clear indication that clicking on this will change it to show recently opened files. Possibly change this to show [ Browse / Recent ] bolding the text of which view the user is on or some other method that makes it more clear to the user this feature exists and can be used.

### Major:

- [x] In the Save As popup when using the browser function there is no way to select the folder/directory to save the file into. It appears to be an exact copy of the browse function from the open dialog that is not adapted to function for a save as feature making it impossible to select a directory to save a file into.
- [x] When hitting the tab button it does not work as expected of a text and code editor. When hitting tab anywhere on a line of text it inputs the tab at the beginning of the line pushing the line over 4 spaces like an indent instead of moving the text at the cursor over the tab amount. If the cursor is at the end of the line, instead of moving the cursor over the tab amount it moves the whole line over 4 spaces. Also the configured tab amount in the configure menu does not change the tab amount input in the editor window.
- [x] Issue with editor scroll jumping and possible cursor positioning when clicking with mouse. When clicking to position the cursor after having scrolled about half way down a larger document like our AGENTS.md the editor scrolls the screen to have the spot clicked on by the mouse as the very last visible line on the screen. The page should stay put and the cursor should just be positioned where the user clicked so they can edit from that point like in other text and code editors. Also when at bottom of the document the cursor can show as being on the bottom information bar instead of in the editor window when the page scrolls itself.
- [x] Terminal resize with word wrap on panicked (`highlight cache must cover visible line N`). Viewport size is now applied and wrap scroll clamped inside `ui()` before the highlight cache is filled.

---

# Features:

- [ ] If there are to many tabs open that span wider than the terminal window they push off the screen and there is no way to scroll the open tabs with the mouse or click on the + button anymore. We need some arrows ( [<] / [>] ) to appear and let the tabs be scrolled.
- [ ] When word wrap isn't on and lines are longer than the visible width of the window a horizontal scrollbar should be displayed along the bottom of the window to allow the user to scroll the line horizontally.
- [ ] If a file is modified outside of the editor while it is currently open the program should notice and ask the user if they would like to reload the file to see the new changes. Auto reload of the file without asking the user each time should be a selectable option when the program asks the user so it doesn’t have to keep asking them each time the file is modified.
- [ ] Spellcheck for normal text files and markdown
- [ ] ctrl+/ to comment out lines in support programming languages
- [ ] Create an installer script for the program.
- [ ] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config file types and coding language support can be added to the syntect crate we are using. It uses sublime text definitions.
- [ ] Possibly have it work on Mac and Windows as well.
