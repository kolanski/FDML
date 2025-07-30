use crate::error::{FdmlError, Result};
use crate::parser::ast::FdmlDocument;

pub struct ValidationRule {
    pub name: String,
    pub description: String,
    pub validator: Box<dyn Fn(&FdmlDocument) -> Result<Vec<String>>>,
}

pub struct Validator {
    rules: Vec<ValidationRule>,
}

impl Validator {
    pub fn new() -> Self {
        let rules = vec![
            ValidationRule {
                name: "required_ids".to_string(),
                description: "All entities, features, and actions must have IDs".to_string(),
                validator: Box::new(validate_required_ids),
            },
            ValidationRule {
                name: "unique_ids".to_string(),
                description: "All IDs must be unique within their scope".to_string(),
                validator: Box::new(validate_unique_ids),
            },
            ValidationRule {
                name: "valid_references".to_string(),
                description: "All references must point to existing elements".to_string(),
                validator: Box::new(validate_references),
            },
            ValidationRule {
                name: "required_fields".to_string(),
                description: "Required fields must be present".to_string(),
                validator: Box::new(validate_required_fields),
            },
        ];
        
        Self { rules }
    }
    
    pub fn validate(&self, document: &FdmlDocument) -> Result<Vec<String>> {
        let mut all_errors = Vec::new();
        
        for rule in &self.rules {
            match (rule.validator)(document) {
                Ok(mut errors) => {
                    all_errors.append(&mut errors);
                }
                Err(e) => {
                    all_errors.push(format!("Validation rule '{}' failed: {}", rule.name, e));
                }
            }
        }
        
        Ok(all_errors)
    }
    
    pub fn validate_strict(&self, document: &FdmlDocument) -> Result<()> {
        let errors = self.validate(document)?;
        if !errors.is_empty() {
            return Err(FdmlError::validation_error(format!(
                "Validation failed with {} errors:\n{}",
                errors.len(),
                errors.join("\n")
            )));
        }
        Ok(())
    }
}

fn validate_required_ids(document: &FdmlDocument) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    
    // Check entities
    for (index, entity) in document.entities.iter().enumerate() {
        if entity.id.is_empty() {
            errors.push(format!("Entity at index {} is missing required 'id' field", index));
        }
    }
    
    // Check features
    for (index, feature) in document.features.iter().enumerate() {
        if feature.id.is_empty() {
            errors.push(format!("Feature at index {} is missing required 'id' field", index));
        }
        if feature.title.is_empty() {
            errors.push(format!("Feature '{}' is missing required 'title' field", feature.id));
        }
        
        // Check scenarios
        for (scenario_index, scenario) in feature.scenarios.iter().enumerate() {
            if scenario.id.is_empty() {
                errors.push(format!(
                    "Scenario at index {} in feature '{}' is missing required 'id' field",
                    scenario_index, feature.id
                ));
            }
            if scenario.title.is_empty() {
                errors.push(format!(
                    "Scenario '{}' in feature '{}' is missing required 'title' field",
                    scenario.id, feature.id
                ));
            }
        }
    }
    
    // Check actions
    for (index, action) in document.actions.iter().enumerate() {
        if action.id.is_empty() {
            errors.push(format!("Action at index {} is missing required 'id' field", index));
        }
    }
    
    // Check flows
    for (index, flow) in document.flows.iter().enumerate() {
        if flow.id.is_empty() {
            errors.push(format!("Flow at index {} is missing required 'id' field", index));
        }
        if flow.name.is_empty() {
            errors.push(format!("Flow '{}' is missing required 'name' field", flow.id));
        }
    }
    
    // Check constraints
    for (index, constraint) in document.constraints.iter().enumerate() {
        if constraint.id.is_empty() {
            errors.push(format!("Constraint at index {} is missing required 'id' field", index));
        }
        if constraint.name.is_empty() {
            errors.push(format!("Constraint '{}' is missing required 'name' field", constraint.id));
        }
        if constraint.rule.is_empty() {
            errors.push(format!("Constraint '{}' is missing required 'rule' field", constraint.id));
        }
    }
    
    Ok(errors)
}

fn validate_unique_ids(document: &FdmlDocument) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let mut all_ids = std::collections::HashSet::new();
    
    // Collect all IDs and check for duplicates
    for entity in &document.entities {
        if !entity.id.is_empty() {
            if !all_ids.insert(entity.id.clone()) {
                errors.push(format!("Duplicate ID found: '{}'", entity.id));
            }
        }
    }
    
    for feature in &document.features {
        if !feature.id.is_empty() {
            if !all_ids.insert(feature.id.clone()) {
                errors.push(format!("Duplicate ID found: '{}'", feature.id));
            }
        }
        
        for scenario in &feature.scenarios {
            if !scenario.id.is_empty() {
                if !all_ids.insert(scenario.id.clone()) {
                    errors.push(format!("Duplicate ID found: '{}'", scenario.id));
                }
            }
        }
    }
    
    for action in &document.actions {
        if !action.id.is_empty() {
            if !all_ids.insert(action.id.clone()) {
                errors.push(format!("Duplicate ID found: '{}'", action.id));
            }
        }
    }
    
    for flow in &document.flows {
        if !flow.id.is_empty() {
            if !all_ids.insert(flow.id.clone()) {
                errors.push(format!("Duplicate ID found: '{}'", flow.id));
            }
        }
    }
    
    for constraint in &document.constraints {
        if !constraint.id.is_empty() {
            if !all_ids.insert(constraint.id.clone()) {
                errors.push(format!("Duplicate ID found: '{}'", constraint.id));
            }
        }
    }
    
    Ok(errors)
}

fn validate_references(document: &FdmlDocument) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let mut all_ids = std::collections::HashSet::new();
    
    // Collect all valid IDs
    for entity in &document.entities {
        all_ids.insert(entity.id.clone());
    }
    for feature in &document.features {
        all_ids.insert(feature.id.clone());
        for scenario in &feature.scenarios {
            all_ids.insert(scenario.id.clone());
        }
    }
    for action in &document.actions {
        all_ids.insert(action.id.clone());
    }
    for flow in &document.flows {
        all_ids.insert(flow.id.clone());
    }
    for constraint in &document.constraints {
        all_ids.insert(constraint.id.clone());
    }
    
    // Check references in features
    for feature in &document.features {
        if let Some(dependencies) = &feature.dependencies {
            for dep in dependencies {
                if !all_ids.contains(dep) {
                    errors.push(format!(
                        "Feature '{}' references unknown dependency: '{}'",
                        feature.id, dep
                    ));
                }
            }
        }
    }
    
    // Check traceability references
    for trace in &document.traceability {
        if !all_ids.contains(&trace.from) {
            errors.push(format!(
                "Traceability references unknown 'from' element: '{}'",
                trace.from
            ));
        }
        if !all_ids.contains(&trace.to) {
            errors.push(format!(
                "Traceability references unknown 'to' element: '{}'",
                trace.to
            ));
        }
    }
    
    Ok(errors)
}

fn validate_required_fields(document: &FdmlDocument) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    
    // Check entity fields
    for entity in &document.entities {
        for field in &entity.fields {
            if field.name.is_empty() {
                errors.push(format!(
                    "Field in entity '{}' is missing required 'name'",
                    entity.id
                ));
            }
            if field.field_type.is_empty() {
                errors.push(format!(
                    "Field '{}' in entity '{}' is missing required 'type'",
                    field.name, entity.id
                ));
            }
        }
    }
    
    Ok(errors)
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::*;
    
    #[test]
    fn test_validator_empty_document() {
        let document = FdmlDocument::default();
        let validator = Validator::new();
        let errors = validator.validate(&document).unwrap();
        // Empty document should have no validation errors
        assert!(errors.is_empty());
    }
    
    #[test]
    fn test_validator_missing_ids() {
        let mut document = FdmlDocument::default();
        document.entities.push(Entity {
            id: String::new(), // Missing ID
            name: Some("Test Entity".to_string()),
            description: None,
            fields: Vec::new(),
            relationships: None,
        });
        
        let validator = Validator::new();
        let errors = validator.validate(&document).unwrap();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.contains("missing required 'id'")));
    }
}