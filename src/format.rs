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

use crate::buffer::Language;
use std::path::{Path, PathBuf};

#[allow(dead_code)] // Wired up by later Format Document tasks (language tools + menu).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: &'static str,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[allow(dead_code)]
fn dummy_filename(language: Language) -> &'static str {
    match language {
        Language::Rust => "buffer.rs",
        Language::Python => "buffer.py",
        Language::Markdown => "buffer.md",
        Language::Json => "buffer.json",
        Language::Yaml => "buffer.yaml",
        Language::Javascript => "buffer.js",
        Language::Typescript => "buffer.ts",
        Language::C => "buffer.c",
        Language::Cpp => "buffer.cpp",
        Language::Toml | Language::Bash | Language::Plain => "buffer.txt",
    }
}

#[allow(dead_code)]
fn stdin_filename(language: Language, path: Option<&Path>) -> String {
    path.map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| dummy_filename(language).to_string())
}

#[allow(dead_code)]
fn parent_cwd(path: Option<&Path>) -> Option<PathBuf> {
    path.and_then(|p| p.parent()).map(Path::to_path_buf)
}

/// Build the stdin/stdout invocation for a language tool, or `None` if this
/// language only has the builtin pass.
#[allow(dead_code)] // Wired up by later Format Document tasks (language tools + menu).
pub fn command_spec(language: Language, path: Option<&Path>) -> Option<CommandSpec> {
    let cwd = parent_cwd(path);
    match language {
        Language::Rust => {
            let mut args = vec!["--emit".into(), "stdout".into(), "--quiet".into()];
            if path.is_none() {
                args.push("--edition".into());
                args.push("2021".into());
            }
            Some(CommandSpec {
                program: "rustfmt",
                args,
                cwd,
            })
        }
        Language::Python => {
            let mut args = vec!["-q".into()];
            if let Some(p) = path {
                args.push("--stdin-filename".into());
                args.push(p.to_string_lossy().into_owned());
            }
            args.push("-".into());
            Some(CommandSpec {
                program: "black",
                args,
                cwd,
            })
        }
        Language::Javascript
        | Language::Typescript
        | Language::Json
        | Language::Yaml
        | Language::Markdown => Some(CommandSpec {
            program: "prettier",
            args: vec!["--stdin-filepath".into(), stdin_filename(language, path)],
            cwd,
        }),
        Language::C | Language::Cpp => Some(CommandSpec {
            program: "clang-format",
            args: vec![format!(
                "--assume-filename={}",
                stdin_filename(language, path)
            )],
            cwd,
        }),
        Language::Plain | Language::Toml | Language::Bash => None,
    }
}

#[cfg(test)]
mod spec_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn rust_uses_rustfmt_stdout_not_dash() {
        let spec = command_spec(Language::Rust, Some(Path::new("/proj/src/main.rs"))).unwrap();
        assert_eq!(spec.program, "rustfmt");
        assert!(spec.args.iter().any(|a| a == "--emit"));
        assert!(spec.args.iter().any(|a| a == "stdout"));
        assert!(!spec.args.iter().any(|a| a == "-"));
        assert_eq!(spec.cwd.as_deref(), Some(Path::new("/proj/src")));
    }

    #[test]
    fn untitled_rust_sets_edition_2021() {
        let spec = command_spec(Language::Rust, None).unwrap();
        assert!(spec
            .args
            .windows(2)
            .any(|w| w[0] == "--edition" && w[1] == "2021"));
        assert!(spec.cwd.is_none());
    }

    #[test]
    fn python_black_reads_stdin_dash() {
        let spec = command_spec(Language::Python, Some(Path::new("/p/app.py"))).unwrap();
        assert_eq!(spec.program, "black");
        assert!(spec.args.iter().any(|a| a == "-"));
        assert!(spec
            .args
            .windows(2)
            .any(|w| w[0] == "--stdin-filename" && w[1].ends_with("app.py")));
        assert_eq!(spec.cwd.as_deref(), Some(Path::new("/p")));
    }

    #[test]
    fn prettier_gets_stdin_filepath_for_js_and_json() {
        for (lang, dummy) in [
            (Language::Javascript, "buffer.js"),
            (Language::Typescript, "buffer.ts"),
            (Language::Json, "buffer.json"),
            (Language::Yaml, "buffer.yaml"),
            (Language::Markdown, "buffer.md"),
        ] {
            let spec = command_spec(lang, None).unwrap();
            assert_eq!(spec.program, "prettier");
            assert!(
                spec.args
                    .windows(2)
                    .any(|w| w[0] == "--stdin-filepath" && w[1] == dummy),
                "{lang:?} missing dummy filepath"
            );
        }
    }

    #[test]
    fn clang_format_for_c_family() {
        let spec = command_spec(Language::C, None).unwrap();
        assert_eq!(spec.program, "clang-format");
        assert!(spec.args.iter().any(|a| a == "--assume-filename=buffer.c"));
        let spec = command_spec(Language::Cpp, Some(Path::new("/x/foo.cpp"))).unwrap();
        assert!(spec
            .args
            .iter()
            .any(|a| a == "--assume-filename=/x/foo.cpp"));
        assert_eq!(spec.cwd.as_deref(), Some(Path::new("/x")));
    }

    #[test]
    fn plain_toml_bash_have_no_external_tool() {
        assert!(command_spec(Language::Plain, None).is_none());
        assert!(command_spec(Language::Toml, None).is_none());
        assert!(command_spec(Language::Bash, None).is_none());
    }
}
