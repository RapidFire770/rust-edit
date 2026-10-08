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

/// Languages the lists know how to read. Others get "No symbols for this file".
pub fn supports(language: Language) -> bool {
    matches!(
        language,
        Language::Rust
            | Language::Python
            | Language::Go
            | Language::Javascript
            | Language::Typescript
            | Language::Java
            | Language::CSharp
            | Language::C
            | Language::Cpp
    )
}

/// Definitions in source order. Family scanners replace this body.
/// Until then every language yields an empty list.
pub fn collect(_language: Language, _rope: &Rope) -> Vec<Symbol> {
    Vec::new()
}
