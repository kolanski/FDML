//! C/C++ scanner built on the same tree-sitter grammar the other languages use.
//!
//! The previous hand-rolled line scanner required `{` on the same line as `)`, so every
//! multi-line signature was invisible, and it modelled no macros or types at all. Measured
//! against `ctags` on a real C codebase that cost ~60% of the symbols. A real parse gets
//! function definitions *and* prototypes, typedefs, structs, enums, macros and file-scope
//! globals — the declarations agents actually search for.
use tree_sitter::{Node, Parser};

use anyhow::Result;
use fdml_types::scan::*;

pub struct CScanner;

impl CScanner {
    pub fn parse_file(source: &str, file_path: &str) -> Result<FileAnalysis> {
        let language = Language::C;
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_c::language())
            .map_err(|e| anyhow::anyhow!("Failed to set C language: {}", e))?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse C file"))?;

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
        // A prototype and its definition are one function, not two. The definition
        // normally follows, so the last occurrence of (kind, name) wins; identity is
        // `path:name`, and two elements with the same identity would be two ids.
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::with_capacity(elements.len());
        for el in elements.into_iter().rev() {
            if seen.insert((format!("{:?}", el.element_type), el.name.clone())) { deduped.push(el); }
        }
        deduped.reverse();
        let elements = deduped;
        let mut calls = Vec::new();
        Self::collect_calls(&root, source, "", &mut calls);
        Ok(FileAnalysis { file_path: file_path.into(), module_path: String::new(), language, elements, imports, calls })
    }

    /// Every call site with the function it sits in. The graph pass turns these into
    /// function→function edges; without them a C module is a list of declarations.
    fn collect_calls(node: &Node, src: &str, enclosing: &str, out: &mut Vec<CallRef>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    let name = declarator_name(&child, src).unwrap_or_default();
                    Self::collect_calls(&child, src, &name, out);
                }
                "call_expression" => {
                    // only a plain identifier is a resolvable callee; `p->fn(x)` is not
                    if let Some(f) = child.child_by_field_name("function") {
                        if f.kind() == "identifier" {
                            out.push(CallRef { caller: enclosing.to_string(), callee: text(&f, src), line: child.start_position().row + 1 });
                        }
                    }
                    Self::collect_calls(&child, src, enclosing, out); // calls inside arguments
                }
                _ => Self::collect_calls(&child, src, enclosing, out),
            }
        }
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
        let a = CScanner::parse_file(SOURCE, "physics.c").unwrap();
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

        let analysis = CScanner::parse_file(SOURCE, "physics.c").unwrap();
        assert_eq!(analysis.imports[0].module, "stdio.h");
        let collide = analysis.elements.iter().find(|e| e.name == "collide_walls").unwrap();
        assert!(collide.signature.as_ref().unwrap().contains("const float obst[][4]"), "signature must span lines");
        assert!(collide.line_end > collide.line_start);
    }

    #[test]
    fn extracts_struct_members_onto_the_name_the_code_uses() {
        // the idiom in real C: an anonymous struct reached only through its typedef
        let src = "typedef struct {\n  N2Mesh *meshes;\n  int count, cap;\n} N2Scene;\nstruct Named { float x; };\n";
        let analysis = CScanner::parse_file(src, "nfsu2.h").unwrap();
        let scene = analysis.elements.iter().find(|e| e.name == "N2Scene").expect("typedef indexed");
        let members: Vec<&str> = scene.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(members, vec!["meshes", "count", "cap"], "one declaration can name several members");
        assert!(scene.children.iter().all(|c| c.element_type == ElementType::Field));

        let named = analysis.elements.iter().find(|e| e.name == "Named").expect("tagged struct indexed");
        assert_eq!(named.children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["x"]);
    }
}
