use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FdmlDocument {
    pub metadata: Option<Metadata>,
    pub system: Option<System>,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub features: Vec<Feature>,
    #[serde(default)]
    pub flows: Vec<Flow>,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub traceability: Vec<Traceability>,
    /// Tests are elements too: a scenario is *verified by* a test, and a link
    /// to something the model does not know is an error, not a shrug.
    #[serde(default)]
    pub tests: Vec<TestRef>,
    #[serde(default)]
    pub generation_rules: Vec<GenerationRule>,

    // --- FDML 1.4: Architectural Level ---
    #[serde(default)]
    pub contours: Vec<Contour>,
    #[serde(default)]
    pub systems: Vec<SystemEntry>,
    #[serde(default)]
    pub integrations: Vec<Integration>,
    #[serde(default)]
    pub cross_flows: Vec<CrossFlow>,
    #[serde(default)]
    pub shared_entities: Vec<SharedEntity>,
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

/// A test the spec can point at. `reference` is whatever the runner calls it
/// (`index::tests::dossier_gathers…` for cargo, a describe/it path for jest);
/// FDML does not run it, it only knows it exists and what it is meant to verify.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestRef {
    pub id: String,
    pub reference: String,
    pub runner: Option<String>,
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

// --- FDML 1.4: Contour ---
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contour {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub trust_level: Option<String>,
}

// --- FDML 1.4: System Entry (plural systems array) ---
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub system_type: String,
    pub technology: Option<String>,
    pub contour: Option<String>,
    pub spec: Option<String>,
    pub owner: Option<String>,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
}

// --- FDML 1.4: Integration ---
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Integration {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub integration_type: String,
    pub protocol: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "async", default)]
    pub is_async: bool,
    #[serde(default)]
    pub endpoints: Vec<IntegrationEndpoint>,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub data_entities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntegrationEndpoint {
    pub method: String,
    pub path: String,
    pub description: Option<String>,
}

// --- FDML 1.4: Cross-Flow ---
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossFlow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub trigger: Option<String>,
    pub steps: Vec<CrossFlowStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossFlowStep {
    pub id: String,
    pub system: String,
    pub action: Option<String>,
    pub description: Option<String>,
    pub integration: Option<String>,
    pub on_success: Option<String>,
    pub on_failure: Option<String>,
}

// --- FDML 1.4: Shared Entity ---
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedEntity {
    #[serde(alias = "id")]
    pub entity: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub canonical_system: Option<String>,
    #[serde(default, alias = "mappings")]
    pub contexts: Vec<SharedEntityContext>,
    #[serde(default)]
    pub fields: Vec<SharedEntityField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedEntityField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedEntityContext {
    pub system: String,
    #[serde(default, alias = "local_entity")]
    pub entity_id: String,
    pub role: Option<String>,
    #[serde(default)]
    pub fields: Vec<String>,
    pub notes: Option<String>,
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
            tests: Vec::new(),
            generation_rules: Vec::new(),
            contours: Vec::new(),
            systems: Vec::new(),
            integrations: Vec::new(),
            cross_flows: Vec::new(),
            shared_entities: Vec::new(),
        }
    }
}