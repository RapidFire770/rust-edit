//! Document formatting: language tools on PATH, else a conservative
//! editor-preference pass (tab size, trailing whitespace, final newline).

use crate::buffer::Language;
use std::path::{Path, PathBuf};

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

#[allow(dead_code)] // Wired up by later Format Document tasks (App menu + SystemRunner).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatVia {
    External(&'static str),
    Builtin,
}

impl FormatVia {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            FormatVia::External(name) => name,
            FormatVia::Builtin => "editor preferences",
        }
    }
}

#[allow(dead_code)] // Wired up by later Format Document tasks (App menu + SystemRunner).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatOutcome {
    Changed { text: String, via: FormatVia },
    Unchanged { via: FormatVia },
    Failed { message: String },
}

#[allow(dead_code)] // Wired up by later Format Document tasks (App menu + SystemRunner).
pub struct FormatRequest<'a> {
    pub language: Language,
    pub path: Option<&'a Path>,
    pub text: &'a str,
    pub tab_size: u8,
}

#[allow(dead_code)] // Wired up by later Format Document tasks (App menu + SystemRunner).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status_ok: bool,
    pub stdout: String,
    pub stderr: String,
}

#[allow(dead_code)] // Wired up by later Format Document tasks (App menu + SystemRunner).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    NotFound,
    TimedOut,
    Io(String),
}

#[allow(dead_code)] // Wired up by later Format Document tasks (App menu + SystemRunner).
pub trait CommandRunner {
    fn run(&self, spec: &CommandSpec, stdin: &str) -> Result<CommandOutput, RunError>;
}

fn builtin_outcome(text: &str, tab_size: u8) -> FormatOutcome {
    let formatted = builtin_format(text, tab_size);
    if formatted == text {
        FormatOutcome::Unchanged {
            via: FormatVia::Builtin,
        }
    } else {
        FormatOutcome::Changed {
            text: formatted,
            via: FormatVia::Builtin,
        }
    }
}

fn format_error_message(program: &str, stderr: &str, fallback: &str) -> String {
    let body = if stderr.trim().is_empty() {
        fallback.to_string()
    } else {
        let mut s = stderr.trim().to_string();
        const CAP: usize = 8 * 1024;
        if s.len() > CAP {
            let mut cap = CAP;
            while cap > 0 && !s.is_char_boundary(cap) {
                cap -= 1;
            }
            s.truncate(cap);
            s.push_str("\n…");
        }
        s
    };
    format!("{program}: {body}")
}

#[allow(dead_code)] // Wired up by later Format Document tasks (App menu + SystemRunner).
pub fn format_document(req: FormatRequest<'_>, runner: &dyn CommandRunner) -> FormatOutcome {
    let Some(spec) = command_spec(req.language, req.path) else {
        return builtin_outcome(req.text, req.tab_size);
    };

    match runner.run(&spec, req.text) {
        Err(RunError::NotFound) => builtin_outcome(req.text, req.tab_size),
        Err(RunError::TimedOut) => FormatOutcome::Failed {
            message: format!("{} timed out after 10s", spec.program),
        },
        Err(RunError::Io(e)) => FormatOutcome::Failed {
            message: format_error_message(spec.program, "", &e),
        },
        Ok(out) if out.status_ok => {
            if out.stdout.is_empty() && !req.text.is_empty() {
                FormatOutcome::Failed {
                    message: format_error_message(spec.program, &out.stderr, "produced no output"),
                }
            } else if out.stdout == req.text {
                FormatOutcome::Unchanged {
                    via: FormatVia::External(spec.program),
                }
            } else {
                FormatOutcome::Changed {
                    text: out.stdout,
                    via: FormatVia::External(spec.program),
                }
            }
        }
        Ok(out) => FormatOutcome::Failed {
            message: format_error_message(spec.program, &out.stderr, "failed"),
        },
    }
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

#[cfg(test)]
mod orchestrate_tests {
    use super::*;
    use std::cell::RefCell;

    struct MockRunner {
        result: Result<CommandOutput, RunError>,
        last_program: RefCell<Option<String>>,
    }

    impl CommandRunner for MockRunner {
        fn run(&self, spec: &CommandSpec, _stdin: &str) -> Result<CommandOutput, RunError> {
            *self.last_program.borrow_mut() = Some(spec.program.to_string());
            self.result.clone()
        }
    }

    fn rust_req(text: &str) -> FormatRequest<'_> {
        FormatRequest {
            language: Language::Rust,
            path: None,
            text,
            tab_size: 4,
        }
    }

    #[test]
    fn missing_tool_falls_back_to_builtin() {
        let runner = MockRunner {
            result: Err(RunError::NotFound),
            last_program: RefCell::new(None),
        };
        let out = format_document(rust_req("fn main(){  }\t\n"), &runner);
        match out {
            FormatOutcome::Changed { text, via } => {
                assert_eq!(via, FormatVia::Builtin);
                assert_eq!(text, builtin_format("fn main(){  }\t\n", 4));
            }
            other => panic!("expected Changed, got {other:?}"),
        }
    }

    #[test]
    fn successful_tool_output_is_applied() {
        let formatted = "fn main() {}\n";
        let runner = MockRunner {
            result: Ok(CommandOutput {
                status_ok: true,
                stdout: formatted.into(),
                stderr: String::new(),
            }),
            last_program: RefCell::new(None),
        };
        let out = format_document(rust_req("fn main(){}\n"), &runner);
        assert_eq!(
            out,
            FormatOutcome::Changed {
                text: formatted.into(),
                via: FormatVia::External("rustfmt"),
            }
        );
    }

    #[test]
    fn identical_tool_output_is_unchanged() {
        let src = "fn main() {}\n";
        let runner = MockRunner {
            result: Ok(CommandOutput {
                status_ok: true,
                stdout: src.into(),
                stderr: String::new(),
            }),
            last_program: RefCell::new(None),
        };
        assert_eq!(
            format_document(rust_req(src), &runner),
            FormatOutcome::Unchanged {
                via: FormatVia::External("rustfmt"),
            }
        );
    }

    #[test]
    fn tool_failure_does_not_apply_builtin() {
        let runner = MockRunner {
            result: Ok(CommandOutput {
                status_ok: false,
                stdout: String::new(),
                stderr: "error: this file doesn't compile".into(),
            }),
            last_program: RefCell::new(None),
        };
        match format_document(rust_req("fn main(){"), &runner) {
            FormatOutcome::Failed { message } => {
                assert!(message.contains("doesn't compile"), "{message}");
                assert!(message.contains("rustfmt"), "{message}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn empty_stdout_on_non_empty_input_is_failure() {
        let runner = MockRunner {
            result: Ok(CommandOutput {
                status_ok: true,
                stdout: String::new(),
                stderr: String::new(),
            }),
            last_program: RefCell::new(None),
        };
        assert!(matches!(
            format_document(rust_req("fn main() {}\n"), &runner),
            FormatOutcome::Failed { .. }
        ));
    }

    #[test]
    fn empty_stdout_on_newline_only_input_is_failure() {
        let runner = MockRunner {
            result: Ok(CommandOutput {
                status_ok: true,
                stdout: String::new(),
                stderr: String::new(),
            }),
            last_program: RefCell::new(None),
        };
        assert!(matches!(
            format_document(rust_req("\n"), &runner),
            FormatOutcome::Failed { .. }
        ));
    }

    #[test]
    fn whitespace_only_stdout_is_applied() {
        let runner = MockRunner {
            result: Ok(CommandOutput {
                status_ok: true,
                stdout: "\n".into(),
                stderr: String::new(),
            }),
            last_program: RefCell::new(None),
        };
        assert_eq!(
            format_document(rust_req("fn main() {}\n"), &runner),
            FormatOutcome::Changed {
                text: "\n".into(),
                via: FormatVia::External("rustfmt"),
            }
        );
    }

    #[test]
    fn timeout_is_failure_not_fallback() {
        let runner = MockRunner {
            result: Err(RunError::TimedOut),
            last_program: RefCell::new(None),
        };
        match format_document(rust_req("fn main() {}\n"), &runner) {
            FormatOutcome::Failed { message } => {
                assert!(message.to_lowercase().contains("timed out"))
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn plain_text_never_spawns_and_uses_builtin() {
        struct PanicRunner;
        impl CommandRunner for PanicRunner {
            fn run(&self, _spec: &CommandSpec, _stdin: &str) -> Result<CommandOutput, RunError> {
                panic!("plain text must not spawn a formatter");
            }
        }
        let req = FormatRequest {
            language: Language::Plain,
            path: None,
            text: "hello  \n",
            tab_size: 4,
        };
        match format_document(req, &PanicRunner) {
            FormatOutcome::Changed { text, via } => {
                assert_eq!(via, FormatVia::Builtin);
                assert_eq!(text, "hello\n");
            }
            other => panic!("{other:?}"),
        }
    }
}
