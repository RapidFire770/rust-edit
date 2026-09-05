# Fixes & Bugs:

### Minor:

- [ ] Edit / new theme not working. Cannot select any colors to change or type color values.
  - The theme editor should have some kind of graphical color picker or at minimum a list of premade colors (red, green, blue, yellow, etc.) instead of just a value depending on the capabitlities of Rataui and our current tech stack.
- [ ] Now that the line numbers are toggling on and off there is a large gap on the side of the screen where the line numbers show. The text should move to fill this gap when line numbers are not being displayed.


### Major:


---

# Features:

- [ ] If there are to many tabs open that span wider than the terminal window they push off the screen and there is no way to scroll the open tabs with the mouse or click on the + button anymore. We need some arrows ( [<] / [>] ) to appear and let the tabs be scrolled.
- [x] When word wrap isn't on and lines are longer than the visible width of the window a horizontal scrollbar should be displayed along the bottom of the window to allow the user to scroll the line horizontally.
- [ ] If a file is modified outside of the editor while it is currently open the program should notice and ask the user if they would like to reload the file to see the new changes. Auto reload of the file without asking the user each time should be a selectable option when the program asks the user so it doesn’t have to keep asking them each time the file is modified.
- [ ] Spellcheck for normal text files and markdown
- [ ] ctrl+/ to comment out lines in support programming languages
- [ ] Create an installer script for the program.
- [ ] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config file types and coding language support can be added to the syntect crate we are using. It uses sublime text definitions.
- [ ] Possibly have it work on Mac and Windows as well.
