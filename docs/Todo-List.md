# Fixes & Bugs:

### Minor:
- [ ] Add an X button to the file tabs to click on and close the tabs as well if possible to fit in with the current ui design and look.
- [x] In the recent files section of the open popups file browser the files list should give a indication of the file path that file comes from so that files with the same name but from different paths cannot be confused with each other. Example: "FilenameHere.md @ /home/user/Documents" truncate the beginning of the file path if it is too long to fit in the available window space for the file browser popup.
- [x] Need the ability to clear the recently opened files list. Both individual files and the whole list. 

### Major:
- [x] Need a check/warning for files that the user doesn't have permission to save to without opening using sudo. Right now it appears to just fail to save silently without any popup message or other kind of warning, just the edit dot remains on the file tab. The program also does not appear to work when trying to open with the sudo command, I just get the error "sudo: rust-edit: command not found"

---

# Features:
- [x] Create an installer script for the program and use the logo.svg for the program to display in the OS application menu, when opened from the application menu instead of the terminal it should open in the users default terminal. If the program has already been installed before it should check the version and ask the user if they want to update if the version is newer than the currently installed version.
- [x] Ctrl+/ (unshifted /? key) toggles line comments in supported languages and config files that have a line comment.
- [ ] Block style comments for supported languages like markdown.
- [ ] Spellcheck for normal text files and markdown
- [ ] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config file types and coding language support can be added to the syntect crate we are using. It uses sublime text definitions.
- [ ] Possibly have it work on Mac and Windows as well.
