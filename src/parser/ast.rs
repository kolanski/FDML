use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FdmlDocument {
    pub metadata: Option<Metadata>,
    pub system: Option<System>,
    pub entities: Vec<Entity>,
    pub actions: Vec<Action>,
    pub features: Vec<Feature>,
    pub flows: Vec<Flow>,
    pub constraints: Vec<Constraint>,
    pub traceability: Vec<Traceability>,
    pub generation_rules: Vec<GenerationRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct System {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub components: Vec<String>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub rel_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub fields: Vec<Field>,
    pub relationships: Option<Vec<EntityRelationship>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub description: Option<String>,
    pub required: Option<bool>,
    pub default: Option<Value>,
    pub constraints: Option<Vec<FieldConstraint>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldConstraint {
    #[serde(rename = "type")]
    pub constraint_type: String,
    pub value: Option<Value>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityRelationship {
    pub entity: String,
    #[serde(rename = "type")]
    pub rel_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub input: Option<ActionData>,
    pub output: Option<ActionData>,
    pub side_effects: Option<Vec<String>>,
    pub preconditions: Option<Vec<String>>,
    pub postconditions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionData {
    pub entity: Option<String>,
    pub fields: Option<Vec<String>>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub scenarios: Vec<Scenario>,
    pub acceptance_criteria: Option<Vec<String>>,
    pub dependencies: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub given: Vec<String>,
    pub when: Vec<String>,
    pub then: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<FlowStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowStep {
    pub id: String,
    pub action: String,
    pub description: Option<String>,
    pub conditions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub constraint_type: String,
    pub rule: String,
    pub entities: Option<Vec<String>>,
    pub actions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Traceability {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationRule {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<String>,
    pub generates: Vec<String>,
    pub template: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

impl Default for FdmlDocument {
    fn default() -> Self {
        Self {
            metadata: None,
            system: None,
            entities: Vec::new(),
            actions: Vec::new(),
            features: Vec::new(),
            flows: Vec::new(),
            constraints: Vec::new(),
            traceability: Vec::new(),
            generation_rules: Vec::new(),
        }
    }
}