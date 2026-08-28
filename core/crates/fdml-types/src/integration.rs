//! Cross-system integration + shared-entity contracts (pass 5). The platform layer
//! over per-system models. FDML-compatible shapes (mirrors `IntegrationHint` /
//! `SharedEntityHint` from FDML's linker).

use serde::{Deserialize, Serialize};

/// A detected integration edge from one system toward another (or an external service).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationHint {
    pub from_system: String,
    /// Inferred target system, when one can be guessed (else an external service).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_system: Option<String>,
    /// http | event | queue | grpc | shared_db | websocket
    pub integration_type: String,
    /// e.g. "REST/JSON", "Redis pub/sub", "gRPC".
    pub technology: String,
    /// Files in `from_system` that evidence the integration (sorted, deduped).
    pub evidence: Vec<String>,
}

/// An entity that appears (by normalized name) in 2+ systems — a cross-system data shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedEntityHint {
    /// Normalized (snake_case) entity name.
    pub entity_name: String,
    /// (system_id, field_names) for each system that has this entity.
    pub systems: Vec<(String, Vec<String>)>,
    /// System with the most fields — likely the source of truth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_system: Option<String>,
}

/// The cross-system links of a platform: the input to a platform-level FDML spec.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformLinks {
    pub integrations: Vec<IntegrationHint>,
    pub shared_entities: Vec<SharedEntityHint>,
}
