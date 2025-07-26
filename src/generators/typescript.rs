use crate::parser::ast::{FdmlDocument, Entity, Action};
use crate::generators::{CodeGenerator, GeneratorConfig};
use crate::error::Result;
use std::path::Path;
use std::fs;

pub struct TypeScriptGenerator {
    config: GeneratorConfig,
}

impl TypeScriptGenerator {
    pub fn new(config: &GeneratorConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    fn generate_entity_interface(&self, entity: &Entity) -> String {
        let mut interface = format!("export interface {} {{\n", self.pascal_case(&entity.id));
        
        for field in &entity.fields {
            let field_type = self.map_type(&field.field_type);
            let optional = if field.required.unwrap_or(true) { "" } else { "?" };
            interface.push_str(&format!("  {}{}: {};\n", field.name, optional, field_type));
        }
        
        interface.push_str("}\n\n");
        interface
    }

    fn generate_api_endpoints(&self, actions: &[Action]) -> String {
        let mut endpoints = String::from("import express from 'express';\n");
        endpoints.push_str("const router = express.Router();\n\n");

        for action in actions {
            let route_name = action.id.replace("_", "-");
            let method = self.infer_http_method(&action.id);
            
            endpoints.push_str(&format!(
                "router.{}('/{}'",
                method.to_lowercase(),
                route_name
            ));

            if let Some(input) = &action.input {
                endpoints.push_str(", (req, res) => {\n");
                endpoints.push_str("  // TODO: Implement action logic\n");
                endpoints.push_str(&format!("  // Input: {}\n", input.entity.as_ref().unwrap_or(&"unknown".to_string())));
                endpoints.push_str("  res.status(501).json({ error: 'Not implemented' });\n");
                endpoints.push_str("});\n\n");
            } else {
                endpoints.push_str(", (req, res) => {\n");
                endpoints.push_str("  // TODO: Implement action logic\n");
                endpoints.push_str("  res.status(501).json({ error: 'Not implemented' });\n");
                endpoints.push_str("});\n\n");
            }
        }

        endpoints.push_str("export default router;\n");
        endpoints
    }

    fn map_type(&self, fdml_type: &str) -> &str {
        match fdml_type {
            "string" => "string",
            "int" | "integer" => "number", 
            "float" | "double" => "number",
            "bool" | "boolean" => "boolean",
            "date" => "Date",
            "datetime" => "Date",
            _ => "any"
        }
    }

    fn pascal_case(&self, s: &str) -> String {
        s.split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect()
    }

    fn infer_http_method(&self, action_id: &str) -> &str {
        if action_id.starts_with("get_") || action_id.starts_with("list_") {
            "GET"
        } else if action_id.starts_with("create_") || action_id.starts_with("add_") {
            "POST"
        } else if action_id.starts_with("update_") || action_id.starts_with("modify_") {
            "PUT"
        } else if action_id.starts_with("delete_") || action_id.starts_with("remove_") {
            "DELETE"
        } else {
            "POST"
        }
    }
}

impl CodeGenerator for TypeScriptGenerator {
    fn generate(&self, document: &FdmlDocument, output_dir: &Path) -> Result<Vec<String>> {
        let mut generated_files = Vec::new();

        // Create output directory
        fs::create_dir_all(output_dir).map_err(|e| {
            crate::error::FdmlError::generator_error(format!(
                "Failed to create output directory: {}", e
            ))
        })?;

        // Generate entity interfaces
        if !document.entities.is_empty() {
            let mut types_content = String::from("// Generated entity interfaces\n\n");
            for entity in &document.entities {
                types_content.push_str(&self.generate_entity_interface(entity));
            }

            let types_file = output_dir.join("types.ts");
            fs::write(&types_file, types_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write types file: {}", e
                ))
            })?;
            generated_files.push(types_file.to_string_lossy().to_string());
        }

        // Generate API endpoints
        if !document.actions.is_empty() {
            let api_content = self.generate_api_endpoints(&document.actions);
            let api_file = output_dir.join("routes.ts");
            fs::write(&api_file, api_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write API file: {}", e
                ))
            })?;
            generated_files.push(api_file.to_string_lossy().to_string());
        }

        // Generate package.json if it doesn't exist
        let package_json_path = output_dir.join("package.json");
        if !package_json_path.exists() {
            let package_json = r#"{
  "name": "fdml-generated-api",
  "version": "1.0.0",
  "description": "Generated API from FDML specification",
  "main": "index.js",
  "scripts": {
    "build": "tsc",
    "start": "node dist/index.js",
    "dev": "ts-node src/index.ts"
  },
  "dependencies": {
    "express": "^4.18.0"
  },
  "devDependencies": {
    "@types/express": "^4.17.0",
    "@types/node": "^18.0.0",
    "typescript": "^4.8.0",
    "ts-node": "^10.9.0"
  }
}
"#;
            fs::write(&package_json_path, package_json).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write package.json: {}", e
                ))
            })?;
            generated_files.push(package_json_path.to_string_lossy().to_string());
        }

        Ok(generated_files)
    }

    fn language(&self) -> &str {
        "typescript"
    }

    fn file_extension(&self) -> &str {
        "ts"
    }
}

impl Clone for GeneratorConfig {
    fn clone(&self) -> Self {
        Self {
            language: self.language.clone(),
            output_dir: self.output_dir.clone(),
            template_dir: self.template_dir.clone(),
            with_tests: self.with_tests,
        }
    }
}