### Fixes & Bugs:

- [x] For find and replace there is an issue with how it finds words. For example the word "Test" should not be equal to "Testing" but when finding or replacing the word "Test" with something like "Hello" the word "Testing" would become "Helloing". Capitalization of words is also not taken into account for words, this should probably be a user toggle. If the replace box is blank/empty and the user hits enter it should not delete/remove/replace the word with a blank/empty space.
- [x] When the file name is to long it pushes the other information on the bottom bar off the screen depending on the current window size. The file name should be shortened/partially hidden if it to long so that the other information stays visible on the screen.
- [x] In the setting’s configure popup window toggle and close buttons highlighting does not function as intended. When hovering over the close button it highlights properly and un-highlights when the mouse moves off. The toggle button stays constantly highlighted red no matter if the mouse is over it or not.
- [x] Configure popup tab size option can be increased with the mouse but not decreased.
- [ ] Edit / new theme not working. Cannot select any colors to change or type color values.
    - The theme editor should have some kind of graphical color picker or at minimum a list of premade colors (red, green, blue, yellow, etc.) instead of just a value.
- [ ] In the file browser window when mouse is outside the popup it still scrolls the files list.
- [ ] Scrollbar should display in the help menu to indicate to the user that the content is scrollable.
- [ ] In the Save As popup when using the browser function there is no way to select the folder/directory to save the file into.
- [ ] Scrolling long files seems to somewhat hit the CPU rather hard and has some very minor slowdown on scrolling, it needs to be optimized in some fashion. I personally believe it could be related to the syntax highlighting trying to constantly update as the window is trying to scroll because as the window scrolls the text can sometimes turns white and it tries to catchup highlighting as I am still scrolling instead of the all the text already being highlighted before I scroll up or down. Also sometimes the highlighting just disappears into plain white text depending on where I am scrolled to on the document. Possibly need to use syntect’s caching and parallelizing listed on the crates.io page if this is the cause of the issues because highlighting should be instant, not noticeable to the user that it is happening and the CPU shouldn’t be spiking up a noticable amount as the user scrolls through the document/code file.

---

### Features:

- [ ] When word wrap isn't on and lines are longer than the visible width of the window a horizontal scrollbar should be displayed along the bottom of the window to allow the user to scroll the line horizontally.
- [ ] If a file is modified outside of the editor while it is currently open the program should notice and ask the user if they would like to reload the file to see the new changes. Auto reload of the file without asking the user each time should be a selectable option when the program asks the user so it doesn’t have to keep asking them each time the file is modified.
- [ ] Spellcheck for normal text files and markdown
- [ ] ctrl+/ to comment out lines in support programming languages
- [ ] Create an installer script for the program.
- [ ] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config file types and coding language support can be added to the syntect crate we are using. It uses sublime text definitions.
- [ ] Possibly have it work on Mac and Windows as well.
