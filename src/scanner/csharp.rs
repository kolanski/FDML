use tree_sitter::{Node, Parser};

use crate::error::Result;
use super::types::*;

pub struct CSharpScanner;

impl CSharpScanner {
    pub fn parse_file(source: &str, file_path: &str) -> Result<FileAnalysis> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_c_sharp::language())
            .map_err(|e| crate::error::FdmlError::simple_parser_error(format!("Failed to set C# language: {}", e)))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| crate::error::FdmlError::simple_parser_error("Failed to parse C# file"))?;

        let root = tree.root_node();
        let mut elements = Vec::new();
        let mut imports = Vec::new();

        Self::walk_node(&root, source, file_path, &mut elements, &mut imports);

        Ok(FileAnalysis {
            file_path: file_path.to_string(),
            language: Language::CSharp,
            elements,
            imports,
        })
    }

    fn walk_node(
        node: &Node,
        source: &str,
        file_path: &str,
        elements: &mut Vec<CodeElement>,
        imports: &mut Vec<ImportInfo>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "class_declaration" | "record_declaration" => {
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
                "using_directive" => {
                    if let Some(imp) = Self::extract_using(&child, source, file_path) {
                        imports.push(imp);
                    }
                }
                "namespace_declaration" | "file_scoped_namespace_declaration" => {
                    // Recurse into namespaces
                    Self::walk_node(&child, source, file_path, elements, imports);
                }
                _ => {
                    // Recurse for nested declarations
                    if child.child_count() > 0 {
                        Self::walk_node(&child, source, file_path, elements, imports);
                    }
                }
            }
        }
    }

    fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
        &source[node.byte_range()]
    }

    fn extract_modifiers(node: &Node, source: &str) -> (Scope, Vec<String>) {
        let mut scope = Scope::Private; // C# default
        let mut decorators = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "modifier" => {
                    match Self::node_text(&child, source) {
                        "public" => scope = Scope::Public,
                        "private" => scope = Scope::Private,
                        "protected" => scope = Scope::Protected,
                        "internal" => scope = Scope::Internal,
                        _ => {}
                    }
                }
                "attribute_list" => {
                    decorators.push(Self::node_text(&child, source).to_string());
                }
                _ => {}
            }
        }
        (scope, decorators)
    }

    fn extract_class(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let (scope, decorators) = Self::extract_modifiers(node, source);

        // Extract bases
        let mut bases = Vec::new();
        if let Some(base_list) = node.child_by_field_name("bases") {
            let mut cursor = base_list.walk();
            for child in base_list.children(&mut cursor) {
                let kind = child.kind();
                if kind == "identifier" || kind == "generic_name" || kind == "qualified_name" {
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
                        for field in Self::extract_fields(&child, source, file_path) {
                            children.push(field);
                        }
                    }
                    "property_declaration" => {
                        if let Some(prop) = Self::extract_property(&child, source, file_path) {
                            children.push(prop);
                        }
                    }
                    _ => {}
                }
            }
        }

        Some(CodeElement {
            element_type: ElementType::Class,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::CSharp,
            scope: Some(scope),
            docstring: Self::extract_xml_doc(node, source),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
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
            language: Language::CSharp,
            scope: Some(scope),
            docstring: Self::extract_xml_doc(node, source),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
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
            language: Language::CSharp,
            scope: Some(scope),
            docstring: Self::extract_xml_doc(node, source),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
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
            language: Language::CSharp,
            scope: Some(scope),
            docstring: Self::extract_xml_doc(node, source),
            signature: Some(signature),
            parameters,
            return_type,
            bases: Vec::new(),
            decorators,
            children: Vec::new(),
        })
    }

    fn extract_fields(node: &Node, source: &str, file_path: &str) -> Vec<CodeElement> {
        let mut fields = Vec::new();
        let (scope, decorators) = Self::extract_modifiers(node, source);

        let type_hint = node.child_by_field_name("type")
            .map(|n| Self::node_text(&n, source).to_string());

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declaration" {
                let mut var_cursor = child.walk();
                for var_child in child.children(&mut var_cursor) {
                    if var_child.kind() == "variable_declarator" {
                        if let Some(name_node) = var_child.child_by_field_name("name") {
                            fields.push(CodeElement {
                                element_type: ElementType::Field,
                                name: Self::node_text(&name_node, source).to_string(),
                                file_path: file_path.to_string(),
                                line_start: node.start_position().row + 1,
                                line_end: node.end_position().row + 1,
                                language: Language::CSharp,
                                scope: Some(scope.clone()),
                                docstring: None,
                                signature: None,
                                parameters: Vec::new(),
                                return_type: type_hint.clone(),
                                bases: Vec::new(),
                                decorators: decorators.clone(),
                                children: Vec::new(),
                            });
                        }
                    }
                }
            }
        }
        fields
    }

    fn extract_property(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let (scope, decorators) = Self::extract_modifiers(node, source);

        let type_hint = node.child_by_field_name("type")
            .map(|n| Self::node_text(&n, source).to_string());

        Some(CodeElement {
            element_type: ElementType::Property,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::CSharp,
            scope: Some(scope),
            docstring: Self::extract_xml_doc(node, source),
            signature: None,
            parameters: Vec::new(),
            return_type: type_hint,
            bases: Vec::new(),
            decorators,
            children: Vec::new(),
        })
    }

    fn extract_parameters(node: &Node, source: &str) -> Vec<Parameter> {
        let mut params = Vec::new();
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "parameter" {
                    let type_hint = child.child_by_field_name("type")
                        .map(|n| Self::node_text(&n, source).to_string());
                    let name = child.child_by_field_name("name")
                        .map(|n| Self::node_text(&n, source).to_string())
                        .unwrap_or_default();
                    let default_value = child.child_by_field_name("default_value")
                        .map(|n| Self::node_text(&n, source).to_string());
                    if !name.is_empty() {
                        params.push(Parameter {
                            name,
                            type_hint,
                            default_value,
                        });
                    }
                }
            }
        }
        params
    }

    fn extract_using(node: &Node, source: &str, file_path: &str) -> Option<ImportInfo> {
        let full_text = Self::node_text(node, source).to_string();
        let module = full_text
            .trim_start_matches("using")
            .trim_end_matches(';')
            .trim()
            .to_string();

        Some(ImportInfo {
            module: module.clone(),
            names: vec![module],
            is_relative: false,
            file_path: file_path.to_string(),
            line: node.start_position().row + 1,
        })
    }

    fn extract_xml_doc(node: &Node, source: &str) -> Option<String> {
        if let Some(prev) = node.prev_sibling() {
            let text = Self::node_text(&prev, source);
            if text.contains("///") {
                let summary = text.lines()
                    .filter_map(|l| {
                        let trimmed = l.trim().trim_start_matches("///").trim();
                        if trimmed.starts_with("<summary>") || trimmed.starts_with("</summary>") {
                            None
                        } else if !trimmed.is_empty() && !trimmed.starts_with('<') {
                            Some(trimmed.to_string())
                        } else {
                            None
                        }
                    })
                    .next();
                return summary;
            }
        }
        None
    }
}
