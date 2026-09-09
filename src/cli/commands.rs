use crate::cli::args::{Cli, Commands, MigrateCommands, TraceCommands, AddCommands, ListCommands};
use crate::error::{print_error, print_info, print_success, print_warning, Result};
use crate::parser::{parse_fdml_yaml, parse_fdml};
use crate::project::ProjectInitializer;
use crate::validator::Validator;
use crate::generators::{create_generator, GeneratorConfig};
use crate::generators::test_gen::TestGenerator;
use crate::migration::{MigrationRunner, Migration, MigrationOperation};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CommandRunner {
    verbose: bool,
}

impl CommandRunner {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
    
    pub fn run(&self, cli: Cli) -> Result<()> {
        match cli.command {
            Commands::Init { name, force } => self.run_init(name, force),
            Commands::Parse { file, output, debug } => self.run_parse(file, output, debug),
            Commands::Validate { file, strict, output } => self.run_validate(file, strict, output),
            Commands::Generate { input, language, output, template, with_tests } => {
                self.run_generate(input, language, output, template, with_tests)
            },
            Commands::Add { operation } => self.run_add(operation),
            Commands::List { operation } => self.run_list(operation),
            Commands::Migrate { operation } => self.run_migrate(operation),
            Commands::Trace { operation } => self.run_trace(operation),
            Commands::Serve { file, port, no_open, generate, fast, model, provider, parallel, ollama_url, num_ctx, chunk_strategy } => {
                self.run_serve(file, port, no_open, generate, fast, model, provider, parallel, ollama_url, num_ctx, chunk_strategy)
            },
            Commands::ParseCode { input, output, format, exclude } => {
                self.run_parse_code(input, output, format, exclude)
            },
            Commands::LinkCode { code, fdml, output, format, llm, no_llm, skip_scenarios, fast, model, provider, ollama_url, num_ctx, chunk_strategy } => {
                self.run_link_code(code, fdml, output, format, llm, no_llm, skip_scenarios, fast, model, provider, ollama_url, num_ctx, chunk_strategy)
            },
            Commands::ScanPlatform { input, output, format, exclude, llm, skip_scenarios, fast, model, provider, ollama_url, num_ctx, chunk_strategy } => {
                self.run_scan_platform(input, output, format, exclude, llm, skip_scenarios, fast, model, provider, ollama_url, num_ctx, chunk_strategy)
            },
            Commands::Index { path } => self.run_index(path),
            Commands::Search { query, path, limit, long, flow, llm, model, ollama_url, json } => self.run_index_search(query, path, limit, long, flow, llm, model, ollama_url, json),
            Commands::Get { symbol, path, json } => self.run_index_get(symbol, path, json),
            Commands::Note { phrase, body, kind, target, aliases, list, delete, path, json } => self.run_note(phrase, body, kind, target, aliases, list, delete, path, json),
            Commands::Dossier { commit, path, json } => self.run_dossier(commit, path, json),
            Commands::Report { out, path } => self.run_report(out, path),
            Commands::History { limit, path, json } => self.run_history(limit, path, json),
            Commands::Candidates { min, since, limit, path, json } => self.run_candidates(min, since, limit, path, json),
            Commands::Heal { apply, limit, min_fails, model, ollama_url, path, json } => self.run_heal(apply, limit, min_fails, model, ollama_url, path, json),
            Commands::Skill { install, global, path } => self.run_skill(install, global, path),
            Commands::Log { path, limit, json } => self.run_index_log(path, limit, json),
            Commands::Facts { symbol, import, provider, path, json } => self.run_index_facts(symbol, import, provider, path, json),
            Commands::Outline { symbol, path, json } => self.run_index_outline(symbol, path, json),
            Commands::Impact { symbol, path, json } => self.run_index_impact(symbol, path, json),
            Commands::Status { path, json } => self.run_index_status(path, json),
            Commands::Mark { query, symbol, aliases, path, model, ollama_url, limit, force } => self.run_index_mark(query, symbol, aliases, path, model, ollama_url, limit, force),
        }
    }

    fn index_root(path: Option<String>) -> Result<PathBuf> {
        let path = path.unwrap_or_else(|| ".".to_string());
        std::fs::canonicalize(&path).map_err(|e| crate::error::FdmlError::project_error(format!("Cannot access repository '{}': {e}", path)))
    }

    fn run_index(&self, path: String) -> Result<()> {
        let root = Self::index_root(Some(path))?;
        let report = crate::index::Indexer::index(&root).map_err(crate::error::FdmlError::project_error)?;
        println!("Indexed {}: {} parsed, {} unchanged, {} removed, {} symbols", root.display(), report.parsed_files, report.unchanged_files, report.removed_files, report.symbols);
        Ok(())
    }

    fn run_index_search(&self, query: String, path: Option<String>, limit: usize, long: bool, flow: bool, llm: bool, model: String, ollama_url: String, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let mut index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        // Self-healing beats a watcher: a stat per file is cheap, an incremental
        // reindex is milliseconds, and an answer from a stale index is worse than a
        // slow one. No daemon to keep alive, no hook to configure per repository.
        let stale = index.stale_files();
        if stale > 0 {
            drop(index);
            let report = crate::index::Indexer::index(&root).map_err(crate::error::FdmlError::project_error)?;
            if !json { println!("(reindexed {} file(s) — the index was behind the code)", report.parsed_files.max(stale)); }
            index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        }
        let mut results = index.search(&query).map_err(crate::error::FdmlError::project_error)?;
        // M2: the honest signal. Below the threshold the deterministic layer says so —
        // and only then may the experimental LLM fallback spend its seconds.
        let total = results.len();
        results.truncate(limit.max(1));
        let useful = results.first().is_some_and(|r| r.score >= crate::index::USEFUL_SCORE);
        let mut rescued = false;
        if !useful && llm {
            let picks = index.llm_fallback(&query, &ollama_url, &model).map_err(crate::error::FdmlError::project_error)?;
            rescued = !picks.is_empty();
            results.splice(0..0, picks);
        }
        // every call feeds the tooling-improvement loop; a failure here is a case, not noise
        let _ = index.log_query(&query, results.first().map(|r| r.score), results.len(), useful || rescued, llm, rescued);
        // `--flow` answers the follow-up question the results raise: how is the best match reached?
        let flows = match (flow, results.first()) {
            (true, Some(top)) => index.flows(&top.qualified_name).unwrap_or_default(),
            _ => Vec::new(),
        };
        if json {
            let payload = if flow { serde_json::json!({ "results": results, "flows": flows }) } else { serde_json::to_value(&results).unwrap() };
            println!("{}", serde_json::to_string_pretty(&payload).unwrap());
        } else {
            if !useful && !results.iter().any(|r| r.kind == "llm-fallback") {
                // an empty answer from a stale index is not an answer — say which it is
                println!("⚠ no useful result{} — fall back to grep{}\n", results.first().map(|r| format!(" (top score {:.2})", r.score)).unwrap_or_default(), if llm { "" } else { ", or retry with --llm" });
            }
            for r in &results {
                if long {
                    // the location to read comes first: that is what the caller does next
                    println!("{}:{}  read {}-{}{}", r.file, r.start_line, r.window[0], r.window[1], if r.marked { "  [marked]" } else { "" });
                    println!("{}  {}  score: {:.2}", r.qualified_name, r.kind, r.score);
                } else if r.file.is_empty() {
                    // a note with no anchor has no place to point at; name it instead
                    println!("{}", r.qualified_name);
                } else {
                    // grep shape: one line per hit, `path:line: source`. An agent reaching
                    // for grep out of habit gets the same silhouette, for fewer bytes.
                    let line = Self::source_line(&root, &r.file, r.start_line);
                    println!("{}:{}:{}{}", r.file, r.start_line, line, if r.marked { "  [marked]" } else { "" });
                }
                for n in &r.notes { println!("  ↳ [{}] {}{}", n.kind, n.body.lines().next().unwrap_or(""), if n.stale { "  ⚠ stale" } else { "" }); }
                if r.oversized { println!("  ⚠ god function: {} lines — use `fdml outline {}`", r.body_lines, r.qualified_name); }
                if long { println!(); }
            }
            if total > results.len() { println!("({} more — raise with --limit)\n", total - results.len()); }
            for f in &flows { println!("FLOW (entry {}, depth {})\n  {}{}\n", f.entry, f.depth, f.chain.join(" -> "), if f.next.is_empty() { String::new() } else { format!(" -> [{}]", f.next.join(", ")) }); }
        }
        Ok(())
    }

    fn run_index_get(&self, symbol: String, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let source = index.get(&symbol).map_err(crate::error::FdmlError::project_error)?;
        let _ = index.log_command("get", &symbol, None, 1, true, false, false);
        if json { println!("{}", serde_json::to_string_pretty(&source).unwrap()); }
        else { println!("{}\n{}:{}-{}\n{}\n{}", source.qualified_name, source.file, source.start_line, source.end_line, source.signature.as_deref().unwrap_or(""), source.source); }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn run_note(&self, phrase: Option<String>, body: Option<String>, kind: String, target: Option<String>, aliases: Vec<String>, list: bool, delete: bool, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let show = |notes: &[crate::index::Note]| {
            for n in notes {
                println!("[{}] {}{}", n.kind, n.phrase, if n.stale { "  ⚠ stale: file changed since this was recorded" } else { "" });
                for line in n.body.lines() { println!("    {line}"); }
                println!("    · {}  · {}{}\n", n.target.clone().unwrap_or_else(|| "—".into()), n.created_at,
                    n.commit_sha.as_ref().map(|c| format!("  · commit {c}")).unwrap_or_default());
            }
        };
        if delete {
            let phrase = phrase.ok_or_else(|| crate::error::FdmlError::project_error("give the phrase of the note to delete"))?;
            let n = index.delete_notes(&phrase).map_err(crate::error::FdmlError::project_error)?;
            let _ = index.log_command("note:delete", &phrase, None, n, n > 0, false, false);
            println!("{}", if n == 0 { format!("no note matches \"{phrase}\"") } else { format!("Deleted {n} row(s) for \"{phrase}\"") });
            return Ok(());
        }
        match (phrase, body) {
            (Some(phrase), Some(body)) => {
                let n = index.add_note(&phrase, &body, &kind, target.as_deref(), &aliases).map_err(crate::error::FdmlError::project_error)?;
                let _ = index.log_command(&format!("note:{kind}"), &phrase, None, n, true, false, false);
                println!("Recorded {kind} \"{phrase}\"{} ({n} phrasing{})", target.map(|t| format!(" @ {t}")).unwrap_or_default(), if n == 1 { "" } else { "s" });
            }
            (Some(query), None) if !list => {
                let tokens: Vec<String> = query.to_lowercase().split(|c: char| !c.is_alphanumeric() && c != '_').filter(|t| !t.is_empty()).map(str::to_string).collect();
                let notes = index.notes_for_query(&tokens).map_err(crate::error::FdmlError::project_error)?;
                let _ = index.log_command("note:read", &query, None, notes.len(), !notes.is_empty(), false, false);
                if json { println!("{}", serde_json::to_string_pretty(&notes).unwrap()); }
                else if notes.is_empty() { println!("no note matches \"{query}\" — record one with `fdml note \"<symptom>\" \"<what you learned>\"`"); }
                else { show(&notes); }
            }
            _ => {
                let filter = if kind == "note" { None } else { Some(kind.as_str()) };
                let notes = index.list_notes(filter, 20).map_err(crate::error::FdmlError::project_error)?;
                if json { println!("{}", serde_json::to_string_pretty(&notes).unwrap()); }
                else if notes.is_empty() { println!("no notes recorded yet"); }
                else { show(&notes); }
            }
        }
        Ok(())
    }

    /// The source line behind a hit, so a result reads like a grep result.
    fn source_line(root: &std::path::Path, file: &str, line: usize) -> String {
        if line == 0 { return String::new() }
        std::fs::read_to_string(root.join(file)).ok()
            .and_then(|t| t.lines().nth(line - 1).map(|l| format!(" {}", l.trim())))
            .map(|l| l.chars().take(120).collect())
            .unwrap_or_default()
    }

    /// One page over the two layers. Nothing is computed here that a command does not
    /// already answer — the page is a join of existing JSON, not a third source of truth.
    fn run_report(&self, out: String, path: Option<String>) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let status = index.status().map_err(crate::error::FdmlError::project_error)?;
        let (total, failed, llm_used, llm_rescued, fails, episodes) = index.query_report(40).map_err(crate::error::FdmlError::project_error)?;
        let data = serde_json::json!({
            "status": status,
            "log": {"queries": total, "failed": failed, "llm_used": llm_used, "llm_rescued": llm_rescued,
                    "cases": fails.iter().map(|(q, n, at)| serde_json::json!({"query": q, "times": n, "last": at})).collect::<Vec<_>>(),
                    "retry_episodes": episodes},
            "history": Self::history_rows(&index, &root, 200),
            "tree": index.file_tree().unwrap_or_default(),
            "candidates": index.tool_candidates(3, 12, None).unwrap_or_default(),
            "notes": index.list_notes(None, 999).unwrap_or_default(),
            "marks": index.list_marks().unwrap_or_default(),
            "symbols": index.symbols_dump().unwrap_or_default(),
        });
        const TEMPLATE: &str = include_str!("report.html");
        let page = TEMPLATE.replace("{{DATA}}", &serde_json::to_string(&data).unwrap().replace("</", "<\\/"));
        let target = std::path::Path::new(&out);
        std::fs::write(target, page).map_err(|e| crate::error::FdmlError::project_error(format!("cannot write {}: {e}", target.display())))?;
        let _ = index.log_command("report", "", None, 1, true, false, false);
        println!("{} — index layer and log layer on one page", target.display());
        Ok(())
    }

    /// Commits joined to the sessions that produced them, shared by `history` and `report`.
    fn history_rows(index: &crate::index::RepositoryIndex, root: &std::path::Path, limit: usize) -> Vec<serde_json::Value> {
        let spans = index.sessions().unwrap_or_default();
        // --name-only rides along: the files a commit touched are what lights the map up
        let Ok(out) = std::process::Command::new("git")
            .args(["-C", &root.display().to_string(), "log", &format!("-{limit}"), "--name-only", "--format=%x1e%h%x1f%s%x1f%at%x1f%p%x1f"]).output() else { return Vec::new() };
        let text = String::from_utf8_lossy(&out.stdout);
        let mut rows = Vec::new();
        for entry in text.split('\x1e') {
            let f: Vec<&str> = entry.split('\x1f').collect();
            if f.len() < 5 || f[0].is_empty() { continue }
            let (sha, subject) = (f[0], f[1]);
            let (scope, title) = subject.split_once(": ").unwrap_or(("", subject));
            let at: i64 = f[2].trim().parse().unwrap_or(0);
            let span = spans.iter().find(|s| at >= s.start && at <= s.end);
            let files: Vec<&str> = f[4].lines().filter(|l| !l.trim().is_empty()).collect();
            rows.push(serde_json::json!({"commit": sha, "kind": Self::work_kind(scope), "scope": scope, "title": title, "at": at,
                "parents": f[3].split_whitespace().count(), "files": files,
                "ask": span.map(|s| s.first_ask.clone()), "prompts": span.map(|s| s.prompts),
                "notes": index.note_count(sha).unwrap_or(0)}));
        }
        rows
    }

    /// Three kinds of work, because three is what a colour scale can carry honestly:
    /// everything past the third slot fails the all-pairs floors, so it folds to "other".
    fn work_kind(scope: &str) -> &'static str {
        let s = scope.split('(').next().unwrap_or("").trim();
        match s { "feat" => "feat", "fix" => "fix", "refactor" | "perf" => "refactor", _ => "other" }
    }

    fn run_history(&self, limit: usize, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let rows = Self::history_rows(&index, &root, limit);
        let _ = index.log_command("history", "", None, rows.len(), !rows.is_empty(), false, false);
        if json { println!("{}", serde_json::to_string_pretty(&rows).unwrap()); return Ok(()); }
        for r in &rows {
            println!("{}  {:<18}{}", r["commit"].as_str().unwrap_or(""), r["scope"].as_str().filter(|s| !s.is_empty()).unwrap_or("—"), r["title"].as_str().unwrap_or(""));
            match r["ask"].as_str() {
                // the ask is evidence, not a label: the kind of work is already in the commit type
                Some(ask) => println!("         «{ask}»  · {} prompts · {} notes", r["prompts"], r["notes"]),
                None => println!("         no session linked · {} notes", r["notes"]),
            }
        }
        Ok(())
    }

    fn run_candidates(&self, min: usize, since: Option<String>, limit: usize, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let found = index.tool_candidates(min, limit, since.as_deref()).map_err(crate::error::FdmlError::project_error)?;
        if let Some(s) = &since { println!("since {s} (UTC)"); }
        let _ = index.log_command("candidates", "", None, found.len(), !found.is_empty(), false, false);
        if json { println!("{}", serde_json::to_string_pretty(&found).unwrap()); return Ok(()); }
        if found.is_empty() { println!("nothing repeated {min}+ times in this repo's transcripts — no tool is missing yet"); return Ok(()); }
        println!("Shell work done by hand repeatedly — each one is a tool that should exist:\n");
        for c in &found {
            println!("x{:<4} {}", c.times, c.shape);
            println!("      e.g. {}", c.example.lines().next().unwrap_or("").chars().take(100).collect::<String>());
            if !c.last_seen.is_empty() { println!("      last {}", c.last_seen.replace('T', " ")); }
            println!();
        }
        Ok(())
    }

    fn run_dossier(&self, commit: Option<String>, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let explicit = commit.is_some();
        let commit = commit.unwrap_or_else(|| {
            std::process::Command::new("git").args(["-C", &root.display().to_string(), "rev-parse", "--short", "HEAD"]).output().ok()
                .filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
        });
        let mut d = index.dossier(&commit).map_err(crate::error::FdmlError::project_error)?;
        // "Where were we" must survive the next commit: an empty card at HEAD means
        // the work was recorded earlier, so fall back to the last commit that has one.
        let mut fell_back = false;
        if !explicit && d.is_empty() {
            let prev = index.list_notes(None, 1).ok().and_then(|n| n.into_iter().next()).and_then(|n| n.commit_sha);
            if let Some(prev) = prev.filter(|p| !p.is_empty() && *p != commit) {
                d = index.dossier(&prev).map_err(crate::error::FdmlError::project_error)?;
                fell_back = true;
            }
        }
        let _ = index.log_command("dossier", &commit, None, d.anchors.len(), d.missing.is_empty(), false, false);
        if json { println!("{}", serde_json::to_string_pretty(&d).unwrap()); return Ok(()); }
        let block = |label: &str, notes: &[crate::index::Note]| {
            for (i, n) in notes.iter().enumerate() {
                let head = if i == 0 { label } else { "" };
                for (j, line) in n.body.lines().enumerate() {
                    println!("{:<9}{}{}", if j == 0 { head } else { "" }, line, if j == 0 && n.stale { "  ⚠ stale" } else { "" });
                }
            }
        };
        println!("DOSSIER  {}{}
", d.commit, if fell_back { format!("  (nothing recorded at {commit} — showing the last card)") } else { String::new() });
        for (i, a) in d.anchors.iter().enumerate() { println!("{:<9}{a}", if i == 0 { "ANCHOR" } else { "" }); }
        if !d.flow.is_empty() { println!("{:<9}{}", "FLOW", d.flow.join(" → ")); }
        block("STATE", &d.state);
        block("NOTE", &d.numbers);
        block("REJECTED", &d.rejected);
        block("VERIFY", &d.verify);
        block("PLAYBOOK", &d.playbook);
        block("PENDING", &d.pending);
        block("LINKS", &d.links);
        for (i, s) in d.symptoms.iter().enumerate() { println!("{:<9}«{s}»", if i == 0 { "SYMPTOMS" } else { "" }); }
        if !d.missing.is_empty() {
            println!("
MISSING — the card has holes, which is worth knowing:");
            for m in &d.missing { println!("  · {m}"); }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn run_heal(&self, apply: bool, limit: usize, min_fails: usize, model: String, ollama_url: String, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let proposals = index.heal(&ollama_url, &model, apply, limit, min_fails).map_err(crate::error::FdmlError::project_error)?;
        let _ = index.log_command(if apply { "heal:apply" } else { "heal" }, "", None, proposals.len(), !proposals.is_empty(), true, proposals.iter().any(|p| p.applied));
        if json { println!("{}", serde_json::to_string_pretty(&proposals).unwrap()); return Ok(()); }
        if proposals.is_empty() { println!("nothing to heal — no repeated unresolved failures, or no evidence found"); return Ok(()); }
        for p in &proposals {
            println!("{} \"{}\" -> {}", if p.applied { "✓ marked " } else { "proposed" }, p.query, p.target);
            if !p.reason.is_empty() { println!("           {}", p.reason); }
        }
        if !apply { println!("\ndry-run: re-run with --apply to write these marks"); }
        Ok(())
    }

    fn run_skill(&self, install: bool, global: bool, path: Option<String>) -> Result<()> {
        // The canonical skill ships inside the binary, so any repo can activate the
        // index-first navigation loop without hunting for files.
        // Embedded at build time so any repo can be activated; but when the canonical
        // file is present on disk (developing FDML itself), prefer it — installing a
        // skill that is one rebuild stale is exactly how the text drifts from the tool.
        const EMBEDDED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/.claude/skills/fdml-nav/SKILL.md"));
        let canonical = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".claude/skills/fdml-nav/SKILL.md");
        let skill = std::fs::read_to_string(&canonical).unwrap_or_else(|_| EMBEDDED.to_string());
        let home = std::env::var("HOME").unwrap_or_default();
        let user_path = std::path::Path::new(&home).join(".claude/skills/fdml-nav/SKILL.md");
        let project_root = Self::index_root(path)?;
        let project_path = project_root.join(".claude/skills/fdml-nav/SKILL.md");
        if install {
            let target = if global { &user_path } else { &project_path };
            if let Some(dir) = target.parent() { std::fs::create_dir_all(dir).map_err(|e| crate::error::FdmlError::project_error(format!("cannot create {}: {e}", dir.display())))?; }
            std::fs::write(target, &skill).map_err(|e| crate::error::FdmlError::project_error(format!("cannot write {}: {e}", target.display())))?;
            println!("Installed fdml-nav skill -> {}", target.display());
            println!("New Claude Code sessions in {} will search the index before grepping.", if global { "any repository" } else { "this repository" });
            // A skill is loaded when its description matches; CLAUDE.md is loaded
            // always. Repos that mention the tool there actually use it — the ones
            // relying on the skill alone see it ignored.
            let claude_md = project_root.join("CLAUDE.md");
            let mentioned = std::fs::read_to_string(&claude_md).map(|t| t.contains("fdml")).unwrap_or(false);
            if !global && !mentioned {
                println!("\nCLAUDE.md does not mention the index. A skill only loads when its\ndescription matches; CLAUDE.md is always in context. Consider adding:\n");
                println!("    ## Navigate with the index, not with grep");
                println!("    `fdml search \"<query>\" --limit 3` before grepping; `fdml outline <symbol>`");
                println!("    for huge functions; `fdml note \"<symptom>\"` for why it broke last time;");
                println!("    `fdml dossier` to pick up where the last session stopped.");
            }
            return Ok(());
        }
        let mark = |p: &std::path::Path| if p.exists() { "✓ active " } else { "✗ missing" };
        println!("fdml-nav skill status for {}:", project_root.display());
        println!("  {} project  {}", mark(&project_path), project_path.display());
        println!("  {} global   {}", mark(&user_path), user_path.display());
        if !project_path.exists() && !user_path.exists() {
            println!("\nnot active anywhere — run `fdml skill --install` (this repo) or `fdml skill --install --global` (all repos)");
        }
        Ok(())
    }

    fn run_index_log(&self, path: Option<String>, limit: usize, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let (total, failed, llm_used, llm_rescued, fails, episodes) = index.query_report(limit).map_err(crate::error::FdmlError::project_error)?;
        if json {
            println!("{}", serde_json::json!({"queries": total, "failed": failed, "llm_used": llm_used, "llm_rescued": llm_rescued,
                "cases": fails.iter().map(|(q,n,at)| serde_json::json!({"query": q, "times": n, "last": at})).collect::<Vec<_>>(),
                "retry_episodes": episodes}));
        } else {
            let rate = if total > 0 { 100 - 100 * failed / total } else { 0 };
            println!("searches: {total}   useful: {rate}%   failed: {failed}   llm fallback: {llm_used} fired / {llm_rescued} rescued");
            let usage = index.command_usage().unwrap_or_default();
            if usage.len() > 1 {
                println!("commands: {}", usage.iter().map(|(c, n)| format!("{c} {n}")).collect::<Vec<_>>().join("  "));
            }
            println!();
            if fails.is_empty() { println!("no failed queries accumulated — nothing to improve yet"); }
            else { println!("CASES (failed queries — what this project needs the tool to learn):"); for (q, n, at) in &fails { println!("  x{n:<3} {q}   (last: {at})"); } }
            if !episodes.is_empty() {
                println!("\nRETRY EPISODES (the agent rephrasing in frustration — strongest passive signal):");
                for chain in &episodes { println!("  {}", chain.join("  ->  ")); }
            }
        }
        Ok(())
    }

    fn run_index_facts(&self, symbol: Option<String>, import: Option<String>, provider: String, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        if let Some(file) = import {
            let text = std::fs::read_to_string(&file).map_err(|e| crate::error::FdmlError::project_error(format!("Cannot read '{file}': {e}")))?;
            let doc: serde_json::Value = serde_json::from_str(&text).map_err(|e| crate::error::FdmlError::project_error(format!("'{file}' is not valid JSON: {e}")))?;
            let (kept, skipped) = index.import_facts(&doc, &provider).map_err(crate::error::FdmlError::project_error)?;
            let _ = index.log_command("facts:import", &provider, None, kept, kept > 0, false, false);
            println!("Imported {kept} facts from {file}{}", if skipped > 0 { format!(" ({skipped} entries had no identifiable target)") } else { String::new() });
            return Ok(());
        }
        let symbol = symbol.ok_or_else(|| crate::error::FdmlError::project_error("give a symbol to read, or --import a facts file"))?;
        let facts = index.facts_for(&symbol).map_err(crate::error::FdmlError::project_error)?;
        if json { println!("{}", serde_json::to_string_pretty(&facts).unwrap()); }
        else if facts.is_empty() { println!("No facts recorded for {symbol}"); }
        else { for f in &facts { println!("{} · {} · {}\n  {}\n", f.fact_kind, f.provider, f.confidence, f.payload); } }
        Ok(())
    }

    fn run_index_outline(&self, symbol: String, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let outline = index.outline(&symbol).map_err(crate::error::FdmlError::project_error)?;
        let _ = index.log_command("outline", &symbol, None, outline.phases.len(), !outline.phases.is_empty(), false, false);
        if json { println!("{}", serde_json::to_string_pretty(&outline).unwrap()); }
        else {
            println!("{}\n{}:{}-{}  {} lines, {} internal call sites, {} phases\n", outline.symbol, outline.file, outline.start_line, outline.end_line, outline.body_lines, outline.call_sites, outline.phases.len());
            for p in &outline.phases {
                println!("{}-{}  {}", p.line, p.end_line, p.label.as_deref().unwrap_or("—"));
                println!("    {}", p.calls.join(", "));
            }
        }
        Ok(())
    }

    fn run_index_impact(&self, symbol: String, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        let impact = index.impact(&symbol).map_err(crate::error::FdmlError::project_error)?;
        let _ = index.log_command("impact", &symbol, None, impact.callers.len() + impact.callees.len(), true, false, false);
        if json { println!("{}", serde_json::to_string_pretty(&impact).unwrap()); }
        else { println!("{}\n\nCALLERS\n{}\n\nCALLEES / DEPENDS ON\n{}\n\nIMPORTS\n{}\n\nIMPLEMENTATIONS\n{}\n\nTESTS\n{}", impact.symbol, impact.callers.join("\n"), impact.callees.join("\n"), impact.imports.join("\n"), impact.implementations.join("\n"), impact.tests.join("\n")); }
        Ok(())
    }

    fn run_index_status(&self, path: Option<String>, json: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let status = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?.status().map_err(crate::error::FdmlError::project_error)?;
        if json { println!("{}", serde_json::to_string_pretty(&status).unwrap()); }
        else { println!("Repository: {}\nFiles: {}\nSymbols: {}\nReferences: {}\nMarks: {}\nLast indexed: {}\nIndex size: {} bytes", status.root_path, status.files, status.symbols, status.references, status.marks, status.last_indexed.unwrap_or_else(|| "never".into()), status.index_size); }
        Ok(())
    }

    fn run_index_mark(&self, query: Option<String>, symbol: Option<String>, aliases: Vec<String>, path: Option<String>, model: String, ollama_url: String, limit: usize, force: bool) -> Result<()> {
        let root = Self::index_root(path)?;
        let index = crate::index::RepositoryIndex::open(&root).map_err(crate::error::FdmlError::project_error)?;
        match (query, symbol) {
            (Some(query), Some(symbol)) => {
                let target = index.mark_association(&query, &symbol).map_err(crate::error::FdmlError::project_error)?;
                println!("Marked \"{query}\" -> {target}");
                let _ = index.log_command("mark", &query, None, 1 + aliases.len(), true, false, false);
                // one place, several ways to ask for it — the lexical key needs each
                for alias in &aliases {
                    index.mark_association(alias, &target).map_err(crate::error::FdmlError::project_error)?;
                    println!("     +  \"{alias}\"");
                }
            }
            (Some(_), None) => return Err(crate::error::FdmlError::project_error("`fdml mark <query> <symbol>` needs both arguments; use `fdml mark` alone for local-LLM descriptions")),
            _ => {
                let marked = index.mark(&model, &ollama_url, limit, force).map_err(crate::error::FdmlError::project_error)?;
                println!("Marked {marked} symbols with {model}");
            }
        }
        Ok(())
    }
    
    fn run_init(&self, name: String, force: bool) -> Result<()> {
        if self.verbose {
            print_info(&format!("Initializing FDML project: {}", name));
        }
        
        if force {
            print_warning("Force flag is not yet implemented - directory must not exist");
        }
        
        let initializer = ProjectInitializer::new(name.clone());
        initializer.initialize()?;
        
        print_success(&format!("Successfully initialized FDML project: {}", name));
        print_info("Next steps:");
        println!("  1. cd {}", name);
        println!("  2. fdml validate specs/example.fdml");
        println!("  3. Edit specs/example.fdml to match your needs");
        
        Ok(())
    }
    
    fn run_validate(&self, file: String, strict: bool, output: String) -> Result<()> {
        if self.verbose {
            print_info(&format!("Validating FDML file: {}", file));
        }
        
        // Read the file
        let content = fs::read_to_string(&file).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", file, e))
        })?;
        
        // Parse the FDML document
        let document = parse_fdml_yaml(&content)?;
        
        if self.verbose {
            print_info("Parsing completed successfully");
        }
        
        // Document validation: this file on its own
        let validator = Validator::new();
        let mut validation_errors = validator.validate(&document)?;

        // Project validation: links resolve against the document set (vision + generated),
        // and an id may exist in only one document. A reference the file alone cannot
        // place but the project can is not a warning — that is what the set is for.
        let set = crate::project::document_set::DocumentSet::around(std::path::Path::new(&file))?;
        if set.docs.len() > 1 {
            let project_ids = set.all_ids();
            validation_errors.retain(|e| !e.starts_with("Traceability references unknown")
                || !project_ids.iter().any(|id| e.ends_with(&format!("'{id}'"))));
            validation_errors.extend(set.collisions());
            if self.verbose {
                print_info(&format!("project set: {} documents", set.docs.len()));
            }
        } else {
            // alone, a vision file pointing at generated ids gets the hint, not a shrug
            validation_errors.extend(set.unresolved().into_iter().filter(|m| m.contains("no generated spec")));
        }
        
        // Output results
        match output.as_str() {
            "json" => self.output_json_results(&validation_errors)?,
            "text" | _ => self.output_text_results(&file, &validation_errors, strict)?,
        }
        
        // Return error if strict mode and there are validation errors
        if strict && !validation_errors.is_empty() {
            return Err(crate::error::FdmlError::validation_error(
                "Validation failed in strict mode"
            ));
        }
        
        Ok(())
    }
    
    fn output_text_results(&self, file: &str, errors: &[String], strict: bool) -> Result<()> {
        if errors.is_empty() {
            print_success(&format!("✓ {} is valid", file));
        } else {
            if strict {
                print_error(&crate::error::FdmlError::validation_error(
                    format!("Validation failed for {}", file)
                ));
            } else {
                print_warning(&format!("Validation warnings for {}", file));
            }
            
            for (i, error) in errors.iter().enumerate() {
                println!("  {}. {}", i + 1, error);
            }
            
            if !strict {
                print_info(&format!("Found {} validation warnings", errors.len()));
                print_info("Use --strict flag to treat warnings as errors");
            }
        }
        
        Ok(())
    }
    
    fn output_json_results(&self, errors: &[String]) -> Result<()> {
        let result = serde_json::json!({
            "valid": errors.is_empty(),
            "error_count": errors.len(),
            "errors": errors
        });
        
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        Ok(())
    }
    
    fn run_parse(&self, file: String, output: String, debug: bool) -> Result<()> {
        if self.verbose {
            print_info(&format!("Parsing FDML file: {}", file));
        }
        
        // Read the file
        let content = fs::read_to_string(&file).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", file, e))
        })?;
        
        // Parse the FDML document
        let document = if file.ends_with(".fdml") || file.ends_with(".yaml") || file.ends_with(".yml") {
            parse_fdml_yaml(&content)?
        } else {
            parse_fdml(&content)?
        };
        
        if debug && self.verbose {
            print_info("Parsing completed successfully");
            print_info(&format!("Found {} entities, {} actions, {} features", 
                document.entities.len(), 
                document.actions.len(), 
                document.features.len()));
        }
        
        // Output results
        match output.as_str() {
            "yaml" => {
                let yaml_output = serde_yaml::to_string(&document)?;
                println!("{}", yaml_output);
            },
            "json" | _ => {
                let json_output = serde_json::to_string_pretty(&document)?;
                println!("{}", json_output);
            }
        }
        
        Ok(())
    }
    
    fn run_generate(&self, input: String, language: String, output: String, template: Option<String>, with_tests: bool) -> Result<()> {
        if self.verbose {
            print_info(&format!("Generating {} code from: {}", language, input));
        }
        
        // Read and parse the FDML file
        let content = fs::read_to_string(&input).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", input, e))
        })?;
        
        let document = if input.ends_with(".fdml") || input.ends_with(".yaml") || input.ends_with(".yml") {
            parse_fdml_yaml(&content)?
        } else {
            parse_fdml(&content)?
        };
        
        // Create generator configuration
        let config = GeneratorConfig {
            language: language.clone(),
            output_dir: output.clone(),
            template_dir: template,
            with_tests,
        };
        
        // Create and run generator
        let generator = create_generator(&config)?;
        let output_path = Path::new(&output);
        let generated_files = generator.generate(&document, output_path)?;
        
        print_success(&format!("Successfully generated {} files:", generated_files.len()));
        for file in &generated_files {
            println!("  - {}", file);
        }
        
        // Generate tests if requested
        if with_tests {
            let test_generator = TestGenerator::new(&config)?;
            let test_files = test_generator.generate_tests(&document, output_path)?;
            
            if !test_files.is_empty() {
                print_success(&format!("Generated {} test files:", test_files.len()));
                for file in &test_files {
                    println!("  - {}", file);
                }
            }
        }
        
        print_info(&format!("Generated code in: {}", output));
        match language.as_str() {
            "typescript" | "ts" => {
                print_info("Next steps:");
                println!("  1. cd {}", output);
                println!("  2. npm install");
                println!("  3. npm run build");
            },
            "python" | "py" => {
                print_info("Next steps:");
                println!("  1. cd {}", output);
                println!("  2. pip install -r requirements.txt");
                println!("  3. python main.py");
            },
            "go" => {
                print_info("Next steps:");
                println!("  1. cd {}", output);
                println!("  2. go mod tidy");
                println!("  3. go run .");
            },
            _ => {}
        }
        
        Ok(())
    }
    
    fn run_migrate(&self, operation: MigrateCommands) -> Result<()> {
        match operation {
            MigrateCommands::Apply { path, target, dry_run } => {
                if self.verbose {
                    print_info(&format!("Applying migrations from: {}", path));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let mut runner = MigrationRunner::new(&path);
                if let Some(target_file) = target {
                    runner = runner.with_target_file(&target_file);
                }
                
                let applied = runner.apply_migrations(dry_run)?;
                
                if applied.is_empty() && !dry_run {
                    print_info("No pending migrations to apply");
                } else if !dry_run {
                    print_success(&format!("Applied {} migrations", applied.len()));
                }
            },
            MigrateCommands::Rollback { path, target, count, dry_run } => {
                if self.verbose {
                    print_info(&format!("Rolling back {} migrations from: {}", count, path));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let mut runner = MigrationRunner::new(&path);
                if let Some(target_file) = target {
                    runner = runner.with_target_file(&target_file);
                }
                
                let rolled_back = runner.rollback_migrations(count, dry_run)?;
                
                if rolled_back.is_empty() && !dry_run {
                    print_info("No migrations to rollback");
                } else if !dry_run {
                    print_success(&format!("Rolled back {} migrations", rolled_back.len()));
                }
            },
            MigrateCommands::Status { path } => {
                if self.verbose {
                    print_info(&format!("Checking migration status in: {}", path));
                }
                
                let runner = MigrationRunner::new(&path);
                let status = runner.migration_status()?;
                
                println!("Migration Status:");
                println!("  Total migrations: {}", status.total_migrations);
                println!("  Applied: {}", status.applied_count);
                println!("  Pending: {}", status.pending_count);
                
                if !status.applied_migrations.is_empty() {
                    println!("\nApplied migrations:");
                    for migration in &status.applied_migrations {
                        println!("  ✓ {}", migration);
                    }
                }
                
                if !status.pending_migrations.is_empty() {
                    println!("\nPending migrations:");
                    for migration in &status.pending_migrations {
                        println!("  - {}", migration);
                    }
                }
            }
        }
        Ok(())
    }
    
    fn run_trace(&self, operation: TraceCommands) -> Result<()> {
        match operation {
            TraceCommands::Validate { path } => {
                if self.verbose {
                    print_info(&format!("Validating traceability in: {}", path));
                }
                
                let content = fs::read_to_string(&path)
                    .map_err(|e| crate::error::FdmlError::project_error(format!("cannot read {path}: {e}")))?;
                let document = parse_fdml_yaml(&content)?;
                // links resolve against the project's document set (vision + generated),
                // not the one file: `realizes` crosses that boundary by design
                let set = crate::project::document_set::DocumentSet::around(std::path::Path::new(&path))?;
                if set.docs.len() > 1 {
                    println!("project set: {}", set.docs.iter().map(|(p, _)| p.strip_prefix(&set.root).unwrap_or(p).display().to_string()).collect::<Vec<_>>().join(" + "));
                }
                let ids = set.all_ids();
                let mut errors = set.collisions();
                errors.extend(set.unresolved());
                // every link, resolved or not — a link the model cannot place is the finding
                for t in &document.traceability {
                    let ok = ids.contains(&t.from) && ids.contains(&t.to) && t.from != t.to;
                    println!("{} {} --{}--> {}", if ok { "✓" } else { "✗" }, t.from, t.relation, t.to);
                }
                let verified: Vec<&str> = document.features.iter().flat_map(|f| f.scenarios.iter())
                    .filter(|s| document.traceability.iter().any(|t| t.to == s.id && t.relation == "verifies"))
                    .map(|s| s.id.as_str()).collect();
                let total: usize = document.features.iter().map(|f| f.scenarios.len()).sum();
                println!("\nscenarios with a verifying test: {}/{}", verified.len(), total);
                for f in &document.features {
                    for s in &f.scenarios {
                        if !verified.contains(&s.id.as_str()) { println!("  ? {} — no test claims to verify it", s.id); }
                    }
                }
                if errors.is_empty() {
                    print_success(&format!("{} traceability links resolve", document.traceability.len()));
                } else {
                    for e in &errors { print_warning(e); }
                    return Err(crate::error::FdmlError::project_error(format!("{} traceability problems", errors.len())));
                }
            },
            TraceCommands::Graph { path, format, output } => {
                if self.verbose {
                    print_info(&format!("Generating traceability graph from: {}", path));
                }
                
                // TODO: Implement traceability graph generation
                print_warning("Traceability graph generation is not yet implemented");
                print_info(&format!("Would generate {} graph in {}", format, output));
            },
            TraceCommands::Matrix { path, format, output } => {
                if self.verbose {
                    print_info(&format!("Generating traceability matrix from: {}", path));
                }
                
                // TODO: Implement traceability matrix generation
                print_warning("Traceability matrix generation is not yet implemented");
                print_info(&format!("Would generate {} matrix in {}", format, output));
            }
        }
        Ok(())
    }
    
    fn run_add(&self, operation: AddCommands) -> Result<()> {
        match operation {
            AddCommands::Feature { id, title, description, target } => {
                if self.verbose {
                    print_info(&format!("Adding feature: {} - {}", id, title));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let op = MigrationOperation::AddFeature {
                    id: id.clone(),
                    title: title.clone(),
                    description,
                    scenarios: None,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added feature: {}", id));
            },
            
            AddCommands::Entity { id, name, description, target } => {
                if self.verbose {
                    print_info(&format!("Adding entity: {} - {}", id, name));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let op = MigrationOperation::AddEntity {
                    id: id.clone(),
                    name: name.clone(),
                    description,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added entity: {}", id));
            },
            
            AddCommands::Action { id, name, description, target } => {
                if self.verbose {
                    print_info(&format!("Adding action: {} - {}", id, name));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let op = MigrationOperation::AddAction {
                    id: id.clone(),
                    name: name.clone(),
                    description,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added action: {}", id));
            },
            
            AddCommands::Constraint { id, name, condition, applies_to, description, message, target } => {
                if self.verbose {
                    print_info(&format!("Adding constraint: {} - {}", id, name));
                    print_info(&format!("Condition: {} (applies to: {})", condition, applies_to));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let op = MigrationOperation::AddConstraint {
                    id: id.clone(),
                    name: name.clone(),
                    description,
                    condition: condition.clone(),
                    applies_to: applies_to.clone(),
                    message,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added constraint: {}", id));
            },
            
            AddCommands::Field { entity_id, field_name, field_type, required, default, target } => {
                if self.verbose {
                    print_info(&format!("Adding field: {} ({}) to entity: {}", field_name, field_type, entity_id));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let default_value = default.map(|d| {
                    // Try to parse as different types
                    if let Ok(b) = d.parse::<bool>() {
                        serde_json::Value::Bool(b)
                    } else if let Ok(n) = d.parse::<f64>() {
                        serde_json::Value::Number(serde_json::Number::from_f64(n).unwrap())
                    } else {
                        serde_json::Value::String(d)
                    }
                });
                
                let op = MigrationOperation::AddField {
                    entity_id: entity_id.clone(),
                    field_name: field_name.clone(),
                    field_type: field_type.clone(),
                    required: Some(required),
                    default: default_value,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added field {} to entity {}", field_name, entity_id));
            },
        }
        Ok(())
    }
    
    fn run_list(&self, operation: ListCommands) -> Result<()> {
        match operation {
            ListCommands::Features { target } => {
                if self.verbose {
                    print_info("Listing features");
                    if let Some(ref target_file) = target {
                        print_info(&format!("From FDML file: {}", target_file));
                    }
                }
                
                let document = self.load_fdml_document(target)?;
                
                if document.features.is_empty() {
                    print_info("No features found");
                } else {
                    println!("Features ({}):", document.features.len());
                    for feature in &document.features {
                        println!("  • {} - {}", feature.id, feature.title);
                        if let Some(ref desc) = feature.description {
                            println!("    Description: {}", desc);
                        }
                        if !feature.scenarios.is_empty() {
                            println!("    Scenarios: {}", feature.scenarios.len());
                        }
                    }
                }
            },
            
            ListCommands::Entities { target } => {
                if self.verbose {
                    print_info("Listing entities");
                    if let Some(ref target_file) = target {
                        print_info(&format!("From FDML file: {}", target_file));
                    }
                }
                
                let document = self.load_fdml_document(target)?;
                
                if document.entities.is_empty() {
                    print_info("No entities found");
                } else {
                    println!("Entities ({}):", document.entities.len());
                    for entity in &document.entities {
                        let name = entity.name.as_deref().unwrap_or(&entity.id);
                        println!("  • {} - {}", entity.id, name);
                        if let Some(ref desc) = entity.description {
                            println!("    Description: {}", desc);
                        }
                        if !entity.fields.is_empty() {
                            println!("    Fields: {}", entity.fields.len());
                        }
                    }
                }
            },
            
            ListCommands::Actions { target } => {
                if self.verbose {
                    print_info("Listing actions");
                    if let Some(ref target_file) = target {
                        print_info(&format!("From FDML file: {}", target_file));
                    }
                }
                
                let document = self.load_fdml_document(target)?;
                
                if document.actions.is_empty() {
                    print_info("No actions found");
                } else {
                    println!("Actions ({}):", document.actions.len());
                    for action in &document.actions {
                        let name = action.name.as_deref().unwrap_or(&action.id);
                        println!("  • {} - {}", action.id, name);
                        if let Some(ref desc) = action.description {
                            println!("    Description: {}", desc);
                        }
                    }
                }
            },
            
            ListCommands::Constraints { target } => {
                if self.verbose {
                    print_info("Listing constraints");
                    if let Some(ref target_file) = target {
                        print_info(&format!("From FDML file: {}", target_file));
                    }
                }
                
                let document = self.load_fdml_document(target)?;
                
                if document.constraints.is_empty() {
                    print_info("No constraints found");
                } else {
                    println!("Constraints ({}):", document.constraints.len());
                    for constraint in &document.constraints {
                        println!("  • {} - {}", constraint.id, constraint.name);
                        if let Some(ref desc) = constraint.description {
                            println!("    Description: {}", desc);
                        }
                        println!("    Rule: {}", constraint.rule);
                    }
                }
            },
        }
        Ok(())
    }
    
    fn run_serve(
        &self,
        file: String,
        port: u16,
        no_open: bool,
        generate: Option<String>,
        fast: bool,
        model: Option<String>,
        provider: Option<String>,
        parallel: usize,
        ollama_url: Option<String>,
        num_ctx: Option<usize>,
        _chunk_strategy: Option<String>,
    ) -> Result<()> {
        use std::sync::Arc;
        use tokio::sync::{broadcast, RwLock};
        use crate::serve::server::{AppState, GenerationStatus, run_server};
        use crate::serve::watcher::start_watcher;

        let is_generating = generate.is_some();

        // If generating, start with empty doc; otherwise load from file
        let (document, file_path) = if let Some(ref gen_dir) = generate {
            let output_path = PathBuf::from(&file);
            print_info(&format!("Generate mode: scanning {} → {}", gen_dir, file));
            (
                crate::parser::ast::FdmlDocument::default(),
                output_path.canonicalize().unwrap_or_else(|_| {
                    // File may not exist yet — use absolute path
                    std::env::current_dir().unwrap_or_default().join(&file)
                }),
            )
        } else {
            let content = fs::read_to_string(&file).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", file, e))
            })?;
            let doc = parse_fdml_yaml(&content)?;
            let fp = PathBuf::from(&file).canonicalize().map_err(|e| {
                crate::error::FdmlError::project_error(format!("Invalid path '{}': {}", file, e))
            })?;

            if !doc.systems.is_empty() {
                print_info(&format!(
                    "Serving {} ({} systems, {} integrations)",
                    file, doc.systems.len(), doc.integrations.len(),
                ));
            } else {
                print_info(&format!(
                    "Serving {} ({} entities, {} actions, {} features)",
                    file, doc.entities.len(), doc.actions.len(), doc.features.len(),
                ));
            }
            (doc, fp)
        };

        // FDML 1.4: Collect per-system spec file paths for watching
        let base_dir = file_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let extra_watch_paths: Vec<PathBuf> = document
            .systems
            .iter()
            .filter_map(|sys| sys.spec.as_ref().map(|p| base_dir.join(p)))
            .collect();

        let document = Arc::new(RwLock::new(document));
        let (tx, _) = broadcast::channel(16);
        let (log_tx, _) = broadcast::channel::<String>(256);
        let generation_status = Arc::new(RwLock::new(if is_generating {
            GenerationStatus {
                phase: "scanning".to_string(),
                progress: 0.0,
                message: "Starting platform scan...".to_string(),
            }
        } else {
            GenerationStatus::default()
        }));

        let log_buffer: std::sync::Arc<tokio::sync::RwLock<Vec<String>>> =
            std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new()));

        let state = AppState {
            document: document.clone(),
            file_path: file_path.clone(),
            tx: tx.clone(),
            log_tx: log_tx.clone(),
            log_buffer: log_buffer.clone(),
            generation_status: generation_status.clone(),
        };

        let rt = tokio::runtime::Runtime::new().map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to create tokio runtime: {}", e))
        })?;

        // In generate mode, ensure the output file exists so the watcher can watch it
        if is_generating && !file_path.exists() {
            if let Some(parent) = file_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&file_path, "# FDML spec — generation in progress\n");
        }

        rt.block_on(async move {
            // Start file watcher
            let _watcher = start_watcher(file_path.clone(), document.clone(), tx.clone(), extra_watch_paths)
                .map_err(|e| crate::error::FdmlError::project_error(format!("Watcher error: {}", e)))?;

            // If --generate: spawn background generation task
            if let Some(gen_dir) = generate {
                let doc = document.clone();
                let spec_tx = tx.clone();
                let log = log_tx.clone();
                let log_buf = log_buffer.clone();
                let gen_status = generation_status.clone();
                let out_file = file_path.clone();
                let fast = fast;
                let model = model.clone();
                let provider = provider.clone();
                let ollama_url = ollama_url.clone();

                let parallel = parallel;
                tokio::task::spawn_blocking(move || {
                    Self::run_generation_pipeline(
                        &gen_dir, &out_file, fast, model.as_deref(), provider.as_deref(),
                        parallel, ollama_url.as_deref(), num_ctx,
                        doc, spec_tx, log, log_buf, gen_status,
                    );
                });
            }

            run_server(state, port, no_open)
                .await
                .map_err(|e| crate::error::FdmlError::project_error(format!("Server error: {}", e)))?;

            Ok(())
        })
    }

    fn run_generation_pipeline(
        gen_dir: &str,
        output_file: &std::path::Path,
        fast: bool,
        model: Option<&str>,
        provider: Option<&str>,
        parallel: usize,
        ollama_url: Option<&str>,
        num_ctx: Option<usize>,
        document: std::sync::Arc<tokio::sync::RwLock<crate::parser::ast::FdmlDocument>>,
        spec_tx: tokio::sync::broadcast::Sender<()>,
        log_tx: tokio::sync::broadcast::Sender<String>,
        log_buffer: std::sync::Arc<tokio::sync::RwLock<Vec<String>>>,
        gen_status: std::sync::Arc<tokio::sync::RwLock<crate::serve::server::GenerationStatus>>,
    ) {
        use crate::linker::platform;

        let log = |msg: String| {
            eprintln!("  [gen] {}", msg);
            let _ = log_tx.send(msg.clone());
            // Buffer for late-connecting clients
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let buf = log_buffer.clone();
                handle.block_on(async move {
                    buf.write().await.push(msg);
                });
            }
        };

        let set_status = |phase: &str, progress: f32, message: &str| {
            let gs = gen_status.clone();
            let p = phase.to_string();
            let m = message.to_string();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.block_on(async move {
                    let mut s = gs.write().await;
                    s.phase = p;
                    s.progress = progress;
                    s.message = m;
                });
            }
        };

        let root = Path::new(gen_dir);
        if !root.exists() || !root.is_dir() {
            log(format!("ERROR: Path does not exist or is not a directory: {}", gen_dir));
            set_status("error", 0.0, "Invalid directory");
            return;
        }

        // Read .fdmlignore
        let ignore_entries = platform::read_fdmlignore(root);
        let exclude: Vec<String> = ignore_entries;

        // ── Stage 1: Detect systems ──
        log("Stage 1: Detecting system boundaries...".to_string());
        set_status("scanning", 0.05, "Detecting system boundaries...");

        let detected = platform::detect_systems(root, &exclude);
        if detected.is_empty() {
            log("ERROR: No systems detected.".to_string());
            set_status("error", 0.0, "No systems detected");
            return;
        }
        log(format!("Found {} system(s):", detected.len()));
        for sys in &detected {
            log(format!("  {} ({}) — {} [{}]", sys.name, sys.id, sys.technology, sys.boundary_marker));
        }

        // ── Stage 2: Per-system scan + LLM ──
        let max_parallel = parallel.max(1);
        log(format!("Stage 2: Per-system analysis (parallel: {})...", max_parallel));
        set_status("scanning", 0.1, "Scanning systems...");

        let system_paths: Vec<String> = detected.iter().map(|s| s.path.clone()).collect();
        let total = detected.len();

        // First pass: scan all systems and prepare prompts (fast, always sequential)
        struct SystemWork {
            sys: crate::linker::types::DetectedSystem,
            scan: crate::scanner::types::ScanResult,
            report: crate::linker::types::LinkReport,
            prompt: String,
        }
        let mut needs_llm: Vec<SystemWork> = Vec::new();
        let mut system_data: Vec<(crate::linker::types::DetectedSystem, crate::scanner::types::ScanResult, crate::linker::types::LinkReport)> = Vec::new();
        let mut per_system_prompts: Vec<(String, String)> = Vec::new();
        let mut per_system_specs: Vec<(String, String)> = Vec::new();

        for (idx, sys) in detected.iter().enumerate() {
            let sys_path = root.join(&sys.path);
            let sys_path_str = sys_path.to_string_lossy().to_string();

            let mut sys_exclude = exclude.clone();
            for other_path in &system_paths {
                if other_path != &sys.path && other_path.starts_with(&format!("{}/", sys.path)) {
                    if let Some(child_dir) = other_path.strip_prefix(&format!("{}/", sys.path)) {
                        let top_dir = child_dir.split('/').next().unwrap_or(child_dir);
                        if !sys_exclude.contains(&top_dir.to_string()) {
                            sys_exclude.push(top_dir.to_string());
                        }
                    }
                }
            }

            // Check if per-system spec already exists — skip LLM if so
            let base = output_file.to_string_lossy();
            let base_str = base.trim_end_matches(".yaml").trim_end_matches(".fdml");
            let existing_spec_path = format!("{}.{}.fdml", base_str, sys.id);
            if let Ok(existing_spec) = fs::read_to_string(&existing_spec_path) {
                let lines = existing_spec.lines().count();
                if lines > 10 {
                    log(format!("[{}/{}] Reusing existing spec for {} ({} lines)", idx + 1, total, sys.name, lines));
                    per_system_specs.push((sys.id.clone(), existing_spec));
                    if let Ok(scan) = crate::scanner::scan_project(&sys_path_str, &sys_exclude) {
                        let report = crate::linker::link_code(&scan, None, &sys_path_str, None);
                        system_data.push((sys.clone(), scan, report));
                    }
                    continue;
                }
            }

            log(format!("[{}/{}] Scanning {}...", idx + 1, total, sys.name));

            match crate::scanner::scan_project(&sys_path_str, &sys_exclude) {
                Ok(scan) => {
                    let report = crate::linker::link_code(&scan, None, &sys_path_str, None);
                    log(format!("  {} files, {} entities, {} actions",
                        scan.metadata.total_files, report.entities.len(), report.actions.len()));
                    let sys_prompt = crate::linker::generate_metaprompt(&report, &scan);
                    per_system_prompts.push((sys.id.clone(), sys_prompt.clone()));
                    needs_llm.push(SystemWork { sys: sys.clone(), scan, report, prompt: sys_prompt });
                }
                Err(e) => {
                    log(format!("  Failed to scan {}: {}", sys.name, e));
                }
            }
        }

        // Second pass: LLM calls — parallel or sequential
        // LPT scheduling: sort jobs by prompt size DESC so the biggest one starts first
        // and runs in parallel with the smaller ones. Otherwise a late-arriving giant
        // (e.g. R3 core was 8th in detection order) blocks wall-clock time.
        needs_llm.sort_by(|a, b| b.prompt.len().cmp(&a.prompt.len()));
        if !needs_llm.is_empty() {
            let llm_count = needs_llm.len();
            let biggest_kb = needs_llm[0].prompt.len() / 1024;
            log(format!("{} system(s) need LLM generation (sorted by size DESC; biggest: {} @ {}KB)",
                llm_count, needs_llm[0].sys.name, biggest_kb));
            set_status("generating", 0.2, &format!("Generating {} specs...", llm_count));

            // Use scoped threads for parallel LLM calls
            let results: Vec<(crate::linker::types::DetectedSystem, crate::scanner::types::ScanResult, crate::linker::types::LinkReport, Option<String>)>;
            let base_str = {
                let base = output_file.to_string_lossy();
                base.trim_end_matches(".yaml").trim_end_matches(".fdml").to_string()
            };

            // Channel-based semaphore for limiting concurrency
            let (sem_tx, sem_rx) = std::sync::mpsc::sync_channel::<()>(max_parallel);
            for _ in 0..max_parallel { let _ = sem_tx.send(()); }
            let sem_rx = std::sync::Arc::new(std::sync::Mutex::new(sem_rx));

            results = std::thread::scope(|scope| {
                let handles: Vec<_> = needs_llm.into_iter().enumerate().map(|(i, work)| {
                    let log_tx = &log_tx;
                    let log_buffer = &log_buffer;
                    let sem_rx = sem_rx.clone();
                    let sem_tx = sem_tx.clone();
                    let base = base_str.clone();

                    scope.spawn(move || {
                        // Acquire semaphore permit (blocks until a slot is free)
                        let _ = sem_rx.lock().unwrap().recv();

                        let runner = CommandRunner::new(false);
                        let prompt_kb = work.prompt.len() / 1024;

                        let send_log = |msg: String| {
                            eprintln!("  [gen] {}", msg);
                            let _ = log_tx.send(msg.clone());
                            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                                let buf = log_buffer.clone();
                                handle.block_on(async move { buf.write().await.push(msg); });
                            }
                        };

                        send_log(format!("[{}/{}] Sending {}KB prompt to LLM for {}...", i + 1, llm_count, prompt_kb, work.sys.name));

                        let llm_start = std::time::Instant::now();

                        // Ticker thread
                        let ticker_log_tx = log_tx.clone();
                        let ticker_buf = log_buffer.clone();
                        let ticker_name = work.sys.name.clone();
                        let ticker_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                        let ticker_flag = ticker_running.clone();
                        std::thread::spawn(move || {
                            let mut secs = 0u64;
                            while ticker_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                std::thread::sleep(std::time::Duration::from_secs(5));
                                if !ticker_flag.load(std::sync::atomic::Ordering::Relaxed) { break; }
                                secs += 5;
                                let msg = format!("  [{}/{}] Waiting for LLM ({})... {}s", i + 1, llm_count, ticker_name, secs);
                                let _ = ticker_log_tx.send(msg.clone());
                                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                                    let buf = ticker_buf.clone();
                                    handle.block_on(async move { buf.write().await.push(msg); });
                                }
                            }
                        });

                        let llm_result = runner.call_llm(&work.prompt, fast, model, provider, ollama_url, num_ctx);
                        ticker_running.store(false, std::sync::atomic::Ordering::Relaxed);

                        // Release semaphore permit
                        let _ = sem_tx.send(());

                        let spec = match llm_result {
                            Ok(spec) => {
                                let elapsed = llm_start.elapsed().as_secs();
                                let spec_lines = spec.lines().count();
                                send_log(format!("[{}/{}] Got {}-line spec for {} ({}s)", i + 1, llm_count, spec_lines, work.sys.name, elapsed));

                                let spec_path = format!("{}.{}.fdml", base, work.sys.id);
                                if let Err(e) = fs::write(&spec_path, &spec) {
                                    send_log(format!("  Warning: failed to save {}: {}", spec_path, e));
                                } else {
                                    send_log(format!("  Saved: {}", spec_path));
                                }
                                Some(spec)
                            }
                            Err(e) => {
                                send_log(format!("[{}/{}] LLM failed for {}: {}", i + 1, llm_count, work.sys.name, e));
                                None
                            }
                        };

                        (work.sys, work.scan, work.report, spec)
                    })
                }).collect();

                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });

            for (sys, scan, report, spec) in results {
                if let Some(s) = spec {
                    per_system_specs.push((sys.id.clone(), s));
                }
                system_data.push((sys, scan, report));
            }
        }

        if per_system_specs.is_empty() {
            log("ERROR: No per-system specs generated. Cannot assemble.".to_string());
            set_status("error", 0.0, "No specs generated");
            return;
        }

        // ── Stage 3: Cross-system analysis + assembly ──
        log("Stage 3: Cross-system analysis...".to_string());
        set_status("generating", 0.7, "Cross-system analysis...");

        let integration_hints = platform::detect_integrations(&system_data);
        log(format!("  {} integration pattern(s)", integration_hints.len()));

        let system_reports: Vec<(crate::linker::types::DetectedSystem, crate::linker::types::LinkReport)> =
            system_data.iter().map(|(s, _, r)| (s.clone(), r.clone())).collect();
        let shared_entity_hints = platform::detect_shared_entities(&system_reports);
        log(format!("  {} shared entity candidate(s)", shared_entity_hints.len()));

        let per_system_reports: Vec<(String, crate::linker::types::LinkReport)> =
            system_data.iter().map(|(s, _, r)| (s.id.clone(), r.clone())).collect();

        let platform_report = crate::linker::types::PlatformReport {
            detected_systems: detected.clone(),
            per_system: per_system_reports,
            integration_hints,
            shared_entity_hints,
        };

        let assembly_prompt = platform::generate_platform_metaprompt(
            &platform_report,
            &per_system_specs,
            &per_system_prompts,
        );

        let assembly_kb = assembly_prompt.len() / 1024;
        log(format!("Assembling platform FDML 1.4 spec via LLM ({}KB prompt, {} system specs)...",
            assembly_kb, per_system_specs.len()));
        set_status("generating", 0.8, "Assembling platform spec via LLM...");

        let assembly_start = std::time::Instant::now();

        // Ticker for assembly LLM
        let ticker_log_tx2 = log_tx.clone();
        let ticker_buf2 = log_buffer.clone();
        let ticker_running2 = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let ticker_flag2 = ticker_running2.clone();
        std::thread::spawn(move || {
            let mut secs = 0u64;
            while ticker_flag2.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_secs(5));
                if !ticker_flag2.load(std::sync::atomic::Ordering::Relaxed) { break; }
                secs += 5;
                let msg = format!("  Assembling... {}s", secs);
                let _ = ticker_log_tx2.send(msg.clone());
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let buf = ticker_buf2.clone();
                    handle.block_on(async move { buf.write().await.push(msg); });
                }
            }
        });

        let assembly_runner = CommandRunner::new(false);
        let assembly_result = assembly_runner.call_llm(&assembly_prompt, fast, model, provider, ollama_url, num_ctx);
        ticker_running2.store(false, std::sync::atomic::Ordering::Relaxed);

        match assembly_result {
            Ok(llm_result) => {
                let elapsed = assembly_start.elapsed().as_secs();
                log(format!("Assembly complete ({}s, {}-line spec)", elapsed, llm_result.lines().count()));

                // Write output file
                if let Err(e) = fs::write(output_file, &llm_result) {
                    log(format!("ERROR: Failed to write output: {}", e));
                    set_status("error", 0.0, &format!("Write failed: {}", e));
                    return;
                }
                log(format!("Written to: {}", output_file.display()));

                // Per-system specs already saved incrementally above

                // Try to parse and load the result into the live viewer
                match crate::parser::parse_fdml_yaml(&llm_result) {
                    Ok(new_doc) => {
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            let doc = document.clone();
                            let stx = spec_tx.clone();
                            handle.block_on(async move {
                                let mut w = doc.write().await;
                                *w = new_doc;
                                let _ = stx.send(());
                            });
                        }
                        log("Spec loaded into viewer — switching to spec view.".to_string());
                        set_status("done", 1.0, "Generation complete");
                    }
                    Err(e) => {
                        log(format!("Warning: Generated spec has parse errors: {}. File saved but viewer may not display correctly.", e));
                        set_status("done", 1.0, "Generation complete (with parse warnings)");
                    }
                }
            }
            Err(e) => {
                log(format!("ERROR: Assembly LLM failed: {}", e));
                set_status("error", 0.0, &format!("Assembly failed: {}", e));
            }
        }
    }

    fn run_parse_code(&self, input: String, output: Option<String>, format: String, exclude: Vec<String>) -> Result<()> {
        if self.verbose {
            print_info(&format!("Scanning source code in: {}", input));
        }

        let result = crate::scanner::scan_project(&input, &exclude)?;

        // Format output
        let output_str = match format.as_str() {
            "json" => serde_json::to_string_pretty(&result)
                .map_err(|e| crate::error::FdmlError::project_error(format!("JSON serialization error: {}", e)))?,
            "yaml" | _ => serde_yaml::to_string(&result)
                .map_err(|e| crate::error::FdmlError::project_error(format!("YAML serialization error: {}", e)))?,
        };

        // Write to file or stdout
        if let Some(output_path) = output {
            fs::write(&output_path, &output_str).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to write output to {}: {}", output_path, e))
            })?;
            print_success(&format!("Code analysis written to: {}", output_path));
        } else {
            println!("{}", output_str);
        }

        // Print summary
        let stats = &result.statistics;
        print_success(&format!(
            "Scan complete: {} files, {} classes, {} functions, {} methods, {} fields, {} relationships",
            result.metadata.total_files,
            stats.classes,
            stats.functions,
            stats.methods,
            stats.fields,
            stats.relationships,
        ));
        print_info(&format!(
            "Languages: {:?} | External imports: {} | Internal imports: {}",
            result.metadata.languages_detected.iter().map(|l| l.name()).collect::<Vec<_>>(),
            stats.imports_external,
            stats.imports_internal,
        ));

        Ok(())
    }

    fn run_link_code(
        &self,
        code: String,
        fdml: Option<String>,
        output: Option<String>,
        format: String,
        llm: bool,
        no_llm: bool,
        skip_scenarios: bool,
        fast: bool,
        model: Option<String>,
        provider: Option<String>,
        ollama_url: Option<String>,
        num_ctx: Option<usize>,
        _chunk_strategy: Option<String>,
    ) -> Result<()> {
        if self.verbose {
            print_info(&format!("Linking code inventory: {}", code));
            if let Some(ref spec) = fdml {
                print_info(&format!("FDML spec: {}", spec));
            }
        }

        // Load inventory
        let inventory_content = fs::read_to_string(&code).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read inventory file '{}': {}", code, e))
        })?;
        let scan: crate::scanner::types::ScanResult = serde_yaml::from_str(&inventory_content)
            .or_else(|_| serde_json::from_str(&inventory_content).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to parse inventory (tried YAML and JSON): {}", e))
            }))?;

        // Load spec if provided
        let spec_doc = if let Some(ref spec_path) = fdml {
            let content = fs::read_to_string(spec_path).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to read spec file '{}': {}", spec_path, e))
            })?;
            Some(crate::parser::parse_fdml_yaml(&content)?)
        } else {
            None
        };

        // Run linking
        let report = crate::linker::link_code(
            &scan,
            spec_doc.as_ref(),
            &code,
            fdml.as_deref(),
        );

        // Generate metaprompt
        let metaprompt = crate::linker::generate_metaprompt(&report, &scan);

        // If --llm flag, send to LLM
        if llm {
            // Use hybrid pipeline for Ollama, monolithic for Claude
            let is_ollama = provider.as_deref() == Some("ollama")
                || (provider.is_none() && Self::is_ollama_running(ollama_url.as_deref()));
            let llm_result = if is_ollama {
                eprintln!("  ℹ Using hybrid pipeline (cluster → classify → assemble)");
                self.call_llm_hybrid(&report, &scan, model.as_deref(), ollama_url.as_deref(), num_ctx, skip_scenarios)?
            } else {
                self.call_llm(&metaprompt, fast, model.as_deref(), provider.as_deref(), ollama_url.as_deref(), num_ctx)?
            };

            // Write LLM result
            if let Some(ref output_path) = output {
                fs::write(output_path, &llm_result).map_err(|e| {
                    crate::error::FdmlError::project_error(format!("Failed to write LLM output to {}: {}", output_path, e))
                })?;
                print_success(&format!("LLM-generated FDML spec written to: {}", output_path));

                // Also save the prompt used
                let prompt_path = format!("{}.prompt.md", output_path.trim_end_matches(".yaml").trim_end_matches(".fdml"));
                fs::write(&prompt_path, &metaprompt).map_err(|e| {
                    crate::error::FdmlError::project_error(format!("Failed to write prompt to {}: {}", prompt_path, e))
                })?;
                print_info(&format!("Prompt saved to: {}", prompt_path));
            } else {
                println!("{}", llm_result);
            }

            return Ok(());
        }

        // If --no-llm flag, build spec deterministically
        if no_llm {
            let spec_yaml = crate::linker::assemble::assemble_spec_no_llm(&report, &scan, None);

            if let Some(ref output_path) = output {
                fs::write(output_path, &spec_yaml).map_err(|e| {
                    crate::error::FdmlError::project_error(format!("Failed to write spec: {}", e))
                })?;
                print_success(&format!("FDML spec generated (no LLM): {}", output_path));

                // Stats
                let entity_count = spec_yaml.matches("  - id:").count();
                let action_count = report.actions.len();
                let lines = spec_yaml.lines().count();
                print_info(&format!("  {} entities, {} actions, {} lines", entity_count, action_count, lines));
            } else {
                println!("{}", spec_yaml);
            }

            return Ok(());
        }

        // Format output — include both report and metaprompt
        let output_str = match format.as_str() {
            "json" => serde_json::to_string_pretty(&report)
                .map_err(|e| crate::error::FdmlError::project_error(format!("JSON serialization error: {}", e)))?,
            "prompt" => metaprompt.clone(),
            "yaml" | _ => {
                let yaml = serde_yaml::to_string(&report)
                    .map_err(|e| crate::error::FdmlError::project_error(format!("YAML serialization error: {}", e)))?;
                yaml
            }
        };

        // Write to file or stdout
        if let Some(ref output_path) = output {
            fs::write(output_path, &output_str).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to write output to {}: {}", output_path, e))
            })?;
            print_success(&format!("Linking report written to: {}", output_path));

            // Also write metaprompt alongside if format is not "prompt"
            if format != "prompt" {
                let prompt_path = format!("{}.prompt.md", output_path.trim_end_matches(".yaml").trim_end_matches(".json"));
                fs::write(&prompt_path, &metaprompt).map_err(|e| {
                    crate::error::FdmlError::project_error(format!("Failed to write prompt to {}: {}", prompt_path, e))
                })?;
                print_success(&format!("LLM metaprompt written to: {}", prompt_path));
            }
        } else {
            println!("{}", output_str);
        }

        // Print summary
        let matched_entities = report.entities.iter().filter(|e| matches!(e.source, crate::linker::types::LinkSource::Matched)).count();
        let matched_actions = report.actions.iter().filter(|a| matches!(a.source, crate::linker::types::LinkSource::Matched)).count();
        print_success(&format!(
            "Link complete: {} entity candidates ({} matched), {} action candidates ({} matched), {} feature suggestions",
            report.entities.len(), matched_entities,
            report.actions.len(), matched_actions,
            report.features.len(),
        ));
        print_info(&format!(
            "Traceability links: {} | Unlinked code: {} | Unlinked spec: {}",
            report.traceability.len(),
            report.unlinked_code.len(),
            report.unlinked_spec.len(),
        ));
        print_info(&format!(
            "Coverage — spec: {:.0}% | code: {:.0}%",
            report.coverage.spec_coverage.percentage,
            report.coverage.code_coverage.percentage,
        ));

        Ok(())
    }

    fn run_scan_platform(
        &self,
        input: String,
        output: Option<String>,
        format: String,
        exclude: Vec<String>,
        llm: bool,
        skip_scenarios: bool,
        fast: bool,
        model: Option<String>,
        provider: Option<String>,
        ollama_url: Option<String>,
        num_ctx: Option<usize>,
        _chunk_strategy: Option<String>,
    ) -> Result<()> {
        use crate::linker::platform;
        use crate::linker::types::PlatformReport;

        let root = Path::new(&input);
        if !root.exists() || !root.is_dir() {
            return Err(crate::error::FdmlError::project_error(
                format!("Path does not exist or is not a directory: {}", input)
            ));
        }

        // Read .fdmlignore if present
        let ignore_entries = platform::read_fdmlignore(root);
        let mut exclude = exclude;
        exclude.extend(ignore_entries);

        // ── Stage 1: Detect system boundaries ─────────────────────────
        print_info(&format!("Scanning platform root: {}", input));
        let detected = platform::detect_systems(root, &exclude);

        if detected.is_empty() {
            print_warning("No system boundaries detected. Ensure subdirectories contain package.json, pyproject.toml, Cargo.toml, go.mod, or Dockerfile.");
            return Ok(());
        }

        print_success(&format!("Stage 1: Detected {} system(s):", detected.len()));
        for sys in &detected {
            print_info(&format!("  {} ({}) — {} [{}]", sys.name, sys.id, sys.technology, sys.boundary_marker));
        }

        // ── Stage 2: Per-system full scan + link-code ─────────────────
        // For each system: scan → link → generate full metaprompt → (optionally) LLM
        print_info("Stage 2: Per-system analysis...");

        let mut system_data: Vec<(crate::linker::types::DetectedSystem, crate::scanner::types::ScanResult, crate::linker::types::LinkReport)> = Vec::new();
        // per-system metaprompts (full link-code prompts)
        let mut per_system_prompts: Vec<(String, String)> = Vec::new();
        // per-system FDML specs (from LLM or empty)
        let mut per_system_specs: Vec<(String, String)> = Vec::new();

        // Build list of child system paths for exclusion during scanning
        let system_paths: Vec<String> = detected.iter().map(|s| s.path.clone()).collect();

        for sys in &detected {
            let sys_path = root.join(&sys.path);
            let sys_path_str = sys_path.to_string_lossy().to_string();

            // Build exclude list: user excludes + sibling/child system dirs
            let mut sys_exclude = exclude.clone();
            for other_path in &system_paths {
                if other_path != &sys.path && other_path.starts_with(&format!("{}/", sys.path)) {
                    // This is a child system — exclude its directory name from scanning
                    if let Some(child_dir) = other_path.strip_prefix(&format!("{}/", sys.path)) {
                        let top_dir = child_dir.split('/').next().unwrap_or(child_dir);
                        if !sys_exclude.contains(&top_dir.to_string()) {
                            sys_exclude.push(top_dir.to_string());
                        }
                    }
                }
            }

            print_info(&format!("  [{}/{}] Scanning {}...",
                detected.iter().position(|s| s.id == sys.id).unwrap_or(0) + 1, detected.len(), sys.name));

            match crate::scanner::scan_project(&sys_path_str, &sys_exclude) {
                Ok(scan) => {
                    let report = crate::linker::link_code(&scan, None, &sys_path_str, None);
                    print_info(&format!("    {} files, {} entities, {} actions",
                        scan.metadata.total_files, report.entities.len(), report.actions.len()));

                    // Generate full link-code metaprompt for this system
                    let sys_prompt = crate::linker::generate_metaprompt(&report, &scan);
                    per_system_prompts.push((sys.id.clone(), sys_prompt.clone()));

                    // If --llm: send each system through LLM to get per-system FDML spec
                    if llm {
                        let is_ollama = provider.as_deref() == Some("ollama")
                            || (provider.is_none() && Self::is_ollama_running(ollama_url.as_deref()));
                        let sys_idx = detected.iter().position(|s| s.id == sys.id).unwrap_or(0) + 1;
                        let llm_start = std::time::Instant::now();

                        let llm_result = if is_ollama {
                            print_info(&format!("    [{}/{}] Hybrid pipeline for {}...",
                                sys_idx, detected.len(), sys.name));
                            self.call_llm_hybrid(&report, &scan, model.as_deref(), ollama_url.as_deref(), num_ctx, skip_scenarios)
                        } else {
                            let prompt_kb = sys_prompt.len() / 1024;
                            print_info(&format!("    [{}/{}] Sending {}KB prompt to LLM for {}...",
                                sys_idx, detected.len(), prompt_kb, sys.name));
                            self.call_llm(&sys_prompt, fast, model.as_deref(), provider.as_deref(), ollama_url.as_deref(), num_ctx)
                        };
                        match llm_result {
                            Ok(spec) => {
                                let elapsed = llm_start.elapsed().as_secs();
                                let spec_lines = spec.lines().count();
                                print_success(&format!("    [{}/{}] Got {}-line FDML spec for {} ({}s)",
                                    sys_idx, detected.len(), spec_lines, sys.name, elapsed));
                                per_system_specs.push((sys.id.clone(), spec));
                            }
                            Err(e) => {
                                let elapsed = llm_start.elapsed().as_secs();
                                print_warning(&format!("    [{}/{}] LLM failed for {} after {}s: {}",
                                    sys_idx, detected.len(), sys.name, elapsed, e));
                            }
                        }
                    }

                    system_data.push((sys.clone(), scan, report));
                }
                Err(e) => {
                    print_warning(&format!("    Failed to scan {}: {}", sys.name, e));
                }
            }
        }

        if system_data.is_empty() {
            print_warning("No systems could be scanned successfully.");
            return Ok(());
        }

        // ── Stage 3: Cross-system analysis + assembly ─────────────────
        print_info("Stage 3: Cross-system analysis...");

        let integration_hints = platform::detect_integrations(&system_data);
        if !integration_hints.is_empty() {
            print_info(&format!("  {} integration pattern(s)", integration_hints.len()));
        }

        let system_reports: Vec<(crate::linker::types::DetectedSystem, crate::linker::types::LinkReport)> =
            system_data.iter().map(|(s, _, r)| (s.clone(), r.clone())).collect();
        let shared_entity_hints = platform::detect_shared_entities(&system_reports);
        if !shared_entity_hints.is_empty() {
            print_info(&format!("  {} shared entity candidate(s)", shared_entity_hints.len()));
        }

        // Build platform report
        let per_system_reports: Vec<(String, crate::linker::types::LinkReport)> =
            system_data.iter().map(|(s, _, r)| (s.id.clone(), r.clone())).collect();

        let platform_report = PlatformReport {
            detected_systems: detected.clone(),
            per_system: per_system_reports,
            integration_hints,
            shared_entity_hints,
        };

        // Generate the assembly metaprompt — includes per-system specs or full prompts
        let assembly_prompt = platform::generate_platform_metaprompt(
            &platform_report,
            &per_system_specs,
            &per_system_prompts,
        );

        // If --llm: final assembly call
        if llm {
            if per_system_specs.is_empty() {
                print_warning("No per-system specs were generated. Cannot assemble platform spec.");
                return Ok(());
            }

            let assembly_kb = assembly_prompt.len() / 1024;
            print_info(&format!("Stage 3: Assembling platform FDML 1.4 spec via LLM ({}KB prompt, {} system specs)...",
                assembly_kb, per_system_specs.len()));
            let assembly_start = std::time::Instant::now();
            let llm_result = self.call_llm(&assembly_prompt, fast, model.as_deref(), provider.as_deref(), ollama_url.as_deref(), num_ctx)?;
            let assembly_elapsed = assembly_start.elapsed().as_secs();
            print_success(&format!("Stage 3: Assembly complete ({}s, {}-line spec)",
                assembly_elapsed, llm_result.lines().count()));

            if let Some(ref output_path) = output {
                fs::write(output_path, &llm_result).map_err(|e| {
                    crate::error::FdmlError::project_error(format!("Failed to write output: {}", e))
                })?;
                print_success(&format!("FDML 1.4 platform spec written to: {}", output_path));

                // Save per-system specs alongside
                let base = output_path.trim_end_matches(".yaml").trim_end_matches(".fdml");
                for (sys_id, spec) in &per_system_specs {
                    let spec_path = format!("{}.{}.fdml", base, sys_id);
                    let _ = fs::write(&spec_path, spec);
                    print_info(&format!("  Per-system spec: {}", spec_path));
                }

                // Save assembly prompt
                let prompt_path = format!("{}.prompt.md", base);
                let _ = fs::write(&prompt_path, &assembly_prompt);
                print_info(&format!("  Assembly prompt: {}", prompt_path));
            } else {
                println!("{}", llm_result);
            }

            return Ok(());
        }

        // Non-LLM output
        let output_str = match format.as_str() {
            "json" => serde_json::to_string_pretty(&platform_report)
                .map_err(|e| crate::error::FdmlError::project_error(format!("JSON error: {}", e)))?,
            "prompt" => assembly_prompt.clone(),
            "yaml" | _ => serde_yaml::to_string(&platform_report)
                .map_err(|e| crate::error::FdmlError::project_error(format!("YAML error: {}", e)))?,
        };

        if let Some(ref output_path) = output {
            fs::write(output_path, &output_str).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to write output: {}", e))
            })?;
            print_success(&format!("Platform report written to: {}", output_path));

            // Save per-system metaprompts for manual LLM usage
            let base = output_path.trim_end_matches(".yaml").trim_end_matches(".json");
            for (sys_id, prompt) in &per_system_prompts {
                let prompt_path = format!("{}.{}.prompt.md", base, sys_id);
                let _ = fs::write(&prompt_path, prompt);
            }
            print_info(&format!("  {} per-system prompts saved (for manual LLM usage)", per_system_prompts.len()));

            if format != "prompt" {
                let prompt_path = format!("{}.prompt.md", base);
                let _ = fs::write(&prompt_path, &assembly_prompt);
                print_success(&format!("Platform assembly prompt: {}", prompt_path));
            }
        } else {
            println!("{}", output_str);
        }

        print_success(&format!(
            "Platform scan complete: {} systems, {} integrations, {} shared entities",
            platform_report.detected_systems.len(),
            platform_report.integration_hints.len(),
            platform_report.shared_entity_hints.len(),
        ));

        Ok(())
    }

    /// Hybrid pipeline: cluster → LLM classify → BDD scenarios → deterministic assembly
    fn call_llm_hybrid(
        &self,
        report: &crate::linker::types::LinkReport,
        scan: &crate::scanner::types::ScanResult,
        model: Option<&str>,
        ollama_url: Option<&str>,
        num_ctx: Option<usize>,
        skip_scenarios: bool,
    ) -> Result<String> {
        // Note: Ollama processes one request at a time (single GPU), so
        // parallelizing within one system doesn't help. Parallelism is
        // applied at the system level in scan-platform instead.
        use crate::linker::cluster::cluster_by_module;
        use crate::linker::llm_classify::*;

        let base_url = ollama_url
            .map(|s| s.to_string())
            .or_else(|| std::env::var("OLLAMA_URL").ok())
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let model_name = model.unwrap_or("gemma4");
        let ctx = num_ctx.unwrap_or(8192);

        // Step 1: Cluster
        let clusters = cluster_by_module(report, 20);
        eprintln!("  ℹ Clustered into {} groups", clusters.len());

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| crate::error::FdmlError::project_error(format!("HTTP client error: {}", e)))?;

        let mut all_entity_class: Vec<(String, String, String)> = Vec::new(); // (id, role, description)
        let mut all_action_class: Vec<(String, String, String)> = Vec::new();

        let sys_name = scan.metadata.codebase_path.split('/').last().unwrap_or("system");

        // Step 2: Classify each cluster via Ollama
        for (i, cluster) in clusters.iter().enumerate() {
            eprintln!("  [{}/{}] Classifying cluster '{}' ({} items)...",
                i + 1, clusters.len(), cluster.name, cluster.total_items());

            let prompt = build_classify_prompt(cluster, report, sys_name);
            let request = build_ollama_request(&prompt, model_name, ctx);

            let start = std::time::Instant::now();
            let response = client.post(format!("{}/api/generate", base_url))
                .json(&request)
                .send()
                .map_err(|e| {
                    if e.is_connect() {
                        crate::error::FdmlError::project_error(
                            format!("Ollama not running at {}. Start with: ollama serve", base_url)
                        )
                    } else {
                        crate::error::FdmlError::project_error(format!("Ollama error: {}", e))
                    }
                })?;

            if !response.status().is_success() {
                let body = response.text().unwrap_or_default();
                eprintln!("  ⚠ Cluster {} failed: {}", cluster.name, body);
                continue;
            }

            let json: serde_json::Value = response.json().map_err(|e| {
                crate::error::FdmlError::project_error(format!("Parse error: {}", e))
            })?;

            let text = json.get("response").and_then(|r| r.as_str()).unwrap_or("");
            let elapsed = start.elapsed();
            let eval_count = json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);

            match parse_classification(text) {
                Ok(result) => {
                    let domain_count = result.entities.iter().filter(|e| e.role == "domain_entity").count();
                    let biz_count = result.actions.iter().filter(|a| a.role == "business_action").count();
                    eprintln!("  ✓ {} entities ({} domain), {} actions ({} business) in {:.1}s ({} tok/s)",
                        result.entities.len(), domain_count,
                        result.actions.len(), biz_count,
                        elapsed.as_secs_f64(),
                        eval_count as f64 / elapsed.as_secs_f64().max(0.1) as f64);

                    for ec in result.entities {
                        all_entity_class.push((ec.id, ec.role, ec.description));
                    }
                    for ac in result.actions {
                        all_action_class.push((ac.id, ac.role, ac.description));
                    }
                }
                Err(e) => {
                    eprintln!("  ⚠ Parse failed for cluster '{}': {}", cluster.name, e);
                    // Fallback: treat all as domain/business
                    for eid in &cluster.entity_ids {
                        all_entity_class.push((eid.clone(), "domain_entity".to_string(), String::new()));
                    }
                    for aid in &cluster.action_ids {
                        all_action_class.push((aid.clone(), "business_action".to_string(), String::new()));
                    }
                }
            }
        }

        // Step 3: Generate BDD scenarios via LLM
        let domain_entities: Vec<(String, String)> = all_entity_class.iter()
            .filter(|(_, role, _)| role == "domain_entity")
            .map(|(id, _, desc)| (id.clone(), desc.clone()))
            .collect();
        let business_actions: Vec<(String, String)> = all_action_class.iter()
            .filter(|(_, role, _)| role == "business_action")
            .map(|(id, _, desc)| (id.clone(), desc.clone()))
            .collect();

        let mut all_scenarios: Vec<crate::linker::llm_classify::FeatureScenarios> = Vec::new();

        if !business_actions.is_empty() && !skip_scenarios {
            // Batch actions into groups of 10
            let batch_size = 10;
            let action_batches: Vec<&[(String, String)]> = business_actions.chunks(batch_size).collect();
            let total_batches = action_batches.len();

            for (i, batch) in action_batches.iter().enumerate() {
                eprintln!("  [{}/{}] Generating BDD scenarios (batch {}/{})...",
                    clusters.len() + i + 1, clusters.len() + total_batches,
                    i + 1, total_batches);

                let prompt = build_scenario_prompt(&domain_entities, batch, sys_name);
                let request = build_scenario_request(&prompt, model_name, ctx);

                let start = std::time::Instant::now();
                match client.post(format!("{}/api/generate", base_url))
                    .json(&request)
                    .send()
                {
                    Ok(response) if response.status().is_success() => {
                        if let Ok(json) = response.json::<serde_json::Value>() {
                            let text = json.get("response").and_then(|r| r.as_str()).unwrap_or("");
                            let elapsed = start.elapsed();
                            match parse_scenarios(text) {
                                Ok(result) => {
                                    let scenario_count: usize = result.features.iter()
                                        .map(|f| f.scenarios.len())
                                        .sum();
                                    eprintln!("  ✓ {} features, {} scenarios in {:.1}s",
                                        result.features.len(), scenario_count, elapsed.as_secs_f64());
                                    all_scenarios.extend(result.features);
                                }
                                Err(e) => {
                                    eprintln!("  ⚠ Scenario parse failed: {}", e);
                                }
                            }
                        }
                    }
                    Ok(response) => {
                        eprintln!("  ⚠ Scenario generation failed: {}", response.status());
                    }
                    Err(e) => {
                        eprintln!("  ⚠ Scenario request failed: {}", e);
                    }
                }
            }
        }

        // Step 4: Build spec from classifications + scenarios + scanner data
        eprintln!("  ℹ Assembling FDML spec ({} entities, {} actions, {} scenario groups)...",
            domain_entities.len(), business_actions.len(), all_scenarios.len());
        let spec = crate::linker::assemble::assemble_from_classifications_with_scenarios(
            report, scan, &all_entity_class, &all_action_class, &all_scenarios);

        Ok(spec)
    }

    /// Call LLM — provider selection: "cli", "api", "ollama", or None = auto
    fn call_llm(&self, prompt: &str, fast: bool, model: Option<&str>, provider: Option<&str>,
                 ollama_url: Option<&str>, num_ctx: Option<usize>) -> Result<String> {
        let prompt_lines = prompt.lines().count();
        let prompt_bytes = prompt.len();
        eprintln!("  ℹ Prompt: {} lines, {:.1} KB", prompt_lines, prompt_bytes as f64 / 1024.0);

        match provider {
            Some("cli") => {
                eprintln!("  ℹ Provider forced: claude CLI");
                self.call_claude_cli(prompt, fast, model)
            }
            Some("api") => {
                let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                    crate::error::FdmlError::project_error(
                        "ANTHROPIC_API_KEY not set. Use --provider cli to use claude CLI instead.".to_string()
                    )
                })?;
                self.call_anthropic_api(&api_key, prompt, fast, model)
            }
            Some("ollama") => {
                eprintln!("  ℹ Provider forced: Ollama");
                self.call_ollama(prompt, model, num_ctx, ollama_url)
            }
            Some("hf") => {
                let token = std::env::var("HF_TOKEN").map_err(|_| {
                    crate::error::FdmlError::project_error(
                        "HF_TOKEN not set. Export your Hugging Face token: export HF_TOKEN=hf_...".to_string()
                    )
                })?;
                eprintln!("  ℹ Provider forced: Hugging Face Router");
                self.call_hf_router(&token, prompt, model)
            }
            _ => {
                // Auto-detect: API key → Ollama → CLI
                if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
                    match self.call_anthropic_api(&api_key, prompt, fast, model) {
                        Ok(result) => Ok(result),
                        Err(e) => {
                            eprintln!("  ⚠ API failed: {}. Falling back to claude CLI...", e);
                            self.call_claude_cli(prompt, fast, model)
                        }
                    }
                } else if Self::is_ollama_running(ollama_url) {
                    eprintln!("  ℹ Auto-detected: Ollama running");
                    self.call_ollama(prompt, model, num_ctx, ollama_url)
                } else {
                    self.call_claude_cli(prompt, fast, model)
                }
            }
        }
    }

    /// Check if Ollama is running
    fn is_ollama_running(ollama_url: Option<&str>) -> bool {
        let base = ollama_url.unwrap_or("http://localhost:11434");
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(500))
            .build()
            .ok()
            .and_then(|c| c.get(base).send().ok())
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Calculate appropriate num_ctx based on prompt size
    fn calculate_num_ctx(prompt: &str) -> usize {
        let estimated_tokens = prompt.len() / 4;
        let needed = estimated_tokens + 4096; // room for response
        let clamped = needed.min(131072).max(2048);
        ((clamped + 1023) / 1024) * 1024 // round up to nearest 1024
    }

    /// Call Ollama API (local LLM)
    fn call_ollama(&self, prompt: &str, model: Option<&str>, num_ctx: Option<usize>, ollama_url: Option<&str>) -> Result<String> {
        let base = ollama_url
            .map(|s| s.to_string())
            .or_else(|| std::env::var("OLLAMA_URL").ok())
            .unwrap_or_else(|| "http://localhost:11434".to_string());

        let model_name = model.unwrap_or("gemma4");
        let ctx = num_ctx.unwrap_or_else(|| Self::calculate_num_ctx(prompt));
        let prompt_tokens_est = prompt.len() / 4;

        eprintln!("  ℹ Provider: Ollama ({})", base);
        eprintln!("  ℹ Model: {}", model_name);
        eprintln!("  ℹ Context window: {} tokens (prompt ~{}K tokens)", ctx, prompt_tokens_est / 1000);

        if prompt_tokens_est > ctx {
            eprintln!("  ⚠ WARNING: Prompt (~{}K tokens) exceeds num_ctx ({}) — Ollama will silently truncate!",
                prompt_tokens_est / 1000, ctx);
            eprintln!("  ⚠ Consider using --chunk-strategy sectional or increasing --num-ctx");
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600)) // 10 min timeout for large generations
            .build()
            .map_err(|e| crate::error::FdmlError::project_error(format!("HTTP client error: {}", e)))?;

        let request = serde_json::json!({
            "model": model_name,
            "prompt": format!(
                "You are an FDML specification generator. Output ONLY valid YAML — no markdown fences, no explanations. Start directly with YAML content.\n\n{}",
                prompt
            ),
            "stream": false,
            "options": {
                "num_ctx": ctx,
                "temperature": 0.1
            }
        });

        eprintln!("  ℹ Sending request to Ollama...");
        let start = std::time::Instant::now();

        let response = client.post(format!("{}/api/generate", base))
            .json(&request)
            .send()
            .map_err(|e| {
                if e.is_connect() {
                    crate::error::FdmlError::project_error(
                        format!("Ollama not running at {}. Start it with: ollama serve", base)
                    )
                } else if e.is_timeout() {
                    crate::error::FdmlError::project_error(
                        "Ollama request timed out (10 min). Model may be too slow for this prompt size.".to_string()
                    )
                } else {
                    crate::error::FdmlError::project_error(format!("Ollama request failed: {}", e))
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            let error_msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("error")?.as_str().map(String::from))
                .unwrap_or(body);
            return Err(crate::error::FdmlError::project_error(
                format!("Ollama error ({}): {}", status, error_msg)
            ));
        }

        let json: serde_json::Value = response.json().map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to parse Ollama response: {}", e))
        })?;

        let text = json.get("response")
            .and_then(|r| r.as_str())
            .ok_or_else(|| crate::error::FdmlError::project_error("No 'response' field in Ollama output".to_string()))?;

        let elapsed = start.elapsed();
        let eval_count = json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
        eprintln!("  ℹ Response: {} lines, {} tokens in {:.1}s ({:.0} tok/s)",
            text.lines().count(), eval_count, elapsed.as_secs_f64(),
            eval_count as f64 / elapsed.as_secs_f64());

        print_success("LLM response received (Ollama)");
        Ok(Self::strip_yaml_fences(text))
    }

    /// Call Hugging Face Inference Router (OpenAI-compatible chat completions).
    /// Default model: meta-llama/Llama-3.3-70B-Instruct:cerebras (≈1500–2000 tok/s).
    /// Other fast variants: ":groq", ":sambanova", ":together", ":fireworks-ai".
    /// Override via --model. Requires env HF_TOKEN.
    fn call_hf_router(&self, token: &str, prompt: &str, model: Option<&str>) -> Result<String> {
        // Default: gpt-oss-120b via Cerebras — benchmarked ~1179 tok/s end-to-end on 2K-token outputs.
        // Alt: "meta-llama/Llama-3.3-70B-Instruct:groq" (~380 tok/s), ":sambanova", ":together".
        let model_name = model.unwrap_or("openai/gpt-oss-120b:cerebras").to_string();

        eprintln!("  ℹ Model: {}", model_name);
        eprintln!("  ℹ Endpoint: https://router.huggingface.co/v1/chat/completions");

        let system_msg = "You are an FDML specification generator. Output ONLY valid YAML — no markdown fences, no explanations. Start directly with YAML content. Every entity MUST have a fields array (use `fields: []` if none). Every scenario MUST have exactly one `when:` key.";

        let prompt_tokens_est = prompt.len() / 4;
        eprintln!("  ℹ Sending request (~{}K tokens)...", prompt_tokens_est / 1000);

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| crate::error::FdmlError::project_error(format!("HTTP client error: {}", e)))?;

        let request = serde_json::json!({
            "model": model_name,
            "max_tokens": 16384,
            "temperature": 0.1,
            "stream": false,
            "messages": [
                {"role": "system", "content": system_msg},
                {"role": "user",   "content": prompt},
            ]
        });

        let start = std::time::Instant::now();

        let response = client
            .post("https://router.huggingface.co/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|e| crate::error::FdmlError::project_error(format!("HF request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(crate::error::FdmlError::project_error(
                format!("HF Router error ({}): {}", status, body)
            ));
        }

        let json: serde_json::Value = response.json().map_err(|e| {
            crate::error::FdmlError::project_error(format!("Bad HF response: {}", e))
        })?;

        if let Some(error) = json.get("error") {
            return Err(crate::error::FdmlError::project_error(
                format!("HF error: {}", error)
            ));
        }

        let text = json.get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| crate::error::FdmlError::project_error(
                format!("No text in HF response: {}", json)
            ))?;

        let elapsed = start.elapsed();
        let usage = json.get("usage");
        let prompt_t = usage.and_then(|u| u.get("prompt_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
        let completion_t = usage.and_then(|u| u.get("completion_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
        eprintln!("  ℹ Tokens: {} in, {} out in {:.1}s ({:.0} out tok/s)",
            prompt_t, completion_t, elapsed.as_secs_f64(),
            completion_t as f64 / elapsed.as_secs_f64().max(0.001));

        print_success("LLM response received (HF Router)");
        Ok(Self::strip_yaml_fences(text))
    }

    /// Call Anthropic API directly via curl (requires ANTHROPIC_API_KEY)
    fn call_anthropic_api(&self, api_key: &str, prompt: &str, fast: bool, model: Option<&str>) -> Result<String> {
        use std::process::Command;

        let model_name = match model {
            Some("haiku") => "claude-haiku-4-5-20251001".to_string(),
            Some("sonnet") => "claude-sonnet-4-5-20250929".to_string(),
            Some("opus") => "claude-opus-4-6".to_string(),
            Some(m) => m.to_string(),
            None if fast => "claude-haiku-4-5-20251001".to_string(),
            None => "claude-sonnet-4-5-20250929".to_string(),
        };

        eprintln!("  ℹ Model: {}", model_name);
        eprintln!("  ℹ Provider: Anthropic API (curl)");

        let system_msg = "You are an FDML specification generator. Output ONLY valid YAML — no markdown fences, no explanations. Start directly with YAML content.";

        // Use serde_json to build the request (safe escaping)
        let request = serde_json::json!({
            "model": model_name,
            "max_tokens": 16384,
            "system": system_msg,
            "messages": [{"role": "user", "content": prompt}]
        });

        let tmp_body = std::env::temp_dir().join(format!("fdml_llm_request_{:?}.json", std::thread::current().id()));
        fs::write(&tmp_body, request.to_string()).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to write request: {}", e))
        })?;

        let prompt_tokens_est = prompt.len() / 4; // rough estimate
        eprintln!("  ℹ Sending request (~{}K tokens)...", prompt_tokens_est / 1000);

        let output = Command::new("curl")
            .arg("-s")
            .arg("-X").arg("POST")
            .arg("https://api.anthropic.com/v1/messages")
            .arg("-H").arg(format!("x-api-key: {}", api_key))
            .arg("-H").arg("anthropic-version: 2023-06-01")
            .arg("-H").arg("content-type: application/json")
            .arg("-d").arg(format!("@{}", tmp_body.display()))
            .output()
            .map_err(|e| {
                crate::error::FdmlError::project_error(format!("curl failed: {}", e))
            })?;

        let _ = fs::remove_file(&tmp_body);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::FdmlError::project_error(format!("curl error: {}", stderr)));
        }

        let response_str = String::from_utf8_lossy(&output.stdout).to_string();
        let json: serde_json::Value = serde_json::from_str(&response_str).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Bad API response: {}", e))
        })?;

        if let Some(error) = json.get("error") {
            let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown");
            return Err(crate::error::FdmlError::project_error(format!("API error: {}", msg)));
        }

        let text = json.get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| crate::error::FdmlError::project_error("No text in API response".to_string()))?;

        if let Some(usage) = json.get("usage") {
            let inp = usage.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
            let out = usage.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
            eprintln!("  ℹ Tokens: {} in, {} out", inp, out);
        }
        eprintln!("  ℹ Response: {} lines", text.lines().count());

        print_success("LLM response received (API)");
        Ok(Self::strip_yaml_fences(text))
    }

    /// Call claude CLI subprocess
    /// Usage: cat prompt.md | claude -p "instruction" --output-format text --max-turns 1
    /// See: https://code.claude.com/docs/en/cli-reference
    fn call_claude_cli(&self, prompt: &str, fast: bool, model: Option<&str>) -> Result<String> {
        use std::process::Command;

        let model_name = if let Some(m) = model {
            m.to_string()
        } else if fast {
            "haiku".to_string()
        } else {
            "sonnet".to_string()
        };

        eprintln!("  ℹ Provider: claude CLI");
        eprintln!("  ℹ Model: {}", model_name);

        // Write prompt to temp file (shell-safe filename: numeric pid + nanos, no parens)
        let unique = format!(
            "{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let tmp_prompt = std::env::temp_dir().join(format!("fdml_link_prompt_{}.md", unique));
        fs::write(&tmp_prompt, prompt).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to write prompt: {}", e))
        })?;

        let system_prompt = "You are an FDML specification generator. Output ONLY valid YAML — no markdown fences, no explanations. Start directly with YAML content.";

        eprintln!("  ℹ Running: claude -p ... --max-turns 1 --output-format text (prompt via stdin)");

        // Invoke claude directly with stdin (no shell, no quoting issues)
        use std::io::Write;
        use std::process::Stdio;
        let prompt_bytes = fs::read(&tmp_prompt).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read prompt back: {}", e))
        })?;
        let mut child = Command::new("claude")
            .arg("-p")
            .arg("Generate a complete FDML YAML specification from this analysis")
            .arg("--model").arg(&model_name)
            .arg("--system-prompt").arg(system_prompt)
            .arg("--output-format").arg("text")
            .arg("--max-turns").arg("1")
            .arg("--no-session-persistence")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| crate::error::FdmlError::project_error(format!("Failed to spawn claude: {}", e)))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&prompt_bytes).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to write to claude stdin: {}", e))
            })?;
        }
        let output = child.wait_with_output()
            .map_err(|e| {
                crate::error::FdmlError::project_error(format!("claude CLI failed to start: {}", e))
            })?;

        let _ = fs::remove_file(&tmp_prompt);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::FdmlError::project_error(
                format!("claude CLI error (exit {}): {}", output.status, stderr)
            ));
        }

        let response = String::from_utf8_lossy(&output.stdout).to_string();
        if response.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::FdmlError::project_error(
                format!("claude CLI empty response. stderr: {}", stderr)
            ));
        }

        eprintln!("  ℹ Response: {} lines", response.lines().count());
        print_success("LLM response received (CLI)");
        Ok(Self::strip_yaml_fences(&response))
    }

    /// Strip markdown YAML fences from LLM output
    fn strip_yaml_fences(text: &str) -> String {
        let cleaned = text.trim();
        // Strip opening ```yaml / ```yml / ``` if present
        let cleaned = if cleaned.starts_with("```yaml") || cleaned.starts_with("```yml") {
            let start = cleaned.find('\n').unwrap_or(0) + 1;
            let end = cleaned.rfind("```").unwrap_or(cleaned.len());
            &cleaned[start..end]
        } else if cleaned.starts_with("```") {
            let start = cleaned.find('\n').unwrap_or(0) + 1;
            let end = cleaned.rfind("```").unwrap_or(cleaned.len());
            &cleaned[start..end]
        } else {
            cleaned
        };
        // Also strip a trailing ``` even when opening fence was absent
        // (some LLMs output bare YAML followed by a stray closing fence).
        let cleaned = cleaned.trim_end();
        let cleaned = if cleaned.ends_with("```") {
            &cleaned[..cleaned.len() - 3]
        } else {
            cleaned
        };
        cleaned.trim().to_string()
    }

    /// Apply a single migration operation directly (used for add commands)
    fn apply_single_operation(&self, operation: MigrationOperation, target: Option<String>) -> Result<()> {
        // Determine target file
        let target_file = if let Some(target_file) = target {
            let path = PathBuf::from(target_file);
            // Check if the target file exists for direct operations
            if !path.exists() {
                return Err(crate::error::FdmlError::project_error(
                    format!("Target file '{}' does not exist. Use 'fdml init' to create a new project first.", path.display())
                ));
            }
            path
        } else {
            // Find default FDML file in current directory
            let current_dir = std::env::current_dir()?;
            self.find_default_fdml_file(&current_dir)?.ok_or_else(|| {
                crate::error::FdmlError::project_error(
                    "No target file specified and no FDML file found in current directory. Use --target to specify a file.".to_string()
                )
            })?
        };
        
        // Create a temporary migration directory for this operation with a unique name
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%f");
        let temp_dir = std::env::temp_dir().join(format!("fdml_direct_{}", timestamp));
        std::fs::create_dir_all(&temp_dir)?;
        
        // Create temporary migration runner
        let runner = MigrationRunner::new(&temp_dir).with_target_file(&target_file);
        
        // Validate the operation
        runner.validate_operation(&operation)?;
        
        // Create a temporary migration file
        let migration_id = format!("direct_{}", timestamp);
        let migration = Migration {
            id: migration_id.clone(),
            title: Some("Direct CLI operation".to_string()),
            description: Some("Migration created by direct CLI command".to_string()),
            up: vec![operation],
            down: vec![], // We don't need rollback for direct operations
            dependencies: None,
        };
        
        // Write the temporary migration file
        let migration_file = temp_dir.join(format!("{}.yaml", migration_id));
        let migration_content = serde_yaml::to_string(&migration)?;
        std::fs::write(&migration_file, migration_content)?;
        
        // Apply the migration using the existing apply_migrations method
        let applied = runner.apply_migrations(false)?;
        
        if applied.is_empty() {
            print_warning("No operations were applied");
        }
        
        // Clean up temporary directory
        std::fs::remove_dir_all(&temp_dir).ok();
        
        Ok(())
    }
    
    /// Load FDML document from target file or find default
    fn load_fdml_document(&self, target: Option<String>) -> Result<crate::parser::ast::FdmlDocument> {
        let file_path = if let Some(target_file) = target {
            PathBuf::from(target_file)
        } else {
            let current_dir = std::env::current_dir()?;
            self.find_default_fdml_file(&current_dir)?.ok_or_else(|| {
                crate::error::FdmlError::project_error(
                    "No target file specified and no FDML file found in current directory. Use --target to specify a file.".to_string()
                )
            })?
        };
        
        let content = fs::read_to_string(&file_path).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", file_path.display(), e))
        })?;
        
        parse_fdml_yaml(&content)
    }
    
    /// Find the default FDML file in a directory
    fn find_default_fdml_file(&self, dir: &Path) -> Result<Option<PathBuf>> {
        let possible_files = [
            "spec.fdml",
            "specification.fdml", 
            "main.fdml",
            "app.fdml"
        ];
        
        for filename in &possible_files {
            let path = dir.join(filename);
            if path.exists() {
                return Ok(Some(path));
            }
        }
        
        // Look for any .fdml file
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("fdml") {
                        return Ok(Some(path));
                    }
                }
            }
        }
        
        Ok(None)
    }
}

impl Default for CommandRunner {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::env;
    
    #[test]
    fn test_init_command() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        
        // Change to temp directory
        env::set_current_dir(temp_dir.path()).unwrap();
        
        let runner = CommandRunner::new(false);
        let result = runner.run_init("test-project".to_string(), false);
        
        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
        
        assert!(result.is_ok());
    }
}
