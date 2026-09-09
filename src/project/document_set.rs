//! The project's FDML documents as one id space.
//!
//! Two documents live side by side and must stay separate: `vision.fdml` is written by a
//! human, `.fdml/spec/*.fdml` is written by the scanner and rewritten on every scan.
//! Links cross that boundary (`generated feature --realizes--> vision feature`), so
//! references resolve against the **set**, while each document is still validated on its
//! own. No manifest: the set is a convention, and a manifest is added only when a real
//! project outgrows the convention.
//!
//! Rules:
//! - set = the anchor document + `<root>/vision.fdml` + `<root>/.fdml/spec/*.fdml`, where
//!   root is the anchor's directory;
//! - ids are unique across the whole set; the same id in two documents is an error;
//! - `realizes` stays `realizes`: only the resolution scope changes, not the relation.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::error::{FdmlError, Result};
use crate::parser::ast::FdmlDocument;
use crate::parser::parse_fdml_yaml;
use crate::validator::rules::{check_traceability_in, collect_ids};

pub struct DocumentSet {
    pub root: PathBuf,
    /// Sorted by path, so every report is deterministic.
    pub docs: Vec<(PathBuf, FdmlDocument)>,
}

impl DocumentSet {
    /// The set around one document. The anchor is always in the set, even when it lives
    /// outside the convention (`specs/foo.fdml`), so validating any file still sees the
    /// project.
    pub fn around(anchor: &Path) -> Result<Self> {
        let anchor_abs = anchor.canonicalize()
            .map_err(|e| FdmlError::project_error(format!("cannot resolve {}: {e}", anchor.display())))?;
        let root = anchor_abs.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        let mut paths: Vec<PathBuf> = vec![anchor_abs.clone()];
        let vision = root.join("vision.fdml");
        if vision.exists() { paths.push(vision.canonicalize().unwrap_or(vision)); }
        if let Ok(entries) = std::fs::read_dir(root.join(".fdml").join("spec")) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("fdml") {
                    paths.push(p.canonicalize().unwrap_or(p));
                }
            }
        }
        paths.sort();
        paths.dedup();
        let mut docs = Vec::with_capacity(paths.len());
        for p in paths {
            let text = std::fs::read_to_string(&p)
                .map_err(|e| FdmlError::project_error(format!("cannot read {}: {e}", p.display())))?;
            let doc = parse_fdml_yaml(&text)
                .map_err(|e| FdmlError::project_error(format!("{}: {e}", p.display())))?;
            docs.push((p, doc));
        }
        Ok(Self { root, docs })
    }

    /// Whether any document in the set came from the scanner.
    pub fn has_generated(&self) -> bool {
        self.docs.iter().any(|(p, _)| p.components().any(|c| c.as_os_str() == ".fdml"))
    }

    /// Every id declared anywhere in the set.
    pub fn all_ids(&self) -> HashSet<String> {
        self.docs.iter().flat_map(|(_, d)| collect_ids(d)).collect()
    }

    /// The same id declared in two documents. Generated ids are `type:path:name` and
    /// cannot collide with each other; a collision is a human id that shadows one, or two
    /// human files disagreeing. Either way the model cannot say which element a link means.
    pub fn collisions(&self) -> Vec<String> {
        let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (p, d) in &self.docs {
            let name = self.display(p);
            for id in collect_ids(d) {
                owners.entry(id).or_default().push(name.clone());
            }
        }
        owners.into_iter()
            .filter(|(_, files)| files.len() > 1)
            .map(|(id, files)| format!("id '{}' is declared in more than one document: {}", id, files.join(", ")))
            .collect()
    }

    /// Traceability of every document, resolved against the whole set. Each message is
    /// prefixed with the file it came from.
    pub fn unresolved(&self) -> Vec<String> {
        let ids = self.all_ids();
        let mut out = Vec::new();
        for (p, d) in &self.docs {
            for e in check_traceability_in(d, &ids) {
                out.push(format!("{}: {e}", self.display(p)));
            }
        }
        // a vision file alone, pointing at `feature:…` ids, is almost always a missing scan
        if !self.has_generated() && out.iter().any(|m| m.contains("'feature:") || m.contains("'action:") || m.contains("'entity:")) {
            out.push(format!("no generated spec under {}/.fdml/spec/ — run the scan pipeline (discover → scan → graph → assemble) to produce the ids these links point at", self.display(&self.root)));
        }
        out
    }

    fn display(&self, p: &Path) -> String {
        p.strip_prefix(&self.root).map(|r| r.display().to_string()).unwrap_or_else(|_| p.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const HEAD: &str = "metadata: {version: \"1.3\", author: \"t\", description: \"t\", created: \"2026-09-09T00:00:00Z\"}\nsystem: {id: \"s\", name: \"s\", description: \"s\", components: [], relationships: []}\n";

    fn vision(extra: &str) -> String {
        format!("{HEAD}features:\n  - id: accel_feel\n    title: \"Accel\"\n    description: \"d\"\n    scenarios: []\n{extra}")
    }
    fn generated(id: &str) -> String {
        format!("{HEAD}features:\n  - id: \"{id}\"\n    title: \"g\"\n    description: \"recovered\"\n    scenarios: []\n")
    }
    fn project(vision_text: &str, generated_text: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("vision.fdml"), vision_text).unwrap();
        if let Some(g) = generated_text {
            fs::create_dir_all(dir.path().join(".fdml/spec")).unwrap();
            fs::write(dir.path().join(".fdml/spec/gen.fdml"), g).unwrap();
        }
        dir
    }

    #[test]
    fn realizes_resolves_across_the_boundary() {
        let dir = project(
            &vision("traceability:\n  - from: \"feature:src.vehicle.handling:driveline\"\n    to: \"accel_feel\"\n    relation: \"realizes\"\n"),
            Some(&generated("feature:src.vehicle.handling:driveline")),
        );
        let set = DocumentSet::around(&dir.path().join("vision.fdml")).unwrap();
        assert_eq!(set.docs.len(), 2, "vision + generated");
        assert!(set.collisions().is_empty());
        assert!(set.unresolved().is_empty(), "the link must resolve in the project set: {:?}", set.unresolved());
        // and the relation is still plain `realizes` — nothing special was added for crossing files
        assert_eq!(set.docs.iter().find(|(p, _)| p.ends_with("vision.fdml")).unwrap().1.traceability[0].relation, "realizes");
    }

    #[test]
    fn the_same_id_in_two_documents_is_an_error() {
        let dir = project(&vision(""), Some(&generated("accel_feel")));
        let set = DocumentSet::around(&dir.path().join("vision.fdml")).unwrap();
        let c = set.collisions();
        assert_eq!(c.len(), 1, "{c:?}");
        assert!(c[0].contains("accel_feel") && c[0].contains("vision.fdml") && c[0].contains("gen.fdml"), "{c:?}");
    }

    #[test]
    fn a_missing_generated_spec_is_named_not_guessed() {
        let dir = project(
            &vision("traceability:\n  - from: \"feature:src.vehicle.handling:driveline\"\n    to: \"accel_feel\"\n    relation: \"realizes\"\n"),
            None,
        );
        let set = DocumentSet::around(&dir.path().join("vision.fdml")).unwrap();
        assert_eq!(set.docs.len(), 1);
        let u = set.unresolved();
        assert!(u.iter().any(|m| m.contains("unknown 'from' element")), "{u:?}");
        assert!(u.iter().any(|m| m.contains("no generated spec")), "the hint names the missing scan: {u:?}");
    }
}
