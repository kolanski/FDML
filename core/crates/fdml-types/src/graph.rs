//! The `LinkReport` contract (pass 3) — ported from FDML's `src/linker/types.rs`,
//! restricted to the GENERATION path (the `--no-llm`, `spec = None` behavior).
//!
//! Divergences from the original `types.rs`, all driven by "generation only":
//!   * Dropped `unlinked_spec` / `UnlinkedSpec` — purely spec-diffing output, never
//!     populated without a spec.
//!   * `LinkMetadata` drops the old `timestamp` (`Utc::now()`) field, which broke
//!     reproducibility — same call made for `ScanMetadata::scan_timestamp` in `scan.rs`.
//!   * Kept `unlinked_code` / `UnlinkedCode`: in the generation path it carries real
//!     data (every suggested class/function with its `module_path` + a `suggestion`
//!     string), not spec-diff noise.
//!
//! Platform-level types (`PlatformReport`, `DetectedSystem`, integration/shared-entity
//! hints) are NOT ported here — those belong to later passes.

use serde::{Deserialize, Serialize};

/// Full output of the deterministic linker (generation path).
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
    /// Code elements suggested as new entities/actions (with module + suggestion).
    pub unlinked_code: Vec<UnlinkedCode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkMetadata {
    pub linker_version: String,
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

/// Code element suggested as a new entity/action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlinkedCode {
    pub element_type: String,
    pub name: String,
    pub code_ref: String,
    pub module_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
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
