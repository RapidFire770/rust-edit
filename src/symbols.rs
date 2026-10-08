// Named functions and variables in the active buffer.
// The lists jump to `byte`, which is the start of the name.

use ropey::Rope;

use crate::buffer::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolCache {
    pub id: u64,
    pub generation: u64,
    pub language: Language,
    pub symbols: Vec<Symbol>,
}

/// Languages with a scanner. Others get "No symbols for this file".
pub fn supports(language: Language) -> bool {
    matches!(language, Language::Rust)
}

/// Definitions in source order. `byte` is the first byte of the name.
pub fn collect(language: Language, rope: &Rope) -> Vec<Symbol> {
    match language {
        Language::Rust => collect_rust(rope),
        _ => Vec::new(),
    }
}

#[derive(Clone)]
enum RustState {
    Code,
    LineComment,
    Block(u32),
    String,
    Raw { hashes: usize },
}

struct RustScan {
    state: RustState,
    depth: usize,
    impls: Vec<(usize, String)>,
    pending_impl: Option<String>,
}

fn collect_rust(rope: &Rope) -> Vec<Symbol> {
    let mut scan = RustScan {
        state: RustState::Code,
        depth: 0,
        impls: Vec::new(),
        pending_impl: None,
    };
    let mut symbols = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        let line_byte = rope.line_to_byte(line_idx);
        feed_rust_line(
            &line.to_string(),
            line_byte,
            line_idx,
            &mut scan,
            &mut symbols,
        );
    }
    symbols
}

fn feed_rust_line(
    line: &str,
    line_byte: usize,
    line_idx: usize,
    scan: &mut RustScan,
    out: &mut Vec<Symbol>,
) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match scan.state.clone() {
            RustState::LineComment => {
                if bytes[i] == b'\n' {
                    scan.state = RustState::Code;
                }
                i += 1;
            }
            RustState::Block(depth) => {
                if bytes[i] == b'/' && peek(bytes, i + 1) == Some(b'*') {
                    scan.state = RustState::Block(depth.saturating_add(1));
                    i += 2;
                } else if bytes[i] == b'*' && peek(bytes, i + 1) == Some(b'/') {
                    i += 2;
                    scan.state = if depth <= 1 {
                        RustState::Code
                    } else {
                        RustState::Block(depth - 1)
                    };
                } else {
                    i += 1;
                }
            }
            RustState::String => {
                if bytes[i] == b'\\' {
                    i = skip_char_at(line, i + 1);
                } else if bytes[i] == b'"' {
                    scan.state = RustState::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            RustState::Raw { hashes } => {
                if bytes[i] == b'"' && has_hashes(&bytes[i + 1..], hashes) {
                    scan.state = RustState::Code;
                    i += 1 + hashes;
                } else {
                    i += 1;
                }
            }
            RustState::Code => {
                if bytes[i] == b'/' && peek(bytes, i + 1) == Some(b'/') {
                    scan.state = RustState::LineComment;
                    i += 2;
                    continue;
                }
                if bytes[i] == b'/' && peek(bytes, i + 1) == Some(b'*') {
                    scan.state = RustState::Block(1);
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\'' {
                    i += skip_tick(&line[i..]);
                    continue;
                }
                if let Some((state, consumed)) = string_open(&line[i..]) {
                    scan.state = state;
                    i += consumed;
                    continue;
                }
                if bytes[i] == b'{' {
                    scan.depth += 1;
                    if let Some(header) = scan.pending_impl.take() {
                        if let Some(name) = impl_type_name(&header) {
                            scan.impls.push((scan.depth, name));
                        }
                    }
                    i += 1;
                    continue;
                }
                if bytes[i] == b'}' {
                    if scan.depth > 0 {
                        scan.depth -= 1;
                    }
                    scan.impls.retain(|(depth, _)| *depth <= scan.depth);
                    i += 1;
                    continue;
                }
                if is_ident_start(bytes[i]) {
                    let start = i;
                    i = skip_ident(line, i);
                    let word = &line[start..i];
                    if line[i..].starts_with('#') && word == "r" {
                        let after = skip_ident(line, i + 1);
                        i = after;
                        continue;
                    }
                    if scan.pending_impl.is_some() {
                        if let Some(header) = scan.pending_impl.as_mut() {
                            if !header.is_empty() && !header.ends_with(|c: char| c.is_whitespace())
                            {
                                header.push(' ');
                            }
                            header.push_str(word);
                        }
                        continue;
                    }
                    match word {
                        "fn" => take_fn(line, line_byte, line_idx, &mut i, scan, out),
                        "let" => take_let(line, line_byte, line_idx, &mut i, out),
                        "const" | "static" => take_const(line, line_byte, line_idx, &mut i, out),
                        "impl" => scan.pending_impl = Some(String::new()),
                        _ => {}
                    }
                    continue;
                }
                if let Some(header) = scan.pending_impl.as_mut() {
                    let ch = line[i..].chars().next().unwrap_or(' ');
                    if ch == '\n' || ch.is_whitespace() {
                        if !header.ends_with(|c: char| c.is_whitespace()) {
                            header.push(' ');
                        }
                    } else {
                        header.push(ch);
                    }
                }
                i = skip_char_at(line, i);
            }
        }
    }
}

fn take_fn(
    line: &str,
    line_byte: usize,
    line_idx: usize,
    i: &mut usize,
    scan: &RustScan,
    out: &mut Vec<Symbol>,
) {
    let rest = line[*i..].trim_start();
    let skipped = line[*i..].len() - rest.len();
    if !rest.starts_with(|c: char| is_ident_start_char(c)) {
        return;
    }
    let name_off = *i + skipped;
    let end = skip_ident(line, name_off);
    let name = &line[name_off..end];
    if name == "fn" {
        return;
    }
    let display = match method_owner(scan) {
        Some(owner) => format!("{owner}::{name}"),
        None => name.to_string(),
    };
    out.push(Symbol {
        name: display,
        kind: SymbolKind::Function,
        line: line_idx,
        byte: line_byte + name_off,
    });
    *i = end;
}

fn take_let(line: &str, line_byte: usize, line_idx: usize, i: &mut usize, out: &mut Vec<Symbol>) {
    let mut j = skip_ws(line, *i);
    if line[j..].starts_with("mut") && !ident_continues(line, j + 3) {
        j = skip_ws(line, j + 3);
    }
    if !line[j..].starts_with(|c: char| is_ident_start_char(c)) {
        return;
    }
    let end = skip_ident(line, j);
    let name = &line[j..end];
    let after = skip_ws(line, end);
    let next = line[after..].chars().next().unwrap_or('\n');
    if matches!(next, '(' | '{' | '<' | ':') && next == ':' {
        // `name:` is a binding. `name::` is a path and is not.
        if line[after..].starts_with("::") {
            return;
        }
    } else if !matches!(next, '=' | ';' | ',' | ')' | '\n' | '/') && next != ':' {
        return;
    }
    if line[after..].starts_with("::")
        || line[after..].starts_with('(')
        || line[after..].starts_with('{')
    {
        return;
    }
    out.push(Symbol {
        name: name.to_string(),
        kind: SymbolKind::Variable,
        line: line_idx,
        byte: line_byte + j,
    });
    *i = end;
}

fn take_const(line: &str, line_byte: usize, line_idx: usize, i: &mut usize, out: &mut Vec<Symbol>) {
    let mut j = skip_ws(line, *i);
    if line[j..].starts_with("mut") && !ident_continues(line, j + 3) {
        j = skip_ws(line, j + 3);
    }
    if line[j..].starts_with("fn") && !ident_continues(line, j + 2) {
        return;
    }
    if !line[j..].starts_with(|c: char| is_ident_start_char(c)) {
        return;
    }
    let end = skip_ident(line, j);
    let name = &line[j..end];
    out.push(Symbol {
        name: name.to_string(),
        kind: SymbolKind::Variable,
        line: line_idx,
        byte: line_byte + j,
    });
    *i = end;
}

fn method_owner(scan: &RustScan) -> Option<&str> {
    scan.impls
        .iter()
        .rev()
        .find(|(depth, _)| *depth == scan.depth)
        .map(|(_, name)| name.as_str())
}

fn impl_type_name(header: &str) -> Option<String> {
    let rest = strip_leading_angles(header.trim());
    let rest = rest.trim_start();
    let type_src = match word_after(rest, "for") {
        Some(after) => after,
        None => rest,
    };
    let type_src = type_src.trim_start();
    let type_src = type_src.strip_prefix('&').unwrap_or(type_src).trim_start();
    let type_src = if let Some(after) = type_src.strip_prefix("mut") {
        if after.starts_with(|c: char| !is_ident_char(c)) {
            after.trim_start()
        } else {
            type_src
        }
    } else {
        type_src
    };
    path_tail(type_src).map(str::to_string)
}

fn path_tail(src: &str) -> Option<&str> {
    let mut rest = src.trim_start();
    let mut last = None;
    loop {
        if !rest.starts_with(|c: char| is_ident_start_char(c)) {
            return last;
        }
        let end = skip_ident(rest, 0);
        last = Some(&rest[..end]);
        rest = rest[end..].trim_start();
        if let Some(after) = rest.strip_prefix("::") {
            rest = after.trim_start();
            continue;
        }
        return last;
    }
}

fn word_after<'a>(src: &'a str, word: &str) -> Option<&'a str> {
    let mut rest = src;
    while !rest.is_empty() {
        if rest.starts_with(word) && !ident_continues_str(rest, word.len()) {
            let before_ok = rest.as_ptr() == src.as_ptr()
                || src[..src.len() - rest.len()]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !is_ident_char(c));
            if before_ok {
                return Some(rest[word.len()..].trim_start());
            }
        }
        let ch = rest.chars().next()?;
        rest = &rest[ch.len_utf8()..];
    }
    None
}

fn strip_leading_angles(src: &str) -> &str {
    let src = src.trim_start();
    if !src.starts_with('<') {
        return src;
    }
    let mut depth = 0i32;
    for (idx, ch) in src.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    let next = idx + ch.len_utf8();
                    return src[next..].trim_start();
                }
            }
            _ => {}
        }
    }
    src
}

fn string_open(src: &str) -> Option<(RustState, usize)> {
    let bytes = src.as_bytes();
    let mut i = 0;
    if matches!(bytes.first(), Some(b'b' | b'c')) {
        i += 1;
    }
    let raw = if peek(bytes, i) == Some(b'r') {
        i += 1;
        true
    } else {
        false
    };
    let mut hashes = 0;
    if raw {
        while peek(bytes, i) == Some(b'#') {
            hashes += 1;
            i += 1;
        }
    }
    if peek(bytes, i) == Some(b'"') {
        let state = if raw {
            RustState::Raw { hashes }
        } else {
            RustState::String
        };
        Some((state, i + 1))
    } else {
        None
    }
}

fn skip_tick(src: &str) -> usize {
    let rest = &src[1.min(src.len())..];
    if rest.starts_with('\\') {
        if let Some(end) = rest.find('\'') {
            return 1 + end + 1;
        }
        return src.len();
    }
    let mut chars = rest.chars();
    if let Some(ch) = chars.next() {
        if chars.as_str().starts_with('\'') {
            return 1 + ch.len_utf8() + 1;
        }
        if is_ident_start_char(ch) {
            let mut n = 1 + ch.len_utf8();
            for c in chars {
                if !is_ident_char(c) {
                    break;
                }
                n += c.len_utf8();
            }
            return n;
        }
    }
    1
}

fn has_hashes(bytes: &[u8], hashes: usize) -> bool {
    bytes.len() >= hashes && bytes[..hashes].iter().all(|b| *b == b'#')
}

fn peek(bytes: &[u8], index: usize) -> Option<u8> {
    bytes.get(index).copied()
}

fn skip_char_at(text: &str, byte: usize) -> usize {
    if byte >= text.len() {
        return text.len();
    }
    let ch = text[byte..].chars().next().unwrap_or('\0');
    byte + ch.len_utf8()
}

fn skip_ws(text: &str, mut i: usize) -> usize {
    while text[i..].starts_with([' ', '\t']) {
        i += 1;
    }
    i
}

fn skip_ident(text: &str, start: usize) -> usize {
    let mut i = start;
    if i >= text.len() {
        return i;
    }
    let first = text[i..].chars().next().unwrap_or('\0');
    if !is_ident_start_char(first) {
        return i;
    }
    i += first.len_utf8();
    while let Some(ch) = text[i..].chars().next() {
        if !is_ident_char(ch) {
            break;
        }
        i += ch.len_utf8();
    }
    i
}

fn ident_continues(text: &str, at: usize) -> bool {
    text[at..].chars().next().is_some_and(is_ident_char)
}

fn ident_continues_str(text: &str, at: usize) -> bool {
    ident_continues(text, at)
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_start_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(src: &str, kind: SymbolKind) -> Vec<String> {
        collect(Language::Rust, &Rope::from_str(src))
            .into_iter()
            .filter(|symbol| symbol.kind == kind)
            .map(|symbol| symbol.name)
            .collect()
    }

    fn symbol_at(src: &str, name: &str) -> Symbol {
        collect(Language::Rust, &Rope::from_str(src))
            .into_iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing {name} in {src:?}"))
    }

    #[test]
    fn rust_functions_methods_and_simple_lets() {
        let src = "fn main() {\n    let count = 1;\n    let (a, b) = (1, 2);\n}\nimpl Foo {\n    fn bar(&self) {}\n}\nimpl Trait for bar::Baz {\n    fn go() {}\n}\n";
        assert_eq!(
            names(src, SymbolKind::Function),
            vec!["main", "Foo::bar", "Baz::go"]
        );
        assert_eq!(names(src, SymbolKind::Variable), vec!["count"]);
        let main = symbol_at(src, "main");
        assert_eq!(&src[main.byte..main.byte + 4], "main");
        let bar = symbol_at(src, "Foo::bar");
        assert_eq!(&src[bar.byte..bar.byte + 3], "bar");
    }

    #[test]
    fn rust_skips_comments_strings_params_and_const_fn() {
        let src = concat!(
            "// fn hidden\n",
            "/* fn also() {} */\n",
            "fn real() {\n",
            "    let s = \"fn nope\";\n",
            "    let raw = r#\"fn raw\"#;\n",
            "    let mut count = 1;\n",
            "}\n",
            "const MAX: i32 = 1;\n",
            "static mut FLAG: bool = false;\n",
            "const fn make() {}\n",
            "fn f(count: i32) {}\n",
        );
        assert_eq!(names(src, SymbolKind::Function), vec!["real", "make", "f"]);
        assert_eq!(
            names(src, SymbolKind::Variable),
            vec!["s", "raw", "count", "MAX", "FLAG"]
        );
        let comment = "/*\nfn hidden() {}\n*/\nfn shown() {}\n";
        assert_eq!(names(comment, SymbolKind::Function), vec!["shown"]);
        let spanning = "fn real() {\n    let s = \"fn\nhidden\";\n}\n";
        assert_eq!(names(spanning, SymbolKind::Function), vec!["real"]);
        assert_eq!(names(spanning, SymbolKind::Variable), vec!["s"]);
    }

    #[test]
    fn plain_text_has_no_symbols() {
        let rope = Rope::from_str("fn main() {}\n");
        assert!(collect(Language::Plain, &rope).is_empty());
        assert!(!supports(Language::Plain));
        assert!(supports(Language::Rust));
    }
}
