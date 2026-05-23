use serde::{Deserialize, Serialize};

/// Full output of link-code command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkReport {
    pub metadata: LinkMetadata,
    /// Suggested system from module tree
    pub system: SuggestedSystem,
    /// Entity ↔ Class matches
    pub entities: Vec<EntityLink>,
    /// Action ↔ Function/Method matches
    pub actions: Vec<ActionLink>,
    /// Feature ↔ Module grouping suggestions
    pub features: Vec<FeatureSuggestion>,
    /// Generated traceability links
    pub traceability: Vec<TraceLink>,
    /// Coverage summary
    pub coverage: CoverageReport,
    /// Code elements with no match in spec
    pub unlinked_code: Vec<UnlinkedCode>,
    /// Spec elements with no match in code
    pub unlinked_spec: Vec<UnlinkedSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkMetadata {
    pub linker_version: String,
    pub timestamp: String,
    pub inventory_file: String,
    pub spec_file: Option<String>,
    pub spec_loaded: bool,
}

/// System derived from module tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedSystem {
    pub id: String,
    pub name: String,
    pub components: Vec<String>,
    pub relationships: Vec<SystemRelationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRelationship {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub rel_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Entity ↔ Class link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityLink {
    pub entity_id: String,
    pub entity_name: String,
    /// Code reference: "file_path:ClassName"
    pub code_ref: String,
    pub confidence: f64,
    pub source: LinkSource,
    pub fields: Vec<FieldLink>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub bases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldLink {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    /// Whether this field exists in the spec already
    pub in_spec: bool,
    /// Whether this field exists in the code
    pub in_code: bool,
}

/// Action ↔ Function/Method link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLink {
    pub action_id: String,
    pub action_name: String,
    /// Code reference: "file_path:Class.method" or "file_path:function"
    pub code_ref: String,
    pub confidence: f64,
    pub source: LinkSource,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub input: Vec<ActionParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionParam {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Feature suggestion from module grouping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSuggestion {
    pub feature_id: String,
    pub title: String,
    pub module_path: String,
    pub confidence: f64,
    pub source: LinkSource,
    /// Entity IDs grouped into this feature
    pub entities: Vec<String>,
    /// Action IDs grouped into this feature
    pub actions: Vec<String>,
}

/// Traceability link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceLink {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Where the link came from
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkSource {
    /// Matched existing spec element to code
    Matched,
    /// Suggested from code (no spec element exists)
    Suggested,
}

/// Code element without spec match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlinkedCode {
    pub element_type: String,
    pub name: String,
    pub code_ref: String,
    pub module_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Spec element without code match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlinkedSpec {
    pub element_type: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Coverage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub spec_coverage: CoverageMetric,
    pub code_coverage: CoverageMetric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageMetric {
    pub total: usize,
    pub linked: usize,
    pub percentage: f64,
}

// ─── Platform-level types (scan-platform) ────────────────────────

/// A detected system boundary within a multi-system project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedSystem {
    /// Relative path from project root
    pub path: String,
    /// Snake_case identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// frontend | service | worker | gateway | library
    pub system_type: String,
    /// e.g. "React + TypeScript", "FastAPI + Python"
    pub technology: String,
    /// File that triggered detection (e.g. "package.json")
    pub boundary_marker: String,
}

/// A detected integration pattern between systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationHint {
    /// System that initiates the integration
    pub from_system: String,
    /// System that receives (if inferrable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_system: Option<String>,
    /// http | event | queue | shared_db | grpc | websocket
    pub integration_type: String,
    /// e.g. "REST/JSON", "Redis pub/sub", "gRPC"
    pub technology: String,
    /// File paths where evidence was found
    pub evidence: Vec<String>,
}

/// A shared entity candidate across systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedEntityHint {
    /// Canonical entity name
    pub entity_name: String,
    /// (system_id, field_names) for each system that has this entity
    pub systems: Vec<(String, Vec<String>)>,
    /// System with the most fields (likely the source of truth)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_system: Option<String>,
}

/// Full output of scan-platform command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformReport {
    pub detected_systems: Vec<DetectedSystem>,
    pub per_system: Vec<(String, LinkReport)>,
    pub integration_hints: Vec<IntegrationHint>,
    pub shared_entity_hints: Vec<SharedEntityHint>,
}
