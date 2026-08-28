use tree_sitter::{Node, Parser};

use crate::error::Result;
use super::types::*;

pub struct GoScanner;

impl GoScanner {
    pub fn parse_file(source: &str, file_path: &str) -> Result<FileAnalysis> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_go::language())
            .map_err(|e| crate::error::FdmlError::simple_parser_error(format!("Failed to set Go language: {}", e)))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| crate::error::FdmlError::simple_parser_error("Failed to parse Go file"))?;

        let root = tree.root_node();
        let mut elements = Vec::new();
        let mut imports = Vec::new();
        // Collect methods separately so we can attach them to their receiver type
        let mut methods: Vec<(String, CodeElement)> = Vec::new();

        Self::walk_top_level(&root, source, file_path, &mut elements, &mut imports, &mut methods);

        // Attach methods to their receiver types
        for (receiver, method) in methods {
            let found = elements.iter_mut().find(|e| {
                e.name == receiver && matches!(e.element_type, ElementType::Class | ElementType::Interface)
            });
            if let Some(struct_elem) = found {
                struct_elem.children.push(method);
            } else {
                // No matching struct in this file — emit as top-level
                elements.push(method);
            }
        }

        Ok(FileAnalysis {
            file_path: file_path.to_string(),
            module_path: String::new(), // set by scan_project
            language: Language::Go,
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
        methods: &mut Vec<(String, CodeElement)>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_declaration" => {
                    if let Some(el) = Self::extract_function(&child, source, file_path) {
                        elements.push(el);
                    }
                }
                "method_declaration" => {
                    if let Some((receiver, method)) = Self::extract_method(&child, source, file_path) {
                        methods.push((receiver, method));
                    }
                }
                "type_declaration" => {
                    Self::extract_type_decls(&child, source, file_path, elements);
                }
                "import_declaration" => {
                    Self::extract_imports(&child, source, file_path, imports);
                }
                "const_declaration" => {
                    Self::extract_const_decl(&child, source, file_path, elements);
                }
                "var_declaration" => {
                    Self::extract_var_decl(&child, source, file_path, elements);
                }
                _ => {}
            }
        }
    }

    fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
        &source[node.byte_range()]
    }

    fn scope_from_name(name: &str) -> Scope {
        if name.starts_with(|c: char| c.is_uppercase()) {
            Scope::Public
        } else {
            Scope::Internal
        }
    }

    // --- Functions ---

    fn extract_function(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let scope = Self::scope_from_name(&name);

        let parameters = Self::extract_parameters(node, source);
        let return_type = Self::extract_result_type(node, source);
        let docstring = Self::extract_doc_comment(node, source);

        let params_str = Self::format_params(&parameters);
        let ret_str = return_type.as_ref().map(|r| format!(" {}", r)).unwrap_or_default();
        let signature = format!("func {}({}){}", name, params_str, ret_str);

        Some(CodeElement {
            element_type: ElementType::Function,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Go,
            scope: Some(scope),
            docstring,
            signature: Some(signature),
            parameters,
            return_type,
            default_value: None,
            bases: Vec::new(),
            decorators: Vec::new(),
            children: Vec::new(),
        })
    }

    // --- Methods ---

    fn extract_method(node: &Node, source: &str, file_path: &str) -> Option<(String, CodeElement)> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let scope = Self::scope_from_name(&name);

        let receiver_type = Self::extract_receiver_type(node, source)?;
        let receiver_text = Self::extract_receiver_text(node, source);

        let parameters = Self::extract_parameters(node, source);
        let return_type = Self::extract_result_type(node, source);
        let docstring = Self::extract_doc_comment(node, source);

        let params_str = Self::format_params(&parameters);
        let ret_str = return_type.as_ref().map(|r| format!(" {}", r)).unwrap_or_default();
        let signature = format!("func ({}) {}({}){}", receiver_text, name, params_str, ret_str);

        let method = CodeElement {
            element_type: ElementType::Method,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Go,
            scope: Some(scope),
            docstring,
            signature: Some(signature),
            parameters,
            return_type,
            default_value: None,
            bases: Vec::new(),
            decorators: Vec::new(),
            children: Vec::new(),
        };

        Some((receiver_type, method))
    }

    fn extract_receiver_type(node: &Node, source: &str) -> Option<String> {
        let receiver = node.child_by_field_name("receiver")?;
        let mut cursor = receiver.walk();
        for child in receiver.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                let type_node = child.child_by_field_name("type")?;
                return Some(Self::unwrap_pointer_type(&type_node, source));
            }
        }
        None
    }

    fn extract_receiver_text(node: &Node, source: &str) -> String {
        node.child_by_field_name("receiver")
            .map(|r| {
                let text = Self::node_text(&r, source);
                // Strip outer parens
                text.trim_start_matches('(').trim_end_matches(')').trim().to_string()
            })
            .unwrap_or_default()
    }

    /// Unwrap *Type to Type, keep Type as-is
    fn unwrap_pointer_type(type_node: &Node, source: &str) -> String {
        if type_node.kind() == "pointer_type" {
            let mut cursor = type_node.walk();
            for child in type_node.children(&mut cursor) {
                if child.kind() == "type_identifier" {
                    return Self::node_text(&child, source).to_string();
                }
            }
        }
        Self::node_text(type_node, source).to_string()
    }

    // --- Type declarations ---

    fn extract_type_decls(node: &Node, source: &str, file_path: &str, elements: &mut Vec<CodeElement>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                if let Some(el) = Self::extract_type_spec(&child, source, file_path) {
                    elements.push(el);
                }
            }
        }
    }

    fn extract_type_spec(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let scope = Self::scope_from_name(&name);
        let type_node = node.child_by_field_name("type")?;
        let docstring = Self::extract_doc_comment(node, source);

        match type_node.kind() {
            "struct_type" => {
                let (fields, bases) = Self::extract_struct_body(&type_node, source, file_path);
                Some(CodeElement {
                    element_type: ElementType::Class,
                    name,
                    file_path: file_path.to_string(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    language: Language::Go,
                    scope: Some(scope),
                    docstring,
                    signature: None,
                    parameters: Vec::new(),
                    return_type: None,
                    default_value: None,
                    bases,
                    decorators: Vec::new(),
                    children: fields,
                })
            }
            "interface_type" => {
                let (methods, embedded) = Self::extract_interface_body(&type_node, source, file_path);
                Some(CodeElement {
                    element_type: ElementType::Interface,
                    name,
                    file_path: file_path.to_string(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    language: Language::Go,
                    scope: Some(scope),
                    docstring,
                    signature: None,
                    parameters: Vec::new(),
                    return_type: None,
                    default_value: None,
                    bases: embedded,
                    decorators: Vec::new(),
                    children: methods,
                })
            }
            _ => {
                // Type alias: type Status int
                let aliased = Self::node_text(&type_node, source).to_string();
                let signature = format!("type {}", name);
                Some(CodeElement {
                    element_type: ElementType::Field,
                    name,
                    file_path: file_path.to_string(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    language: Language::Go,
                    scope: Some(scope),
                    docstring,
                    signature: Some(signature),
                    parameters: Vec::new(),
                    return_type: Some(aliased),
                    default_value: None,
                    bases: Vec::new(),
                    decorators: Vec::new(),
                    children: Vec::new(),
                })
            }
        }
    }

    // --- Struct body ---

    fn extract_struct_body(node: &Node, source: &str, file_path: &str) -> (Vec<CodeElement>, Vec<String>) {
        let mut fields = Vec::new();
        let mut bases = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "field_declaration_list" {
                let mut fcursor = child.walk();
                for field_node in child.children(&mut fcursor) {
                    if field_node.kind() == "field_declaration" {
                        // Check if this is a named field or embedded field
                        let name_node = field_node.child_by_field_name("name");
                        let type_node = field_node.child_by_field_name("type");

                        if let Some(name_n) = name_node {
                            // Named field
                            let fname = Self::node_text(&name_n, source).to_string();
                            let fscope = Self::scope_from_name(&fname);
                            let type_hint = type_node.map(|t| Self::node_text(&t, source).to_string());

                            // Extract struct tag if present
                            let tag = field_node.child_by_field_name("tag")
                                .map(|t| Self::node_text(&t, source).to_string());

                            fields.push(CodeElement {
                                element_type: ElementType::Field,
                                name: fname,
                                file_path: file_path.to_string(),
                                line_start: field_node.start_position().row + 1,
                                line_end: field_node.end_position().row + 1,
                                language: Language::Go,
                                scope: Some(fscope),
                                docstring: tag,
                                signature: None,
                                parameters: Vec::new(),
                                return_type: type_hint,
                                default_value: None,
                                bases: Vec::new(),
                                decorators: Vec::new(),
                                children: Vec::new(),
                            });
                        } else if let Some(type_n) = type_node {
                            // Embedded field (composition) — treat as base
                            let embedded_type = Self::unwrap_pointer_type(&type_n, source);
                            bases.push(embedded_type);
                        }
                    }
                }
            }
        }

        (fields, bases)
    }

    // --- Interface body ---

    fn extract_interface_body(node: &Node, source: &str, file_path: &str) -> (Vec<CodeElement>, Vec<String>) {
        let mut methods = Vec::new();
        let mut embedded = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // Could be method_spec_list or direct children depending on grammar version
            if child.kind() == "method_spec_list" || child.kind() == "method_elem" || child.kind() == "interface_type" {
                let mut mcursor = child.walk();
                for mchild in child.children(&mut mcursor) {
                    Self::process_interface_member(&mchild, source, file_path, &mut methods, &mut embedded);
                }
            } else {
                Self::process_interface_member(&child, source, file_path, &mut methods, &mut embedded);
            }
        }

        (methods, embedded)
    }

    fn process_interface_member(
        node: &Node,
        source: &str,
        file_path: &str,
        methods: &mut Vec<CodeElement>,
        embedded: &mut Vec<String>,
    ) {
        match node.kind() {
            "method_spec" | "method_elem" => {
                if let Some(method) = Self::extract_interface_method(node, source, file_path) {
                    methods.push(method);
                }
            }
            "type_identifier" | "qualified_type" => {
                // Embedded interface
                embedded.push(Self::node_text(node, source).to_string());
            }
            "struct_elem" => {
                // In newer grammars, struct_elem wraps embedded types
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_identifier" || child.kind() == "qualified_type" {
                        embedded.push(Self::node_text(&child, source).to_string());
                    }
                }
            }
            _ => {}
        }
    }

    fn extract_interface_method(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let scope = Self::scope_from_name(&name);

        let parameters = Self::extract_parameters(node, source);
        let return_type = Self::extract_result_type(node, source);

        let params_str = Self::format_params(&parameters);
        let ret_str = return_type.as_ref().map(|r| format!(" {}", r)).unwrap_or_default();
        let signature = format!("{}({}){}", name, params_str, ret_str);

        Some(CodeElement {
            element_type: ElementType::Method,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Go,
            scope: Some(scope),
            docstring: None,
            signature: Some(signature),
            parameters,
            return_type,
            default_value: None,
            bases: Vec::new(),
            decorators: Vec::new(),
            children: Vec::new(),
        })
    }

    // --- Const / Var declarations ---

    fn extract_const_decl(node: &Node, source: &str, file_path: &str, elements: &mut Vec<CodeElement>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "const_spec" {
                if let Some(el) = Self::extract_const_or_var_spec(&child, source, file_path) {
                    elements.push(el);
                }
            }
        }
    }

    fn extract_var_decl(node: &Node, source: &str, file_path: &str, elements: &mut Vec<CodeElement>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "var_spec" {
                if let Some(el) = Self::extract_const_or_var_spec(&child, source, file_path) {
                    elements.push(el);
                }
            }
        }
    }

    fn extract_const_or_var_spec(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let scope = Self::scope_from_name(&name);

        let type_hint = node.child_by_field_name("type")
            .map(|t| Self::node_text(&t, source).to_string());
        let default_value = node.child_by_field_name("value")
            .map(|v| Self::node_text(&v, source).to_string());

        Some(CodeElement {
            element_type: ElementType::Field,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Go,
            scope: Some(scope),
            docstring: None,
            signature: None,
            parameters: Vec::new(),
            return_type: type_hint,
            default_value,
            bases: Vec::new(),
            decorators: Vec::new(),
            children: Vec::new(),
        })
    }

    // --- Imports ---

    fn extract_imports(node: &Node, source: &str, file_path: &str, imports: &mut Vec<ImportInfo>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "import_spec" => {
                    if let Some(imp) = Self::extract_import_spec(&child, source, file_path) {
                        imports.push(imp);
                    }
                }
                "import_spec_list" => {
                    let mut scursor = child.walk();
                    for spec in child.children(&mut scursor) {
                        if spec.kind() == "import_spec" {
                            if let Some(imp) = Self::extract_import_spec(&spec, source, file_path) {
                                imports.push(imp);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn extract_import_spec(node: &Node, source: &str, file_path: &str) -> Option<ImportInfo> {
        let path_node = node.child_by_field_name("path")?;
        let raw_path = Self::node_text(&path_node, source);
        let module_path = raw_path.trim_matches('"').to_string();

        // Alias or default package name (last segment)
        let alias = node.child_by_field_name("name")
            .map(|n| Self::node_text(&n, source).to_string());
        let pkg_name = alias.unwrap_or_else(|| {
            module_path.rsplit('/').next().unwrap_or(&module_path).to_string()
        });

        Some(ImportInfo {
            module: module_path,
            names: vec![pkg_name],
            is_relative: false, // Go has no relative imports
            file_path: file_path.to_string(),
            line: node.start_position().row + 1,
        })
    }

    // --- Parameters ---

    fn extract_parameters(node: &Node, source: &str) -> Vec<Parameter> {
        let mut params = Vec::new();
        let Some(params_node) = node.child_by_field_name("parameters") else {
            return params;
        };

        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter_declaration" || child.kind() == "variadic_parameter_declaration" {
                let type_hint = child.child_by_field_name("type")
                    .map(|t| Self::node_text(&t, source).to_string());

                // Go allows multiple names per parameter declaration: a, b int
                let mut found_name = false;
                let mut pcursor = child.walk();
                for pchild in child.children(&mut pcursor) {
                    if pchild.kind() == "identifier" {
                        let pname = Self::node_text(&pchild, source).to_string();
                        params.push(Parameter {
                            name: pname,
                            type_hint: type_hint.clone(),
                            default_value: None,
                        });
                        found_name = true;
                    }
                }

                // Unnamed parameter (just a type): func foo(int, string)
                if !found_name {
                    if let Some(ref t) = type_hint {
                        params.push(Parameter {
                            name: String::new(),
                            type_hint: Some(t.clone()),
                            default_value: None,
                        });
                    }
                }
            }
        }
        params
    }

    fn extract_result_type(node: &Node, source: &str) -> Option<String> {
        let result = node.child_by_field_name("result")?;
        Some(Self::node_text(&result, source).to_string())
    }

    // --- Helpers ---

    fn format_params(parameters: &[Parameter]) -> String {
        parameters.iter().map(|p| {
            if p.name.is_empty() {
                p.type_hint.clone().unwrap_or_default()
            } else {
                match &p.type_hint {
                    Some(t) => format!("{} {}", p.name, t),
                    None => p.name.clone(),
                }
            }
        }).collect::<Vec<_>>().join(", ")
    }

    fn extract_doc_comment(node: &Node, source: &str) -> Option<String> {
        // Go doc comments are // comments immediately before the declaration
        if let Some(prev) = node.prev_sibling() {
            if prev.kind() == "comment" {
                let text = Self::node_text(&prev, source);
                let trimmed = text.trim_start_matches("//").trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }
}
