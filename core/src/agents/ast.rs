//! AST-aware source reads via tree-sitter.
//!
//! `read_symbol` parses a file and returns only the span of a named definition
//! (function, struct, class, …) instead of the whole file. A 500-line module
//! costs ~5-8k tokens to `file_read`; the single function the model actually
//! wants is usually a few hundred. This is the token-saving win of Fase 3.

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::Path;
use tree_sitter::{Language, Node, Parser};

/// A located definition matching the requested symbol name.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct SymbolSpan {
    /// The node kind (e.g. `function_item`, `class_declaration`).
    pub kind: String,
    /// 1-based inclusive start line.
    pub start_line: usize,
    /// 1-based inclusive end line.
    pub end_line: usize,
    /// The exact source text of the definition.
    pub text: String,
}

/// Resolve a file extension to its tree-sitter language + the node kinds that
/// count as named definitions for that language.
fn language_for(path: &Path) -> Option<(Language, &'static [&'static str])>{
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "rs" => Some((
            tree_sitter_rust::LANGUAGE.into(),
            &[
                "function_item",
                "struct_item",
                "enum_item",
                "trait_item",
                "mod_item",
                "const_item",
                "static_item",
                "type_item",
                "union_item",
                "macro_definition",
            ],
        )),
        "ts" => Some((
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            TS_DEFS,
        )),
        "tsx" => Some((tree_sitter_typescript::LANGUAGE_TSX.into(), TS_DEFS)),
        "js" | "jsx" | "mjs" | "cjs" => Some((
            tree_sitter_javascript::LANGUAGE.into(),
            &[
                "function_declaration",
                "class_declaration",
                "method_definition",
                "generator_function_declaration",
            ],
        )),
        "py" => Some((
            tree_sitter_python::LANGUAGE.into(),
            &["function_definition", "class_definition"],
        )),
        _ => None,
    }
}

const TS_DEFS: &[&str] = &[
    "function_declaration",
    "class_declaration",
    "method_definition",
    "interface_declaration",
    "type_alias_declaration",
    "enum_declaration",
    "abstract_class_declaration",
];

/// Parse `source` (whose language is inferred from `path`) and return every
/// top-level-or-nested definition named `symbol`.
pub fn read_symbol(path: &Path, source: &str, symbol: &str) -> Result<Vec<SymbolSpan>> {
    let (language, def_kinds) =
        language_for(path).ok_or_else(|| anyhow!("read_symbol: unsupported file type for {}", path.display()))?;

    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .context("failed to set tree-sitter language")?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("tree-sitter failed to parse {}", path.display()))?;

    let src = source.as_bytes();
    let mut out = Vec::new();
    collect_named(tree.root_node(), src, def_kinds, symbol, &mut out);
    Ok(out)
}

/// Walk the tree, collecting definition nodes whose `name` field equals `symbol`.
fn collect_named(
    node: Node,
    src: &[u8],
    def_kinds: &[&str],
    symbol: &str,
    out: &mut Vec<SymbolSpan>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if def_kinds.contains(&child.kind())
            && let Some(name_node) = child.child_by_field_name("name")
            && name_node.utf8_text(src).ok() == Some(symbol)
        {
            if let Ok(text) = child.utf8_text(src) {
                out.push(SymbolSpan {
                    kind: child.kind().to_string(),
                    start_line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                    text: text.to_string(),
                });
            }
        }
        // Recurse to catch nested definitions (methods in impls/classes, etc).
        collect_named(child, src, def_kinds, symbol, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extracts_rust_function() {
        let src = "fn a() { 1 }\nfn target() {\n    let x = 2;\n}\nstruct S;\n";
        let out = read_symbol(&PathBuf::from("x.rs"), src, "target").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "function_item");
        assert_eq!(out[0].start_line, 2);
        assert_eq!(out[0].end_line, 4);
        assert!(out[0].text.contains("let x = 2;"));
        assert!(!out[0].text.contains("fn a()"), "only the target span");
    }

    #[test]
    fn extracts_rust_struct() {
        let src = "struct Foo {\n    a: u8,\n}\n";
        let out = read_symbol(&PathBuf::from("x.rs"), src, "Foo").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "struct_item");
    }

    #[test]
    fn finds_nested_method() {
        let src = "impl Foo {\n    fn bar(&self) {}\n}\n";
        let out = read_symbol(&PathBuf::from("x.rs"), src, "bar").unwrap();
        assert_eq!(out.len(), 1, "method inside impl should be found");
    }

    #[test]
    fn typescript_class_and_function() {
        let src = "function foo() { return 1; }\nclass Bar { baz() {} }\n";
        let foo = read_symbol(&PathBuf::from("x.ts"), src, "foo").unwrap();
        assert_eq!(foo.len(), 1);
        let baz = read_symbol(&PathBuf::from("x.ts"), src, "baz").unwrap();
        assert_eq!(baz.len(), 1, "method should be found");
    }

    #[test]
    fn python_function() {
        let src = "def foo():\n    return 1\n\nclass Bar:\n    pass\n";
        let out = read_symbol(&PathBuf::from("x.py"), src, "foo").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "function_definition");
    }

    #[test]
    fn missing_symbol_is_empty() {
        let src = "fn a() {}\n";
        let out = read_symbol(&PathBuf::from("x.rs"), src, "nope").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn unsupported_extension_errors() {
        assert!(read_symbol(&PathBuf::from("x.bin"), "data", "y").is_err());
    }
}
