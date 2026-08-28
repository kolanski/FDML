//! Pass 5 — cross-system integration + shared-entity detection.
//!
//! Faithful lift of FDML's `detect_integrations` + `detect_shared_entities`
//! (`src/linker/platform.rs`), with two determinism/perf changes:
//!   * output is sorted (FDML iterated HashMaps → nondeterministic order);
//!   * the old O(n²) × edit-distance fuzzy shared-entity cross-match is DROPPED —
//!     exact normalized-name match only (see `detect_shared_entities`).

use std::collections::BTreeMap;

use fdml_types::graph::LinkReport;
use fdml_types::integration::{IntegrationHint, PlatformLinks, SharedEntityHint};
use fdml_types::scan::{FileAnalysis, ScanResult};
use fdml_types::System;
use fdml_util::normalize_name;

/// A per-system bundle: discovery + scan + linker output.
pub type SystemBundle = (System, ScanResult, LinkReport);

/// Detect all cross-system links. `systems` should be in a stable order (sort by id).
pub fn run(systems: &[SystemBundle]) -> PlatformLinks {
    PlatformLinks {
        integrations: detect_integrations(systems),
        shared_entities: detect_shared_entities(systems),
    }
}

// ─── Integration detection (import-keyword based, lifted verbatim) ───

struct IntegrationPattern {
    keywords: &'static [&'static str],
    integration_type: &'static str,
    technology: &'static str,
}

const INTEGRATION_PATTERNS: &[IntegrationPattern] = &[
    IntegrationPattern {
        keywords: &["import requests", "import httpx", "from requests", "from httpx", "fetch(", "axios", "import axios", "require(\"axios\")", "require('axios')"],
        integration_type: "http",
        technology: "REST/JSON",
    },
    IntegrationPattern {
        keywords: &["import redis", "from redis", "aioredis", "ioredis", "require(\"redis\")", "require('redis')"],
        integration_type: "event",
        technology: "Redis pub/sub",
    },
    IntegrationPattern {
        keywords: &["import pika", "from pika", "amqplib", "amqp"],
        integration_type: "queue",
        technology: "RabbitMQ",
    },
    IntegrationPattern {
        keywords: &["from kafka", "confluent_kafka", "kafkajs", "import kafka"],
        integration_type: "event",
        technology: "Kafka",
    },
    IntegrationPattern {
        keywords: &["import psycopg", "from psycopg", "sqlalchemy", "pg.", "require(\"pg\")", "require('pg')", "prisma"],
        integration_type: "shared_db",
        technology: "PostgreSQL",
    },
    IntegrationPattern {
        keywords: &["import pymongo", "from pymongo", "mongoose", "mongodb"],
        integration_type: "shared_db",
        technology: "MongoDB",
    },
    IntegrationPattern {
        keywords: &["import grpc", "from grpc", "@grpc/", "google.golang.org/grpc"],
        integration_type: "grpc",
        technology: "gRPC",
    },
    IntegrationPattern {
        keywords: &["WebSocket", "socket.io", "ws.", "import ws", "require(\"ws\")", "require('ws')"],
        integration_type: "websocket",
        technology: "WebSocket",
    },
];

pub fn detect_integrations(systems: &[SystemBundle]) -> Vec<IntegrationHint> {
    let mut hints = Vec::new();

    for (system, scan, _report) in systems {
        // BTreeMap (not HashMap) for a deterministic per-system order.
        let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for file in &scan.files {
            for (itype, tech, evidence_file) in scan_file_for_integrations(file) {
                seen.entry(format!("{itype}|{tech}")).or_default().push(evidence_file);
            }
        }

        for (key, mut evidence) in seen {
            let (itype, tech) = key.split_once('|').unwrap_or((key.as_str(), ""));
            evidence.sort();
            evidence.dedup();
            let to_system = infer_target_system(itype, &system.id, systems);
            hints.push(IntegrationHint {
                from_system: system.id.clone(),
                to_system,
                integration_type: itype.to_string(),
                technology: tech.to_string(),
                evidence,
            });
        }
    }

    hints.sort_by(|a, b| {
        a.from_system
            .cmp(&b.from_system)
            .then(a.integration_type.cmp(&b.integration_type))
            .then(a.technology.cmp(&b.technology))
    });
    hints
}

fn scan_file_for_integrations(file: &FileAnalysis) -> Vec<(String, String, String)> {
    let mut results = Vec::new();
    for import in &file.imports {
        for pattern in INTEGRATION_PATTERNS {
            for keyword in pattern.keywords {
                if import.module.contains(&keyword.replace("import ", "").replace("from ", ""))
                    || import.names.iter().any(|n| keyword.contains(n.as_str()))
                {
                    results.push((
                        pattern.integration_type.to_string(),
                        pattern.technology.to_string(),
                        file.file_path.clone(),
                    ));
                    break;
                }
            }
        }
    }
    results
}

/// Guess which system an integration targets. (FDML heuristic: an HTTP caller likely
/// talks to the first other `service`.) Deterministic given a sorted `systems`.
fn infer_target_system(integration_type: &str, from_system_id: &str, systems: &[SystemBundle]) -> Option<String> {
    match integration_type {
        "http" => systems
            .iter()
            .find(|(s, _, _)| s.id != from_system_id && s.system_type == "service")
            .map(|(s, _, _)| s.id.clone()),
        _ => None,
    }
}

// ─── Shared entity detection (O(n²) fuzzy pass dropped) ───

pub fn detect_shared_entities(systems: &[SystemBundle]) -> Vec<SharedEntityHint> {
    let mut entity_map: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
    for (system, _scan, report) in systems {
        for entity in &report.entities {
            let normalized = normalize_name(&entity.entity_name);
            let fields: Vec<String> = entity.fields.iter().map(|f| f.name.clone()).collect();
            entity_map.entry(normalized).or_default().push((system.id.clone(), fields));
        }
    }

    // ponytail: dropped FDML's O(n²) × edit-distance fuzzy cross-match pass — exact
    // normalized-name match only. Re-add a token-bucketed fuzzy pass if real platforms
    // miss near-name matches. (CQL idea: a field-level map + union-find would go here.)
    // FDML filtered on entry count, so the same entity in two FILES of ONE system showed
    // up as "shared" (e.g. a TS interface declared twice). Require 2+ DISTINCT systems,
    // collapsing per-system duplicates to the richest (most-fields) variant.
    let mut hints: Vec<SharedEntityHint> = entity_map
        .into_iter()
        .filter_map(|(name, entries)| {
            let mut by_system: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (sid, fields) in entries {
                by_system
                    .entry(sid)
                    .and_modify(|f| {
                        if fields.len() > f.len() {
                            *f = fields.clone();
                        }
                    })
                    .or_insert(fields);
            }
            if by_system.len() < 2 {
                return None;
            }
            let systems: Vec<(String, Vec<String>)> = by_system.into_iter().collect();
            let canonical = systems.iter().max_by_key(|(_, fields)| fields.len()).map(|(id, _)| id.clone());
            Some(SharedEntityHint { entity_name: name, systems, canonical_system: canonical })
        })
        .collect();
    hints.sort_by(|a, b| a.entity_name.cmp(&b.entity_name));
    hints
}

#[cfg(test)]
mod tests;
