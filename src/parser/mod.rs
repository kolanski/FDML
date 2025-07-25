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
    // Try YAML parsing first for simpler cases
    match serde_yaml::from_str::<FdmlDocument>(content) {
        Ok(doc) => Ok(doc),
        Err(_) => {
            // Fall back to custom parser
            parse_fdml(content)
        }
    }
}