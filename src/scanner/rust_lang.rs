//! Rust scanner on the shared tree-sitter stack — added the day the index tried to
//! search its own repository and could not: FDML is written in Rust and the tool
//! must eat its own dogfood. Extracts functions, impl methods, structs, enums,
//! traits, type aliases, consts/statics and macros, plus the same navigation layer
//! (anchors in oversized bodies, classified string literals) the C scanner builds.
use tree_sitter::{Node, Parser};

use crate::error::Result;
use super::types::*;

pub struct RustScanner;

const ANCHOR_MIN_LINES: usize = 300;
const ANCHOR_MIN_BRANCHES: usize = 30;
const ANCHOR_KINDS: &[&str] = &["if_expression", "for_expression", "while_expression", "match_expression", "loop_expression"];

impl RustScanner {
    pub fn parse_file(source: &str, file_path: &str) -> Result<FileAnalysis> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::language())
            .map_err(|e| crate::error::FdmlError::simple_parser_error(format!("Failed to set Rust language: {}", e)))?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| crate::error::FdmlError::simple_parser_error("Failed to parse Rust file"))?;

        let mut elements = Vec::new();
        let mut imports = Vec::new();
        let mut anchors = Vec::new();
        let mut literals = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            Self::visit(&child, source, file_path, &mut elements, &mut imports, &mut anchors, &mut literals, "");
        }
        Ok(FileAnalysis { file_path: file_path.into(), module_path: String::new(), language: Language::Rust, elements, imports, anchors, literals })
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(node: &Node, src: &str, path: &str, out: &mut Vec<CodeElement>, imports: &mut Vec<ImportInfo>, anchors: &mut Vec<CodeAnchor>, literals: &mut Vec<CodeLiteral>, scope: &str) {
        match node.kind() {
            "use_declaration" => {
                let module = node.child_by_field_name("argument").map(|a| text(&a, src)).unwrap_or_default();
                imports.push(ImportInfo { module: module.clone(), names: vec![], is_relative: module.starts_with("crate") || module.starts_with("super") || module.starts_with("self"), file_path: path.into(), line: node.start_position().row + 1 });
            }
            "function_item" => {
                if let Some(name) = named(node, src) {
                    let kind = if scope.is_empty() { ElementType::Function } else { ElementType::Method };
                    out.push(element(kind, &name, node, src, path));
                    Self::navigation(node, src, &name, anchors, literals);
                }
            }
            "struct_item" | "union_item" => {
                if let Some(name) = named(node, src) {
                    let mut el = element(ElementType::Class, &name, node, src, path);
                    el.children = struct_fields(node, src, path);
                    out.push(el);
                }
            }
            "enum_item" => {
                if let Some(name) = named(node, src) { out.push(element(ElementType::Enum, &name, node, src, path)); }
            }
            "trait_item" => {
                if let Some(name) = named(node, src) {
                    out.push(element(ElementType::Interface, &name, node, src, path));
                    // trait methods are searchable too
                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            Self::visit(&child, src, path, out, imports, anchors, literals, &name);
                        }
                    }
                }
            }
            "type_item" => {
                if let Some(name) = named(node, src) { out.push(element(ElementType::TypeAlias, &name, node, src, path)); }
            }
            "const_item" | "static_item" => {
                if let Some(name) = named(node, src) { out.push(element(ElementType::Field, &name, node, src, path)); }
            }
            "macro_definition" => {
                if let Some(name) = named(node, src) { out.push(element(ElementType::Macro, &name, node, src, path)); }
            }
            // methods live inside impl blocks; the impl target becomes their scope
            "impl_item" => {
                let target = node.child_by_field_name("type").map(|t| text(&t, src)).unwrap_or_default();
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        Self::visit(&child, src, path, out, imports, anchors, literals, &target);
                    }
                }
            }
            "mod_item" => {
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        Self::visit(&child, src, path, out, imports, anchors, literals, scope);
                    }
                }
            }
            _ => {}
        }
    }

    fn navigation(func: &Node, src: &str, name: &str, anchors: &mut Vec<CodeAnchor>, literals: &mut Vec<CodeLiteral>) {
        let Some(body) = func.child_by_field_name("body") else { return };
        collect_literals(&body, src, name, literals);
        let lines = func.end_position().row.saturating_sub(func.start_position().row) + 1;
        if lines >= ANCHOR_MIN_LINES || count_branches(&body) >= ANCHOR_MIN_BRANCHES {
            collect_anchors(&body, src, name, 1, anchors);
        }
    }
}

fn text(node: &Node, src: &str) -> String { src[node.byte_range()].to_string() }

fn named(node: &Node, src: &str) -> Option<String> {
    node.child_by_field_name("name").map(|n| text(&n, src))
}

fn element(element_type: ElementType, name: &str, node: &Node, src: &str, path: &str) -> CodeElement {
    let sig = src[node.byte_range()].lines().next().map(|l| l.trim().to_string());
    CodeElement {
        element_type, name: name.to_string(), file_path: path.into(),
        line_start: node.start_position().row + 1, line_end: node.end_position().row + 1,
        language: Language::Rust, scope: None, docstring: None, signature: sig,
        parameters: vec![], return_type: None, default_value: None,
        bases: vec![], decorators: vec![], children: vec![],
    }
}

fn struct_fields(node: &Node, src: &str, path: &str) -> Vec<CodeElement> {
    let Some(body) = node.child_by_field_name("body") else { return Vec::new() };
    let mut out = Vec::new();
    let mut cursor = body.walk();
    for field in body.children(&mut cursor) {
        if field.kind() == "field_declaration" {
            if let Some(name) = named(&field, src) { out.push(element(ElementType::Field, &name, &field, src, path)); }
        }
    }
    out
}

fn count_branches(node: &Node) -> usize {
    let mut n = 0;
    walk(node, &mut |c| if ANCHOR_KINDS.contains(&c.kind()) { n += 1 });
    n
}

fn collect_anchors(body: &Node, src: &str, parent: &str, depth: usize, out: &mut Vec<CodeAnchor>) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        let inner = match child.kind() {
            k if ANCHOR_KINDS.contains(&k) => child,
            // an expression statement wrapping a control expression
            "expression_statement" => match child.child(0) { Some(c) if ANCHOR_KINDS.contains(&c.kind()) => c, _ => continue },
            _ => continue,
        };
        let (start, end) = (inner.start_position().row + 1, inner.end_position().row + 1);
        let condition_ids = inner.child_by_field_name("condition").or_else(|| inner.child_by_field_name("value"))
            .map(|c| identifiers_in(&c, src)).unwrap_or_default();
        let mut anchor = CodeAnchor {
            parent_symbol: parent.to_string(),
            kind: inner.kind().trim_end_matches("_expression").to_string(),
            name: condition_ids.first().cloned().unwrap_or_else(|| format!("{}@{start}", inner.kind().trim_end_matches("_expression"))),
            line_start: start, line_end: end, depth,
            label: comment_above(src, start),
            condition_ids,
            calls: direct_calls(&inner, src),
            declared: let_bindings(&inner, src),
            literals: literal_values(&inner, src),
        };
        anchor.calls.truncate(12); anchor.declared.truncate(12); anchor.literals.truncate(8);
        out.push(anchor);
        if depth < 3 {
            if let Some(block) = inner.child_by_field_name("consequence").or_else(|| inner.child_by_field_name("body")) {
                collect_anchors(&block, src, parent, depth + 1, out);
            }
        }
    }
}

fn identifiers_in(node: &Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |n| if n.kind() == "identifier" { let t = text(n, src); if !out.contains(&t) { out.push(t) } });
    out
}

fn direct_calls(node: &Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |n| if n.kind() == "call_expression" {
        if let Some(f) = n.child_by_field_name("function") {
            let t = text(&f, src);
            let short = t.rsplit("::").next().unwrap_or(&t).rsplit('.').next().unwrap_or(&t).to_string();
            if short.chars().all(|c| c.is_alphanumeric() || c == '_') && !out.contains(&short) { out.push(short) }
        }
    });
    out
}

fn let_bindings(node: &Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |n| if n.kind() == "let_declaration" {
        if let Some(p) = n.child_by_field_name("pattern") {
            for id in identifiers_in(&p, src) { if !out.contains(&id) { out.push(id) } }
        }
    });
    out
}

fn literal_values(node: &Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |n| if n.kind() == "string_literal" {
        let v = text(n, src).trim_matches('"').to_string();
        if v.len() >= 2 && !out.contains(&v) { out.push(v) }
    });
    out
}

fn collect_literals(node: &Node, src: &str, scope: &str, out: &mut Vec<CodeLiteral>) {
    walk(node, &mut |n| {
        if n.kind() != "string_literal" { return }
        let value = text(n, src).trim_matches('"').to_string();
        if value.is_empty() { return }
        let kind = if value.starts_with("--") { "cli_flag" }
            else if value.contains("{}") || value.contains("{:") || value.contains('%') { "format" }
            else if value.contains('/') || value.rsplit_once('.').is_some_and(|(s, e)| !s.is_empty() && (2..=5).contains(&e.len()) && e.chars().all(|c| c.is_ascii_alphanumeric())) { "asset" }
            else { "generic" };
        out.push(CodeLiteral { value, line: n.start_position().row + 1, kind: kind.into(), usage_kind: "generic".into(), parent_symbol: scope.into() });
    });
}

fn walk<'a>(node: &Node<'a>, visit: &mut dyn FnMut(&Node<'a>)) {
    visit(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) { walk(&child, visit); }
}

fn comment_above(src: &str, line: usize) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    let start = line.saturating_sub(1);
    for i in (start.saturating_sub(6)..start).rev() {
        let t = lines.get(i)?.trim();
        let body = t.trim_start_matches(['/', '*', ' ', '\t']).trim_end_matches(['*', '/']).trim();
        if (t.starts_with("//") || t.starts_with("/*") || t.starts_with('*')) && body.chars().filter(|c| c.is_alphabetic()).count() >= 8 {
            return Some(body.chars().take(90).collect());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
use std::collections::HashSet;
const FLOW_MAX_DEPTH: usize = 6;
pub struct RepositoryIndex { root: String, db: u32 }
pub enum Verdict { Real, Noise }
pub trait Scanner { fn parse(&self) -> u32; }
type Result2 = std::result::Result<u32, String>;
macro_rules! check { () => {} }
impl RepositoryIndex {
    pub fn search(&self, q: &str) -> u32 {
        let tokens = q.len();
        if tokens > 0 { return 1 }
        0
    }
}
pub fn mark_key(q: &str) -> String { q.to_lowercase() }
"#;

    #[test]
    fn extracts_rust_symbols_methods_and_fields() {
        let a = RustScanner::parse_file(SOURCE, "index.rs").unwrap();
        let names = |k: ElementType| a.elements.iter().filter(|e| e.element_type == k).map(|e| e.name.clone()).collect::<Vec<_>>();
        assert!(names(ElementType::Function).contains(&"mark_key".to_string()));
        assert!(names(ElementType::Method).contains(&"search".to_string()), "impl methods: {:?}", names(ElementType::Method));
        assert!(names(ElementType::Class).contains(&"RepositoryIndex".to_string()));
        assert!(names(ElementType::Enum).contains(&"Verdict".to_string()));
        assert!(names(ElementType::Interface).contains(&"Scanner".to_string()));
        assert!(names(ElementType::TypeAlias).contains(&"Result2".to_string()));
        assert!(names(ElementType::Field).contains(&"FLOW_MAX_DEPTH".to_string()), "consts are fields");
        let idx = a.elements.iter().find(|e| e.name == "RepositoryIndex").unwrap();
        assert_eq!(idx.children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["root", "db"]);
        assert_eq!(a.imports[0].module, "std::collections::HashSet");
    }
}
