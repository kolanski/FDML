use crate::error::{FdmlError, Result};
use crate::parser::ast::*;
use crate::parser::lexer::{Token, TokenType};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }
    
    pub fn parse(&mut self) -> Result<FdmlDocument> {
        let mut document = FdmlDocument::default();
        
        while !self.is_at_end() {
            if let Some(token) = self.peek() {
                match &token.token_type {
                    TokenType::Metadata => {
                        document.metadata = Some(self.parse_metadata()?);
                    }
                    TokenType::System => {
                        document.system = Some(self.parse_system()?);
                    }
                    TokenType::Entity => {
                        document.entities.push(self.parse_entity()?);
                    }
                    TokenType::Action => {
                        document.actions.push(self.parse_action()?);
                    }
                    TokenType::Feature => {
                        document.features.push(self.parse_feature()?);
                    }
                    TokenType::Flow => {
                        document.flows.push(self.parse_flow()?);
                    }
                    TokenType::Constraint => {
                        document.constraints.push(self.parse_constraint()?);
                    }
                    TokenType::Traceability => {
                        document.traceability.push(self.parse_traceability()?);
                    }
                    TokenType::GenerationRule => {
                        document.generation_rules.push(self.parse_generation_rule()?);
                    }
                    TokenType::Newline | TokenType::Indent | TokenType::Dedent => {
                        self.advance(); // Skip whitespace tokens
                    }
                    _ => {
                        return Err(FdmlError::parser_error(
                            token.line,
                            token.column,
                            format!("Unexpected token: {}", token.value),
                        ));
                    }
                }
            } else {
                break;
            }
        }
        
        Ok(document)
    }
    
    fn parse_metadata(&mut self) -> Result<Metadata> {
        self.consume(TokenType::Metadata, "Expected 'metadata'")?;
        self.consume(TokenType::Colon, "Expected ':' after 'metadata'")?;
        self.consume_newline_and_indent()?;
        
        let mut metadata = Metadata {
            version: String::new(),
            author: None,
            description: None,
            created: None,
            updated: None,
        };
        
        while self.match_identifier() {
            let key_token = self.previous().unwrap().clone();
            let key = if let TokenType::Identifier(name) = &key_token.token_type {
                name.clone()
            } else {
                return Err(FdmlError::parser_error(
                    key_token.line,
                    key_token.column,
                    "Expected identifier".to_string(),
                ));
            };
            
            self.consume(TokenType::Colon, "Expected ':' after metadata field")?;
            let value = self.parse_string_value()?;
            
            match key.as_str() {
                "version" => metadata.version = value,
                "author" => metadata.author = Some(value),
                "description" => metadata.description = Some(value),
                "created" => metadata.created = Some(value),
                "updated" => metadata.updated = Some(value),
                _ => {
                    return Err(FdmlError::parser_error(
                        key_token.line,
                        key_token.column,
                        format!("Unknown metadata field: {}", key),
                    ));
                }
            }
            
            self.consume_optional_newline();
        }
        
        self.consume_dedent_if_present();
        Ok(metadata)
    }
    
    fn parse_system(&mut self) -> Result<System> {
        self.consume(TokenType::System, "Expected 'system'")?;
        self.consume(TokenType::Colon, "Expected ':' after 'system'")?;
        self.consume_newline_and_indent()?;
        
        let mut system = System {
            id: String::new(),
            name: String::new(),
            description: None,
            components: Vec::new(),
            relationships: Vec::new(),
        };
        
        while self.match_identifier() {
            let key_token = self.previous().unwrap().clone();
            let key = if let TokenType::Identifier(name) = &key_token.token_type {
                name.clone()
            } else {
                continue;
            };
            
            self.consume(TokenType::Colon, "Expected ':' after system field")?;
            
            match key.as_str() {
                "id" => system.id = self.parse_string_value()?,
                "name" => system.name = self.parse_string_value()?,
                "description" => system.description = Some(self.parse_string_value()?),
                "components" => system.components = self.parse_string_array()?,
                "relationships" => system.relationships = self.parse_relationships()?,
                _ => {
                    return Err(FdmlError::parser_error(
                        key_token.line,
                        key_token.column,
                        format!("Unknown system field: {}", key),
                    ));
                }
            }
            
            self.consume_optional_newline();
        }
        
        self.consume_dedent_if_present();
        Ok(system)
    }
    
    fn parse_entity(&mut self) -> Result<Entity> {
        self.consume(TokenType::Entity, "Expected 'entity'")?;
        self.consume(TokenType::Colon, "Expected ':' after 'entity'")?;
        self.consume_newline_and_indent()?;
        
        let mut entity = Entity {
            id: String::new(),
            name: None,
            description: None,
            fields: Vec::new(),
            relationships: None,
        };
        
        while self.match_identifier() {
            let key_token = self.previous().unwrap().clone();
            let key = if let TokenType::Identifier(name) = &key_token.token_type {
                name.clone()
            } else {
                continue;
            };
            
            self.consume(TokenType::Colon, "Expected ':' after entity field")?;
            
            match key.as_str() {
                "id" => entity.id = self.parse_string_value()?,
                "name" => entity.name = Some(self.parse_string_value()?),
                "description" => entity.description = Some(self.parse_string_value()?),
                "fields" => entity.fields = self.parse_fields()?,
                _ => {
                    // Skip unknown fields for now
                    self.skip_value();
                }
            }
            
            self.consume_optional_newline();
        }
        
        self.consume_dedent_if_present();
        Ok(entity)
    }
    
    fn parse_feature(&mut self) -> Result<Feature> {
        self.consume(TokenType::Feature, "Expected 'feature'")?;
        self.consume(TokenType::Colon, "Expected ':' after 'feature'")?;
        self.consume_newline_and_indent()?;
        
        let mut feature = Feature {
            id: String::new(),
            title: String::new(),
            description: None,
            scenarios: Vec::new(),
            acceptance_criteria: None,
            dependencies: None,
        };
        
        while self.match_identifier() {
            let key_token = self.previous().unwrap().clone();
            let key = if let TokenType::Identifier(name) = &key_token.token_type {
                name.clone()
            } else {
                continue;
            };
            
            self.consume(TokenType::Colon, "Expected ':' after feature field")?;
            
            match key.as_str() {
                "id" => feature.id = self.parse_string_value()?,
                "title" => feature.title = self.parse_string_value()?,
                "description" => feature.description = Some(self.parse_string_value()?),
                "scenarios" => feature.scenarios = self.parse_scenarios()?,
                "acceptance_criteria" => feature.acceptance_criteria = Some(self.parse_string_array()?),
                "dependencies" => feature.dependencies = Some(self.parse_string_array()?),
                _ => {
                    self.skip_value();
                }
            }
            
            self.consume_optional_newline();
        }
        
        self.consume_dedent_if_present();
        Ok(feature)
    }
    
    // Simplified implementations for other parse methods
    fn parse_action(&mut self) -> Result<Action> {
        self.consume(TokenType::Action, "Expected 'action'")?;
        self.consume(TokenType::Colon, "Expected ':'")?;
        self.skip_to_next_section();
        
        Ok(Action {
            id: "placeholder".to_string(),
            name: None,
            description: None,
            input: None,
            output: None,
            side_effects: None,
            preconditions: None,
            postconditions: None,
        })
    }
    
    fn parse_flow(&mut self) -> Result<Flow> {
        self.consume(TokenType::Flow, "Expected 'flow'")?;
        self.consume(TokenType::Colon, "Expected ':'")?;
        self.skip_to_next_section();
        
        Ok(Flow {
            id: "placeholder".to_string(),
            name: "placeholder".to_string(),
            description: None,
            steps: Vec::new(),
        })
    }
    
    fn parse_constraint(&mut self) -> Result<Constraint> {
        self.consume(TokenType::Constraint, "Expected 'constraint'")?;
        self.consume(TokenType::Colon, "Expected ':'")?;
        self.skip_to_next_section();
        
        Ok(Constraint {
            id: "placeholder".to_string(),
            name: "placeholder".to_string(),
            description: None,
            constraint_type: "placeholder".to_string(),
            rule: "placeholder".to_string(),
            entities: None,
            actions: None,
        })
    }
    
    fn parse_traceability(&mut self) -> Result<Traceability> {
        self.consume(TokenType::Traceability, "Expected 'traceability'")?;
        self.consume(TokenType::Colon, "Expected ':'")?;
        self.skip_to_next_section();
        
        Ok(Traceability {
            from: "placeholder".to_string(),
            to: "placeholder".to_string(),
            relation: "placeholder".to_string(),
            description: None,
        })
    }
    
    fn parse_generation_rule(&mut self) -> Result<GenerationRule> {
        self.consume(TokenType::GenerationRule, "Expected 'generation_rule'")?;
        self.consume(TokenType::Colon, "Expected ':'")?;
        self.skip_to_next_section();
        
        Ok(GenerationRule {
            id: "placeholder".to_string(),
            name: "placeholder".to_string(),
            description: None,
            triggers: Vec::new(),
            generates: Vec::new(),
            template: None,
        })
    }
    
    // Helper methods
    fn parse_string_value(&mut self) -> Result<String> {
        if let Some(token) = self.peek() {
            match &token.token_type {
                TokenType::String(s) => {
                    let result = s.clone();
                    self.advance();
                    Ok(result)
                }
                TokenType::Identifier(s) => {
                    let result = s.clone();
                    self.advance();
                    Ok(result)
                }
                _ => {
                    Err(FdmlError::parser_error(
                        token.line,
                        token.column,
                        "Expected string value".to_string(),
                    ))
                }
            }
        } else {
            Err(FdmlError::parser_error(0, 0, "Unexpected end of input".to_string()))
        }
    }
    
    fn parse_string_array(&mut self) -> Result<Vec<String>> {
        let mut result = Vec::new();
        
        // Handle simple case - single line value
        if let Some(token) = self.peek() {
            if matches!(token.token_type, TokenType::String(_) | TokenType::Identifier(_)) {
                result.push(self.parse_string_value()?);
                return Ok(result);
            }
        }
        
        // Handle array case
        self.consume_newline_and_indent()?;
        while self.check(TokenType::Dash) {
            self.advance(); // consume dash
            result.push(self.parse_string_value()?);
            self.consume_optional_newline();
        }
        self.consume_dedent_if_present();
        
        Ok(result)
    }
    
    fn parse_fields(&mut self) -> Result<Vec<Field>> {
        let mut fields = Vec::new();
        
        self.consume_newline_and_indent()?;
        while self.check(TokenType::Dash) {
            self.advance(); // consume dash
            
            let mut field = Field {
                name: String::new(),
                field_type: String::new(),
                description: None,
                required: None,
                default: None,
                constraints: None,
            };
            
            // Parse field properties
            while self.match_identifier() {
                let key_token = self.previous().unwrap().clone();
                let key = if let TokenType::Identifier(name) = &key_token.token_type {
                    name.clone()
                } else {
                    continue;
                };
                
                self.consume(TokenType::Colon, "Expected ':' after field property")?;
                
                match key.as_str() {
                    "name" => field.name = self.parse_string_value()?,
                    "type" => field.field_type = self.parse_string_value()?,
                    "description" => field.description = Some(self.parse_string_value()?),
                    _ => {
                        self.skip_value();
                    }
                }
                
                self.consume_optional_newline();
            }
            
            fields.push(field);
        }
        
        self.consume_dedent_if_present();
        Ok(fields)
    }
    
    fn parse_scenarios(&mut self) -> Result<Vec<Scenario>> {
        let mut scenarios = Vec::new();
        
        self.consume_newline_and_indent()?;
        while self.check(TokenType::Dash) {
            self.advance(); // consume dash
            
            let mut scenario = Scenario {
                id: String::new(),
                title: String::new(),
                description: None,
                given: Vec::new(),
                when: Vec::new(),
                then: Vec::new(),
            };
            
            // Parse scenario properties
            while self.match_identifier() {
                let key_token = self.previous().unwrap().clone();
                let key = if let TokenType::Identifier(name) = &key_token.token_type {
                    name.clone()
                } else {
                    continue;
                };
                
                self.consume(TokenType::Colon, "Expected ':' after scenario property")?;
                
                match key.as_str() {
                    "id" => scenario.id = self.parse_string_value()?,
                    "title" => scenario.title = self.parse_string_value()?,
                    "description" => scenario.description = Some(self.parse_string_value()?),
                    "given" => scenario.given = self.parse_string_array()?,
                    "when" => scenario.when = self.parse_string_array()?,
                    "then" => scenario.then = self.parse_string_array()?,
                    _ => {
                        self.skip_value();
                    }
                }
                
                self.consume_optional_newline();
            }
            
            scenarios.push(scenario);
        }
        
        self.consume_dedent_if_present();
        Ok(scenarios)
    }
    
    fn parse_relationships(&mut self) -> Result<Vec<Relationship>> {
        // Simplified implementation
        self.skip_value();
        Ok(Vec::new())
    }
    
    fn skip_value(&mut self) {
        // Skip the current value (could be string, array, object)
        if let Some(token) = self.peek() {
            match &token.token_type {
                TokenType::String(_) | TokenType::Identifier(_) | TokenType::Number(_) | TokenType::Boolean(_) => {
                    self.advance();
                }
                _ => {
                    self.skip_to_next_line();
                }
            }
        }
    }
    
    fn skip_to_next_section(&mut self) {
        let mut indent_level = 0;
        while !self.is_at_end() {
            if let Some(token) = self.peek() {
                match &token.token_type {
                    TokenType::Indent => {
                        indent_level += 1;
                        self.advance();
                    }
                    TokenType::Dedent => {
                        if indent_level > 0 {
                            indent_level -= 1;
                            self.advance();
                            if indent_level == 0 {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    TokenType::System | TokenType::Entity | TokenType::Action | TokenType::Feature |
                    TokenType::Flow | TokenType::Constraint | TokenType::Traceability | TokenType::GenerationRule => {
                        if indent_level == 0 {
                            break;
                        }
                        self.advance();
                    }
                    _ => {
                        self.advance();
                    }
                }
            } else {
                break;
            }
        }
    }
    
    fn skip_to_next_line(&mut self) {
        while !self.is_at_end() && !self.check(TokenType::Newline) {
            self.advance();
        }
        if self.check(TokenType::Newline) {
            self.advance();
        }
    }
    
    // Token manipulation methods
    fn advance(&mut self) -> Option<&Token> {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }
    
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }
    
    fn previous(&self) -> Option<&Token> {
        if self.current > 0 {
            self.tokens.get(self.current - 1)
        } else {
            None
        }
    }
    
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len() || 
        self.peek().map_or(true, |t| matches!(t.token_type, TokenType::Eof))
    }
    
    fn check(&self, token_type: TokenType) -> bool {
        if let Some(token) = self.peek() {
            std::mem::discriminant(&token.token_type) == std::mem::discriminant(&token_type)
        } else {
            false
        }
    }
    
    fn match_token(&mut self, token_type: TokenType) -> bool {
        if self.check(token_type) {
            self.advance();
            true
        } else {
            false
        }
    }
    
    fn match_identifier(&mut self) -> bool {
        if let Some(token) = self.peek() {
            if matches!(token.token_type, TokenType::Identifier(_)) {
                self.advance();
                return true;
            }
        }
        false
    }
    
    fn consume(&mut self, token_type: TokenType, message: &str) -> Result<()> {
        if self.check(token_type) {
            self.advance();
            Ok(())
        } else if let Some(token) = self.peek() {
            Err(FdmlError::parser_error(
                token.line,
                token.column,
                message.to_string(),
            ))
        } else {
            Err(FdmlError::parser_error(0, 0, message.to_string()))
        }
    }
    
    fn consume_newline_and_indent(&mut self) -> Result<()> {
        self.consume_optional_newline();
        if self.check(TokenType::Indent) {
            self.advance();
        }
        Ok(())
    }
    
    fn consume_optional_newline(&mut self) {
        if self.check(TokenType::Newline) {
            self.advance();
        }
    }
    
    fn consume_dedent_if_present(&mut self) {
        if self.check(TokenType::Dedent) {
            self.advance();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::lexer::Lexer;
    
    #[test]
    fn test_basic_parsing() {
        let input = r#"
entity:
  id: user
  name: "User Entity"
"#;
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let doc = parser.parse().unwrap();
        
        assert_eq!(doc.entities.len(), 1);
        assert_eq!(doc.entities[0].id, "user");
    }
}