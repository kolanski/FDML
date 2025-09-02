use colored::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FdmlError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    
    #[error("Parser error at line {line}, column {column}: {message}")]
    Parser {
        line: usize,
        column: usize,
        message: String,
    },
    
    #[error("Parser error: {0}")]
    SimpleParser(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Project error: {0}")]
    Project(String),
    
    #[error("Generator error: {0}")]
    Generator(String),
    
    #[error("Migration error: {0}")]
    Migration(String),
}

impl FdmlError {
    pub fn parser_error(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self::Parser {
            line,
            column,
            message: message.into(),
        }
    }
    
    pub fn simple_parser_error(message: impl Into<String>) -> Self {
        Self::SimpleParser(message.into())
    }
    
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
    
    pub fn project_error(message: impl Into<String>) -> Self {
        Self::Project(message.into())
    }
    
    pub fn generator_error(message: impl Into<String>) -> Self {
        Self::Generator(message.into())
    }
    
    pub fn migration_error(message: impl Into<String>) -> Self {
        Self::Migration(message.into())
    }
}

pub type Result<T> = std::result::Result<T, FdmlError>;

pub fn print_error(error: &FdmlError) {
    match error {
        FdmlError::Parser { line, column, message } => {
            eprintln!("{}: Parser error at {}:{}", "Error".red().bold(), line, column);
            eprintln!("  {}", message.yellow());
        }
        FdmlError::SimpleParser(msg) => {
            eprintln!("{}: {}", "Parser Error".red().bold(), msg.yellow());
        }
        FdmlError::Validation(msg) => {
            eprintln!("{}: {}", "Validation Error".red().bold(), msg.yellow());
        }
        FdmlError::Project(msg) => {
            eprintln!("{}: {}", "Project Error".red().bold(), msg.yellow());
        }
        FdmlError::Generator(msg) => {
            eprintln!("{}: {}", "Generator Error".red().bold(), msg.yellow());
        }
        FdmlError::Migration(msg) => {
            eprintln!("{}: {}", "Migration Error".red().bold(), msg.yellow());
        }
        _ => {
            eprintln!("{}: {}", "Error".red().bold(), error.to_string().yellow());
        }
    }
}

pub fn print_success(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}

pub fn print_warning(message: &str) {
    println!("{} {}", "⚠".yellow().bold(), message);
}