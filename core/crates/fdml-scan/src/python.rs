use tree_sitter::{Node, Parser};

use anyhow::Result;
use fdml_types::scan::*;

pub struct PythonScanner;

impl PythonScanner {
    pub fn parse_file(source: &str, file_path: &str) -> Result<FileAnalysis> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::language())
            .map_err(|e| anyhow::anyhow!(format!("Failed to set Python language: {}", e)))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Python file"))?;

        let root = tree.root_node();
        let mut elements = Vec::new();
        let mut imports = Vec::new();

        Self::walk_top_level(&root, source, file_path, &mut elements, &mut imports);

        let mut calls = Vec::new();
        Self::collect_calls(&root, source, "", &mut calls);

        Ok(FileAnalysis {
            file_path: file_path.to_string(),
            module_path: String::new(), // set by scan_project
            language: Language::Python,
            elements,
            imports,
            calls,
        })
    }

    /// Recursively collect call sites, tracking the enclosing function/method name.
    fn collect_calls(node: &Node, source: &str, enclosing: &str, out: &mut Vec<CallRef>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| Self::node_text(&n, source).to_string())
                        .unwrap_or_default();
                    Self::collect_calls(&child, source, &name, out);
                }
                "call" => {
                    if let Some(callee) = Self::call_callee(&child, source) {
                        out.push(CallRef {
                            caller: enclosing.to_string(),
                            callee,
                            line: child.start_position().row + 1,
                        });
                    }
                    Self::collect_calls(&child, source, enclosing, out); // nested calls in args
                }
                _ => Self::collect_calls(&child, source, enclosing, out),
            }
        }
    }

    /// The called name: `foo()` -> "foo", `obj.method()` -> "method" (last segment).
    fn call_callee(call: &Node, source: &str) -> Option<String> {
        let func = call.child_by_field_name("function")?;
        match func.kind() {
            "identifier" => Some(Self::node_text(&func, source).to_string()),
            "attribute" => func
                .child_by_field_name("attribute")
                .map(|a| Self::node_text(&a, source).to_string()),
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
                "class_definition" => {
                    if let Some(el) = Self::extract_class(&child, source, file_path) {
                        elements.push(el);
                    }
                }
                "function_definition" | "decorated_definition" => {
                    if let Some(el) = Self::extract_function(&child, source, file_path) {
                        elements.push(el);
                    }
                }
                "import_statement" => {
                    if let Some(imp) = Self::extract_import(&child, source, file_path) {
                        imports.push(imp);
                    }
                }
                "import_from_statement" => {
                    if let Some(imp) = Self::extract_import_from(&child, source, file_path) {
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

    fn extract_class(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        // Handle decorated classes
        let (class_node, decorators) = if node.kind() == "decorated_definition" {
            let mut decos = Vec::new();
            let mut actual_node = None;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "decorator" {
                    decos.push(Self::node_text(&child, source).trim_start_matches('@').to_string());
                } else if child.kind() == "class_definition" {
                    actual_node = Some(child);
                }
            }
            match actual_node {
                Some(n) => (n, decos),
                None => return None,
            }
        } else {
            (node.clone(), Vec::new())
        };

        let name_node = class_node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();

        // Extract base classes
        let mut bases = Vec::new();
        if let Some(arg_list) = class_node.child_by_field_name("superclasses") {
            let mut cursor = arg_list.walk();
            for child in arg_list.children(&mut cursor) {
                let kind = child.kind();
                if kind == "identifier" || kind == "attribute" {
                    bases.push(Self::node_text(&child, source).to_string());
                }
            }
        }

        // Extract body: methods, fields, inner classes, nested functions
        let mut children = Vec::new();
        if let Some(body) = class_node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                match child.kind() {
                    "class_definition" | "decorated_definition" if Self::is_class_def(&child) => {
                        // Nested class inside a class body
                        if let Some(nested) = Self::extract_class(&child, source, file_path) {
                            children.push(nested);
                        }
                    }
                    "function_definition" | "decorated_definition" => {
                        if let Some(method) = Self::extract_method(&child, source, file_path) {
                            // Extract fields from __init__
                            if method.name == "__init__" {
                                let fields = Self::extract_init_fields(&child, source, file_path);
                                for f in fields {
                                    children.push(f);
                                }
                            }
                            // Extract nested classes/functions inside this method
                            let nested = Self::extract_nested_defs(&child, source, file_path);
                            for n in nested {
                                children.push(n);
                            }
                            children.push(method);
                        }
                    }
                    "expression_statement" => {
                        // Class-level assignments (class attributes)
                        if let Some(field) = Self::extract_class_attribute(&child, source, file_path) {
                            children.push(field);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Extract docstring
        let docstring = Self::extract_docstring(&class_node, source);

        Some(CodeElement {
            element_type: ElementType::Class,
            name,
            file_path: file_path.to_string(),
            line_start: class_node.start_position().row + 1,
            line_end: class_node.end_position().row + 1,
            language: Language::Python,
            scope: Some(Scope::Public),
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

    fn extract_method(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        // Handle decorated methods
        let (func_node, decorators) = if node.kind() == "decorated_definition" {
            let mut decos = Vec::new();
            let mut actual_node = None;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "decorator" {
                    decos.push(Self::node_text(&child, source).trim_start_matches('@').to_string());
                } else if child.kind() == "function_definition" {
                    actual_node = Some(child);
                }
            }
            match actual_node {
                Some(n) => (n, decos),
                None => return None,
            }
        } else {
            (node.clone(), Vec::new())
        };

        let name_node = func_node.child_by_field_name("name")?;
        let name = Self::node_text(&name_node, source).to_string();

        let scope = if name.starts_with("__") && !name.ends_with("__") {
            Scope::Private
        } else if name.starts_with('_') {
            Scope::Protected
        } else {
            Scope::Public
        };

        let parameters = Self::extract_parameters(&func_node, source);
        let return_type = func_node
            .child_by_field_name("return_type")
            .map(|n| Self::node_text(&n, source).to_string());
        let docstring = Self::extract_docstring(&func_node, source);

        let params_str: Vec<String> = parameters.iter().map(|p| {
            let mut s = p.name.clone();
            if let Some(ref t) = p.type_hint {
                s.push_str(&format!(": {}", t));
            }
            s
        }).collect();
        let signature = format!("def {}({})", name, params_str.join(", "));

        Some(CodeElement {
            element_type: ElementType::Method,
            name,
            file_path: file_path.to_string(),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            language: Language::Python,
            scope: Some(scope),
            docstring,
            signature: Some(signature),
            parameters,
            return_type,
            default_value: None,
            bases: Vec::new(),
            decorators,
            children: Vec::new(),
        })
    }

    fn extract_function(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let mut el = Self::extract_method(node, source, file_path)?;
        el.element_type = ElementType::Function;
        Some(el)
    }

    fn extract_parameters(func_node: &Node, source: &str) -> Vec<Parameter> {
        let mut params = Vec::new();
        let Some(params_node) = func_node.child_by_field_name("parameters") else {
            return params;
        };
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    let name = Self::node_text(&child, source).to_string();
                    if name != "self" && name != "cls" {
                        params.push(Parameter {
                            name,
                            type_hint: None,
                            default_value: None,
                        });
                    }
                }
                "typed_parameter" | "typed_default_parameter" => {
                    let pname = child.child_by_field_name("name")
                        .map(|n| Self::node_text(&n, source).to_string())
                        .unwrap_or_default();
                    if pname == "self" || pname == "cls" {
                        continue;
                    }
                    let type_hint = child.child_by_field_name("type")
                        .map(|n| Self::node_text(&n, source).to_string());
                    let default_value = child.child_by_field_name("value")
                        .map(|n| Self::node_text(&n, source).to_string());
                    params.push(Parameter {
                        name: pname,
                        type_hint,
                        default_value,
                    });
                }
                "default_parameter" => {
                    let pname = child.child_by_field_name("name")
                        .map(|n| Self::node_text(&n, source).to_string())
                        .unwrap_or_default();
                    if pname == "self" || pname == "cls" {
                        continue;
                    }
                    let default_value = child.child_by_field_name("value")
                        .map(|n| Self::node_text(&n, source).to_string());
                    params.push(Parameter {
                        name: pname,
                        type_hint: None,
                        default_value,
                    });
                }
                _ => {}
            }
        }
        params
    }

    fn extract_init_fields(func_node: &Node, source: &str, file_path: &str) -> Vec<CodeElement> {
        let mut fields = Vec::new();

        // Get the actual function_definition node
        let func = if func_node.kind() == "decorated_definition" {
            let mut cursor = func_node.walk();
            let found = func_node.children(&mut cursor)
                .find(|c| c.kind() == "function_definition");
            match found {
                Some(f) => f,
                None => return fields,
            }
        } else {
            func_node.clone()
        };
        let Some(body) = func.child_by_field_name("body") else { return fields };

        let mut cursor = body.walk();
        for stmt in body.children(&mut cursor) {
            if stmt.kind() == "expression_statement" {
                let mut inner_cursor = stmt.walk();
                for expr in stmt.children(&mut inner_cursor) {
                    if expr.kind() == "assignment" {
                        let left = expr.child_by_field_name("left");
                        let right = expr.child_by_field_name("right");
                        if let Some(left_node) = left {
                            if left_node.kind() == "attribute" {
                                let text = Self::node_text(&left_node, source);
                                if text.starts_with("self.") {
                                    let field_name = text.strip_prefix("self.").unwrap_or(text);
                                    let (type_hint, default_val) = match right {
                                        Some(r) => {
                                            let raw = Self::node_text(&r, source).to_string();
                                            let inferred = match r.kind() {
                                                "string" => Some("str".to_string()),
                                                "integer" => Some("int".to_string()),
                                                "float" => Some("float".to_string()),
                                                "true" | "false" => Some("bool".to_string()),
                                                "list" => Some("list".to_string()),
                                                "dictionary" => Some("dict".to_string()),
                                                "none" => Some("None".to_string()),
                                                _ => None,
                                            };
                                            (inferred, Some(raw))
                                        }
                                        None => (None, None),
                                    };
                                    let scope = if field_name.starts_with("__") {
                                        Scope::Private
                                    } else if field_name.starts_with('_') {
                                        Scope::Protected
                                    } else {
                                        Scope::Public
                                    };
                                    fields.push(CodeElement {
                                        element_type: ElementType::Field,
                                        name: field_name.to_string(),
                                        file_path: file_path.to_string(),
                                        line_start: stmt.start_position().row + 1,
                                        line_end: stmt.end_position().row + 1,
                                        language: Language::Python,
                                        scope: Some(scope),
                                        docstring: None,
                                        signature: None,
                                        parameters: Vec::new(),
                                        return_type: type_hint,
                                        default_value: default_val,
                                        bases: Vec::new(),
                                        decorators: Vec::new(),
                                        children: Vec::new(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        fields
    }

    fn extract_class_attribute(node: &Node, source: &str, file_path: &str) -> Option<CodeElement> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "assignment" {
                let left = child.child_by_field_name("left")?;
                if left.kind() == "identifier" {
                    let name = Self::node_text(&left, source).to_string();
                    let (type_hint, default_val) = match child.child_by_field_name("right") {
                        Some(r) => {
                            let raw = Self::node_text(&r, source).to_string();
                            let inferred = match r.kind() {
                                "string" => Some("str".to_string()),
                                "integer" => Some("int".to_string()),
                                "float" => Some("float".to_string()),
                                "true" | "false" => Some("bool".to_string()),
                                "list" => Some("list".to_string()),
                                "dictionary" => Some("dict".to_string()),
                                "none" => Some("None".to_string()),
                                _ => None,
                            };
                            (inferred, Some(raw))
                        }
                        None => (None, None),
                    };
                    return Some(CodeElement {
                        element_type: ElementType::Field,
                        name,
                        file_path: file_path.to_string(),
                        line_start: node.start_position().row + 1,
                        line_end: node.end_position().row + 1,
                        language: Language::Python,
                        scope: Some(Scope::Public),
                        docstring: None,
                        signature: None,
                        parameters: Vec::new(),
                        return_type: type_hint,
                        default_value: default_val,
                        bases: Vec::new(),
                        decorators: Vec::new(),
                        children: Vec::new(),
                    });
                }
            }
        }
        None
    }

    /// Check if a node is a class definition (or decorated class)
    fn is_class_def(node: &Node) -> bool {
        if node.kind() == "class_definition" {
            return true;
        }
        if node.kind() == "decorated_definition" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "class_definition" {
                    return true;
                }
            }
        }
        false
    }

    /// Extract nested class/function definitions from inside a method body
    fn extract_nested_defs(func_node: &Node, source: &str, file_path: &str) -> Vec<CodeElement> {
        let mut nested = Vec::new();

        // Get the actual function_definition node
        let func = if func_node.kind() == "decorated_definition" {
            let mut cursor = func_node.walk();
            let found = func_node.children(&mut cursor)
                .find(|c| c.kind() == "function_definition");
            match found {
                Some(f) => f,
                None => return nested,
            }
        } else {
            func_node.clone()
        };

        let Some(body) = func.child_by_field_name("body") else { return nested };

        let mut cursor = body.walk();
        for stmt in body.children(&mut cursor) {
            match stmt.kind() {
                "class_definition" | "decorated_definition" if Self::is_class_def(&stmt) => {
                    if let Some(cls) = Self::extract_class(&stmt, source, file_path) {
                        nested.push(cls);
                    }
                }
                "function_definition" | "decorated_definition" => {
                    if let Some(inner_fn) = Self::extract_function(&stmt, source, file_path) {
                        nested.push(inner_fn);
                    }
                }
                _ => {}
            }
        }

        nested
    }

    fn extract_docstring(node: &Node, source: &str) -> Option<String> {
        let body = node.child_by_field_name("body")?;
        let mut cursor = body.walk();
        let first_child = body.children(&mut cursor).next()?;
        if first_child.kind() == "expression_statement" {
            let mut inner_cursor = first_child.walk();
            let expr = first_child.children(&mut inner_cursor).next()?;
            if expr.kind() == "string" {
                let text = Self::node_text(&expr, source);
                // Strip triple quotes
                let trimmed = text
                    .trim_start_matches("\"\"\"")
                    .trim_start_matches("'''")
                    .trim_end_matches("\"\"\"")
                    .trim_end_matches("'''")
                    .trim();
                // Take first line for brevity
                let first_line = trimmed.lines().next().unwrap_or("").trim();
                if !first_line.is_empty() {
                    return Some(first_line.to_string());
                }
            }
        }
        None
    }

    fn extract_import(node: &Node, source: &str, file_path: &str) -> Option<ImportInfo> {
        let mut names = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "dotted_name" {
                names.push(Self::node_text(&child, source).to_string());
            } else if child.kind() == "aliased_import" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    names.push(Self::node_text(&name_node, source).to_string());
                }
            }
        }
        if names.is_empty() {
            return None;
        }
        let module = names[0].clone();
        Some(ImportInfo {
            module,
            names,
            is_relative: false,
            file_path: file_path.to_string(),
            line: node.start_position().row + 1,
        })
    }

    fn extract_import_from(node: &Node, source: &str, file_path: &str) -> Option<ImportInfo> {
        // Reliable approach: parse from the text directly
        // "from .errors import RateLimitExceeded"
        // "from limits import RateLimitItem"
        // "from typing import (\n    Any,\n    Callable,\n)"
        let full_text = Self::node_text(node, source);

        let mut module = String::new();
        let mut names = Vec::new();
        let mut is_relative = false;

        // Extract module: everything between "from" and "import"
        if let Some(from_part) = full_text.strip_prefix("from") {
            if let Some(mod_part) = from_part.split("import").next() {
                module = mod_part.trim().to_string();
                is_relative = module.starts_with('.');
            }
        }

        // Extract names: everything after "import"
        if let Some(import_part) = full_text.split("import").nth(1) {
            let cleaned = import_part
                .replace('(', "")
                .replace(')', "");
            for name in cleaned.split(',') {
                let clean = name.trim()
                    .split(" as ").next().unwrap_or("")
                    .split('\n').next().unwrap_or("")
                    .split('#').next().unwrap_or("")
                    .trim();
                if !clean.is_empty() {
                    names.push(clean.to_string());
                }
            }
        }

        Some(ImportInfo {
            module,
            names,
            is_relative,
            file_path: file_path.to_string(),
            line: node.start_position().row + 1,
        })
    }
}
