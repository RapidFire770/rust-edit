# Fixes & Bugs:

### Minor:

- [x] When a whole line of text gets pasted at one time by the user and they do ctrl+z it should erase the whole pasted line instead of one letter at a time. This appears to be an issue when it is an item pasted from the Operating systems clipboard instead of something copied and pasted from within the rust-edit program.
- [x] Cut, copy and paste shortcuts are not using the Operating systems clipboard correctly. Nothing copied or cut appears in the operating systems clipboard for use outside of the rust-edit program.
- [x] Add better descriptions to the help.rs in the "View & Tools" section for Format Document, Function List and Variables List. Current descriptions do not properly describe to the user what they do or open.
- [x] Esc unfocuses from the Function or Variable list sidebars but there doesn't seem to be a way to refocus on either of the sidebars with the keyboard once you leave focus on them. This leaves the mouse as the only option for using the sidebars which is against our Keyboard and Mouse being first class citizens with every action having to be reachable both ways.
- [ ] Project Browser Sidebar has an issue with the scrollbar. When several folders are open with many files the scrollbar stops allowing scrolling to the bottom of the file list. If you do manage to get it to scroll to the bottom of the list by opening and closing several folders it will get stuck not allowing scrolling back to the top of the list. This bug appears to be happening in the variables list and the function list as well. Scrolling appears to work fine when using the keyboard arrows to move up and down the list but not when using the mouse scroll wheel.

### Major:

- [ ] None

---

# Features:

- [x] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window. They cannot be open at the same time unless they are both on separate sides of the window.
- [x] A project file browser sidebar that can be opened on left or right side for the user to quickly open more files that are within the projects directory. This will require a new option in the file menu to open a project folder. Will need the ability to hide and reopen the sidebar without having to reselect the folder each time you open the sidebar. Cannot overlap an open function list or variable list sidebar. Should remember recent project folders like how we remember recently opened files in the file browser popup so the user can quickly select and reopen a project folder.
- [x] Add an edit menu option to the top bar between file and tools. It should have options for undo, redo, cut, copy, paste, select all.
- [ ] Make side panels re-sizable with a keyboard shortcut when in focus. Also add a keyboard shortcut to close them when they are in focus.
- [ ] Add BASIC and Assembly as a supported programming language so projects in my ~/Documents/Programming/C64 can be opened and highlighted, commented, etc.
- [ ] Possibly have it work on Mac and Windows as well.
