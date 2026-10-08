# Fixes & Bugs:

### Minor:

- [x] When a whole line of text gets pasted at one time by the user and they do ctrl+z it should erase the whole pasted line instead of one letter at a time. This appears to be an issue when it is an item pasted from the Operating systems clipboard instead of something copied and pasted from within the rust-edit program.
- [x] Cut, copy and paste shortcuts are not using the Operating systems clipboard correctly. Nothing copied or cut appears in the operating systems clipboard for use outside of the rust-edit program.

### Major:

- [ ] None

---

# Features:

- [ ] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window. They cannot be open at the same time unless they are both on separate sides of the window. 
- [ ] A project file browser sidebar that can be opened on left or right side for the user to quickly open more files that are within the projects directory. This will require a new option in the file menu to open a project folder. Cannot overlap an open function list or variable list sidebar.
- [x] Add an edit menu option to the top bar between file and tools. It should have options for undo, redo, cut, copy, paste, select all.
- [ ] Possibly have it work on Mac and Windows as well.