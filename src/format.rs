//! Document formatting: language tools on PATH, else a conservative
//! editor-preference pass (tab size, trailing whitespace, final newline).

/// Conservative in-editor formatting using the user's tab size.
///
/// - Empty input stays empty (do not invent a newline on the untitled placeholder).
/// - Expand every remaining `\t` to `tab_size` spaces (`tab_size` is clamped to at least 1).
/// - Strip trailing spaces/tabs per line; drop `\r` so CRLF becomes LF.
/// - Non-empty input is given a trailing `\n` if it lacks one. Extra blank lines at EOF are kept.
#[allow(dead_code)] // Wired up by later Format Document tasks (language tools + menu).
pub fn builtin_format(text: &str, tab_size: u8) -> String {
    if text.is_empty() {
        return String::new();
    }
    let width = tab_size.max(1) as usize;
    let spaces = " ".repeat(width);
    let mut out = String::with_capacity(text.len());
    for (i, raw) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let line = raw.trim_end_matches('\r');
        let line = line.trim_end_matches([' ', '\t']);
        out.push_str(&line.replace('\t', &spaces));
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod builtin_tests {
    use super::builtin_format;

    #[test]
    fn empty_stays_empty() {
        assert_eq!(builtin_format("", 4), "");
    }

    #[test]
    fn expands_tabs_using_tab_size() {
        assert_eq!(builtin_format("a\tb\n", 4), "a    b\n");
        assert_eq!(builtin_format("\tfn\n", 2), "  fn\n");
    }

    #[test]
    fn strips_trailing_spaces_and_tabs() {
        assert_eq!(builtin_format("hello  \nworld\t\n", 4), "hello\nworld\n");
    }

    #[test]
    fn adds_trailing_newline_when_missing() {
        assert_eq!(builtin_format("hello", 4), "hello\n");
    }

    #[test]
    fn keeps_existing_trailing_newline_and_blank_lines() {
        assert_eq!(builtin_format("hello\n\n", 4), "hello\n\n");
    }

    #[test]
    fn strips_crlf_to_lf() {
        assert_eq!(builtin_format("a  \r\nb\t\r\n", 4), "a\nb\n");
    }

    #[test]
    fn already_clean_is_stable() {
        let s = "fn main() {\n    let x = 1;\n}\n";
        assert_eq!(builtin_format(s, 4), s);
    }
}
