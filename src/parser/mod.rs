pub mod ast;
pub mod lexer;
pub mod parser;

use crate::error::Result;
use self::lexer::Lexer;
use self::parser::Parser;
use self::ast::FdmlDocument;

/// Parse FDML content from a string
pub fn parse_fdml(content: &str) -> Result<FdmlDocument> {
    let mut lexer = Lexer::new(content);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

/// Parse FDML content from YAML (for compatibility)
pub fn parse_fdml_yaml(content: &str) -> Result<FdmlDocument> {
    // Use YAML parsing directly
    serde_yaml::from_str::<FdmlDocument>(content)
        .map_err(|e| crate::error::FdmlError::simple_parser_error(format!("YAML parsing failed: {}", e)))
}