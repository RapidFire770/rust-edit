// Link a few grammars syntect 5.3 does not ship onto the default newlines set.
// The result is an uncompressed packdump so startup stays a deserialize.
// Linking here, not at launch, is the point: SyntaxSet::into_builder().build()
// re-resolves every context in the default set.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/syntaxes");
    let dir = Path::new("assets/syntaxes");
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|e| panic!("read syntax dir entry: {e}"))
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("sublime-syntax"))
        .collect();
    paths.sort();
    for path in &paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let mut builder = syntect::parsing::SyntaxSet::load_defaults_newlines().into_builder();
    for path in &paths {
        let text =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let stem = path.file_stem().and_then(|name| name.to_str());
        let def = syntect::parsing::SyntaxDefinition::load_from_str(&text, true, stem)
            .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        builder.add(def);
    }
    let set = builder.build();
    let out = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("syntaxes.packdump");
    syntect::dumps::dump_to_uncompressed_file(&set, &out)
        .unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
}
