# Fixes & Bugs:

### Minor:

- [x] When a whole line of text gets pasted at one time by the user and they do ctrl+z it should erase the whole pasted line instead of one letter at a time. This appears to be an issue when it is an item pasted from the Operating systems clipboard instead of something copied and pasted from within the rust-edit program.
- [x] Cut, copy and paste shortcuts are not using the Operating systems clipboard correctly. Nothing copied or cut appears in the operating systems clipboard for use outside of the rust-edit program.
- [x] Add better descriptions to the help.rs in the "View & Tools" section for Format Document, Function List and Variables List. Current descriptions do not properly describe to the user what they do or open.
- [x] Esc unfocuses from the Function or Variable list sidebars but there doesn't seem to be a way to refocus on either of the sidebars with the keyboard once you leave focus on them. This leaves the mouse as the only option for using the sidebars which is against our Keyboard and Mouse being first class citizens with every action having to be reachable both ways.
- [x] Project Browser Sidebar has an issue with the scrollbar. When several folders are open with many files the scrollbar stops allowing scrolling to the bottom of the file list. If you do manage to get it to scroll to the bottom of the list by opening and closing several folders it will get stuck not allowing scrolling back to the top of the list. This bug appears to be happening in the variables list and the function list as well. Scrolling appears to work fine when using the keyboard arrows to move up and down the list but not when using the mouse scroll wheel.
- [x] The buttons in the open project folder popup do not highlight when the mouse is over them as it does in our other popup windows.
- [x] Clicking the [x] on a Recent row in the open project folder popup removes the keyboard-selected row instead of the row that was clicked.
- [x] The text cursor does not appear in many of the popup text entry fields making it hard to tell if the text field is selected and able to be typed into.

### Major:

- [ ] None

---

# Features:

- [x] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window. They cannot be open at the same time unless they are both on separate sides of the window.
- [x] A project file browser sidebar that can be opened on left or right side for the user to quickly open more files that are within the projects directory. This will require a new option in the file menu to open a project folder. Will need the ability to hide and reopen the sidebar without having to reselect the folder each time you open the sidebar. Cannot overlap an open function list or variable list sidebar. Should remember recent project folders like how we remember recently opened files in the file browser popup so the user can quickly select and reopen a project folder.
- [x] Add an edit menu option to the top bar between file and tools. It should have options for undo, redo, cut, copy, paste, select all.
- [x] Make side panels re-sizable with a keyboard shortcut when in focus. Also add a keyboard shortcut to close them when they are in focus.
- [x] Add BASIC and Assembly as a supported programming language so projects in my ~/Documents/Programming/C64 can be opened and highlighted, commented, etc.
- [ ] Possibly have it work on Mac and Windows as well.
- [ ] Implement a compatibility mode for the Idun cartridge’s VT-100 80-column terminal for the C64.
    How the Idun VT-100 terminal works:
    The Idun cartridge ([github.com/idun-project/idun-cartridge](https://github.com/idun-project/idun-cartridge)) gives the Commodore a full VT-100 terminal into the Raspberry Pi’s Linux environment.

    - On the C128 this uses the hardware VDC (proper 80-column display).
    - On the C64 (or C128 in 64 mode) it uses a software 80-column mode (bitmap-based). It’s usable but slower, with limited colors/attributes and jump scrolling.
    You can already run normal Linux tools (including vi) from the Commodore keyboard via this terminal. The default is typically 80×25 or 80×27.

    What a compatibility mode would need:
    rust-edit is a full Ratatui + crossterm TUI that currently expects modern terminal features (mouse, true color, UTF-8, rich menus). For Idun you’d want a reduced mode that:

    Detects the environment
    - TERM=vt100 (or similar)
    - Terminal size around 80×25/27
    - Optional explicit flag (--idun / --compat) or config setting
    - Optionally check hostname (idunpi) or other Idun-specific signals

    Simplifies the UI
    - Switch to a monochrome or basic ANSI theme (you already have a monochrome theme and others). Disable truecolor.
    - Completely disable mouse handling — the Commodore keyboard terminal almost certainly has none.
    - Keep menus keyboard-navigable (arrow keys + Enter, or single-key mnemonics). Shorten labels if needed.
    - Stick to ASCII or the VT-100 alternate character set for line drawing.

    Adjusts keyboard handling
    - Commodore keyboards have a different layout and limited modifier support.
    - Provide a simplified shortcut set (more Ctrl-based, fewer Alt/function-key combos).
    - Document the expected mappings once you test on real hardware.
    
    Tones down features
    - Basic syntax highlighting only (or off).
    - Simpler status bar / fewer panels.
    - Minimize full redraws if the software 80-col mode on C64 feels laggy.
