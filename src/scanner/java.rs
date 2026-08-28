use tree_sitter::{Node, Parser};

use crate::error::Result;
use super::types::*;

pub struct JavaScanner;

impl JavaScanner {
    pub fn parse_file(source: &str, file_path: &str) -> Result<FileAnalysis> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_java::language())
            .map_err(|e| crate::error::FdmlError::simple_parser_error(format!("Failed to set Java language: {}", e)))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| crate::error::FdmlError::simple_parser_error("Failed to parse Java file"))?;

        let root = tree.root_node();
        let mut elements = Vec::new();
        let mut imports = Vec::new();

        Self::walk_top_level(&root, source, file_path, &mut elements, &mut imports);

        Ok(FileAnalysis {
            file_path: file_path.to_string(),
            module_path: String::new(), // set by scan_project
            language: Language::Java,
            elements,
            imports,
            anchors: Vec::new(),
            literals: Vec::new(),
        })
    }

    fn walk_top_level(
        node: &Node,
        source: &str,
        file_path: &str,
        elements: &mut Vec<CodeElement>,
        imports: &mut Vec<ImportInfo>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "class_declaration" => {
                    if let Some(el) = Self::extract_class(&child, source, file_path) {
                        elements.push(el);
                    }
                }
                "interface_declaration" => {
                    if let Some(el) = Self::extract_interface(&child, source, file_path) {
                        elements.push(el);
                    }
                }
                "enum_declaration" => {
                    if let Some(el) = Self::extract_enum(&child, source, file_path) {
                        elements.push(el);
                    }
                }
                "import_declaration" => {
                    if let Some(imp) = Self::extract_import(&child, source, file_path) {
                        imports.push(imp);
                    }
                }
                _ => {}
            }
        }
    }

    fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
        &source[node.byte_range()]
    }

    fn extract_modifiers(node: &Node, source: &str) -> (Scope, Vec<String>) {
        let mut scope = Scope::Internal; // Java default: package-private
        let mut decorators = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for modifier in child.children(&mut mod_cursor) {
                    match modifier.kind() {
                        "public" => scope = Scope::Public,
                        "private" => scope = Scope::Private,
                        "protected" => scope = Scope::Protected,
                        "marker_annotation" | "annotation" => {
                            decorators.push(Self::node_text(&modifier, source).to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
        (scope, decorators)
    }

    fn extract_class(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let (scope, decorators) = Self::extract_modifiers(node, source);

        // Extract superclass
        let mut bases = Vec::new();
        if let Some(superclass) = node.child_by_field_name("superclass") {
            bases.push(Self::node_text(&superclass, source).to_string());
        }

        // Extract interfaces
        if let Some(interfaces) = node.child_by_field_name("interfaces") {
            let mut cursor = interfaces.walk();
            for child in interfaces.children(&mut cursor) {
                if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                    bases.push(Self::node_text(&child, source).to_string());
                }
            }
        }

        // Extract body
        let mut children = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                match child.kind() {
                    "method_declaration" | "constructor_declaration" => {
                        if let Some(method) = Self::extract_method(&child, source, file_path) {
                            children.push(method);
                        }
                    }
                    "field_declaration" => {
                        if let Some(field) = Self::extract_field(&child, source, file_path) {
                            children.push(field);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Extract Javadoc
        let docstring = Self::extract_javadoc(node, source);

        Some(CodeElement {
            element_type: ElementType::Class,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Java,
            scope: Some(scope),
            docstring,
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            default_value: None,
            bases,
            decorators,
            children,
        })
    }

    fn extract_interface(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let (scope, decorators) = Self::extract_modifiers(node, source);

        let mut children = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "method_declaration" {
                    if let Some(method) = Self::extract_method(&child, source, file_path) {
                        children.push(method);
                    }
                }
            }
        }

        Some(CodeElement {
            element_type: ElementType::Interface,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Java,
            scope: Some(scope),
            docstring: Self::extract_javadoc(node, source),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            default_value: None,
            bases: Vec::new(),
            decorators,
            children,
        })
    }

    fn extract_enum(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let (scope, decorators) = Self::extract_modifiers(node, source);

        Some(CodeElement {
            element_type: ElementType::Enum,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Java,
            scope: Some(scope),
            docstring: Self::extract_javadoc(node, source),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            default_value: None,
            bases: Vec::new(),
            decorators,
            children: Vec::new(),
        })
    }

    fn extract_method(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let (scope, decorators) = Self::extract_modifiers(node, source);

        let return_type = node.child_by_field_name("type")
            .map(|n| Self::node_text(&n, source).to_string());

        let parameters = Self::extract_parameters(node, source);

        let params_str: Vec<String> = parameters.iter().map(|p| {
            match &p.type_hint {
                Some(t) => format!("{} {}", t, p.name),
                None => p.name.clone(),
            }
        }).collect();
        let ret = return_type.as_deref().unwrap_or("void");
        let signature = format!("{} {}({})", ret, name, params_str.join(", "));

        Some(CodeElement {
            element_type: ElementType::Method,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Java,
            scope: Some(scope),
            docstring: Self::extract_javadoc(node, source),
            signature: Some(signature),
            parameters,
            return_type,
            default_value: None,
            bases: Vec::new(),
            decorators,
            children: Vec::new(),
        })
    }

    fn extract_field(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let (scope, decorators) = Self::extract_modifiers(node, source);

        let type_node = node.child_by_field_name("type")?;
        let type_name = Self::node_text(&type_node, source).to_string();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                let name_node = child.child_by_field_name("name")?;
                let name = Self::node_text(&name_node, source).to_string();
                return Some(CodeElement {
                    element_type: ElementType::Field,
                    name,
                    file_path: file_path.to_string(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    language: Language::Java,
                    scope: Some(scope),
                    docstring: None,
                    signature: None,
                    parameters: Vec::new(),
                    return_type: Some(type_name),
                    default_value: None,
                    bases: Vec::new(),
                    decorators,
                    children: Vec::new(),
                });
            }
        }
        None
    }

    fn extract_parameters(node: &Node, source: &str) -> Vec<Parameter> {
        let mut params = Vec::new();
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
                    let type_hint = child.child_by_field_name("type")
                        .map(|n| Self::node_text(&n, source).to_string());
                    let name = child.child_by_field_name("name")
                        .map(|n| Self::node_text(&n, source).to_string())
                        .unwrap_or_default();
                    if !name.is_empty() {
                        params.push(Parameter {
                            name,
                            type_hint,
                            default_value: None,
                        });
                    }
                }
            }
        }
        params
    }

    fn extract_import(node: &Node, source: &str, file_path: &str) -> Option<ImportInfo> {
        let full_text = Self::node_text(node, source).to_string();
        let module = full_text
            .trim_start_matches("import")
            .trim_end_matches(';')
            .trim()
            .to_string();

        let parts: Vec<&str> = module.rsplitn(2, '.').collect();
        let (names, mod_path) = if parts.len() == 2 {
            (vec![parts[0].to_string()], parts[1].to_string())
        } else {
            (vec![module.clone()], module.clone())
        };

        Some(ImportInfo {
            module: mod_path,
            names,
            is_relative: false,
            file_path: file_path.to_string(),
            line: node.start_position().row + 1,
        })
    }

    fn extract_javadoc(node: &Node, source: &str) -> Option<String> {
        // Look for block_comment or line_comment before this node
        if let Some(prev) = node.prev_sibling() {
            if prev.kind() == "block_comment" {
                let text = Self::node_text(&prev, source);
                if text.starts_with("/**") {
                    let cleaned = text
                        .trim_start_matches("/**")
                        .trim_end_matches("*/")
                        .lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .find(|l| !l.is_empty());
                    return cleaned.map(|s| s.to_string());
                }
            }
        }
        None
    }
}
