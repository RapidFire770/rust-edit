// Python, Go, and JavaScript/TypeScript definition scans.
// Comments and strings are blanked in place so a symbol's byte still
// indexes the original source.

use ropey::Rope;

use crate::symbols::{Symbol, SymbolKind};

#[derive(Clone)]
struct PyCarry {
    /// `b'"'` or `b'\''` while a triple-quoted string is open.
    triple: Option<u8>,
    /// `(indent, class name)` for classes whose body is still open.
    classes: Vec<(usize, String)>,
}

#[derive(Clone)]
enum GoState {
    Code,
    Block,
    Raw,
    String,
}

#[derive(Clone)]
struct JsCarry {
    state: JsLex,
    depth: usize,
    paren: usize,
    /// Class body depth and name. The depth is the depth inside the `{`.
    classes: Vec<(usize, String)>,
    pending_class: Option<String>,
}

#[derive(Clone)]
enum JsLex {
    Code,
    Block,
    String(u8),
    Template,
}

pub fn collect_python(rope: &Rope) -> Vec<Symbol> {
    let mut carry = PyCarry {
        triple: None,
        classes: Vec::new(),
    };
    let mut out = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        python_feed(
            &line.to_string(),
            rope.line_to_byte(line_idx),
            line_idx,
            &mut carry,
            &mut out,
        );
    }
    out
}

pub fn collect_go(rope: &Rope) -> Vec<Symbol> {
    let mut state = GoState::Code;
    let mut out = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        go_feed(
            &line.to_string(),
            rope.line_to_byte(line_idx),
            line_idx,
            &mut state,
            &mut out,
        );
    }
    out
}

pub fn collect_js(rope: &Rope) -> Vec<Symbol> {
    let mut carry = JsCarry {
        state: JsLex::Code,
        depth: 0,
        paren: 0,
        classes: Vec::new(),
        pending_class: None,
    };
    let mut out = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        js_feed(
            &line.to_string(),
            rope.line_to_byte(line_idx),
            line_idx,
            &mut carry,
            &mut out,
        );
    }
    out
}

fn python_feed(
    line: &str,
    line_byte: usize,
    line_idx: usize,
    carry: &mut PyCarry,
    out: &mut Vec<Symbol>,
) {
    let masked = mask_python(line, &mut carry.triple);
    let trimmed = masked.trim();
    if trimmed.is_empty() {
        return;
    }
    let indent = masked.len() - masked.trim_start().len();
    while carry
        .classes
        .last()
        .is_some_and(|(class_indent, _)| *class_indent >= indent)
    {
        carry.classes.pop();
    }
    let mut start = 0;
    for (end, _) in masked.match_indices(';').chain([(masked.len(), "")]) {
        python_stmt(
            &masked[start..end],
            line_byte + start,
            line_idx,
            indent,
            carry,
            out,
        );
        start = end + 1;
    }
}

fn python_stmt(
    masked: &str,
    line_byte: usize,
    line_idx: usize,
    indent: usize,
    carry: &mut PyCarry,
    out: &mut Vec<Symbol>,
) {
    if masked.trim().is_empty() {
        return;
    }
    let code_off = masked.len() - masked.trim_start().len();
    let code = masked.trim_start();
    if let Some(name_rel) = after_keyword(code, "class") {
        let name_off = code_off + name_rel;
        if let Some(name) = ident_at(masked, name_off) {
            carry.classes.push((indent, name.to_string()));
        }
        return;
    }
    if let Some(name_off) = def_name_off(masked, code_off) {
        if let Some(name) = ident_at(masked, name_off) {
            let owner = carry
                .classes
                .last()
                .filter(|(class_indent, _)| indent > *class_indent)
                .map(|(_, class_name)| class_name.as_str());
            push_fn(out, owner, ".", name, line_idx, line_byte + name_off);
        }
        return;
    }
    if let Some(name_off) = for_name_off(masked, code_off) {
        if let Some(name) = ident_at(masked, name_off) {
            push_var(out, name, line_idx, line_byte + name_off);
        }
        return;
    }
    if let Some(name_off) = assign_name_off(masked, code_off) {
        if let Some(name) = ident_at(masked, name_off) {
            push_var(out, name, line_idx, line_byte + name_off);
        }
    }
}

fn def_name_off(masked: &str, code_off: usize) -> Option<usize> {
    let code = &masked[code_off..];
    let rest = if let Some(after) = code.strip_prefix("async") {
        if ident_continues(after) {
            return None;
        }
        after.trim_start()
    } else {
        code
    };
    let skipped = code.len() - rest.len();
    let after_def = after_keyword(rest, "def")?;
    Some(code_off + skipped + after_def)
}

fn for_name_off(masked: &str, code_off: usize) -> Option<usize> {
    let code = &masked[code_off..];
    let name_rel = after_keyword(code, "for")?;
    let name_off = code_off + name_rel;
    let name = ident_at(masked, name_off)?;
    let after = masked[name_off + name.len()..].trim_start();
    let after_in = after.strip_prefix("in")?;
    if ident_continues(after_in) {
        None
    } else {
        Some(name_off)
    }
}

fn assign_name_off(masked: &str, code_off: usize) -> Option<usize> {
    let name = ident_at(masked, code_off)?;
    if matches!(
        name,
        "def"
            | "class"
            | "for"
            | "if"
            | "elif"
            | "else"
            | "while"
            | "with"
            | "return"
            | "async"
            | "try"
            | "except"
            | "finally"
            | "raise"
            | "pass"
            | "break"
            | "continue"
            | "yield"
            | "lambda"
            | "assert"
            | "import"
            | "from"
            | "global"
            | "nonlocal"
            | "del"
    ) {
        return None;
    }
    let after = masked[code_off + name.len()..].trim_start();
    if after.starts_with('=') || after.starts_with(':') {
        if after.starts_with("==") {
            return None;
        }
        Some(code_off)
    } else {
        None
    }
}

fn mask_python(line: &str, triple: &mut Option<u8>) -> String {
    let mut buf = line.as_bytes().to_vec();
    let mut i = 0;
    if let Some(q) = *triple {
        if let Some(end) = find_triple(&buf, 0, q) {
            blank(&mut buf, 0, end);
            *triple = None;
            i = end;
        } else {
            blank_rest(&mut buf, 0);
            return bytes_to_string(buf);
        }
    }
    while i < buf.len() {
        if buf[i] == b'#' {
            blank_rest(&mut buf, i);
            break;
        }
        if buf[i] == b'"' || buf[i] == b'\'' {
            let q = buf[i];
            if buf.get(i + 1) == Some(&q) && buf.get(i + 2) == Some(&q) {
                if let Some(end) = find_triple(&buf, i + 3, q) {
                    blank(&mut buf, i, end);
                    i = end;
                } else {
                    blank_rest(&mut buf, i);
                    *triple = Some(q);
                    break;
                }
            } else if let Some(end) = find_single(&buf, i + 1, q) {
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                break;
            }
            continue;
        }
        i += 1;
    }
    bytes_to_string(buf)
}

fn find_triple(buf: &[u8], start: usize, q: u8) -> Option<usize> {
    let mut i = start;
    while i + 2 < buf.len() {
        if buf[i] == q && buf[i + 1] == q && buf[i + 2] == q {
            return Some(i + 3);
        }
        i += 1;
    }
    None
}

fn find_single(buf: &[u8], start: usize, q: u8) -> Option<usize> {
    let mut i = start;
    while i < buf.len() {
        if buf[i] == b'\\' {
            i += 2;
            continue;
        }
        if buf[i] == q {
            return Some(i + 1);
        }
        if buf[i] == b'\n' {
            return None;
        }
        i += 1;
    }
    None
}

fn go_feed(
    line: &str,
    line_byte: usize,
    line_idx: usize,
    state: &mut GoState,
    out: &mut Vec<Symbol>,
) {
    let masked = mask_go(line, state);
    if let Some(name_off) = go_method(&masked) {
        if let Some(name) = ident_at(&masked, name_off) {
            let owner = go_recv_type(&masked);
            push_fn(out, owner, ".", name, line_idx, line_byte + name_off);
        }
        return;
    }
    if let Some(name_off) = after_keyword_at(&masked, "func") {
        if let Some(name) = ident_at(&masked, name_off) {
            push_fn(out, None, ".", name, line_idx, line_byte + name_off);
        }
        return;
    }
    if let Some(name_off) =
        after_keyword_at(&masked, "var").or_else(|| after_keyword_at(&masked, "const"))
    {
        if let Some(name) = ident_at(&masked, name_off) {
            push_var(out, name, line_idx, line_byte + name_off);
        }
        return;
    }
    if let Some(name_off) = go_short_decl(&masked) {
        if let Some(name) = ident_at(&masked, name_off) {
            push_var(out, name, line_idx, line_byte + name_off);
        }
    }
}

fn go_method(masked: &str) -> Option<usize> {
    let rest = masked.trim_start();
    let head = rest.strip_prefix("func")?;
    if ident_continues(head) {
        return None;
    }
    let after_func = head.trim_start();
    let recv = after_func.strip_prefix('(')?;
    let close = recv.find(')')?;
    let after = recv[close + 1..].trim_start();
    if !after.starts_with(|c: char| is_ident_start_char(c)) {
        return None;
    }
    // `after` is a suffix of `masked` (trims only drop a prefix of this slice).
    Some(masked.len() - after.len())
}

fn go_recv_type(masked: &str) -> Option<&str> {
    let rest = masked.trim_start().strip_prefix("func")?.trim_start();
    let recv = rest.strip_prefix('(')?;
    let close = recv.find(')')?;
    let inside = &recv[..close];
    if let Some(bracket) = inside.find('[') {
        let head = inside[..bracket].trim_end();
        return head.rsplit(|c: char| !is_ident_char(c)).next();
    }
    let mut last = None;
    let mut i = 0;
    let bytes = inside.as_bytes();
    while i < bytes.len() {
        if is_ident_start(bytes[i]) {
            let end = skip_ident(inside, i);
            last = Some(&inside[i..end]);
            i = end;
        } else {
            i += 1;
        }
    }
    last
}

fn go_short_decl(masked: &str) -> Option<usize> {
    let code = masked.trim_start();
    let off = masked.len() - code.len();
    if !code.starts_with(|c: char| is_ident_start_char(c)) {
        return None;
    }
    let name = ident_at(masked, off)?;
    let after = &masked[off + name.len()..];
    if after.contains(',') && after.find(',').unwrap() < after.find(":=").unwrap_or(usize::MAX) {
        return None;
    }
    if after.trim_start().starts_with(":=") {
        Some(off)
    } else {
        None
    }
}

fn mask_go(line: &str, state: &mut GoState) -> String {
    let mut buf = line.as_bytes().to_vec();
    let mut i = 0;
    match *state {
        GoState::Block => {
            if let Some(end) = find_bytes(&buf, b"*/") {
                blank(&mut buf, 0, end + 2);
                *state = GoState::Code;
                i = end + 2;
            } else {
                blank_rest(&mut buf, 0);
                return bytes_to_string(buf);
            }
        }
        GoState::Raw => {
            if let Some(end) = buf.iter().position(|b| *b == b'`') {
                blank(&mut buf, 0, end + 1);
                *state = GoState::Code;
                i = end + 1;
            } else {
                blank_rest(&mut buf, 0);
                return bytes_to_string(buf);
            }
        }
        GoState::String => {
            if let Some(end) = find_single(&buf, 0, b'"') {
                blank(&mut buf, 0, end);
                *state = GoState::Code;
                i = end;
            } else {
                blank_rest(&mut buf, 0);
                return bytes_to_string(buf);
            }
        }
        GoState::Code => {}
    }
    while i < buf.len() {
        if buf[i] == b'/' && buf.get(i + 1) == Some(&b'/') {
            blank_rest(&mut buf, i);
            break;
        }
        if buf[i] == b'/' && buf.get(i + 1) == Some(&b'*') {
            if let Some(end) = find_bytes(&buf[i + 2..], b"*/") {
                let end = i + 2 + end + 2;
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                *state = GoState::Block;
                break;
            }
            continue;
        }
        if buf[i] == b'`' {
            if let Some(rel) = buf[i + 1..].iter().position(|b| *b == b'`') {
                let end = i + 1 + rel + 1;
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                *state = GoState::Raw;
                break;
            }
            continue;
        }
        if buf[i] == b'\'' {
            if let Some(end) = find_single(&buf, i + 1, b'\'') {
                blank(&mut buf, i, end);
                i = end;
            } else {
                i += 1;
            }
            continue;
        }
        if buf[i] == b'"' {
            if let Some(end) = find_single(&buf, i + 1, b'"') {
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                *state = GoState::String;
                break;
            }
            continue;
        }
        i += 1;
    }
    bytes_to_string(buf)
}

fn js_feed(
    line: &str,
    line_byte: usize,
    line_idx: usize,
    carry: &mut JsCarry,
    out: &mut Vec<Symbol>,
) {
    let masked = mask_js(line, &mut carry.state);
    let bytes = masked.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            carry.paren += 1;
            i += 1;
            continue;
        }
        if bytes[i] == b')' {
            if carry.paren > 0 {
                carry.paren -= 1;
            }
            i += 1;
            continue;
        }
        if bytes[i] == b'{' {
            carry.depth += 1;
            if let Some(name) = carry.pending_class.take() {
                carry.classes.push((carry.depth, name));
            }
            i += 1;
            continue;
        }
        if bytes[i] == b'}' {
            if carry.depth > 0 {
                carry.depth -= 1;
            }
            carry.classes.retain(|(depth, _)| *depth <= carry.depth);
            i += 1;
            continue;
        }
        if !is_ident_start(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        i = skip_ident(&masked, i);
        let word = &masked[start..i];
        if word == "function" {
            let name_off = skip_ws(&masked, i);
            if let Some(name) = ident_at(&masked, name_off) {
                let owner = class_owner(carry);
                push_fn(out, owner, ".", name, line_idx, line_byte + name_off);
                i = name_off + name.len();
            }
            continue;
        }
        if word == "class" {
            let name_off = skip_ws(&masked, i);
            if let Some(name) = ident_at(&masked, name_off) {
                carry.pending_class = Some(name.to_string());
                i = name_off + name.len();
            }
            continue;
        }
        if matches!(word, "const" | "let" | "var") {
            let name_off = skip_ws(&masked, i);
            if masked[name_off..].starts_with(['{', '[']) {
                continue;
            }
            if let Some(name) = ident_at(&masked, name_off) {
                let after = masked[name_off + name.len()..].trim_start();
                if let Some(value) = after.strip_prefix('=').map(str::trim_start) {
                    if is_js_function_value(value) {
                        push_fn(out, None, ".", name, line_idx, line_byte + name_off);
                    } else if !value.starts_with(',') {
                        push_var(out, name, line_idx, line_byte + name_off);
                    }
                } else if !after.starts_with(',') {
                    push_var(out, name, line_idx, line_byte + name_off);
                }
                i = name_off + name.len();
            }
            continue;
        }
        if class_owner(carry).is_some()
            && !is_js_keyword(word)
            && next_code(&masked[i..]).starts_with('(')
        {
            push_fn(
                out,
                class_owner(carry),
                ".",
                word,
                line_idx,
                line_byte + start,
            );
            continue;
        }
        if carry.paren == 0
            && class_owner(carry).is_some()
            && !is_js_keyword(word)
            && (next_code(&masked[i..]).starts_with('=')
                || next_code(&masked[i..]).starts_with(';'))
        {
            let after = next_code(&masked[i..]);
            if after.starts_with('=')
                && is_js_function_value(after.trim_start_matches('=').trim_start())
            {
                push_fn(
                    out,
                    class_owner(carry),
                    ".",
                    word,
                    line_idx,
                    line_byte + start,
                );
            } else {
                push_var(out, word, line_idx, line_byte + start);
            }
        }
    }
}

fn class_owner(carry: &JsCarry) -> Option<&str> {
    carry
        .classes
        .iter()
        .rev()
        .find(|(depth, _)| *depth == carry.depth)
        .map(|(_, name)| name.as_str())
}

fn is_js_function_value(after: &str) -> bool {
    let after = after.trim_start();
    if let Some(rest) = after.strip_prefix("async") {
        if !ident_continues(rest) {
            return is_js_function_value(rest);
        }
    }
    if let Some(rest) = after.strip_prefix("function") {
        if !ident_continues(rest) {
            return true;
        }
    }
    if after.starts_with('(') {
        let bytes = after.as_bytes();
        let mut depth = 0i32;
        for (idx, byte) in bytes.iter().enumerate() {
            if *byte == b'(' {
                depth += 1;
            } else if *byte == b')' {
                depth -= 1;
                if depth == 0 {
                    return after[idx + 1..].trim_start().starts_with("=>");
                }
            }
        }
        return false;
    }
    if after.starts_with(|c: char| is_ident_start_char(c)) {
        let end = skip_ident(after, 0);
        return after[end..].trim_start().starts_with("=>");
    }
    false
}

fn is_js_keyword(word: &str) -> bool {
    matches!(
        word,
        "if" | "for"
            | "while"
            | "switch"
            | "catch"
            | "return"
            | "new"
            | "function"
            | "class"
            | "const"
            | "let"
            | "var"
            | "async"
            | "get"
            | "set"
            | "static"
    )
}

fn regex_before(buf: &[u8], slash: usize) -> bool {
    let mut j = slash;
    while j > 0 && matches!(buf[j - 1], b' ' | b'\t') {
        j -= 1;
    }
    if j == 0 {
        return true;
    }
    !is_ident_char(buf[j - 1] as char) && buf[j - 1] != b')'
}

fn find_regex_end(buf: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i < buf.len() {
        if buf[i] == b'\\' {
            i += 2;
            continue;
        }
        if buf[i] == b'/' {
            return Some(i + 1);
        }
        if buf[i] == b'\n' {
            return None;
        }
        i += 1;
    }
    None
}

fn next_code(src: &str) -> &str {
    src.trim_start()
}

fn mask_js(line: &str, state: &mut JsLex) -> String {
    let mut buf = line.as_bytes().to_vec();
    let mut i = 0;
    match *state {
        JsLex::Block => {
            if let Some(end) = find_bytes(&buf, b"*/") {
                blank(&mut buf, 0, end + 2);
                *state = JsLex::Code;
                i = end + 2;
            } else {
                blank_rest(&mut buf, 0);
                return bytes_to_string(buf);
            }
        }
        JsLex::String(q) => {
            if let Some(end) = find_single(&buf, 0, q) {
                blank(&mut buf, 0, end);
                *state = JsLex::Code;
                i = end;
            } else {
                blank_rest(&mut buf, 0);
                return bytes_to_string(buf);
            }
        }
        JsLex::Template => {
            if let Some(end) = buf.iter().position(|b| *b == b'`') {
                blank(&mut buf, 0, end + 1);
                *state = JsLex::Code;
                i = end + 1;
            } else {
                blank_rest(&mut buf, 0);
                return bytes_to_string(buf);
            }
        }
        JsLex::Code => {}
    }
    while i < buf.len() {
        if buf[i] == b'/' && buf.get(i + 1) == Some(&b'/') {
            blank_rest(&mut buf, i);
            break;
        }
        if buf[i] == b'/' && buf.get(i + 1) != Some(&b'*') && regex_before(&buf, i) {
            if let Some(end) = find_regex_end(&buf, i + 1) {
                blank(&mut buf, i, end);
                i = end;
                continue;
            }
        }
        if buf[i] == b'/' && buf.get(i + 1) == Some(&b'*') {
            if let Some(rel) = find_bytes(&buf[i + 2..], b"*/") {
                let end = i + 2 + rel + 2;
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                *state = JsLex::Block;
                break;
            }
            continue;
        }
        if matches!(buf[i], b'"' | b'\'') {
            let q = buf[i];
            if let Some(end) = find_single(&buf, i + 1, q) {
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                *state = JsLex::String(q);
                break;
            }
            continue;
        }
        if buf[i] == b'`' {
            if let Some(rel) = buf[i + 1..].iter().position(|b| *b == b'`') {
                let end = i + 1 + rel + 1;
                blank(&mut buf, i, end);
                i = end;
            } else {
                blank_rest(&mut buf, i);
                *state = JsLex::Template;
                break;
            }
            continue;
        }
        i += 1;
    }
    bytes_to_string(buf)
}

fn push_fn(
    out: &mut Vec<Symbol>,
    owner: Option<&str>,
    sep: &str,
    name: &str,
    line: usize,
    byte: usize,
) {
    let display = match owner {
        Some(owner) => format!("{owner}{sep}{name}"),
        None => name.to_string(),
    };
    out.push(Symbol {
        name: display,
        kind: SymbolKind::Function,
        line,
        byte,
    });
}

fn push_var(out: &mut Vec<Symbol>, name: &str, line: usize, byte: usize) {
    out.push(Symbol {
        name: name.to_string(),
        kind: SymbolKind::Variable,
        line,
        byte,
    });
}

/// Byte offset of the ident after a leading keyword, relative to `code`.
fn after_keyword(code: &str, keyword: &str) -> Option<usize> {
    let rest = code.trim_start();
    let after = rest.strip_prefix(keyword)?;
    if ident_continues(after) {
        return None;
    }
    let skipped = code.len() - rest.len() + keyword.len();
    let name = after.trim_start();
    if !name.starts_with(|c: char| is_ident_start_char(c)) {
        return None;
    }
    Some(skipped + (after.len() - name.len()))
}

fn after_keyword_at(masked: &str, keyword: &str) -> Option<usize> {
    let code = masked.trim_start();
    let rel = after_keyword(code, keyword)?;
    Some(masked.len() - code.len() + rel)
}

fn ident_at(text: &str, off: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    if off >= bytes.len() || !is_ident_start(bytes[off]) {
        return None;
    }
    let end = skip_ident(text, off);
    Some(&text[off..end])
}

fn ident_continues(rest: &str) -> bool {
    rest.starts_with(|c: char| is_ident_char(c))
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

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn bytes_to_string(buf: Vec<u8>) -> String {
    String::from_utf8(buf).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of_kind(src: &str, kind: SymbolKind, collect: fn(&Rope) -> Vec<Symbol>) -> Vec<String> {
        collect(&Rope::from_str(src))
            .into_iter()
            .filter(|symbol| symbol.kind == kind)
            .map(|symbol| symbol.name)
            .collect()
    }

    fn byte_is_name(src: &str, display: &str, collect: fn(&Rope) -> Vec<Symbol>) {
        let symbol = collect(&Rope::from_str(src))
            .into_iter()
            .find(|symbol| symbol.name == display)
            .unwrap_or_else(|| panic!("missing {display}"));
        let bare = display.rsplit(['.', ':']).next().unwrap();
        assert_eq!(&src[symbol.byte..symbol.byte + bare.len()], bare);
    }

    #[test]
    fn python_defs_methods_and_assignments_skip_comments() {
        let src = concat!(
            "# def hidden\n",
            "s = \"def nope\"\n",
            "text = '''def\n",
            "still'''\n",
            "def real():\n",
            "    count = 1\n",
            "class Box:\n",
            "    def open(self):\n",
            "        pass\n",
            "for name in items:\n",
            "    pass\n",
            "a, b = 1, 2\n",
        );
        assert_eq!(
            of_kind(src, SymbolKind::Function, collect_python),
            vec!["real", "Box.open"]
        );
        assert_eq!(
            of_kind(src, SymbolKind::Variable, collect_python),
            vec!["s", "text", "count", "name"]
        );
        byte_is_name(src, "Box.open", collect_python);
        let stmts = "x = 1; def later():\n    pass\nelse:\n    try:\n        pass\n";
        assert_eq!(
            of_kind(stmts, SymbolKind::Function, collect_python),
            vec!["later"]
        );
        assert_eq!(
            of_kind(stmts, SymbolKind::Variable, collect_python),
            vec!["x"]
        );
    }

    #[test]
    fn go_funcs_methods_and_declarations_skip_comments() {
        let src = concat!(
            "// func hidden\n",
            "const label = \"func nope\"\n",
            "func real() {}\n",
            "func (s *Server) Start() {}\n",
            "var count int\n",
            "name := 1\n",
            "a, b := 2, 3\n",
        );
        assert_eq!(
            of_kind(src, SymbolKind::Function, collect_go),
            vec!["real", "Server.Start"]
        );
        assert_eq!(
            of_kind(src, SymbolKind::Variable, collect_go),
            vec!["label", "count", "name"]
        );
        byte_is_name(src, "Server.Start", collect_go);
        let generic = "func (s *Server[T]) Start() {}\nvar q = '\"'\nfunc after() {}\n";
        assert_eq!(
            of_kind(generic, SymbolKind::Function, collect_go),
            vec!["Server.Start", "after"]
        );
    }

    #[test]
    fn js_functions_arrows_and_fields_skip_comments() {
        let src = concat!(
            "// function hidden\n",
            "const s = \"function nope\";\n",
            "function real() {}\n",
            "class Box {\n",
            "  count = 1;\n",
            "  open() {}\n",
            "}\n",
            "const make = () => 1;\n",
            "const also = function () {};\n",
            "let value = 2;\n",
            "const {a} = obj;\n",
        );
        assert_eq!(
            of_kind(src, SymbolKind::Function, collect_js),
            vec!["real", "Box.open", "make", "also"]
        );
        assert_eq!(
            of_kind(src, SymbolKind::Variable, collect_js),
            vec!["s", "count", "value"]
        );
        byte_is_name(src, "Box.open", collect_js);
        byte_is_name(src, "make", collect_js);
        let tricky = concat!(
            "const n = foo(x => x);\n",
            "class Box {\n",
            "  open(count = 1) {}\n",
            "}\n",
            "const s = /function hidden()/;\n",
        );
        assert_eq!(
            of_kind(tricky, SymbolKind::Function, collect_js),
            vec!["Box.open"]
        );
        assert_eq!(
            of_kind(tricky, SymbolKind::Variable, collect_js),
            vec!["n", "s"]
        );
    }
}
