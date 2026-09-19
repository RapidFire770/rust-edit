# Fixes & Bugs:

### Minor:

- [x] Edit / new theme not working. Cannot select any colors to change or type color values.
  - [x] The theme editor should have some kind of graphical color picker or at minimum a list of premade colors (red, green, blue, yellow, etc.) instead of just a value depending on the capabitlities of Rataui and our current tech stack.
- [x] Highlight and clicking on custom themes is slightly off. The mouse has to be above the theme the user is trying to select for it to be highlighted and clickable. Default themes highlight and click correctly.
- [ ] Add an X button to the file tabs to click on and close the tabs as well if possible to fit in with the current ui design and look.

### Major:
- [x] The format document option in the tools menu does not have any function. This should be implemented to format the document according to the user's preferences or use a coding language's formatter if available (e.g. rustfmt).


---

# Features:
- [ ] Create an installer script for the program and use the logo.svg for the program.
- [x] If a file is modified outside of the editor while it is currently open the program should notice and ask the user if they would like to reload the file to see the new changes. Auto reload of the file without asking the user each time should be a selectable option when the program asks the user so it doesn't have to keep asking them each time the file is modified.
- [ ] Spellcheck for normal text files and markdown
- [ ] ctrl+/ to comment out lines in support programming languages
- [ ] Create an installer script for the program.
- [ ] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config file types and coding language support can be added to the syntect crate we are using. It uses sublime text definitions.
- [ ] Possibly have it work on Mac and Windows as well.
