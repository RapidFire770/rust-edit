# Fixes & Bugs:

### Minor:
- [ ] Add an X button to the file tabs to click on and close the tabs as well if possible to fit in with the current ui design and look.

### Major:
- [ ] Need a check/warning for files that the user doesn't have permission to save to without opening using sudo. Right now it appears to just fail to save silently without any popup message or other kind of warning, just the edit dot remains on the file tab. The program also does not appear to work when trying to open with the sudo command, I just get the error "sudo: rust-edit: command not found"

---

# Features:
- [x] Create an installer script for the program and use the logo.svg for the program to display in the OS application menu, when opened from the application menu instead of the terminal it should open in the users default terminal. If the program has already been installed before it should check the version and ask the user if they want to update if the version is newer than the currently installed version.
- [ ] Spellcheck for normal text files and markdown
- [ ] ctrl+/ to comment out lines in support programming languages
- [ ] Tools for code files: Function List and Variables List - when in a code file you can open these side panels on the left or right side of the window to show defined functions or variables in that file and select them to quickly jump to that function in the editor window.
- [ ] See if more config file types and coding language support can be added to the syntect crate we are using. It uses sublime text definitions.
- [ ] Possibly have it work on Mac and Windows as well.
