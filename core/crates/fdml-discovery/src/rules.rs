//! Experimental declarative discovery rules — a FLAT table, tried before built-in detection.
//!
//! Deliberately NOT a DSL: each rule is `marker (+ optional contains) -> (kind, tech)`.
//! If it ever grows conditions/logic, stop and reconsider (see plan ⚠ in PLAN.md).

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DiscoveryRules {
    /// Flat rule list, matched top-to-bottom; first match wins.
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    /// Marker file: exact name ("go.mod") or "*.ext" suffix glob ("*.csproj").
    pub marker: String,
    /// Optional substring (case-insensitive) that must appear in the matched marker file.
    #[serde(default)]
    pub contains: Option<String>,
    /// system_type to assign: frontend | service | worker | gateway | library.
    pub kind: String,
    /// technology label.
    pub tech: String,
}

impl DiscoveryRules {
    pub fn from_yaml(s: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_yaml(&s).map_err(|e| e.to_string())
    }

    /// First matching rule → (kind, tech, marker_filename). None if no rule matches.
    pub fn match_dir(&self, dir: &Path) -> Option<(String, String, String)> {
        for rule in &self.rules {
            if let Some(marker_name) = rule.matches(dir) {
                return Some((rule.kind.clone(), rule.tech.clone(), marker_name));
            }
        }
        None
    }
}

impl Rule {
    /// If this rule matches `dir`, return the marker filename used as evidence.
    fn matches(&self, dir: &Path) -> Option<String> {
        let found = find_marker(dir, &self.marker)?;
        if let Some(needle) = &self.contains {
            let content = std::fs::read_to_string(dir.join(&found)).unwrap_or_default();
            if !content.to_lowercase().contains(&needle.to_lowercase()) {
                return None;
            }
        }
        Some(found)
    }
}

/// Resolve a marker spec to an actual file in `dir`: "*.ext" suffix glob or exact filename.
fn find_marker(dir: &Path, marker: &str) -> Option<String> {
    if let Some(suffix) = marker.strip_prefix('*') {
        let entries = std::fs::read_dir(dir).ok()?;
        for e in entries.flatten() {
            if let Some(name) = e.path().file_name().and_then(|n| n.to_str()) {
                if name.ends_with(suffix) {
                    return Some(name.to_string());
                }
            }
        }
        None
    } else if dir.join(marker).exists() {
        Some(marker.to_string())
    } else {
        None
    }
}
