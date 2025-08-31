use crate::parser::ast::{FdmlDocument, Entity, Action};
use crate::generators::{CodeGenerator, GeneratorConfig};
use crate::error::Result;
use std::path::Path;
use std::fs;

pub struct PythonGenerator {
    config: GeneratorConfig,
}

impl PythonGenerator {
    pub fn new(config: &GeneratorConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    fn generate_pydantic_model(&self, entity: &Entity) -> String {
        let class_name = self.pascal_case(&entity.id);
        let mut model = format!("class {}(BaseModel):\n", class_name);
        
        if entity.description.is_some() {
            model.push_str(&format!("    \"\"\"{}\"\"\"\n", entity.description.as_ref().unwrap()));
        }

        for field in &entity.fields {
            let field_type = self.map_type(&field.field_type);
            let optional = if field.required.unwrap_or(true) { 
                field_type.to_string()
            } else { 
                format!("Optional[{}]", field_type)
            };
            
            if let Some(default) = &field.default {
                model.push_str(&format!("    {}: {} = {}\n", 
                    field.name, optional, self.format_default_value(default)));
            } else if !field.required.unwrap_or(true) {
                model.push_str(&format!("    {}: {} = None\n", field.name, optional));
            } else {
                model.push_str(&format!("    {}: {}\n", field.name, optional));
            }
        }
        
        model.push_str("\n");
        model
    }

    fn generate_fastapi_routes(&self, actions: &[Action]) -> String {
        let mut routes = String::from("from fastapi import APIRouter, HTTPException\n");
        routes.push_str("from typing import List, Optional\n");
        routes.push_str("from .models import *\n\n");
        routes.push_str("router = APIRouter()\n\n");

        for action in actions {
            let route_name = action.id.replace("_", "-");
            let method = self.infer_http_method(&action.id);
            let function_name = action.id.clone();
            
            routes.push_str(&format!("@router.{}(\"/{}\")\n", method.to_lowercase(), route_name));
            
            if let Some(input) = &action.input {
                let input_type = input.entity.as_ref().map(|e| self.pascal_case(e)).unwrap_or("dict".to_string());
                routes.push_str(&format!("async def {}(data: {}):\n", function_name, input_type));
            } else {
                routes.push_str(&format!("async def {}():\n", function_name));
            }
            
            routes.push_str("    \"\"\"TODO: Implement action logic\"\"\"\n");
            routes.push_str("    raise HTTPException(status_code=501, detail=\"Not implemented\")\n\n");
        }

        routes
    }

    fn map_type(&self, fdml_type: &str) -> &str {
        match fdml_type {
            "string" => "str",
            "int" | "integer" => "int",
            "float" | "double" => "float", 
            "bool" | "boolean" => "bool",
            "date" => "date",
            "datetime" => "datetime",
            _ => "Any"
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

    fn format_default_value(&self, value: &crate::parser::ast::Value) -> String {
        match value {
            crate::parser::ast::Value::String(s) => format!("\"{}\"", s),
            crate::parser::ast::Value::Number(n) => n.to_string(),
            crate::parser::ast::Value::Boolean(b) => if *b { "True".to_string() } else { "False".to_string() },
            _ => "None".to_string(),
        }
    }

    fn infer_http_method(&self, action_id: &str) -> &str {
        if action_id.starts_with("get_") || action_id.starts_with("list_") {
            "get"
        } else if action_id.starts_with("create_") || action_id.starts_with("add_") {
            "post"
        } else if action_id.starts_with("update_") || action_id.starts_with("modify_") {
            "put"
        } else if action_id.starts_with("delete_") || action_id.starts_with("remove_") {
            "delete"
        } else {
            "post"
        }
    }
}

impl CodeGenerator for PythonGenerator {
    fn generate(&self, document: &FdmlDocument, output_dir: &Path) -> Result<Vec<String>> {
        let mut generated_files = Vec::new();

        // Create output directory
        fs::create_dir_all(output_dir).map_err(|e| {
            crate::error::FdmlError::generator_error(format!(
                "Failed to create output directory: {}", e
            ))
        })?;

        // Generate Pydantic models
        if !document.entities.is_empty() {
            let mut models_content = String::from("from pydantic import BaseModel\n");
            models_content.push_str("from typing import Optional, List, Any\n");
            models_content.push_str("from datetime import date, datetime\n\n");
            
            for entity in &document.entities {
                models_content.push_str(&self.generate_pydantic_model(entity));
            }

            let models_file = output_dir.join("models.py");
            fs::write(&models_file, models_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write models file: {}", e
                ))
            })?;
            generated_files.push(models_file.to_string_lossy().to_string());
        }

        // Generate FastAPI routes
        if !document.actions.is_empty() {
            let routes_content = self.generate_fastapi_routes(&document.actions);
            let routes_file = output_dir.join("routes.py");
            fs::write(&routes_file, routes_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write routes file: {}", e
                ))
            })?;
            generated_files.push(routes_file.to_string_lossy().to_string());
        }

        // Generate requirements.txt
        let requirements_path = output_dir.join("requirements.txt");
        if !requirements_path.exists() {
            let requirements = "fastapi>=0.100.0\nuvicorn>=0.23.0\npydantic>=2.0.0\n";
            fs::write(&requirements_path, requirements).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write requirements.txt: {}", e
                ))
            })?;
            generated_files.push(requirements_path.to_string_lossy().to_string());
        }

        // Generate main.py
        let main_path = output_dir.join("main.py");
        if !main_path.exists() {
            let main_content = r#"from fastapi import FastAPI
from .routes import router

app = FastAPI(title="FDML Generated API", version="1.0.0")
app.include_router(router, prefix="/api")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
"#;
            fs::write(&main_path, main_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write main.py: {}", e
                ))
            })?;
            generated_files.push(main_path.to_string_lossy().to_string());
        }

        Ok(generated_files)
    }

    fn language(&self) -> &str {
        "python"
    }

    fn file_extension(&self) -> &str {
        "py"
    }
}