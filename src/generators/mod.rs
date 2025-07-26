pub mod typescript;
pub mod python;
pub mod go;
pub mod test_gen;

use crate::parser::ast::FdmlDocument;
use crate::error::Result;
use std::path::Path;

pub trait CodeGenerator {
    fn generate(&self, document: &FdmlDocument, output_dir: &Path) -> Result<Vec<String>>;
    fn language(&self) -> &str;
    fn file_extension(&self) -> &str;
}

pub struct GeneratorConfig {
    pub language: String,
    pub output_dir: String,
    pub template_dir: Option<String>,
    pub with_tests: bool,
}

pub fn create_generator(config: &GeneratorConfig) -> Result<Box<dyn CodeGenerator>> {
    match config.language.as_str() {
        "typescript" | "ts" => Ok(Box::new(typescript::TypeScriptGenerator::new(config)?)),
        "python" | "py" => Ok(Box::new(python::PythonGenerator::new(config)?)),
        "go" => Ok(Box::new(go::GoGenerator::new(config)?)),
        _ => Err(crate::error::FdmlError::generator_error(format!(
            "Unsupported language: {}. Supported languages: typescript, python, go",
            config.language
        )))
    }
}