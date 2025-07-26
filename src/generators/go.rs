use crate::parser::ast::{FdmlDocument, Entity, Action};
use crate::generators::{CodeGenerator, GeneratorConfig};
use crate::error::Result;
use std::path::Path;
use std::fs;

pub struct GoGenerator {
    config: GeneratorConfig,
}

impl GoGenerator {
    pub fn new(config: &GeneratorConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    fn generate_struct(&self, entity: &Entity) -> String {
        let struct_name = self.pascal_case(&entity.id);
        let mut struct_def = format!("type {} struct {{\n", struct_name);
        
        for field in &entity.fields {
            let field_type = self.map_type(&field.field_type);
            let field_name = self.pascal_case(&field.name);
            let json_tag = format!("`json:\"{}\"`", field.name);
            
            struct_def.push_str(&format!("    {} {} {}\n", field_name, field_type, json_tag));
        }
        
        struct_def.push_str("}\n\n");
        struct_def
    }

    fn generate_handlers(&self, actions: &[Action]) -> String {
        let mut handlers = String::from("package main\n\n");
        handlers.push_str("import (\n");
        handlers.push_str("    \"encoding/json\"\n");
        handlers.push_str("    \"net/http\"\n");
        handlers.push_str("    \"github.com/gin-gonic/gin\"\n");
        handlers.push_str(")\n\n");

        for action in actions {
            let function_name = self.pascal_case(&action.id);
            
            handlers.push_str(&format!("func {}(c *gin.Context) {{\n", function_name));
            
            if let Some(input) = &action.input {
                let input_type = input.entity.as_ref().map(|e| self.pascal_case(e)).unwrap_or("interface{}".to_string());
                handlers.push_str(&format!("    var req {}\n", input_type));
                handlers.push_str("    if err := c.ShouldBindJSON(&req); err != nil {\n");
                handlers.push_str("        c.JSON(http.StatusBadRequest, gin.H{\"error\": err.Error()})\n");
                handlers.push_str("        return\n");
                handlers.push_str("    }\n\n");
            }
            
            handlers.push_str("    // TODO: Implement action logic\n");
            handlers.push_str("    c.JSON(http.StatusNotImplemented, gin.H{\"error\": \"Not implemented\"})\n");
            handlers.push_str("}\n\n");
        }

        // Generate router setup
        handlers.push_str("func SetupRoutes(r *gin.Engine) {\n");
        handlers.push_str("    api := r.Group(\"/api\")\n");
        
        for action in actions {
            let function_name = self.pascal_case(&action.id);
            let route_name = action.id.replace("_", "-");
            let method = self.infer_http_method(&action.id).to_uppercase();
            
            handlers.push_str(&format!("    api.{}(\"/{}\", {})\n", method, route_name, function_name));
        }
        handlers.push_str("}\n");

        handlers
    }

    fn map_type(&self, fdml_type: &str) -> &str {
        match fdml_type {
            "string" => "string",
            "int" | "integer" => "int64",
            "float" | "double" => "float64",
            "bool" | "boolean" => "bool", 
            "date" | "datetime" => "time.Time",
            _ => "interface{}"
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

impl CodeGenerator for GoGenerator {
    fn generate(&self, document: &FdmlDocument, output_dir: &Path) -> Result<Vec<String>> {
        let mut generated_files = Vec::new();

        // Create output directory
        fs::create_dir_all(output_dir).map_err(|e| {
            crate::error::FdmlError::generator_error(format!(
                "Failed to create output directory: {}", e
            ))
        })?;

        // Generate struct definitions
        if !document.entities.is_empty() {
            let mut types_content = String::from("package main\n\n");
            types_content.push_str("import \"time\"\n\n");
            
            for entity in &document.entities {
                types_content.push_str(&self.generate_struct(entity));
            }

            let types_file = output_dir.join("types.go");
            fs::write(&types_file, types_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write types file: {}", e
                ))
            })?;
            generated_files.push(types_file.to_string_lossy().to_string());
        }

        // Generate handlers
        if !document.actions.is_empty() {
            let handlers_content = self.generate_handlers(&document.actions);
            let handlers_file = output_dir.join("handlers.go");
            fs::write(&handlers_file, handlers_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write handlers file: {}", e
                ))
            })?;
            generated_files.push(handlers_file.to_string_lossy().to_string());
        }

        // Generate go.mod
        let go_mod_path = output_dir.join("go.mod");
        if !go_mod_path.exists() {
            let go_mod = "module fdml-generated-api\n\ngo 1.21\n\nrequire github.com/gin-gonic/gin v1.9.0\n";
            fs::write(&go_mod_path, go_mod).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write go.mod: {}", e
                ))
            })?;
            generated_files.push(go_mod_path.to_string_lossy().to_string());
        }

        // Generate main.go
        let main_path = output_dir.join("main.go");
        if !main_path.exists() {
            let main_content = r#"package main

import (
    "github.com/gin-gonic/gin"
)

func main() {
    r := gin.Default()
    SetupRoutes(r)
    r.Run(":8080")
}
"#;
            fs::write(&main_path, main_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write main.go: {}", e
                ))
            })?;
            generated_files.push(main_path.to_string_lossy().to_string());
        }

        Ok(generated_files)
    }

    fn language(&self) -> &str {
        "go"
    }

    fn file_extension(&self) -> &str {
        "go"
    }
}