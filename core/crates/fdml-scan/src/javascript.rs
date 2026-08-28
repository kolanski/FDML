use tree_sitter::{Node, Parser};

use anyhow::Result;
use fdml_types::scan::*;

pub struct JavaScriptScanner;

impl JavaScriptScanner {
    pub fn parse_file(source: &str, file_path: &str) -> Result<FileAnalysis> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::language())
            .map_err(|e| anyhow::anyhow!(format!("Failed to set JavaScript language: {}", e)))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse JavaScript file"))?;

        let root = tree.root_node();
        let mut elements = Vec::new();
        let mut imports = Vec::new();

        Self::walk_top_level(&root, source, file_path, &mut elements, &mut imports);

        let mut calls = Vec::new();
        Self::collect_calls(&root, source, "", &mut calls);

        Ok(FileAnalysis {
            file_path: file_path.to_string(),
            module_path: String::new(), // set by scan_project
            language: Language::JavaScript,
            elements,
            imports,
            calls,
        })
    }

    /// Recursively collect `call_expression` sites, tracking the enclosing function name.
    fn collect_calls(node: &Node, source: &str, enclosing: &str, out: &mut Vec<CallRef>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "call_expression" {
                if let Some(callee) = Self::call_callee(&child, source) {
                    out.push(CallRef {
                        caller: enclosing.to_string(),
                        callee,
                        line: child.start_position().row + 1,
                    });
                }
            }
            let next = match child.kind() {
                "function_declaration" | "method_definition" | "generator_function_declaration" => child
                    .child_by_field_name("name")
                    .map(|n| Self::node_text(&n, source).to_string())
                    .unwrap_or_else(|| enclosing.to_string()),
                _ => enclosing.to_string(),
            };
            Self::collect_calls(&child, source, &next, out);
        }
    }

    /// `foo()` -> "foo", `obj.method()` / `this.method()` -> "method" (the property).
    fn call_callee(call: &Node, source: &str) -> Option<String> {
        let func = call.child_by_field_name("function")?;
        match func.kind() {
            "identifier" => Some(Self::node_text(&func, source).to_string()),
            "member_expression" => func
                .child_by_field_name("property")
                .map(|p| Self::node_text(&p, source).to_string()),
            _ => None,
        }
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
                    if let Some(el) = Self::extract_class(&child, source, file_path, false) {
                        elements.push(el);
                    }
                }
                "function_declaration" | "generator_function_declaration" => {
                    if let Some(el) = Self::extract_function(&child, source, file_path, false) {
                        elements.push(el);
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    Self::extract_variable_functions(&child, source, file_path, false, elements);
                }
                "export_statement" => {
                    Self::extract_export(&child, source, file_path, elements, imports);
                }
                "import_statement" => {
                    if let Some(imp) = Self::extract_import(&child, source, file_path) {
                        imports.push(imp);
                    }
                }
                "expression_statement" => {
                    // Check for require() calls assigned to variables
                    if let Some(imp) = Self::extract_require(&child, source, file_path) {
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

    fn extract_class(node: &Node, source: &str, file_path: &str, exported: bool) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let scope = if exported { Scope::Public } else { Scope::Internal };

        // Extract superclass from class_heritage
        let mut bases = Vec::new();
        let mut decorators = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "class_heritage" => {
                    let mut hcursor = child.walk();
                    for hchild in child.children(&mut hcursor) {
                        let k = hchild.kind();
                        if k != "extends" && k != "," {
                            bases.push(Self::node_text(&hchild, source).to_string());
                        }
                    }
                }
                "decorator" => {
                    decorators.push(Self::node_text(&child, source).trim_start_matches('@').to_string());
                }
                _ => {}
            }
        }

        // Extract body members
        let mut children = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut bcursor = body.walk();
            for child in body.children(&mut bcursor) {
                match child.kind() {
                    "method_definition" => {
                        if let Some(method) = Self::extract_method(&child, source, file_path) {
                            children.push(method);
                        }
                    }
                    "field_definition" | "public_field_definition" => {
                        if let Some(field) = Self::extract_field(&child, source, file_path) {
                            children.push(field);
                        }
                    }
                    _ => {}
                }
            }
        }

        let docstring = Self::extract_jsdoc(node, source);

        Some(CodeElement {
            element_type: ElementType::Class,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::JavaScript,
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

    fn extract_function(node: &Node, source: &str, file_path: &str, exported: bool) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();
        let scope = if exported { Scope::Public } else { Scope::Internal };

        let parameters = Self::extract_parameters(node, source);
        let docstring = Self::extract_jsdoc(node, source);

        let is_generator = node.kind() == "generator_function_declaration";
        let params_str: Vec<String> = parameters.iter().map(|p| p.name.clone()).collect();
        let prefix = if is_generator { "function*" } else { "function" };
        let signature = format!("{} {}({})", prefix, name, params_str.join(", "));

        Some(CodeElement {
            element_type: ElementType::Function,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::JavaScript,
            scope: Some(scope),
            docstring,
            signature: Some(signature),
            parameters,
            return_type: None,
            default_value: None,
            bases: Vec::new(),
            decorators: Vec::new(),
            children: Vec::new(),
        })
    }

    fn extract_variable_functions(
        node: &Node,
        source: &str,
        file_path: &str,
        exported: bool,
        elements: &mut Vec<CodeElement>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                let name_node = match child.child_by_field_name("name") {
                    Some(n) if n.kind() == "identifier" => n,
                    _ => continue,
                };
                let name = Self::node_text(&name_node, source).to_string();

                let value_node = match child.child_by_field_name("value") {
                    Some(n) => n,
                    None => continue,
                };

                match value_node.kind() {
                    "arrow_function" | "function" => {
                        let scope = if exported { Scope::Public } else { Scope::Internal };
                        let parameters = Self::extract_parameters(&value_node, source);
                        let docstring = Self::extract_jsdoc(node, source);

                        let params_str: Vec<String> = parameters.iter().map(|p| p.name.clone()).collect();
                        let signature = if value_node.kind() == "arrow_function" {
                            format!("const {} = ({}) =>", name, params_str.join(", "))
                        } else {
                            format!("const {} = function({})", name, params_str.join(", "))
                        };

                        elements.push(CodeElement {
                            element_type: ElementType::Function,
                            name,
                            file_path: file_path.to_string(),
                            line_start: node.start_position().row + 1,
                            line_end: node.end_position().row + 1,
                            language: Language::JavaScript,
                            scope: Some(scope),
                            docstring,
                            signature: Some(signature),
                            parameters,
                            return_type: None,
                            default_value: None,
                            bases: Vec::new(),
                            decorators: Vec::new(),
                            children: Vec::new(),
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    fn extract_method(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let name_node = node.child_by_field_name("name")?;
        let name_text = Self::node_text(&name_node, source);

        // #private prefix → Private scope
        let scope = if name_text.starts_with('#') {
            Scope::Private
        } else {
            Scope::Public
        };

        let name = name_text.trim_start_matches('#').to_string();
        let parameters = Self::extract_parameters(node, source);
        let docstring = Self::extract_jsdoc(node, source);

        // Check for static, get, set keywords
        let mut is_static = false;
        let mut kind_prefix = String::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let text = Self::node_text(&child, source);
            match text {
                "static" => is_static = true,
                "get" if child.kind() != "property_identifier" => kind_prefix = "get ".to_string(),
                "set" if child.kind() != "property_identifier" => kind_prefix = "set ".to_string(),
                _ => {}
            }
        }

        let params_str: Vec<String> = parameters.iter().map(|p| p.name.clone()).collect();
        let static_str = if is_static { "static " } else { "" };
        let signature = format!("{}{}{}({})", static_str, kind_prefix, name, params_str.join(", "));

        Some(CodeElement {
            element_type: ElementType::Method,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::JavaScript,
            scope: Some(scope),
            docstring,
            signature: Some(signature),
            parameters,
            return_type: None,
            default_value: None,
            bases: Vec::new(),
            decorators: Vec::new(),
            children: Vec::new(),
        })
    }

    fn extract_field(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let prop_node = node.child_by_field_name("property")?;
        let name_text = Self::node_text(&prop_node, source);

        let scope = if name_text.starts_with('#') {
            Scope::Private
        } else {
            Scope::Public
        };

        let name = name_text.trim_start_matches('#').to_string();
        let default_value = node.child_by_field_name("value")
            .map(|n| Self::node_text(&n, source).to_string());

        Some(CodeElement {
            element_type: ElementType::Field,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::JavaScript,
            scope: Some(scope),
            docstring: None,
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            default_value,
            bases: Vec::new(),
            decorators: Vec::new(),
            children: Vec::new(),
        })
    }

    fn extract_export(
        node: &Node,
        source: &str,
        file_path: &str,
        elements: &mut Vec<CodeElement>,
        imports: &mut Vec<ImportInfo>,
    ) {
        // Check for re-exports: export { ... } from '...'
        let mut has_source = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                has_source = true;
            }
        }
        if has_source {
            // This is a re-export, treat as import
            if let Some(imp) = Self::extract_reexport_as_import(node, source, file_path) {
                imports.push(imp);
            }
            return;
        }

        // Extract exported declarations
        let mut cursor2 = node.walk();
        for child in node.children(&mut cursor2) {
            match child.kind() {
                "class_declaration" => {
                    if let Some(el) = Self::extract_class(&child, source, file_path, true) {
                        elements.push(el);
                    }
                }
                "function_declaration" | "generator_function_declaration" => {
                    if let Some(el) = Self::extract_function(&child, source, file_path, true) {
                        elements.push(el);
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    Self::extract_variable_functions(&child, source, file_path, true, elements);
                }
                _ => {}
            }
        }
    }

    fn extract_import(node: &Node, source: &str, file_path: &str) -> Option<ImportInfo> {
        // Get the source module path
        let source_node = node.child_by_field_name("source");
        let module_path = if let Some(src) = source_node {
            Self::node_text(&src, source)
                .trim_matches(|c: char| c == '\'' || c == '"')
                .to_string()
        } else {
            // Bare import: import 'module'
            let mut cursor = node.walk();
            let found = node.children(&mut cursor).find(|c| c.kind() == "string");
            match found {
                Some(s) => Self::node_text(&s, source)
                    .trim_matches(|c: char| c == '\'' || c == '"')
                    .to_string(),
                None => return None,
            }
        };

        let is_relative = module_path.starts_with('.');

        // Extract imported names
        let mut names = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "import_clause" {
                Self::extract_import_names(&child, source, &mut names);
            }
        }

        Some(ImportInfo {
            module: module_path,
            names,
            is_relative,
            file_path: file_path.to_string(),
            line: node.start_position().row + 1,
        })
    }

    fn extract_import_names(node: &Node, source: &str, names: &mut Vec<String>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    names.push(Self::node_text(&child, source).to_string());
                }
                "named_imports" => {
                    let mut ncursor = child.walk();
                    for spec in child.children(&mut ncursor) {
                        if spec.kind() == "import_specifier" {
                            if let Some(name_node) = spec.child_by_field_name("name") {
                                names.push(Self::node_text(&name_node, source).to_string());
                            }
                        }
                    }
                }
                "namespace_import" => {
                    let mut nscursor = child.walk();
                    for ns_child in child.children(&mut nscursor) {
                        if ns_child.kind() == "identifier" {
                            names.push(format!("* as {}", Self::node_text(&ns_child, source)));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn extract_reexport_as_import(node: &Node, source: &str, file_path: &str) -> Option<ImportInfo> {
        let mut module_path = String::new();
        let mut names = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                module_path = Self::node_text(&child, source)
                    .trim_matches(|c: char| c == '\'' || c == '"')
                    .to_string();
            } else if child.kind() == "export_clause" {
                let mut ecursor = child.walk();
                for spec in child.children(&mut ecursor) {
                    if spec.kind() == "export_specifier" {
                        if let Some(name_node) = spec.child_by_field_name("name") {
                            names.push(Self::node_text(&name_node, source).to_string());
                        }
                    }
                }
            }
        }

        if module_path.is_empty() {
            return None;
        }

        let is_relative = module_path.starts_with('.');
        Some(ImportInfo {
            module: module_path,
            names,
            is_relative,
            file_path: file_path.to_string(),
            line: node.start_position().row + 1,
        })
    }

    fn extract_require(node: &Node, source: &str, file_path: &str) -> Option<ImportInfo> {
        // Look for: const x = require('module')
        // expression_statement > assignment_expression or
        // This is typically in variable_declaration, not expression_statement
        // Check for standalone require('...')
        let full_text = Self::node_text(node, source);
        if !full_text.contains("require(") {
            return None;
        }

        // Extract module path from require('...')
        let start = full_text.find("require(")? + 8;
        let rest = &full_text[start..];
        let quote_char = rest.chars().next()?;
        if quote_char != '\'' && quote_char != '"' {
            return None;
        }
        let end = rest[1..].find(quote_char)?;
        let module_path = rest[1..end + 1].to_string();

        let is_relative = module_path.starts_with('.');

        Some(ImportInfo {
            module: module_path,
            names: Vec::new(),
            is_relative,
            file_path: file_path.to_string(),
            line: node.start_position().row + 1,
        })
    }

    fn extract_parameters(node: &Node, source: &str) -> Vec<Parameter> {
        let mut params = Vec::new();
        let Some(params_node) = node.child_by_field_name("parameters") else {
            return params;
        };

        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    params.push(Parameter {
                        name: Self::node_text(&child, source).to_string(),
                        type_hint: None,
                        default_value: None,
                    });
                }
                "assignment_pattern" => {
                    let left = child.child_by_field_name("left")
                        .map(|n| Self::node_text(&n, source).to_string())
                        .unwrap_or_default();
                    let right = child.child_by_field_name("right")
                        .map(|n| Self::node_text(&n, source).to_string());
                    if !left.is_empty() {
                        params.push(Parameter {
                            name: left,
                            type_hint: None,
                            default_value: right,
                        });
                    }
                }
                "rest_pattern" => {
                    params.push(Parameter {
                        name: Self::node_text(&child, source).to_string(),
                        type_hint: None,
                        default_value: None,
                    });
                }
                "object_pattern" | "array_pattern" => {
                    params.push(Parameter {
                        name: Self::node_text(&child, source).to_string(),
                        type_hint: None,
                        default_value: None,
                    });
                }
                _ => {}
            }
        }
        params
    }

    fn extract_jsdoc(node: &Node, source: &str) -> Option<String> {
        if let Some(prev) = node.prev_sibling() {
            if prev.kind() == "comment" {
                let text = Self::node_text(&prev, source);
                if text.starts_with("/**") {
                    let cleaned = text
                        .trim_start_matches("/**")
                        .trim_end_matches("*/")
                        .lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .find(|l| !l.is_empty() && !l.starts_with('@'));
                    return cleaned.map(|s| s.to_string());
                }
            }
        }
        None
    }
}
