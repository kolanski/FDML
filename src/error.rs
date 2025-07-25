use colored::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FdmlError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    
    #[error("Parser error at line {line}, column {column}: {message}")]
    Parser {
        line: usize,
        column: usize,
        message: String,
    },
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Project error: {0}")]
    Project(String),
}

impl FdmlError {
    pub fn parser_error(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self::Parser {
            line,
            column,
            message: message.into(),
        }
    }
    
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
    
    pub fn project_error(message: impl Into<String>) -> Self {
        Self::Project(message.into())
    }
}

pub type Result<T> = std::result::Result<T, FdmlError>;

pub fn print_error(error: &FdmlError) {
    match error {
        FdmlError::Parser { line, column, message } => {
            eprintln!("{}: Parser error at {}:{}", "Error".red().bold(), line, column);
            eprintln!("  {}", message.yellow());
        }
        FdmlError::Validation(msg) => {
            eprintln!("{}: {}", "Validation Error".red().bold(), msg.yellow());
        }
        FdmlError::Project(msg) => {
            eprintln!("{}: {}", "Project Error".red().bold(), msg.yellow());
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