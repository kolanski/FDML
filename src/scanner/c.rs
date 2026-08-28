//! C/C++ scanner built on the same tree-sitter grammar the other languages use.
//!
//! The previous hand-rolled line scanner required `{` on the same line as `)`, so every
//! multi-line signature was invisible, and it modelled no macros or types at all. Measured
//! against `ctags` on a real C codebase that cost ~60% of the symbols. A real parse gets
//! function definitions *and* prototypes, typedefs, structs, enums, macros and file-scope
//! globals — the declarations agents actually search for.
use tree_sitter::{Node, Parser};

use crate::error::Result;
use super::types::*;

pub struct CScanner;

impl CScanner {
    pub fn parse_file(source: &str, file_path: &str, language: Language) -> Result<FileAnalysis> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_c::language())
            .map_err(|e| crate::error::FdmlError::simple_parser_error(format!("Failed to set C language: {}", e)))?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| crate::error::FdmlError::simple_parser_error("Failed to parse C file"))?;

        let mut elements = Vec::new();
        let mut imports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            Self::visit(&child, source, file_path, &language, &mut elements, &mut imports);
        }
        // The preprocessor ignores braces, so a macro is file-scope wherever it is written —
        // including inside a function body, which the declaration walk deliberately skips.
        Self::collect_macros(&root, source, file_path, &language, &mut elements);
        let (anchors, literals) = navigation_layer(&root, source, &elements);
        Ok(FileAnalysis { file_path: file_path.into(), module_path: String::new(), language, elements, imports, anchors, literals })
    }

    /// Top-level declarations only. Nested scopes are bodies, not separate symbols —
    /// `outline` covers navigation inside a body.
    fn visit(node: &Node, src: &str, path: &str, lang: &Language, out: &mut Vec<CodeElement>, imports: &mut Vec<ImportInfo>) {
        match node.kind() {
            "preproc_include" => {
                if let Some(p) = node.child_by_field_name("path") {
                    let raw = text(&p, src);
                    let module = raw.trim_matches(|c| c == '<' || c == '>' || c == '"').to_string();
                    imports.push(ImportInfo { module, names: vec![], is_relative: raw.starts_with('"'), file_path: path.into(), line: node.start_position().row + 1 });
                }
            }
            // Include guards and `#if` blocks are transparent: their contents are still
            // file-scope. Without this, every guarded header yields nothing at all.
            "preproc_ifdef" | "preproc_if" | "preproc_else" | "preproc_elif" | "linkage_specification" | "declaration_list" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::visit(&child, src, path, lang, out, imports);
                }
            }
            "function_definition" => {
                if let Some(name) = declarator_name(node, src) {
                    out.push(element(ElementType::Function, name, node, src, path, lang, signature_of(node, src)));
                }
            }
            // Prototypes, typedefs, file-scope globals, and bare struct/enum declarations
            // all arrive as `declaration` / `type_definition`.
            "declaration" | "type_definition" => {
                let is_typedef = node.kind() == "type_definition";
                // `typedef struct {...} Foo;` — the members belong to the name the code uses
                let members = struct_of(node).map(|s| members_of(&s, src, path, lang)).unwrap_or_default();
                for (name, kind) in declared_names(node, src, is_typedef) {
                    let mut el = element(kind, name, node, src, path, lang, first_line(node, src));
                    if is_typedef { el.children = members.clone(); }
                    out.push(el);
                }
                Self::visit_types(node, src, path, lang, out);
            }
            "struct_specifier" | "union_specifier" | "enum_specifier" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let kind = if node.kind() == "enum_specifier" { ElementType::Enum } else { ElementType::Class };
                    let mut el = element(kind, text(&name, src), node, src, path, lang, first_line(node, src));
                    el.children = members_of(node, src, path, lang);
                    out.push(el);
                }
            }
            _ => {}
        }
    }

    /// Macros anywhere in the file, including inside function bodies. Include guards
    /// (`#define FOO_H` with no value) are skipped — they name a file, not a symbol.
    fn collect_macros(node: &Node, src: &str, path: &str, lang: &Language, out: &mut Vec<CodeElement>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(child.kind(), "preproc_def" | "preproc_function_def") {
                if let Some(name) = child.child_by_field_name("name") {
                    let name = text(&name, src);
                    let guard = child.child_by_field_name("value").is_none()
                        && (name.ends_with("_H") || name.ends_with("_H_") || name.ends_with("_HPP") || name.ends_with("_INCLUDED"));
                    if !guard {
                        out.push(element(ElementType::Macro, name, &child, src, path, lang, first_line(&child, src)));
                    }
                }
            }
            Self::collect_macros(&child, src, path, lang, out);
        }
    }

    /// A `typedef struct Foo {...} Bar;` names two things; index both.
    fn visit_types(node: &Node, src: &str, path: &str, lang: &Language, out: &mut Vec<CodeElement>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(child.kind(), "struct_specifier" | "union_specifier" | "enum_specifier") {
                if let Some(name) = child.child_by_field_name("name") {
                    let kind = if child.kind() == "enum_specifier" { ElementType::Enum } else { ElementType::Class };
                    let mut el = element(kind, text(&name, src), &child, src, path, lang, first_line(node, src));
                    el.children = members_of(&child, src, path, lang);
                    out.push(el);
                }
            }
        }
    }
}

/// A body past this many lines cannot be navigated as one unit, so its top-level
/// statement regions become anchors. A dense body qualifies too: 250 lines with 60
/// branches needs anchors as much as 400 lines with three.
const ANCHOR_MIN_LINES: usize = 300;
const ANCHOR_MIN_BRANCHES: usize = 30;
/// Regions worth navigating to. A brace alone does not qualify — `foo({...})` is an
/// argument, not a destination.
const ANCHOR_KINDS: &[&str] = &["if_statement", "for_statement", "while_statement", "switch_statement", "do_statement", "case_statement"];

/// Build the navigation layer: anchors inside oversized bodies, and every string
/// literal with the role it plays. Both are searchable evidence, neither is a symbol.
fn navigation_layer(root: &Node, src: &str, elements: &[CodeElement]) -> (Vec<CodeAnchor>, Vec<CodeLiteral>) {
    let mut anchors = Vec::new();
    let mut literals = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        collect_navigation(&child, src, elements, "", &mut anchors, &mut literals);
    }
    (anchors, literals)
}

fn collect_navigation(node: &Node, src: &str, elements: &[CodeElement], scope: &str, anchors: &mut Vec<CodeAnchor>, literals: &mut Vec<CodeLiteral>) {
    if node.kind() == "function_definition" {
        let name = declarator_name(node, src).unwrap_or_default();
        let lines = node.end_position().row.saturating_sub(node.start_position().row) + 1;
        if let Some(body) = node.child_by_field_name("body") {
            let branches = count_branches(&body);
            if lines >= ANCHOR_MIN_LINES || branches >= ANCHOR_MIN_BRANCHES {
                collect_anchors(&body, src, &name, 1, anchors);
            }
            collect_literals(&body, src, &name, literals);
        }
        let _ = elements;
        return;
    }
    collect_literals(node, src, scope, literals);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_navigation(&child, src, elements, scope, anchors, literals);
    }
}

fn count_branches(node: &Node) -> usize {
    let mut n = 0;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if ANCHOR_KINDS.contains(&child.kind()) { n += 1 }
    }
    n
}

/// Statement regions at one level. Nested regions become their own anchors so search
/// can return the smallest relevant one rather than a 5000-line branch.
fn collect_anchors(body: &Node, src: &str, parent: &str, depth: usize, out: &mut Vec<CodeAnchor>) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if !ANCHOR_KINDS.contains(&child.kind()) { continue }
        let (start, end) = (child.start_position().row + 1, child.end_position().row + 1);
        let condition_ids = child.child_by_field_name("condition")
            .map(|c| identifiers_in(&c, src)).unwrap_or_default();
        let mut anchor = CodeAnchor {
            parent_symbol: parent.to_string(),
            kind: child.kind().trim_end_matches("_statement").to_string(),
            name: condition_ids.first().cloned().unwrap_or_else(|| format!("{}@{start}", child.kind().trim_end_matches("_statement"))),
            line_start: start, line_end: end, depth,
            label: comment_above(src, start),
            condition_ids,
            calls: direct_calls(&child, src),
            declared: initialized_locals(&child, src),
            literals: literal_values(&child, src),
        };
        anchor.calls.truncate(12); anchor.declared.truncate(12); anchor.literals.truncate(8);
        out.push(anchor);
        // recurse into the region's own body for finer anchors
        if let Some(inner) = child.child_by_field_name("consequence").or_else(|| child.child_by_field_name("body")) {
            if depth < 3 { collect_anchors(&inner, src, parent, depth + 1, out); }
        }
    }
}

fn identifiers_in(node: &Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |n| {
        if n.kind() == "identifier" { let t = text(n, src); if !out.contains(&t) { out.push(t) } }
    });
    out
}

fn direct_calls(node: &Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |n| {
        if n.kind() == "call_expression" {
            if let Some(f) = n.child_by_field_name("function") {
                let t = text(&f, src);
                if t.chars().all(|c| c.is_alphanumeric() || c == '_') && !out.contains(&t) { out.push(t) }
            }
        }
    });
    out
}

/// Level 1/2 declarations only: a local with an initializer carries meaning
/// (`const int COUNTDOWN = 180`), a bare `int i;` does not.
fn initialized_locals(node: &Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |n| {
        if n.kind() == "init_declarator" {
            if let Some(name) = n.child_by_field_name("declarator").and_then(|d| leaf_identifier(&d, src)) {
                if !out.contains(&name) { out.push(name) }
            }
        }
    });
    out
}

fn literal_values(node: &Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |n| {
        if n.kind() == "string_literal" {
            let v = text(n, src).trim_matches('"').to_string();
            if v.len() >= 2 && !out.contains(&v) { out.push(v) }
        }
    });
    out
}

fn collect_literals(node: &Node, src: &str, scope: &str, out: &mut Vec<CodeLiteral>) {
    walk(node, &mut |n| {
        if n.kind() != "string_literal" { return }
        let raw = text(n, src);
        let value = raw.trim_matches('"').to_string();
        if value.is_empty() { return }
        out.push(CodeLiteral {
            kind: literal_kind(&value).to_string(),
            usage_kind: literal_usage(n, src).to_string(),
            line: n.start_position().row + 1,
            parent_symbol: scope.to_string(),
            value,
        });
    });
}

/// Classification decides ranking weight later; length is a poor proxy for usefulness.
fn literal_kind(value: &str) -> &'static str {
    if value.starts_with("--") || value.starts_with('-') && value.len() > 2 { return "cli_flag" }
    if value.contains('%') && value.len() > 2 { return "format" }
    let asset = value.contains('/') || value.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty() && (2..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())
    });
    if asset { return "asset" }
    "generic"
}

/// The role a literal plays at its use site: compared against, printed, or passed along.
fn literal_usage(node: &Node, src: &str) -> &'static str {
    let Some(call) = enclosing_call(node) else { return "generic" };
    let Some(callee) = call.child_by_field_name("function").map(|f| text(&f, src)) else { return "generic" };
    if callee.contains("cmp") { return "comparison" }
    if callee.contains("printf") || callee.contains("log") || callee.contains("fprintf") { return "printf_format" }
    "argument"
}

fn enclosing_call<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut current = node.parent();
    for _ in 0..4 {
        let n = current?;
        if n.kind() == "call_expression" { return Some(n) }
        current = n.parent();
    }
    None
}

fn walk<'a>(node: &Node<'a>, visit: &mut dyn FnMut(&Node<'a>)) {
    visit(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) { walk(&child, visit); }
}

/// Nearest comment above a line — a free, human-written label.
fn comment_above(src: &str, line: usize) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    let start = line.saturating_sub(1);
    for i in (start.saturating_sub(6)..start).rev() {
        let t = lines.get(i)?.trim();
        let body = t.trim_start_matches(['/', '*', ' ', '\t']).trim_end_matches(['*', '/']).trim();
        if (t.starts_with("/*") || t.starts_with("//") || t.starts_with('*')) && body.chars().filter(|c| c.is_alphabetic()).count() >= 8 {
            return Some(body.chars().take(90).collect());
        }
    }
    None
}

/// The struct/union specifier a declaration is built from, if any.
fn struct_of<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node.children(&mut cursor).find(|c| matches!(c.kind(), "struct_specifier" | "union_specifier"));
    found
}

/// Struct members, as child symbols of the type. `int count, cap;` names two.
fn members_of(node: &Node, src: &str, path: &str, lang: &Language) -> Vec<CodeElement> {
    let Some(body) = node.child_by_field_name("body") else { return Vec::new() };
    let mut out = Vec::new();
    let mut cursor = body.walk();
    for field in body.children(&mut cursor) {
        if field.kind() != "field_declaration" { continue }
        let mut inner = field.walk();
        for part in field.children(&mut inner) {
            if !matches!(part.kind(), "field_identifier" | "pointer_declarator" | "array_declarator") { continue }
            if let Some(name) = leaf_identifier(&part, src) {
                out.push(element(ElementType::Field, name, &field, src, path, lang, first_line(&field, src)));
            }
        }
    }
    out
}

/// Walk a declarator down to the identifier it ultimately names.
fn leaf_identifier(node: &Node, src: &str) -> Option<String> {
    let mut current = *node;
    loop {
        match current.kind() {
            "identifier" | "type_identifier" | "field_identifier" => return Some(text(&current, src)),
            _ => current = current.child_by_field_name("declarator")?,
        }
    }
}

fn text(node: &Node, src: &str) -> String {
    src[node.byte_range()].to_string()
}

/// The declaration's own first line — enough to identify it, never the whole body.
fn first_line(node: &Node, src: &str) -> Option<String> {
    Some(src[node.byte_range()].lines().next()?.trim().to_string())
}

/// Everything from `int` up to the body brace, joined — this is what a multi-line
/// signature looks like to a reader, and what the old scanner could not see.
fn signature_of(node: &Node, src: &str) -> Option<String> {
    let end = node.child_by_field_name("body").map(|b| b.start_byte()).unwrap_or(node.end_byte());
    let sig: String = src[node.start_byte()..end].split_whitespace().collect::<Vec<_>>().join(" ");
    Some(sig.trim_end_matches('{').trim().to_string())
}

/// Unwrap pointers, arrays and parameter lists down to the identifier being declared.
fn declarator_name(node: &Node, src: &str) -> Option<String> {
    let mut current = node.child_by_field_name("declarator")?;
    loop {
        match current.kind() {
            "identifier" | "type_identifier" | "field_identifier" => return Some(text(&current, src)),
            _ => current = current.child_by_field_name("declarator")?,
        }
    }
}

/// Names introduced by a `declaration` / `type_definition`: prototypes and globals keep
/// their natural kind, typedef'd names become type aliases.
fn declared_names(node: &Node, src: &str, is_typedef: bool) -> Vec<(String, ElementType)> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = match child.kind() {
            "function_declarator" => ElementType::Function,
            "identifier" | "type_identifier" | "pointer_declarator" | "array_declarator" | "init_declarator" => {
                if is_typedef { ElementType::TypeAlias } else { ElementType::Field }
            }
            _ => continue,
        };
        let name = if matches!(child.kind(), "identifier" | "type_identifier") {
            Some(text(&child, src))
        } else {
            let mut c = child;
            loop {
                match c.kind() {
                    "identifier" | "type_identifier" => break Some(text(&c, src)),
                    _ => match c.child_by_field_name("declarator") {
                        Some(next) => c = next,
                        None => break None,
                    },
                }
            }
        };
        // A typedef's function_declarator is a function-pointer type, not a function.
        let kind = if is_typedef { ElementType::TypeAlias } else { kind };
        if let Some(name) = name {
            if !name.is_empty() { out.push((name, kind)); }
        }
    }
    out
}

fn element(element_type: ElementType, name: String, node: &Node, _src: &str, file_path: &str, language: &Language, signature: Option<String>) -> CodeElement {
    CodeElement {
        element_type,
        name,
        file_path: file_path.into(),
        line_start: node.start_position().row + 1,
        line_end: node.end_position().row + 1,
        language: language.clone(),
        scope: None,
        docstring: None,
        signature,
        parameters: vec![],
        return_type: None,
        default_value: None,
        bases: vec![],
        decorators: vec![],
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
#include <stdio.h>
#define MAXOBST 64
#define SQ(x) ((x)*(x))
typedef struct { float x, y; } GpuMesh;
enum Stage { STAGE_IDLE, STAGE_RUN };
int world2_on = 0;
void phys_collect_walls(int n);
int collide_walls(float *pos, float *vel,
                  const float obst[][4],
                  int n) {
    return 1;
}
"#;

    fn names(kind: ElementType) -> Vec<String> {
        let a = CScanner::parse_file(SOURCE, "physics.c", Language::C).unwrap();
        a.elements.iter().filter(|e| e.element_type == kind).map(|e| e.name.clone()).collect()
    }

    #[test]
    fn extracts_multi_line_signatures_macros_types_and_globals() {
        // the exact shape the old line scanner missed: `)` and `{` on different lines
        let functions = names(ElementType::Function);
        assert!(functions.contains(&"collide_walls".to_string()), "got {functions:?}");
        assert!(functions.contains(&"phys_collect_walls".to_string()), "prototypes count too: {functions:?}");

        assert_eq!(names(ElementType::Macro), vec!["MAXOBST", "SQ"]);
        assert!(names(ElementType::TypeAlias).contains(&"GpuMesh".to_string()));
        assert!(names(ElementType::Enum).contains(&"Stage".to_string()));
        assert!(names(ElementType::Field).contains(&"world2_on".to_string()));

        let analysis = CScanner::parse_file(SOURCE, "physics.c", Language::C).unwrap();
        assert_eq!(analysis.imports[0].module, "stdio.h");
        let collide = analysis.elements.iter().find(|e| e.name == "collide_walls").unwrap();
        assert!(collide.signature.as_ref().unwrap().contains("const float obst[][4]"), "signature must span lines");
        assert!(collide.line_end > collide.line_start);
    }

    #[test]
    fn extracts_struct_members_onto_the_name_the_code_uses() {
        // the idiom in real C: an anonymous struct reached only through its typedef
        let src = "typedef struct {\n  N2Mesh *meshes;\n  int count, cap;\n} N2Scene;\nstruct Named { float x; };\n";
        let analysis = CScanner::parse_file(src, "nfsu2.h", Language::C).unwrap();
        let scene = analysis.elements.iter().find(|e| e.name == "N2Scene").expect("typedef indexed");
        let members: Vec<&str> = scene.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(members, vec!["meshes", "count", "cap"], "one declaration can name several members");
        assert!(scene.children.iter().all(|c| c.element_type == ElementType::Field));

        let named = analysis.elements.iter().find(|e| e.name == "Named").expect("tagged struct indexed");
        assert_eq!(named.children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["x"]);
    }
}
