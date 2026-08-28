//! The `Flow` contract (pass 4): reconstructed data-flow paths through the system,
//! each an ordered ingress → … → sink chain of actions.
//!
//! This mirrors FDML's old `flows:` section shape (`id`, `name`, `steps` with an
//! `action` reference + a human `description`) so the assembled spec stays
//! FDML-compatible — but the *engine* that produces it is rewritten (see `fdml-flows`):
//! the old fuzzy param-type call-graph + all-simple-paths enumeration is replaced by a
//! bounded BFS over the real `DepGraph`.
//!
//! Determinism contract: `fdml-flows` emits flows sorted by (ingress file, sink file)
//! with ids `flow_0..flow_N`, each step in path order. No HashMap iteration order, no
//! wall-clock, leaks into output. Two reconstruction runs serialize byte-identically.

use serde::{Deserialize, Serialize};

/// A reconstructed data-flow path: an ordered sequence of steps from an ingress point
/// (handler/route/entry) to a sink (DB write / response / publish).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Flow {
    /// Stable id (`flow_0`, `flow_1`, …), assigned in the deterministic flow order.
    pub id: String,
    /// Human-readable name derived from the ingress action (e.g. "Handle Request Flow").
    pub name: String,
    /// The steps along the path, in order (ingress first, sink last).
    pub steps: Vec<FlowStep>,
}

/// One step on a [`Flow`]: a representative action in a file on the ingress→sink path
/// (or the file itself, when that file carries no extracted action).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FlowStep {
    /// The representative action's id, or empty when the path-file has no action.
    pub action_id: String,
    /// The representative action's name, or the file stem when there is no action.
    pub action_name: String,
    /// The project-relative POSIX file this step lives in (a `DepGraph` node).
    pub file: String,
    /// Human description: `"<action_name> (<code_ref>)"` for an action step, or the
    /// file's module/path for a pass-through file with no action.
    pub description: String,
}
