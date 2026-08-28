//! FDML core (rework) CLI. The single place passes meet.
//!
//! Artifacts live in `<root>/.fdml/` by default (like `.git`): `discover` writes
//! `systems.json`, `scan` writes `scans/<id>.json`. Override the location with
//! `--workdir`. Later passes (graph/flows/integrations/assemble) get their own
//! subcommands in phases 3–6.

use std::path::{Path, PathBuf};

mod serve;

use clap::{Parser, Subcommand};
use fdml_discovery::DiscoveryRules;
use fdml_types::flow::Flow;
use fdml_types::graph::LinkReport;
use fdml_types::integration::PlatformLinks;
use fdml_types::scan::ScanResult;
use fdml_types::System;

#[derive(Parser)]
#[command(name = "fdml", version, about = "FDML core (rework) — deterministic project map")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pass 1: discover independent systems in a folder.
    Discover {
        /// Project root to scan.
        path: PathBuf,
        /// Artifact dir (default: <path>/.fdml). systems.json is written here.
        #[arg(long)]
        workdir: Option<PathBuf>,
        /// Extra directory names to exclude (merged with .fdmlignore).
        #[arg(long)]
        exclude: Vec<String>,
        /// Optional discovery-rules.yaml to override built-in detection.
        #[arg(long)]
        rules: Option<PathBuf>,
    },
    /// Pass 2: scan source files into a code inventory (ScanResult).
    Scan {
        /// Project root to scan (system paths resolve relative to this).
        root: PathBuf,
        /// Artifact dir (default: <root>/.fdml). Reads systems.json, writes scans/.
        #[arg(long)]
        workdir: Option<PathBuf>,
        /// Extra directory names to exclude.
        #[arg(long)]
        exclude: Vec<String>,
    },
    /// Pass 3: link scans into deterministic graphs (entities/actions/features).
    Graph {
        /// Project root (only used to locate the default workdir).
        root: PathBuf,
        /// Artifact dir (default: <root>/.fdml). Reads scans/, writes graph/.
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    /// Pass 6: assemble a complete FDML YAML spec per system (deterministic, no LLM).
    Assemble {
        /// Project root (only used to locate the default workdir).
        root: PathBuf,
        /// Artifact dir (default: <root>/.fdml). Reads graph/ + scans/, writes spec/.
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    /// Pass 5: detect cross-system integrations + shared entities.
    Integrate {
        /// Project root (only used to locate the default workdir).
        root: PathBuf,
        /// Artifact dir (default: <root>/.fdml). Reads systems.json + scans/ + graph/.
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    /// Platform: assemble systems + integrations into one FDML 1.4 platform spec.
    Platform {
        /// Project root (its name becomes the platform name; locates the workdir).
        root: PathBuf,
        /// Artifact dir (default: <root>/.fdml). Reads systems.json + integrations.json.
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    /// Run the whole deterministic pipeline: discover -> scan -> graph -> integrate ->
    /// assemble -> platform, writing every artifact into <root>/.fdml/.
    All {
        /// Project root to analyze.
        path: PathBuf,
        /// Artifact dir (default: <path>/.fdml).
        #[arg(long)]
        workdir: Option<PathBuf>,
        /// Extra directory names to exclude.
        #[arg(long)]
        exclude: Vec<String>,
        /// Optional discovery-rules.yaml.
        #[arg(long)]
        rules: Option<PathBuf>,
    },
    /// Open a self-contained HTML map of the analyzed project in the browser.
    View {
        /// Project root (locates the workdir).
        root: PathBuf,
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    /// Serve the pipeline output into the bundled React viewer (the rich SpecView).
    Serve {
        /// Project root (locates the workdir).
        root: PathBuf,
        #[arg(long)]
        workdir: Option<PathBuf>,
        /// Port to listen on.
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// Don't open a browser window.
        #[arg(long)]
        no_open: bool,
    },
    /// Overlay prose onto the spec (optional, scoped, tiered). Writes `.fdml/enrich/<id>.json`.
    Enrich {
        /// Project root (locates the workdir).
        root: PathBuf,
        #[arg(long)]
        workdir: Option<PathBuf>,
        /// Limit to one system id (default: all systems).
        #[arg(long)]
        system: Option<String>,
        /// `instant` (deterministic, no LLM) | `llm` (local Ollama, schema-constrained prose).
        #[arg(long, default_value = "instant")]
        tier: String,
        /// Ollama model for `--tier llm`.
        #[arg(long, default_value = "qwen2.5-coder:7b-instruct-q4_K_M")]
        model: String,
        /// Re-enrich even if an equal-or-higher tier already exists (per-element cache still applies).
        #[arg(long)]
        force: bool,
    },
}

/// Default artifact directory inside the scanned repo (like `.git`).
fn workdir_for(root: &Path, workdir: Option<PathBuf>) -> PathBuf {
    workdir.unwrap_or_else(|| root.join(".fdml"))
}

fn load_rules(rules: Option<PathBuf>) -> anyhow::Result<Option<DiscoveryRules>> {
    match rules {
        Some(p) => Ok(Some(
            DiscoveryRules::load(&p).map_err(|e| anyhow::anyhow!("failed to load rules {}: {e}", p.display()))?,
        )),
        None => Ok(None),
    }
}

/// Pass 1 driver. Discover systems under `path`, write `<dir>/systems.json`.
fn run_discover(path: &Path, dir: &Path, exclude: &[String], rules: Option<&DiscoveryRules>) -> anyhow::Result<()> {
    let mut excludes = fdml_discovery::read_fdmlignore(path);
    excludes.extend_from_slice(exclude);
    let systems = fdml_discovery::run(path, &excludes, rules);
    std::fs::create_dir_all(dir)?;
    let out = dir.join("systems.json");
    std::fs::write(&out, serde_json::to_string_pretty(&systems)?)?;
    print!("{}", systems_to_fdml_yaml(&systems));
    eprintln!("discovered {} system(s) -> {}", systems.len(), out.display());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Discover { path, workdir, exclude, rules } => {
            let rules = load_rules(rules)?;
            run_discover(&path, &workdir_for(&path, workdir), &exclude, rules.as_ref())?;
        }
        Command::All { path, workdir, exclude, rules } => {
            let rules = load_rules(rules)?;
            let dir = workdir_for(&path, workdir);
            run_discover(&path, &dir, &exclude, rules.as_ref())?;
            run_scan(&path, &dir, &exclude)?;
            run_graph(&dir)?;
            run_integrate(&dir)?;
            run_assemble(&dir)?;
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("platform").to_string();
            run_platform(&dir, &name)?;
        }
        Command::Scan { root, workdir, exclude } => {
            run_scan(&root, &workdir_for(&root, workdir), &exclude)?;
        }
        Command::Graph { root, workdir } => {
            run_graph(&workdir_for(&root, workdir))?;
        }
        Command::Assemble { root, workdir } => {
            run_assemble(&workdir_for(&root, workdir))?;
        }
        Command::Integrate { root, workdir } => {
            run_integrate(&workdir_for(&root, workdir))?;
        }
        Command::Platform { root, workdir } => {
            let name = root.file_name().and_then(|s| s.to_str()).unwrap_or("platform").to_string();
            run_platform(&workdir_for(&root, workdir), &name)?;
        }
        Command::View { root, workdir } => {
            run_view(&workdir_for(&root, workdir))?;
        }
        Command::Serve { root, workdir, port, no_open } => {
            serve::run_serve(&workdir_for(&root, workdir), port, no_open)?;
        }
        Command::Enrich { root, workdir, system, tier, model, force } => {
            run_enrich(&workdir_for(&root, workdir), system.as_deref(), &tier, &model, force)?;
        }
    }
    Ok(())
}

/// Enrich driver. For each system in scope, mine prose onto its LinkReport and write an
/// overlay `.fdml/enrich/<id>.json` carrying its `tier`. Idempotent + escalation-only:
/// won't redo or downgrade an existing tier unless `--force`.
fn run_enrich(dir: &Path, only: Option<&str>, tier: &str, model: &str, force: bool) -> anyhow::Result<()> {
    let systems: Vec<System> = serde_json::from_str(&std::fs::read_to_string(dir.join("systems.json"))?)
        .map_err(|e| anyhow::anyhow!("failed to parse systems.json (run `fdml all` first): {e}"))?;
    let graph_dir = dir.join("graph");
    let enrich_dir = dir.join("enrich");
    std::fs::create_dir_all(&enrich_dir)?;
    let want = fdml_enrich::tier_rank(tier);
    let (mut done, mut skipped) = (0, 0);
    for s in &systems {
        if only.is_some_and(|o| o != s.id) {
            continue;
        }
        let out = enrich_dir.join(format!("{}.json", s.id));
        // Idempotency: read the existing overlay's tier; skip unless we're escalating or forcing.
        if !force {
            if let Ok(existing) = std::fs::read_to_string(&out) {
                let have = serde_json::from_str::<serde_json::Value>(&existing)
                    .ok()
                    .and_then(|v| v["tier"].as_str().map(fdml_enrich::tier_rank))
                    .unwrap_or(0);
                if have >= want {
                    skipped += 1;
                    continue;
                }
            }
        }
        let Ok(raw) = std::fs::read_to_string(graph_dir.join(format!("{}.json", s.id))) else { continue };
        let report: serde_json::Value = serde_json::from_str(&raw)?;
        let root = report["metadata"]["inventory_file"].as_str().map(PathBuf::from).unwrap_or_default();
        let overlay = match tier {
            "instant" => fdml_enrich::run(&report, &root),
            "llm" => {
                // prev = existing overlay (field types + per-element cache); bootstrap tier-0 if none.
                let prev = std::fs::read_to_string(&out).ok()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .unwrap_or_else(|| fdml_enrich::run(&report, &root));
                let cfg = fdml_enrich::LlmCfg { model: model.to_string(), ..Default::default() };
                eprintln!("  enriching {} via {} …", s.id, model);
                fdml_enrich::run_llm(&report, &root, &prev, &cfg).map_err(|e| anyhow::anyhow!(e))?
            }
            other => anyhow::bail!("unknown tier `{other}` (use `instant` or `llm`)"),
        };
        std::fs::write(&out, serde_json::to_string_pretty(&overlay)?)?;
        done += 1;
    }
    eprintln!("enriched {done} system(s) at tier `{tier}` ({skipped} already up to date) -> {}", enrich_dir.display());
    Ok(())
}

/// Build a self-contained HTML map: embed the `.fdml/` artifacts into the viewer
/// template and write `<dir>/view.html`, then open it. No server, no build, no LLM.
fn run_view(dir: &Path) -> anyhow::Result<()> {
    let systems_raw = std::fs::read_to_string(dir.join("systems.json")).map_err(|e| {
        anyhow::anyhow!("failed to read {} (run `fdml all` first): {e}", dir.join("systems.json").display())
    })?;
    let systems: Vec<System> = serde_json::from_str(&systems_raw)
        .map_err(|e| anyhow::anyhow!("failed to parse systems.json: {e}"))?;
    let integrations = std::fs::read_to_string(dir.join("integrations.json"))
        .unwrap_or_else(|_| "{\"integrations\":[],\"shared_entities\":[]}".to_string());

    let graph_dir = dir.join("graph");
    let read_or = |p: PathBuf, d: &str| std::fs::read_to_string(p).unwrap_or_else(|_| d.to_string());
    let entries: Vec<String> = systems
        .iter()
        .map(|s| {
            let clusters = read_or(graph_dir.join(format!("{}.clusters.json", s.id)), "{\"clusters\":[]}");
            let depgraph = read_or(graph_dir.join(format!("{}.depgraph.json", s.id)), "{\"nodes\":[],\"edges\":[]}");
            let flows = read_or(graph_dir.join(format!("{}.flows.json", s.id)), "[]");
            let model = read_or(graph_dir.join(format!("{}.json", s.id)), "{}"); // LinkReport: entities/actions/features
            let enrich = read_or(dir.join("enrich").join(format!("{}.json", s.id)), "{}"); // optional prose overlay
            format!("\"{}\":{{\"clusters\":{clusters},\"depgraph\":{depgraph},\"flows\":{flows},\"model\":{model},\"enrich\":{enrich}}}", s.id)
        })
        .collect();

    let data = format!(
        "{{\"systems\":{systems_raw},\"integrations\":{integrations},\"graph\":{{{}}}}}",
        entries.join(",")
    );
    let html = include_str!("view.html").replace("/*__DATA__*/", &data);
    let out = dir.join("view.html");
    std::fs::write(&out, html)?;
    println!("view -> {}", out.display());
    let _ = std::process::Command::new("open").arg(&out).status();
    Ok(())
}

/// Platform driver. Combine systems.json + integrations.json into one FDML 1.4 spec.
fn run_platform(dir: &Path, name: &str) -> anyhow::Result<()> {
    let systems_path = dir.join("systems.json");
    let systems: Vec<System> = serde_json::from_str(
        &std::fs::read_to_string(&systems_path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", systems_path.display()))?,
    )
    .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", systems_path.display()))?;

    // integrations.json is optional — a platform with no detected cross-links still assembles.
    let links: PlatformLinks = std::fs::read_to_string(dir.join("integrations.json"))
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default();

    let spec = fdml_assemble::run_platform(&systems, &links, name);
    let out = dir.join("platform.fdml");
    std::fs::write(&out, &spec)?;
    println!(
        "platform spec: {} systems, {} integrations, {} shared entities -> {}",
        systems.len(), links.integrations.len(), links.shared_entities.len(), out.display()
    );
    Ok(())
}

/// Pass 5 driver. Read all systems + their scans + linker reports, detect cross-system
/// integrations and shared entities, write `<dir>/integrations.json`.
fn run_integrate(dir: &Path) -> anyhow::Result<()> {
    let systems_path = dir.join("systems.json");
    let sdata = std::fs::read_to_string(&systems_path).map_err(|e| {
        anyhow::anyhow!("failed to read {} (run discover/scan/graph first): {e}", systems_path.display())
    })?;
    let mut systems: Vec<System> = serde_json::from_str(&sdata)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", systems_path.display()))?;
    systems.sort_by(|a, b| a.id.cmp(&b.id)); // stable order → deterministic target inference

    let scans_dir = dir.join("scans");
    let graph_dir = dir.join("graph");
    let mut bundles: Vec<fdml_integrations::SystemBundle> = Vec::new();
    for system in systems {
        let sp = scans_dir.join(format!("{}.json", system.id));
        let scan: ScanResult = serde_json::from_str(
            &std::fs::read_to_string(&sp).map_err(|e| anyhow::anyhow!("failed to read {}: {e}", sp.display()))?,
        )
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", sp.display()))?;
        let gp = graph_dir.join(format!("{}.json", system.id));
        let report: LinkReport = serde_json::from_str(
            &std::fs::read_to_string(&gp).map_err(|e| anyhow::anyhow!("failed to read {}: {e}", gp.display()))?,
        )
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", gp.display()))?;
        bundles.push((system, scan, report));
    }

    let links = fdml_integrations::run(&bundles);
    let out = dir.join("integrations.json");
    std::fs::write(&out, serde_json::to_string_pretty(&links)?)?;
    println!(
        "integrations={} shared_entities={} -> {}",
        links.integrations.len(), links.shared_entities.len(), out.display()
    );
    for i in &links.integrations {
        println!(
            "    {} --{}/{}-> {}",
            i.from_system, i.integration_type, i.technology, i.to_system.as_deref().unwrap_or("(external)")
        );
    }
    for s in &links.shared_entities {
        let sys: Vec<&str> = s.systems.iter().map(|(id, _)| id.as_str()).collect();
        println!("    shared '{}' in [{}]", s.entity_name, sys.join(", "));
    }
    Ok(())
}

/// Pass 6 driver. For each `<dir>/graph/<id>.json` (LinkReport) with a matching
/// `<dir>/scans/<id>.json` (ScanResult), assemble a complete FDML YAML spec and write
/// `<dir>/spec/<id>.fdml`. One summary line per system + a total.
fn run_assemble(dir: &Path) -> anyhow::Result<()> {
    let graph_dir = dir.join("graph");
    let scans_dir = dir.join("scans");
    let spec_dir = dir.join("spec");
    std::fs::create_dir_all(&spec_dir)?;

    // LinkReports are `<id>.json` in graph/ — skip the `.depgraph.json` / `.clusters.json`
    // sidecar artifacts the graph pass also writes there.
    let mut graph_files: Vec<PathBuf> = std::fs::read_dir(&graph_dir)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", graph_dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|s| s.to_str()) == Some("json")
                && !p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n.ends_with(".depgraph.json") || n.ends_with(".clusters.json") || n.ends_with(".flows.json"))
                    .unwrap_or(false)
        })
        .collect();
    graph_files.sort();

    let mut total_bytes = 0usize;
    let mut count = 0usize;
    for graph_path in &graph_files {
        let id = graph_path.file_stem().and_then(|s| s.to_str()).unwrap_or("system");

        let report_data = std::fs::read_to_string(graph_path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", graph_path.display()))?;
        let report: LinkReport = serde_json::from_str(&report_data)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", graph_path.display()))?;

        let scan_path = scans_dir.join(format!("{id}.json"));
        let scan_data = std::fs::read_to_string(&scan_path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", scan_path.display()))?;
        let scan: ScanResult = serde_json::from_str(&scan_data)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", scan_path.display()))?;

        let flows_path = graph_dir.join(format!("{id}.flows.json"));
        let flows: Vec<Flow> = std::fs::read_to_string(&flows_path)
            .ok()
            .and_then(|d| serde_json::from_str(&d).ok())
            .unwrap_or_default();
        let spec = fdml_assemble::run(&report, &scan, &flows, Some(id));
        let out = spec_dir.join(format!("{id}.fdml"));
        std::fs::write(&out, &spec)?;

        let bytes = spec.len();
        total_bytes += bytes;
        count += 1;
        println!("{id} -> spec/{id}.fdml ({bytes} bytes)");
    }
    println!("total: {count} spec(s), {total_bytes} bytes");
    Ok(())
}

/// Pass 3 driver. Read each `<dir>/scans/*.json` (ScanResult), run the deterministic
/// linker, and write `<dir>/graph/<id>.json` (LinkReport). One summary line per system.
fn run_graph(dir: &Path) -> anyhow::Result<()> {
    let scans_dir = dir.join("scans");
    let graph_dir = dir.join("graph");
    std::fs::create_dir_all(&graph_dir)?;

    let mut scan_files: Vec<PathBuf> = std::fs::read_dir(&scans_dir)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", scans_dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    scan_files.sort();

    let (mut tot_entities, mut tot_actions, mut tot_features) = (0usize, 0usize, 0usize);
    let (mut tot_edges, mut tot_ext, mut tot_unresolved) = (0usize, 0usize, 0usize);
    let mut tot_clusters = 0usize;
    let mut tot_flows = 0usize;
    for scan_path in &scan_files {
        let id = scan_path.file_stem().and_then(|s| s.to_str()).unwrap_or("system");
        let data = std::fs::read_to_string(scan_path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", scan_path.display()))?;
        let scan: ScanResult = serde_json::from_str(&data)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", scan_path.display()))?;

        let report = fdml_graph::run(&scan);
        let out = graph_dir.join(format!("{id}.json"));
        std::fs::write(&out, serde_json::to_string_pretty(&report)?)?;

        // Phase 3B.1: resolve raw imports into real cross-file dependency edges.
        // The system's files live under its real root (the scan's codebase_path),
        // which the resolver reads for tsconfig.json / go.mod.
        let root = Path::new(&scan.metadata.codebase_path);
        let mut dep = fdml_graph::resolve_imports(&scan, root);
        fdml_graph::resolve_calls(&scan, &mut dep); // import-level → call-level edges

        let dep_out = graph_dir.join(format!("{id}.depgraph.json"));
        std::fs::write(&dep_out, serde_json::to_string_pretty(&dep)?)?;

        // Phase 3B.2: cluster the dependency graph into a readable map of named boxes.
        // Phase 3B.3: tag each cluster with its dominant architectural roles.
        let mut clusters = fdml_graph::cluster(&dep);
        fdml_graph::tag_roles(&scan, &mut clusters);
        let cl_out = graph_dir.join(format!("{id}.clusters.json"));
        std::fs::write(&cl_out, serde_json::to_string_pretty(&clusters)?)?;

        // Phase 4: reconstruct ingress->sink data flows over the dependency graph.
        let flows = fdml_flows::run(&report, &dep, &scan);
        let fl_out = graph_dir.join(format!("{id}.flows.json"));
        std::fs::write(&fl_out, serde_json::to_string_pretty(&flows)?)?;

        tot_flows += flows.len();
        tot_entities += report.entities.len();
        tot_actions += report.actions.len();
        tot_features += report.features.len();
        tot_edges += dep.edges.len();
        tot_ext += dep.stats.external_imports;
        tot_unresolved += dep.stats.unresolved_imports;
        tot_clusters += clusters.clusters.len();
        println!(
            "{:<24} entities={} actions={} features={} edges={} ext={} unresolved={} clusters={} flows={} -> {}",
            id,
            report.entities.len(),
            report.actions.len(),
            report.features.len(),
            dep.edges.len(),
            dep.stats.external_imports,
            dep.stats.unresolved_imports,
            clusters.clusters.len(),
            flows.len(),
            out.display(),
        );
        // Phase 3B.3: a labeled, role-tagged map — one line per cluster.
        for c in &clusters.clusters {
            let roles = if c.roles.is_empty() { "—".to_string() } else { c.roles.join(", ") };
            println!("    {:<4} {:<20} size={:<3} roles=[{}]", c.id, c.name, c.size, roles);
        }
    }
    println!(
        "total: {} system(s), entities={} actions={} features={} edges={} ext={} unresolved={} clusters={} flows={}",
        scan_files.len(), tot_entities, tot_actions, tot_features, tot_edges, tot_ext, tot_unresolved, tot_clusters, tot_flows,
    );
    Ok(())
}

/// Pass 2 driver. If `<dir>/systems.json` exists, scan each system (excluding nested
/// sub-systems so the root system doesn't re-scan them) and write `<dir>/scans/<id>.json`;
/// otherwise scan `<root>` as a single ScanResult.
fn run_scan(root: &Path, dir: &Path, exclude: &[String]) -> anyhow::Result<()> {
    let systems_path = dir.join("systems.json");
    let scans_dir = dir.join("scans");
    std::fs::create_dir_all(&scans_dir)?;

    if systems_path.exists() {
        let data = std::fs::read_to_string(&systems_path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", systems_path.display()))?;
        let systems: Vec<System> = serde_json::from_str(&data)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", systems_path.display()))?;

        let (mut tot_files, mut tot_classes, mut tot_functions) = (0usize, 0usize, 0usize);
        for system in &systems {
            // A system whose path contains other systems (notably the root, path ".")
            // must not re-scan them — exclude each nested system's own directory.
            let mut sys_exclude = exclude.to_vec();
            sys_exclude.extend(nested_excludes(system, &systems));

            let scan = fdml_scan::run(&root.join(&system.path), &sys_exclude)?;
            let out = scans_dir.join(format!("{}.json", system.id));
            std::fs::write(&out, serde_json::to_string_pretty(&scan)?)?;

            tot_files += scan.metadata.total_files;
            tot_classes += scan.statistics.classes;
            tot_functions += scan.statistics.functions;
            println!(
                "{:<24} files={} classes={} functions={} -> {}",
                system.id,
                scan.metadata.total_files,
                scan.statistics.classes,
                scan.statistics.functions,
                out.display(),
            );
        }
        println!(
            "total: {} system(s), files={} classes={} functions={}",
            systems.len(), tot_files, tot_classes, tot_functions,
        );
    } else {
        let scan = fdml_scan::run(root, exclude)?;
        let out = scans_dir.join("_root.json");
        std::fs::write(&out, serde_json::to_string_pretty(&scan)?)?;
        print_stats(&scan);
        eprintln!("no systems.json in {} — scanned root as one system -> {}", dir.display(), out.display());
    }
    Ok(())
}

/// Directory names of other systems nested inside `system` (so it skips them when scanned).
fn nested_excludes(system: &System, all: &[System]) -> Vec<String> {
    all.iter()
        .filter(|o| o.id != system.id && is_nested(&o.path, &system.path))
        .filter_map(|o| o.path.rsplit('/').next().map(str::to_string))
        .collect()
}

/// Is `child` path nested inside `parent`? (root path "." contains everything.)
fn is_nested(child: &str, parent: &str) -> bool {
    if parent == "." {
        child != "."
    } else {
        child.starts_with(&format!("{parent}/"))
    }
}

/// Print a compact one-screen summary of a single ScanResult.
fn print_stats(scan: &ScanResult) {
    let s = &scan.statistics;
    println!(
        "files={} classes={} functions={} methods={} interfaces={} enums={} fields={} imports(ext/int)={}/{} relationships={}",
        scan.metadata.total_files,
        s.classes, s.functions, s.methods, s.interfaces, s.enums, s.fields,
        s.imports_external, s.imports_internal, s.relationships,
    );
}

/// Render systems as an FDML-compatible `systems:` YAML block. Presentation lives here,
/// not in `fdml-types`. Assemble (phase 6) owns the full FDML spec emission.
fn systems_to_fdml_yaml(systems: &[System]) -> String {
    let mut out = String::from("systems:\n");
    for s in systems {
        out.push_str(&format!(
            "  - id: {}\n    name: \"{}\"\n    path: {}\n    type: {}\n    technology: \"{}\"\n    boundary_marker: {}\n",
            s.id, s.name, s.path, s.system_type, s.technology, s.boundary_marker,
        ));
    }
    out
}
