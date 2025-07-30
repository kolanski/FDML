use crate::error::{FdmlError, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Identifiers and literals
    Identifier(String),
    String(String),
    Number(f64),
    Boolean(bool),
    
    // Keywords
    System,
    Entity,
    Action,
    Feature,
    Flow,
    Constraint,
    Traceability,
    GenerationRule,
    Metadata,
    
    // Symbols
    Colon,
    Dash,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    
    // Special
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub line: usize,
    pub column: usize,
    pub value: String,
}

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
    indent_stack: Vec<usize>,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0],
        }
    }
    
    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        
        while !self.is_at_end() {
            if let Some(token) = self.next_token()? {
                tokens.push(token);
            }
        }
        
        // Add any remaining dedent tokens
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            tokens.push(Token {
                token_type: TokenType::Dedent,
                line: self.line,
                column: self.column,
                value: String::new(),
            });
        }
        
        tokens.push(Token {
            token_type: TokenType::Eof,
            line: self.line,
            column: self.column,
            value: String::new(),
        });
        
        Ok(tokens)
    }
    
    fn next_token(&mut self) -> Result<Option<Token>> {
        self.skip_whitespace_and_comments();
        
        if self.is_at_end() {
            return Ok(None);
        }
        
        let start_line = self.line;
        let start_column = self.column;
        
        let ch = self.advance();
        
        let token_type = match ch {
            ':' => TokenType::Colon,
            '-' => {
                if self.peek() == Some(' ') || self.peek() == Some('\t') {
                    TokenType::Dash
                } else {
                    return self.identifier_or_keyword(ch);
                }
            }
            '[' => TokenType::LeftBracket,
            ']' => TokenType::RightBracket,
            '{' => TokenType::LeftBrace,
            '}' => TokenType::RightBrace,
            ',' => TokenType::Comma,
            '\n' => {
                self.line += 1;
                self.column = 1;
                return self.handle_newline_and_indentation();
            }
            '"' => return self.string_literal(),
            '\'' => return self.string_literal(),
            _ if ch.is_ascii_digit() => return self.number_literal(ch),
            _ if ch.is_alphabetic() || ch == '_' => return self.identifier_or_keyword(ch),
            _ => {
                return Err(FdmlError::parser_error(
                    start_line,
                    start_column,
                    format!("Unexpected character: '{}'", ch),
                ));
            }
        };
        
        Ok(Some(Token {
            token_type,
            line: start_line,
            column: start_column,
            value: ch.to_string(),
        }))
    }
    
    fn handle_newline_and_indentation(&mut self) -> Result<Option<Token>> {
        let start_line = self.line - 1;
        let start_column = self.column;
        
        // Skip empty lines and comments
        while !self.is_at_end() && (self.peek() == Some('\n') || self.peek() == Some('#')) {
            if self.peek() == Some('\n') {
                self.advance();
                self.line += 1;
                self.column = 1;
            } else {
                self.skip_comment();
            }
        }
        
        if self.is_at_end() {
            return Ok(Some(Token {
                token_type: TokenType::Newline,
                line: start_line,
                column: start_column,
                value: "\n".to_string(),
            }));
        }
        
        // Count indentation
        let mut indent_level = 0;
        while self.peek() == Some(' ') || self.peek() == Some('\t') {
            if self.peek() == Some(' ') {
                indent_level += 1;
            } else {
                indent_level += 4; // Tab counts as 4 spaces
            }
            self.advance();
        }
        
        let current_indent = *self.indent_stack.last().unwrap();
        
        if indent_level > current_indent {
            self.indent_stack.push(indent_level);
            Ok(Some(Token {
                token_type: TokenType::Indent,
                line: self.line,
                column: 1,
                value: " ".repeat(indent_level),
            }))
        } else if indent_level < current_indent {
            // Find matching indentation level
            while let Some(&stack_indent) = self.indent_stack.last() {
                if stack_indent <= indent_level {
                    break;
                }
                self.indent_stack.pop();
            }
            
            if self.indent_stack.last() != Some(&indent_level) {
                return Err(FdmlError::parser_error(
                    self.line,
                    1,
                    "Indentation error: invalid dedent".to_string(),
                ));
            }
            
            Ok(Some(Token {
                token_type: TokenType::Dedent,
                line: self.line,
                column: 1,
                value: " ".repeat(indent_level),
            }))
        } else {
            Ok(Some(Token {
                token_type: TokenType::Newline,
                line: start_line,
                column: start_column,
                value: "\n".to_string(),
            }))
        }
    }
    
    fn identifier_or_keyword(&mut self, first_char: char) -> Result<Option<Token>> {
        let start_line = self.line;
        let start_column = self.column - 1;
        let mut value = String::new();
        value.push(first_char);
        
        while let Some(&ch) = self.input.get(self.position) {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        
        let token_type = match value.as_str() {
            "system" => TokenType::System,
            "entity" => TokenType::Entity,
            "action" => TokenType::Action,
            "feature" => TokenType::Feature,
            "flow" => TokenType::Flow,
            "constraint" => TokenType::Constraint,
            "traceability" => TokenType::Traceability,
            "generation_rule" => TokenType::GenerationRule,
            "metadata" => TokenType::Metadata,
            "true" => TokenType::Boolean(true),
            "false" => TokenType::Boolean(false),
            _ => TokenType::Identifier(value.clone()),
        };
        
        Ok(Some(Token {
            token_type,
            line: start_line,
            column: start_column,
            value,
        }))
    }
    
    fn string_literal(&mut self) -> Result<Option<Token>> {
        let start_line = self.line;
        let start_column = self.column - 1;
        let quote_char = self.input[self.position - 1];
        let mut value = String::new();
        
        while !self.is_at_end() && self.peek() != Some(quote_char) {
            let ch = self.advance();
            if ch == '\\' && !self.is_at_end() {
                let escaped = self.advance();
                match escaped {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '\'' => value.push('\''),
                    '"' => value.push('"'),
                    _ => {
                        value.push('\\');
                        value.push(escaped);
                    }
                }
            } else {
                value.push(ch);
            }
        }
        
        if self.is_at_end() {
            return Err(FdmlError::parser_error(
                start_line,
                start_column,
                "Unterminated string literal".to_string(),
            ));
        }
        
        self.advance(); // Consume closing quote
        
        Ok(Some(Token {
            token_type: TokenType::String(value.clone()),
            line: start_line,
            column: start_column,
            value,
        }))
    }
    
    fn number_literal(&mut self, first_digit: char) -> Result<Option<Token>> {
        let start_line = self.line;
        let start_column = self.column - 1;
        let mut value = String::new();
        value.push(first_digit);
        
        while let Some(&ch) = self.input.get(self.position) {
            if ch.is_ascii_digit() || ch == '.' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        
        let number: f64 = value.parse().map_err(|_| {
            FdmlError::parser_error(start_line, start_column, "Invalid number format".to_string())
        })?;
        
        Ok(Some(Token {
            token_type: TokenType::Number(number),
            line: start_line,
            column: start_column,
            value,
        }))
    }
    
    fn skip_whitespace_and_comments(&mut self) {
        while let Some(&ch) = self.input.get(self.position) {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '#' => {
                    self.skip_comment();
                }
                _ => break,
            }
        }
    }
    
    fn skip_comment(&mut self) {
        while !self.is_at_end() && self.peek() != Some('\n') {
            self.advance();
        }
    }
    
    fn advance(&mut self) -> char {
        let ch = self.input[self.position];
        self.position += 1;
        self.column += 1;
        ch
    }
    
    fn peek(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }
    
    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_tokenization() {
        let input = r#"
entity:
  id: user
  name: "User Entity"
"#;
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        // Basic smoke test - just ensure it doesn't crash
        assert!(!tokens.is_empty());
        assert_eq!(tokens.last().unwrap().token_type, TokenType::Eof);
    }
}