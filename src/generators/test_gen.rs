use crate::parser::ast::{FdmlDocument, Entity, Feature};
use crate::generators::GeneratorConfig;
use crate::error::Result;
use std::path::Path;
use std::fs;

pub struct TestGenerator {
    config: GeneratorConfig,
}

impl TestGenerator {
    pub fn new(config: &GeneratorConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    pub fn generate_tests(&self, document: &FdmlDocument, output_dir: &Path) -> Result<Vec<String>> {
        match self.config.language.as_str() {
            "typescript" | "ts" => self.generate_typescript_tests(document, output_dir),
            "python" | "py" => self.generate_python_tests(document, output_dir),
            "go" => self.generate_go_tests(document, output_dir),
            _ => Err(crate::error::FdmlError::generator_error(format!(
                "Test generation not supported for language: {}", self.config.language
            )))
        }
    }

    fn generate_typescript_tests(&self, document: &FdmlDocument, output_dir: &Path) -> Result<Vec<String>> {
        let mut generated_files = Vec::new();
        let test_dir = output_dir.join("tests");
        fs::create_dir_all(&test_dir).map_err(|e| {
            crate::error::FdmlError::generator_error(format!(
                "Failed to create test directory: {}", e
            ))
        })?;

        // Generate entity tests
        for entity in &document.entities {
            let test_content = self.generate_entity_test_ts(entity);
            let test_file = test_dir.join(format!("{}.test.ts", entity.id));
            fs::write(&test_file, test_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write test file: {}", e
                ))
            })?;
            generated_files.push(test_file.to_string_lossy().to_string());
        }

        // Generate feature tests
        for feature in &document.features {
            let test_content = self.generate_feature_test_ts(feature);
            let test_file = test_dir.join(format!("{}.feature.test.ts", feature.id));
            fs::write(&test_file, test_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write feature test file: {}", e
                ))
            })?;
            generated_files.push(test_file.to_string_lossy().to_string());
        }

        Ok(generated_files)
    }

    fn generate_python_tests(&self, document: &FdmlDocument, output_dir: &Path) -> Result<Vec<String>> {
        let mut generated_files = Vec::new();
        let test_dir = output_dir.join("tests");
        fs::create_dir_all(&test_dir).map_err(|e| {
            crate::error::FdmlError::generator_error(format!(
                "Failed to create test directory: {}", e
            ))
        })?;

        // Generate entity tests
        for entity in &document.entities {
            let test_content = self.generate_entity_test_py(entity);
            let test_file = test_dir.join(format!("test_{}.py", entity.id));
            fs::write(&test_file, test_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write test file: {}", e
                ))
            })?;
            generated_files.push(test_file.to_string_lossy().to_string());
        }

        Ok(generated_files)
    }

    fn generate_go_tests(&self, document: &FdmlDocument, output_dir: &Path) -> Result<Vec<String>> {
        let mut generated_files = Vec::new();

        // Generate entity tests
        for entity in &document.entities {
            let test_content = self.generate_entity_test_go(entity);
            let test_file = output_dir.join(format!("{}_test.go", entity.id));
            fs::write(&test_file, test_content).map_err(|e| {
                crate::error::FdmlError::generator_error(format!(
                    "Failed to write test file: {}", e
                ))
            })?;
            generated_files.push(test_file.to_string_lossy().to_string());
        }

        Ok(generated_files)
    }

    fn generate_entity_test_ts(&self, entity: &Entity) -> String {
        let entity_name = self.pascal_case(&entity.id);
        let mut test = format!("import {{ {} }} from '../types';\n\n", entity_name);
        
        test.push_str(&format!("describe('{}', () => {{\n", entity_name));
        test.push_str("  it('should create valid entity', () => {\n");
        test.push_str(&format!("    const entity: {} = {{\n", entity_name));
        
        for field in &entity.fields {
            let test_value = self.generate_test_value_ts(&field.field_type);
            test.push_str(&format!("      {}: {},\n", field.name, test_value));
        }
        
        test.push_str("    };\n");
        test.push_str("    expect(entity).toBeDefined();\n");
        test.push_str("  });\n");
        test.push_str("});\n");
        
        test
    }

    fn generate_entity_test_py(&self, entity: &Entity) -> String {
        let entity_name = self.pascal_case(&entity.id);
        let mut test = "import pytest\n".to_string();
        test.push_str(&format!("from ..models import {}\n\n", entity_name));
        
        test.push_str(&format!("def test_{}():\n", entity.id));
        test.push_str(&format!("    entity = {}(\n", entity_name));
        
        for field in &entity.fields {
            let test_value = self.generate_test_value_py(&field.field_type);
            test.push_str(&format!("        {}={},\n", field.name, test_value));
        }
        
        test.push_str("    )\n");
        test.push_str("    assert entity is not None\n");
        
        test
    }

    fn generate_entity_test_go(&self, entity: &Entity) -> String {
        let mut test = "package main\n\n".to_string();
        test.push_str("import (\n");
        test.push_str("    \"testing\"\n");
        test.push_str(")\n\n");
        
        let entity_name = self.pascal_case(&entity.id);
        test.push_str(&format!("func Test{}(t *testing.T) {{\n", entity_name));
        test.push_str(&format!("    entity := {} {{\n", entity_name));
        
        for field in &entity.fields {
            let test_value = self.generate_test_value_go(&field.field_type);
            let field_name = self.pascal_case(&field.name);
            test.push_str(&format!("        {}: {},\n", field_name, test_value));
        }
        
        test.push_str("    }\n");
        test.push_str("    if entity == (struct{}) {\n");
        test.push_str("        t.Error(\"Entity should not be empty\")\n");
        test.push_str("    }\n");
        test.push_str("}\n");
        
        test
    }

    fn generate_feature_test_ts(&self, feature: &Feature) -> String {
        let mut test = "import request from 'supertest';\n".to_string();
        test.push_str("import app from '../app';\n\n");
        
        test.push_str(&format!("describe('Feature: {}', () => {{\n", feature.title));
        
        for scenario in &feature.scenarios {
            test.push_str(&format!("  describe('Scenario: {}', () => {{\n", scenario.title));
            test.push_str("    it('should pass scenario steps', async () => {\n");
            
            for given in &scenario.given {
                test.push_str(&format!("      // Given: {}\n", given));
            }
            for when in &scenario.when {
                test.push_str(&format!("      // When: {}\n", when));
            }
            for then in &scenario.then {
                test.push_str(&format!("      // Then: {}\n", then));
            }
            
            test.push_str("      // TODO: Implement actual test steps\n");
            test.push_str("      expect(true).toBe(true);\n");
            test.push_str("    });\n");
            test.push_str("  });\n");
        }
        
        test.push_str("});\n");
        test
    }

    fn generate_test_value_ts(&self, field_type: &str) -> &str {
        match field_type {
            "string" => "\"test\"",
            "int" | "integer" | "float" | "double" => "42",
            "bool" | "boolean" => "true",
            "date" | "datetime" => "new Date()",
            _ => "null"
        }
    }

    fn generate_test_value_py(&self, field_type: &str) -> &str {
        match field_type {
            "string" => "\"test\"",
            "int" | "integer" => "42",
            "float" | "double" => "42.0",
            "bool" | "boolean" => "True",
            "date" => "date.today()",
            "datetime" => "datetime.now()",
            _ => "None"
        }
    }

    fn generate_test_value_go(&self, field_type: &str) -> &str {
        match field_type {
            "string" => "\"test\"",
            "int64" => "42",
            "float64" => "42.0",
            "bool" => "true",
            "time.Time" => "time.Now()",
            _ => "nil"
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
}