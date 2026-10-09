// Java, C#, C, and C++ definition scans.
// A name is kept only when its type token is on the same line, so a C
// function whose return type sits on the previous line is missed on purpose.

use ropey::Rope;

use crate::symbols::{Symbol, SymbolKind};

#[derive(Clone)]
enum Lex {
    Code,
    Block,
    String,
}

#[derive(Clone)]
struct Carry {
    lex: Lex,
    paren: usize,
    brace: usize,
    classes: Vec<(usize, String)>,
    pending_class: Option<String>,
}

pub fn collect_java(rope: &Rope) -> Vec<Symbol> {
    collect(rope)
}

pub fn collect_c(rope: &Rope) -> Vec<Symbol> {
    collect(rope)
}

fn collect(rope: &Rope) -> Vec<Symbol> {
    let mut carry = Carry {
        lex: Lex::Code,
        paren: 0,
        brace: 0,
        classes: Vec::new(),
        pending_class: None,
    };
    let mut out = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        feed(
            &line.to_string(),
            rope.line_to_byte(line_idx),
            line_idx,
            &mut carry,
            &mut out,
        );
    }
    out
}

fn feed(line: &str, line_byte: usize, line_idx: usize, carry: &mut Carry, out: &mut Vec<Symbol>) {
    let masked = mask(line, &mut carry.lex);
    let bytes = masked.as_bytes();
    let mut i = 0;
    let mut last_type: Option<usize> = None;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                carry.brace += 1;
                if let Some(name) = carry.pending_class.take() {
                    carry.classes.push((carry.brace, name));
                }
                last_type = None;
                i += 1;
            }
            b'}' => {
                if carry.brace > 0 {
                    carry.brace -= 1;
                }
                carry.classes.retain(|(depth, _)| *depth <= carry.brace);
                last_type = None;
                i += 1;
            }
            b'(' => {
                carry.paren += 1;
                i += 1;
            }
            b')' => {
                if carry.paren > 0 {
                    carry.paren -= 1;
                }
                last_type = None;
                i += 1;
            }
            b'*' | b'&' => {
                i += 1;
            }
            b if is_ident_start(b) => {
                let start = i;
                i = skip_ident(&masked, i);
                let word = &masked[start..i];
                if matches!(word, "class" | "struct") {
                    let name_off = skip_ws(&masked, i);
                    if let Some(name) = ident_at(&masked, name_off) {
                        carry.pending_class = Some(name.to_string());
                        i = name_off + name.len();
                    }
                    last_type = None;
                    continue;
                }
                let next = masked[i..].trim_start();
                if is_control(word) {
                    last_type = None;
                    continue;
                }
                if is_type_keyword(word) {
                    last_type = Some(start);
                    continue;
                }
                if carry.paren == 0 && next.starts_with('(') && last_type.is_some() {
                    push(
                        out,
                        owner(carry),
                        word,
                        SymbolKind::Function,
                        line_idx,
                        line_byte + start,
                    );
                    last_type = None;
                    continue;
                }
                if carry.paren == 0
                    && last_type.is_some()
                    && (next.starts_with('=') || next.starts_with(';') || next.starts_with(','))
                {
                    push(
                        out,
                        None,
                        word,
                        SymbolKind::Variable,
                        line_idx,
                        line_byte + start,
                    );
                    last_type = None;
                    continue;
                }
                last_type = Some(start);
            }
            _ => {
                if !masked[i..].starts_with(|c: char| c.is_whitespace()) {
                    // A non-type token such as `=` ends the declarator run,
                    // except stars and parens which are handled above.
                    if !matches!(bytes[i], b'[' | b']') {
                        last_type = None;
                    }
                }
                i += 1;
            }
        }
    }
}

fn owner(carry: &Carry) -> Option<&str> {
    carry
        .classes
        .iter()
        .rev()
        .find(|(depth, _)| *depth == carry.brace)
        .map(|(_, name)| name.as_str())
}

fn push(
    out: &mut Vec<Symbol>,
    owner: Option<&str>,
    name: &str,
    kind: SymbolKind,
    line: usize,
    byte: usize,
) {
    let display = match owner {
        Some(owner) if kind == SymbolKind::Function => format!("{owner}.{name}"),
        _ => name.to_string(),
    };
    out.push(Symbol {
        name: display,
        kind,
        line,
        byte,
    });
}

fn is_control(word: &str) -> bool {
    matches!(
        word,
        "if" | "for" | "while" | "switch" | "catch" | "return" | "else" | "do" | "new" | "sizeof"
    )
}

/// Modifiers and type keywords that may sit in front of a declarator.
fn is_type_keyword(word: &str) -> bool {
    matches!(
        word,
        "public"
            | "private"
            | "protected"
            | "static"
            | "final"
            | "void"
            | "const"
            | "unsigned"
            | "signed"
            | "virtual"
            | "inline"
            | "override"
            | "extern"
    )
}

fn mask(line: &str, state: &mut Lex) -> String {
    let mut buf = line.as_bytes().to_vec();
    let mut i = 0;
    if matches!(*state, Lex::Block) {
        if let Some(end) = find_bytes(&buf, b"*/") {
            blank(&mut buf, 0, end + 2);
            *state = Lex::Code;
            i = end + 2;
        } else {
            blank_rest(&mut buf, 0);
            return bytes_to_string(buf);
        }
    } else if matches!(*state, Lex::String) {
        if let Some(end) = find_quote(&buf, 0) {
            blank(&mut buf, 0, end);
            *state = Lex::Code;
            i = end;
        } else {
            blank_rest(&mut buf, 0);
            return bytes_to_string(buf);
        }
    }
    while i < buf.len() {
        if buf[i] == b'/' && buf.get(i + 1) == Some(&b'/') {
            blank_rest(&mut buf, i);
            break;
        }
        if buf[i] == b'/' && buf.get(i + 1) == Some(&b'*') {
            if let Some(rel) = find_bytes(&buf[i + 2..], b"*/") {
                let end = i + 2 + rel + 2;
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                *state = Lex::Block;
                break;
            }
            continue;
        }
        if buf[i] == b'"' {
            if let Some(end) = find_quote(&buf, i + 1) {
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                *state = Lex::String;
                break;
            }
            continue;
        }
        i += 1;
    }
    bytes_to_string(buf)
}

fn find_quote(buf: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i < buf.len() {
        if buf[i] == b'\\' {
            i += 2;
            continue;
        }
        if buf[i] == b'"' {
            return Some(i + 1);
        }
        if buf[i] == b'\n' {
            return None;
        }
        i += 1;
    }
    None
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn blank(buf: &mut [u8], start: usize, end: usize) {
    let end = end.min(buf.len());
    for byte in &mut buf[start..end] {
        *byte = b' ';
    }
}

fn blank_rest(buf: &mut [u8], start: usize) {
    let end = buf.len();
    blank(buf, start, end);
}

fn bytes_to_string(buf: Vec<u8>) -> String {
    String::from_utf8(buf).unwrap_or_default()
}

fn ident_at(text: &str, off: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    if off >= bytes.len() || !is_ident_start(bytes[off]) {
        return None;
    }
    let end = skip_ident(text, off);
    Some(&text[off..end])
}

fn skip_ws(text: &str, mut i: usize) -> usize {
    while text[i..].starts_with([' ', '\t']) {
        i += 1;
    }
    i
}

fn skip_ident(text: &str, start: usize) -> usize {
    let mut i = start;
    let Some(first) = text[i..].chars().next() else {
        return i;
    };
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
        collect(&Rope::from_str(src))
            .into_iter()
            .filter(|symbol| symbol.kind == kind)
            .map(|symbol| symbol.name)
            .collect()
    }

    #[test]
    fn java_methods_and_fields_skip_comments_and_parameters() {
        let src = concat!(
            "// void hidden()\n",
            "String s = \"void nope()\";\n",
            "public class Box {\n",
            "  private int count = 1;\n",
            "  public void open() {}\n",
            "  void run(int count) {}\n",
            "}\n",
        );
        assert_eq!(
            names(src, SymbolKind::Function),
            vec!["Box.open", "Box.run"]
        );
        assert_eq!(names(src, SymbolKind::Variable), vec!["s", "count"]);
    }

    #[test]
    fn c_keeps_prototypes_and_misses_a_split_return_type() {
        let src = concat!(
            "/* int hidden() */\n",
            "const char *s = \"int nope()\";\n",
            "int real(int count);\n",
            "int\n",
            "split(void);\n",
            "int main(void) { int local = 1; }\n",
        );
        assert_eq!(names(src, SymbolKind::Function), vec!["real", "main"]);
        assert_eq!(names(src, SymbolKind::Variable), vec!["s", "local"]);
        assert!(!names(src, SymbolKind::Variable)
            .iter()
            .any(|n| n == "count"));
        assert!(!names(src, SymbolKind::Function)
            .iter()
            .any(|n| n == "split"));
    }
}
