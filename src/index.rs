//! Persistent, disposable repository navigation index.  This module deliberately has
//! no CLI dependency so it can later back a small MCP surface.
use crate::scanner::{self, types::{CodeElement, ElementType, FileAnalysis, Language}};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::{collections::HashSet, fs, hash::{Hash, Hasher}, path::{Path, PathBuf}, time::UNIX_EPOCH};

const SKIP_DIRS: &[&str] = &[".git", ".fdml", "node_modules", "target", "build", "dist", "vendor", "third_party", "third-party", "external", "deps", ".venv", "venv", "__pycache__", "coverage"];
// 4: C parsed with tree-sitter — macros, typedefs, structs, globals, prototypes,
// and declarations inside include guards.
const INDEX_FORMAT_VERSION: &str = "10";
/// Bounds for `flows`, mirroring the deterministic `fdml-flows` pass: a visited set
/// plus a hard depth cap, so cyclic call graphs still terminate.
const FLOW_MAX_DEPTH: usize = 6;
const FLOW_MAX_CHAINS: usize = 8;
/// A function past this many lines cannot be retrieved or described as one unit;
/// callers should navigate it by line anchors instead. Flagged, never hidden.
const OVERSIZED_BODY_LINES: usize = 300;
/// Call sites further apart than this start a new phase in an outline. Matches the
/// ~40-line window agents actually read around an anchor.
const PHASE_GAP_LINES: i64 = 40;
/// Connective words carry no signal but dilute a lexical key: "почему всё белое и
/// пересвечено" must still reach a note keyed "всё пересвечено". Filtered on the
/// query side only — a note may legitimately contain them in its phrasing.
const STOPWORDS: &[&str] = &["почему","что","как","где","куда","зачем","когда","это","этот","эта","при","для","из","на","в","и","или","не","бы","же","ли","то","так","the","a","an","why","what","how","where","when","is","are","was","were","and","or","not","of","in","on","for","to","do","does","did"];
/// Prefix length used to match a query word against a differently-spelled identifier.
const STEM_LEN: usize = 4;
/// Lines an agent actually reads around a hit (measured: 34-49).
const READ_WINDOW: usize = 40;
/// Below this top score the answer is noise, and the honest reply is "nothing useful"
/// (M2). Exact-name hedge floor is 0.92; weak substring matches sit around 0.3-0.5.
pub const USEFUL_SCORE: f64 = 0.55;
/// Grep evidence lines fed to the LLM fallback.
const EVIDENCE_CAP: usize = 40;
/// An anchor is a navigation aid, not a definition: it may beat a weak symbol match
/// but never an exact one.
const ANCHOR_WEIGHT: f64 = 0.8;
/// A query token merely occurring inside a longer string is a hint, not an answer —
/// otherwise a debug print naming a function outranks the function.
const PARTIAL_LITERAL: f64 = 0.4;

#[derive(Debug, Serialize)] pub struct IndexReport { pub parsed_files: usize, pub unchanged_files: usize, pub removed_files: usize, pub symbols: usize }
#[derive(Debug, Serialize)] pub struct SearchResult { pub symbol: String, pub qualified_name: String, pub kind: String, pub file: String, pub start_line: usize, pub end_line: usize, pub score: f64, #[serde(default)] pub marked: bool, pub body_lines: usize, pub oversized: bool,
    /// `main -> bcensus` when the hit is inside a body, so the caller knows where it landed
    #[serde(skip_serializing_if = "Option::is_none")] pub anchor: Option<String>,
    /// Notes anchored to this symbol — surfaced inline, because a note nobody
    /// sees is a note nobody wrote
    #[serde(skip_serializing_if = "Vec::is_empty", default)] pub notes: Vec<Note>,
    /// The lines to actually read: agents read ~40 around a hit, never a whole function
    pub window: [usize; 2] }
#[derive(Debug, Serialize)] pub struct SymbolSource { pub symbol: String, pub qualified_name: String, pub file: String, pub start_line: usize, pub end_line: usize, pub signature: Option<String>, pub description: Option<String>, pub input_summary: Option<String>, pub process_summary: Option<String>, pub output_summary: Option<String>, pub source: String, pub source_file_tokens_approx: usize, pub retrieved_tokens_approx: usize, pub tokens_saved_approx: usize, pub reduction_ratio: f64 }
#[derive(Debug, Serialize)] pub struct Flow { pub entry: String, pub chain: Vec<String>, pub depth: usize, pub next: Vec<String>, #[serde(default)] pub augmented_by: Vec<String> }
#[derive(Debug, Serialize)] pub struct Phase { pub line: usize, pub end_line: usize, pub label: Option<String>, pub calls: Vec<String> }
#[derive(Debug, Serialize)] pub struct Outline { pub symbol: String, pub file: String, pub start_line: usize, pub end_line: usize, pub body_lines: usize, pub call_sites: usize, pub phases: Vec<Phase> }
#[derive(Debug, Serialize)] pub struct Evidence { pub file: String, pub line: usize, pub text: String, pub symbol: Option<String> }
/// Knowledge with no address in the code: a repro recipe, a postmortem, an
/// invariant, a rejected hypothesis, a method. Words -> text, where `marks` are
/// words -> place. Optionally anchored to a symbol so it surfaces alongside it.
#[derive(Debug, Serialize, Clone)] pub struct Note { pub kind: String, pub phrase: String, pub body: String, pub target: Option<String>, pub commit_sha: Option<String>, pub created_at: String, pub stale: bool }
/// One failed query turned into a proposed mark.
/// Everything recorded against one commit, gathered into one card: the change's
/// dossier. Not a new store — a view over notes, marks and the call graph, keyed
/// by the commit they were written on.
#[derive(Debug, Serialize)] pub struct Dossier { pub commit: String, pub anchors: Vec<String>, pub flow: Vec<String>, pub state: Vec<Note>, pub numbers: Vec<Note>, pub rejected: Vec<Note>, pub verify: Vec<Note>, pub pending: Vec<Note>, pub links: Vec<Note>, pub symptoms: Vec<String>, pub missing: Vec<String> }
#[derive(Debug, Serialize)] pub struct HealProposal { pub query: String, pub target: String, pub reason: String, pub applied: bool }
#[derive(Debug, Serialize)] pub struct SymbolFact { pub provider: String, pub target: String, pub fact_kind: String, pub payload: serde_json::Value, pub confidence: String }
#[derive(Debug, Serialize)] pub struct ImpactGraph { pub symbol: String, pub callers: Vec<String>, pub callees: Vec<String>, pub imports: Vec<String>, pub implementations: Vec<String>, pub tests: Vec<String> }
#[derive(Debug, Serialize)] pub struct IndexStatus { pub root_path: String, pub files: usize, pub symbols: usize, pub references: usize, pub marks: usize, pub last_indexed: Option<String>, pub index_size: u64 }

pub struct Indexer;
pub struct RepositoryIndex { root: PathBuf, db: Connection }

impl Indexer {
    pub fn index(root: &Path) -> std::result::Result<IndexReport, String> {
        let mut index = RepositoryIndex::open(root)?;
        let refresh: bool = index.db.query_row("SELECT value FROM metadata WHERE key='index_format_version'", [], |r| r.get::<_,String>(0)).optional().map_err(|e|e.to_string())?.as_deref() != Some(INDEX_FORMAT_VERSION);
        let files = source_files(root)?;
        let mut seen = HashSet::new(); let mut parsed = 0; let mut unchanged = 0; let mut dirty = false;
        for path in files {
            let rel = rel(root, &path); seen.insert(rel.clone());
            let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
            let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos() as i64).unwrap_or(0);
            let size = meta.len() as i64;
            let prior: Option<(i64,i64)> = index.db.query_row("SELECT mtime,size FROM files WHERE path=?1", params![rel], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e| e.to_string())?;
            if !refresh && prior == Some((mtime, size)) { unchanged += 1; continue; }
            let text = match fs::read_to_string(&path) { Ok(s) => s, Err(_) => continue };
            let hash = content_hash(&text);
            let old_hash: Option<String> = index.db.query_row("SELECT content_hash FROM files WHERE path=?1", params![rel], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
            if !refresh && old_hash.as_deref() == Some(&hash) { index.db.execute("UPDATE files SET mtime=?1,size=?2 WHERE path=?3", params![mtime,size,rel]).map_err(|e| e.to_string())?; unchanged += 1; continue; }
            index.replace_file(&rel, language(&path).unwrap(), &text, mtime, size, &hash)?; parsed += 1; dirty = true;
        }
        let mut stale = Vec::new(); { let mut s = index.db.prepare("SELECT path FROM files").map_err(|e| e.to_string())?; let rows = s.query_map([], |r| r.get::<_,String>(0)).map_err(|e| e.to_string())?; for p in rows { let p=p.map_err(|e|e.to_string())?; if !seen.contains(&p) { stale.push(p); } } }
        for p in &stale { index.db.execute("DELETE FROM files WHERE path=?1", params![p]).map_err(|e|e.to_string())?; dirty=true; }
        if dirty { index.rebuild_references()?; }
        index.db.execute("INSERT INTO metadata(key,value) VALUES('last_indexed',datetime('now')) ON CONFLICT(key) DO UPDATE SET value=excluded.value", []).map_err(|e|e.to_string())?;
        index.db.execute("INSERT INTO metadata(key,value) VALUES('index_format_version',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![INDEX_FORMAT_VERSION]).map_err(|e|e.to_string())?;
        index.write_fdml_map()?;
        let symbols: usize = index.db.query_row("SELECT count(*) FROM symbols", [], |r| r.get(0)).map_err(|e|e.to_string())?;
        Ok(IndexReport { parsed_files: parsed, unchanged_files: unchanged, removed_files: stale.len(), symbols })
    }
}

impl RepositoryIndex {
    pub fn open(root: &Path) -> std::result::Result<Self, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let dir=root.join(".fdml"); fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
        let db=Connection::open(dir.join("index.sqlite")).map_err(|e|e.to_string())?;
        db.execute_batch("PRAGMA foreign_keys=ON;
CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS files(id INTEGER PRIMARY KEY,path TEXT UNIQUE NOT NULL,language TEXT NOT NULL,content_hash TEXT NOT NULL,mtime INTEGER NOT NULL,size INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS symbols(id INTEGER PRIMARY KEY,file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,name TEXT NOT NULL,qualified_name TEXT NOT NULL,kind TEXT NOT NULL,parent_symbol_id INTEGER,start_line INTEGER NOT NULL,start_column INTEGER NOT NULL,end_line INTEGER NOT NULL,end_column INTEGER NOT NULL,signature TEXT,description TEXT,input_summary TEXT,process_summary TEXT,output_summary TEXT,description_model TEXT,description_version TEXT,description_hash TEXT,tags TEXT,name_tokens TEXT,doc TEXT,UNIQUE(file_id,qualified_name,start_line));
CREATE TABLE IF NOT EXISTS relation_edges(id INTEGER PRIMARY KEY,file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,source_qn TEXT NOT NULL,target_name TEXT NOT NULL,kind TEXT NOT NULL,line INTEGER NOT NULL,inferred INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS references_idx(id INTEGER PRIMARY KEY,source_symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,target_symbol_id INTEGER REFERENCES symbols(id) ON DELETE SET NULL,file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,line INTEGER NOT NULL,kind TEXT NOT NULL,inferred INTEGER NOT NULL DEFAULT 0);
CREATE VIRTUAL TABLE IF NOT EXISTS symbol_search USING fts5(symbol_id UNINDEXED,name,qualified_name,signature,path);
CREATE TABLE IF NOT EXISTS marks(id INTEGER PRIMARY KEY,query_key TEXT NOT NULL,query TEXT NOT NULL,target TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT (datetime('now')),UNIQUE(query_key,target));
CREATE TABLE IF NOT EXISTS facts(id INTEGER PRIMARY KEY,provider TEXT NOT NULL,target TEXT NOT NULL,fact_kind TEXT NOT NULL,payload TEXT NOT NULL,confidence TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT (datetime('now')),UNIQUE(provider,target,fact_kind));
CREATE INDEX IF NOT EXISTS facts_target ON facts(target);
CREATE TABLE IF NOT EXISTS anchors(id INTEGER PRIMARY KEY,file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,parent_symbol TEXT NOT NULL,kind TEXT NOT NULL,name TEXT NOT NULL,start_line INTEGER NOT NULL,end_line INTEGER NOT NULL,depth INTEGER NOT NULL,label TEXT,condition_ids TEXT,calls TEXT,declared TEXT,literals TEXT);
CREATE INDEX IF NOT EXISTS anchors_file ON anchors(file_id);
CREATE TABLE IF NOT EXISTS literal_occurrences(id INTEGER PRIMARY KEY,file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,value TEXT NOT NULL,normalized TEXT NOT NULL,kind TEXT NOT NULL,usage_kind TEXT NOT NULL,line INTEGER NOT NULL,parent_symbol TEXT NOT NULL,anchor_id INTEGER);
CREATE INDEX IF NOT EXISTS literals_value ON literal_occurrences(value);
CREATE TABLE IF NOT EXISTS notes(id INTEGER PRIMARY KEY,kind TEXT NOT NULL,query_key TEXT NOT NULL,phrase TEXT NOT NULL,body TEXT NOT NULL,target TEXT,target_hash TEXT,commit_sha TEXT,created_at TEXT NOT NULL DEFAULT (datetime('now')),UNIQUE(query_key,kind,body));
CREATE INDEX IF NOT EXISTS notes_target ON notes(target);
CREATE TABLE IF NOT EXISTS query_log(id INTEGER PRIMARY KEY,query TEXT NOT NULL,top_score REAL,results INTEGER NOT NULL,useful INTEGER NOT NULL,llm_used INTEGER NOT NULL DEFAULT 0,llm_rescued INTEGER NOT NULL DEFAULT 0,command TEXT NOT NULL DEFAULT 'search',created_at TEXT NOT NULL DEFAULT (datetime('now')));").map_err(|e|e.to_string())?;
        // Existing logs predate per-command telemetry; give them the column.
        let _ = db.execute("ALTER TABLE query_log ADD COLUMN command TEXT NOT NULL DEFAULT 'search'", []);
        // Forward-compatible migration for indexes created before local marking.
        for column in ["doc TEXT", "name_tokens TEXT", "input_summary TEXT", "process_summary TEXT", "output_summary TEXT", "description_model TEXT", "description_version TEXT", "description_hash TEXT", "tags TEXT"] { let _ = db.execute(&format!("ALTER TABLE symbols ADD COLUMN {column}"), []); }
        Ok(Self { root, db })
    }

    fn replace_file(&mut self, path:&str, lang:Language, text:&str, mtime:i64,size:i64,hash:&str)->std::result::Result<(),String> {
        self.db.execute("INSERT INTO files(path,language,content_hash,mtime,size) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(path) DO UPDATE SET language=excluded.language,content_hash=excluded.content_hash,mtime=excluded.mtime,size=excluded.size", params![path,lang.name(),hash,mtime,size]).map_err(|e|e.to_string())?;
        let file_id:i64=self.db.query_row("SELECT id FROM files WHERE path=?1",params![path],|r|r.get(0)).map_err(|e|e.to_string())?;
        self.db.execute("DELETE FROM symbols WHERE file_id=?1",params![file_id]).map_err(|e|e.to_string())?;
        self.db.execute("DELETE FROM relation_edges WHERE file_id=?1",params![file_id]).map_err(|e|e.to_string())?;
        let analysis=parse(lang.clone(),text,path)?; let module=module_name(path,lang);
        let module_qn=if module.is_empty(){path.to_string()}else{module};
        self.insert_symbol(file_id, &module_qn, &module_qn, "module", None, 1, 0, text.lines().count().max(1), 0, None)?;
        self.db.execute("DELETE FROM anchors WHERE file_id=?1",params![file_id]).map_err(|e|e.to_string())?;
        self.db.execute("DELETE FROM literal_occurrences WHERE file_id=?1",params![file_id]).map_err(|e|e.to_string())?;
        for el in &analysis.elements { self.insert_element(file_id,el,&module_qn,None)?; self.insert_calls(file_id, el, &module_qn, text)?; }
        self.insert_navigation(file_id,&analysis)?;
        for imp in analysis.imports { self.db.execute("INSERT INTO relation_edges(file_id,source_qn,target_name,kind,line,inferred) VALUES(?1,?2,?3,'import',?4,0)",params![file_id,module_qn,imp.names.first().cloned().unwrap_or(imp.module),imp.line as i64]).map_err(|e|e.to_string())?; }
        // The comment above a declaration is the author explaining the concept — the
        // richest searchable text in the file, previously invisible to search.
        self.attach_docs(file_id,text)?;
        Ok(())
    }
    /// Anchors and literals: searchable evidence about *where to read*, deliberately
    /// stored outside `symbols` so they can never pollute symbol ranking.
    fn insert_navigation(&self,file_id:i64,analysis:&FileAnalysis)->std::result::Result<(),String>{
        for a in &analysis.anchors {
            self.db.execute("INSERT INTO anchors(file_id,parent_symbol,kind,name,start_line,end_line,depth,label,condition_ids,calls,declared,literals) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![file_id,a.parent_symbol,a.kind,a.name,a.line_start as i64,a.line_end as i64,a.depth as i64,a.label,a.condition_ids.join(" "),a.calls.join(" "),a.declared.join(" "),a.literals.join(" ")]).map_err(|e|e.to_string())?;
        }
        for l in &analysis.literals {
            // the anchor whose range contains the literal, narrowest first
            let anchor_id:Option<i64>=self.db.query_row("SELECT id FROM anchors WHERE file_id=?1 AND start_line<=?2 AND end_line>=?2 ORDER BY end_line-start_line LIMIT 1",params![file_id,l.line as i64],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
            self.db.execute("INSERT INTO literal_occurrences(file_id,value,normalized,kind,usage_kind,line,parent_symbol,anchor_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![file_id,l.value,normalize_format(&l.value),l.kind,l.usage_kind,l.line as i64,l.parent_symbol,anchor_id]).map_err(|e|e.to_string())?;
        }
        Ok(())
    }
    fn insert_element(&self,file_id:i64,el:&CodeElement,parent_qn:&str,parent:Option<i64>)->std::result::Result<(),String>{
        let qn=format!("{parent_qn}.{}",el.name); let kind=kind(&el.element_type);
        let id=self.insert_symbol(file_id,&el.name,&qn,kind,parent,el.line_start,0,el.line_end,0,el.signature.as_deref())?;
        for base in &el.bases { self.db.execute("INSERT INTO relation_edges(file_id,source_qn,target_name,kind,line,inferred) VALUES(?1,?2,?3,?4,?5,0)",params![file_id,qn,base.split('.').last().unwrap_or(base),if matches!(el.element_type,ElementType::Interface){"implementation"}else{"inheritance"},el.line_start as i64]).map_err(|e|e.to_string())?; }
        for child in &el.children { self.insert_element(file_id,child,&qn,Some(id))?; } Ok(())
    }
    fn insert_calls(&self, file_id:i64, el:&CodeElement, parent_qn:&str, text:&str)->std::result::Result<(),String>{
        let qn=format!("{parent_qn}.{}", el.name);
        let lines:Vec<&str>=text.lines().collect();
        for line_no in (el.line_start + 1)..=el.line_end.min(lines.len()) {
            for target in call_names(lines[line_no - 1]) { self.db.execute("INSERT INTO relation_edges(file_id,source_qn,target_name,kind,line,inferred) VALUES(?1,?2,?3,'call',?4,0)",params![file_id,qn,target,line_no as i64]).map_err(|e|e.to_string())?; }
        }
        for child in &el.children { self.insert_calls(file_id,child,&qn,text)?; }
        Ok(())
    }
    fn attach_docs(&self,file_id:i64,text:&str)->std::result::Result<(),String>{
        let lines:Vec<&str>=text.lines().collect();
        let mut st=self.db.prepare("SELECT id,start_line FROM symbols WHERE file_id=?1 AND kind!='module'").map_err(|e|e.to_string())?;
        let rows:Vec<(i64,i64)>=st.query_map(params![file_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        for (id,start) in rows {
            if let Some(doc)=doc_above(&lines,start as usize) {
                self.db.execute("UPDATE symbols SET doc=?1 WHERE id=?2",params![doc,id]).map_err(|e|e.to_string())?;
            }
        }
        Ok(())
    }
    fn insert_symbol(&self,file:i64,name:&str,qn:&str,kind:&str,parent:Option<i64>,sl:usize,sc:usize,el:usize,ec:usize,sig:Option<&str>)->std::result::Result<i64,String>{
        // Two declarations can legitimately land on one line (`typedef struct X {...} X;`).
        // A duplicate is not a reason to abort indexing a repository.
        self.db.execute("INSERT INTO symbols(file_id,name,qualified_name,kind,parent_symbol_id,start_line,start_column,end_line,end_column,signature,name_tokens) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT DO NOTHING",params![file,name,qn,kind,parent,sl as i64,sc as i64,el as i64,ec as i64,sig,name_tokens(name)]).map_err(|e|e.to_string())?;
        let id=match self.db.last_insert_rowid() { 0 => self.db.query_row("SELECT id FROM symbols WHERE file_id=?1 AND qualified_name=?2 AND start_line=?3",params![file,qn,sl as i64],|r|r.get(0)).map_err(|e|e.to_string())?, id => id }; let p:String=self.db.query_row("SELECT path FROM files WHERE id=?1",params![file],|r|r.get(0)).map_err(|e|e.to_string())?;
        self.db.execute("INSERT INTO symbol_search(symbol_id,name,qualified_name,signature,path) VALUES(?1,?2,?3,?4,?5)",params![id,name,qn,sig.unwrap_or(""),p]).map_err(|e|e.to_string())?; Ok(id)
    }
    fn rebuild_references(&self)->std::result::Result<(),String>{
        self.db.execute("DELETE FROM references_idx; DELETE FROM symbol_search; INSERT INTO symbol_search(symbol_id,name,qualified_name,signature,path) SELECT s.id,s.name,s.qualified_name,coalesce(s.signature,''),f.path FROM symbols s JOIN files f ON f.id=s.file_id;",[]).map_err(|e|e.to_string())?;
        self.db.execute("INSERT INTO references_idx(source_symbol_id,target_symbol_id,file_id,line,kind,inferred) SELECT s.id,(SELECT t.id FROM symbols t WHERE t.name=e.target_name OR t.qualified_name=e.target_name ORDER BY t.kind='module',t.id LIMIT 1),e.file_id,e.line,e.kind,e.inferred FROM relation_edges e JOIN symbols s ON s.file_id=e.file_id AND s.qualified_name=e.source_qn",[]).map_err(|e|e.to_string())?; Ok(())
    }
    fn find(&self,q:&str)->std::result::Result<(i64,String),String>{ self.db.query_row("SELECT id,qualified_name FROM symbols WHERE qualified_name=?1 OR name=?1 ORDER BY kind='module',qualified_name=?1 DESC,id LIMIT 1",params![q],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?.ok_or_else(||format!("symbol '{q}' not found (run `fdml index` first)")) }
    pub fn search(&self,q:&str)->std::result::Result<Vec<SearchResult>,String>{
        // same tokenizer as `mark_key`, so remembered wording and typed wording meet
        let term=q.to_lowercase(); let tokens:Vec<String>=term.split(|c:char|!c.is_alphanumeric()&&c!='_').filter(|t|!t.is_empty()).map(str::to_string).collect(); if tokens.is_empty(){return Ok(vec![])};
        let fields="lower(s.name) LIKE '%'||?IDX||'%' OR lower(s.qualified_name) LIKE '%'||?IDX||'%' OR lower(f.path) LIKE '%'||?IDX||'%' OR lower(coalesce(s.signature,'')) LIKE '%'||?IDX||'%' OR lower(coalesce(s.description,'')) LIKE '%'||?IDX||'%' OR lower(coalesce(s.input_summary,'')) LIKE '%'||?IDX||'%' OR lower(coalesce(s.process_summary,'')) LIKE '%'||?IDX||'%' OR lower(coalesce(s.output_summary,'')) LIKE '%'||?IDX||'%' OR lower(coalesce(s.tags,'')) LIKE '%'||?IDX||'%' OR lower(coalesce(s.doc,'')) LIKE '%'||?IDX||'%'";
        // Any token may match, and coverage decides the rank. An AND over every word
        // returns nothing for the way people actually ask ("audio callback" vs `audio_cb`),
        // which measured as the dominant failure: 8 of 14 real questions came back empty.
        let per_token:Vec<String>=tokens.iter().enumerate().map(|(i,_)|format!("({})",fields.replace("?IDX",&format!("?{}",i+2)))).collect();
        // A word and the identifier spelling it rarely agree past the stem: "collision"
        // must still reach `collide_walls`, "damping" must reach `VH_DAMP_ANG`. Prefix
        // hits count half, so an exact match always outranks a stem match.
        let n=tokens.len();
        let per_stem:Vec<String>=tokens.iter().enumerate().map(|(i,_)|format!("({})",fields.replace("?IDX",&format!("?{}",i+2+n)))).collect();
        // the query word IS one of the identifier's words — the strongest lexical signal
        let per_word:Vec<String>=tokens.iter().enumerate().map(|(i,_)|format!("(lower(coalesce(s.name_tokens,'')) LIKE '% '||?{}||' %')",i+2)).collect();
        let where_clause=per_token.iter().chain(per_stem.iter()).cloned().collect::<Vec<_>>().join(" OR ");
        let matched=per_word.iter().zip(per_token.iter().zip(per_stem.iter())).map(|(w,(t,st))|format!("CASE WHEN {w} THEN 1.3 WHEN {t} THEN 1.0 WHEN {st} THEN 0.5 ELSE 0 END")).collect::<Vec<_>>().join(" + ");
        // Hedge semantics: half the multi-word queries agents write are alternative
        // guesses at ONE name. If any single token IS this symbol's name, that is the
        // answer — coverage over the other guesses must not dilute it.
        let exact_name=tokens.iter().enumerate().map(|(i,_)|format!("lower(s.name)=?{}",i+2)).collect::<Vec<_>>().join(" OR ");
        // score = how good the best single match is, scaled by how much of the query it covers
        let sql=format!("SELECT s.name,s.qualified_name,s.kind,f.path,s.start_line,s.end_line,MAX((CASE WHEN lower(s.qualified_name)=?1 THEN 1.0 WHEN lower(s.name)=?1 THEN .95 WHEN lower(s.qualified_name) LIKE '%'||?1||'%' THEN .80 WHEN lower(f.path) LIKE '%'||?1||'%' THEN .65 WHEN lower(coalesce(s.tags,'')) LIKE '%'||?1||'%' THEN .62 ELSE .55 END) * (({matched}) * 1.0 / {total}), CASE WHEN {exact_name} THEN 0.92 ELSE 0 END) AS score FROM symbols s JOIN files f ON f.id=s.file_id WHERE {where_clause} ORDER BY score DESC,(lower(f.path) LIKE '%test%' OR lower(f.path) LIKE '%mock%'),s.kind='module',length(s.qualified_name),s.qualified_name LIMIT 30",total=tokens.len());
        let mut out=self.marked_hits(&tokens)?;
        let mut seen:HashSet<String>=out.iter().map(|r|format!("{}:{}",r.file,r.start_line)).collect();
        // stem = the token's first STEM_LEN characters; short words stay whole
        let stems:Vec<String>=tokens.iter().map(|t|if t.chars().count()>=6 { t.chars().take(STEM_LEN).collect() } else { t.clone() }).collect();
        let mut values=vec![term];values.extend(tokens.clone());values.extend(stems); let mut st=self.db.prepare(&sql).map_err(|e|e.to_string())?;
        let rows = st.query_map(rusqlite::params_from_iter(values.iter()),|r|Ok(SearchResult{symbol:r.get(0)?,qualified_name:r.get(1)?,kind:r.get(2)?,file:r.get(3)?,start_line:r.get::<_,i64>(4)? as usize,end_line:r.get::<_,i64>(5)? as usize,score:r.get(6)?,marked:false,body_lines:0,oversized:false,anchor:None,notes:Vec::new(),window:[0,0]})).map_err(|e|e.to_string())?;
        for row in rows { let r=row.map_err(|e|e.to_string())?; if seen.insert(format!("{}:{}",r.file,r.start_line)) { out.push(r); } }
        for note in self.notes_for_query(&tokens)? {
            out.push(SearchResult{symbol:note.phrase.chars().take(48).collect(),qualified_name:format!("note:{} \"{}\"",note.kind,note.phrase),kind:format!("note:{}",note.kind),
                file:note.target.clone().unwrap_or_default(),start_line:0,end_line:0,score:0.97,marked:true,body_lines:0,oversized:false,
                anchor:Some(note.body.lines().next().unwrap_or("").chars().take(90).collect()),notes:vec![note],window:[0,0]});
        }
        out.extend(self.anchor_hits(&tokens)?);
        out.extend(self.literal_hits(&tokens)?);
        // a body too large to retrieve or describe as one unit is a navigation hazard, not a hit
        for r in &mut out {
            r.body_lines=r.end_line.saturating_sub(r.start_line)+1;
            r.oversized=r.kind!="module" && r.body_lines>OVERSIZED_BODY_LINES;
            r.window=[r.start_line.saturating_sub(READ_WINDOW/2).max(1), r.start_line+READ_WINDOW/2];
            r.score=r.score.min(1.0);
            // A precise location beats a range that merely contains the answer. Anchors
            // and literals already ARE precise, so the penalty applies to symbols only —
            // it exists to demote "the answer is somewhere in this 6527-line function".
            let is_symbol=!r.kind.starts_with("block:") && !r.kind.starts_with("literal:");
            if is_symbol && !r.marked && r.body_lines>READ_WINDOW { r.score*=(READ_WINDOW as f64/r.body_lines as f64).powf(0.15); }
        }
        let mut seen_line=HashSet::new();
        out.retain(|r| r.kind.starts_with("note:") || seen_line.insert(format!("{}:{}",r.file,r.start_line)));
        // an invariant nobody reads is an invariant nobody wrote: attach it to the hit
        for r in &mut out { if r.notes.is_empty() && !r.kind.starts_with("note:") { r.notes=self.notes_for_symbol(&r.qualified_name)?; } }
        out.sort_by(|a,b| b.marked.cmp(&a.marked).then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)).then(a.qualified_name.cmp(&b.qualified_name)));
        out.truncate(30); Ok(out)
    }

    /// Anchors inside oversized bodies. Every feature is matched separately and weighted:
    /// the comment above a region and an identifier in its condition are stronger evidence
    /// than a function it happens to call.
    fn anchor_hits(&self,tokens:&[String])->std::result::Result<Vec<SearchResult>,String>{
        let mut st=self.db.prepare("SELECT a.parent_symbol,a.kind,a.name,f.path,a.start_line,a.end_line,coalesce(a.label,''),coalesce(a.condition_ids,''),coalesce(a.calls,''),coalesce(a.declared,''),coalesce(a.literals,'') FROM anchors a JOIN files f ON f.id=a.file_id").map_err(|e|e.to_string())?;
        let rows:Vec<(String,String,String,String,i64,i64,String,String,String,String,String)>=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        let mut out=Vec::new();
        for (parent,kind,name,path,start,end,label,cond,calls,declared,lits) in rows {
            // (field, weight) — a named field carries more signal than an incidental call
            let fields=[(cond.as_str(),1.0),(declared.as_str(),1.0),(name.as_str(),0.9),(label.as_str(),0.8),(lits.as_str(),0.45),(calls.as_str(),0.35)];
            let mut score=0.0;
            for token in tokens {
                let mut best=0.0f64;
                for (text,weight) in &fields {
                    let low=text.to_lowercase();
                    // split on punctuation too: `--bundle-census` must offer the words
                    // "bundle" and "census", or prose mentioning them wins by accident
                    let hit=if low.split(|c:char|!c.is_alphanumeric()&&c!='_').any(|w|w==token) {1.0} else if low.contains(token.as_str()) {0.7}
                        else if token.chars().count()>=6 && low.contains(&token.chars().take(STEM_LEN).collect::<String>()) {0.35} else {0.0};
                    best=best.max(hit*weight);
                }
                score+=best;
            }
            if score<=0.0 { continue }
            let mut score=score/tokens.len() as f64;
            // adjacency: "bundle census" naming `--bundle-census` beats the same two
            // words scattered through a sentence that happens to mention both
            if tokens.len()>1 {
                let phrase=tokens.join(" ");
                let flat=|t:&str|t.to_lowercase().chars().map(|c|if c.is_alphanumeric()||c=='_'{c}else{' '}).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
                if flat(&label).contains(&phrase) || flat(&name).contains(&phrase) || flat(&cond).contains(&phrase) { score=(score+0.15).min(1.0); }
            }
            out.push(SearchResult{symbol:name.clone(),qualified_name:format!("{parent} → {name}"),kind:format!("block:{kind}"),file:path,start_line:start as usize,end_line:end as usize,
                score:(score*ANCHOR_WEIGHT).min(0.9),marked:false,body_lines:0,oversized:false,anchor:Some(format!("{parent} → {name}")),notes:Vec::new(),window:[0,0]});
        }
        Ok(out)
    }

    /// String literals: flags, log formats and asset names are what the code *does*,
    /// invisible to a symbol index. Generic strings are indexed but weighted down so
    /// they never crowd out real answers.
    fn literal_hits(&self,tokens:&[String])->std::result::Result<Vec<SearchResult>,String>{
        let phrase=tokens.join(" ");
        let mut st=self.db.prepare("SELECT l.value,l.normalized,l.kind,l.usage_kind,f.path,l.line,l.parent_symbol,coalesce(a.parent_symbol||' → '||a.name,'') FROM literal_occurrences l JOIN files f ON f.id=l.file_id LEFT JOIN anchors a ON a.id=l.anchor_id").map_err(|e|e.to_string())?;
        let rows:Vec<(String,String,String,String,String,i64,String,String)>=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        let mut out=Vec::new(); let mut seen=HashSet::new();
        for (value,normalized,kind,usage,path,line,parent,anchor) in rows {
            let low=value.to_lowercase();
            let stripped=low.trim_start_matches('-');
            let exact=stripped==phrase||low==phrase||normalize_format(&low)==normalize_format(&phrase);
            let covered=tokens.iter().filter(|t|low.contains(t.as_str())).count();
            if !exact && covered==0 { continue }
            // classification decides the weight, not length
            let weight=match kind.as_str() { "cli_flag"=>1.0, "format"=>0.9, "asset"=>0.85, _=>0.45 };
            let usage_bonus=match usage.as_str() { "comparison"=>0.1, "printf_format"=>0.05, _=>0.0 };
            let base=if exact {1.0} else { covered as f64/tokens.len() as f64*PARTIAL_LITERAL };
            let score=(base*weight+usage_bonus).min(1.0);
            if score<0.25 { continue }
            if !seen.insert(format!("{path}:{line}")) { continue }
            out.push(SearchResult{symbol:value.chars().take(48).collect(),qualified_name:format!("\"{}\"",value.chars().take(48).collect::<String>()),kind:format!("literal:{kind}"),file:path,start_line:line as usize,end_line:line as usize,
                score,marked:false,body_lines:0,oversized:false,anchor:if anchor.is_empty(){Some(parent)}else{Some(anchor)},notes:Vec::new(),window:[0,0]});
            let _=normalized;
        }
        Ok(out)
    }

    /// Record knowledge no scanner can recover: how to reproduce a bug, why a symptom
    /// happened, an invariant, a hypothesis already disproved, a method. Stamped with
    /// the commit and the target file's hash, so a later read can say "this was true
    /// two hundred commits ago".
    pub fn add_note(&self,phrase:&str,body:&str,kind:&str,target:Option<&str>,aliases:&[String])->std::result::Result<usize,String>{
        let body=body.trim();
        if body.is_empty() { return Err("note body is empty".into()) }
        let resolved=match target { Some(t)=>Some(self.resolve_place(t)?), None=>None };
        let target_hash=match &resolved { Some(t)=>self.hash_of_place(t), None=>None };
        let commit=std::process::Command::new("git").args(["-C",&self.root.display().to_string(),"rev-parse","--short","HEAD"]).output().ok()
            .filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s|!s.is_empty());
        let mut written=0;
        for phrase in std::iter::once(phrase.to_string()).chain(aliases.iter().cloned()) {
            let key=mark_key(&phrase);
            if key.is_empty() { continue }
            self.db.execute("INSERT INTO notes(kind,query_key,phrase,body,target,target_hash,commit_sha) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(query_key,kind,body) DO UPDATE SET phrase=excluded.phrase,target=excluded.target,target_hash=excluded.target_hash,commit_sha=excluded.commit_sha",
                params![kind,key,phrase.trim(),body,resolved,target_hash,commit]).map_err(|e|e.to_string())?;
            written+=1;
        }
        Ok(written)
    }
    /// What both marks and notes consider "a place": a symbol, a file, or file:line.
    fn resolve_place(&self,target:&str)->std::result::Result<String,String>{
        match self.find(target) { Ok((_,qn))=>Ok(qn), Err(_)=>{
            let (path,line)=match target.split_once(':'){Some((p,l))=>(p,Some(l)),None=>(target,None)};
            let hit:Option<String>=self.db.query_row("SELECT path FROM files WHERE path=?1 OR path LIKE '%'||?1 ORDER BY length(path) LIMIT 1",params![path],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
            match hit { Some(p)=>Ok(match line{Some(l)=>format!("{p}:{l}"),None=>p}), None=>Err(format!("'{target}' is not a known symbol or file (run `fdml index` first)")) } } }
    }
    fn hash_of_place(&self,place:&str)->Option<String>{
        let path=place.split(':').next().unwrap_or(place);
        self.db.query_row("SELECT content_hash FROM files WHERE path=?1",params![path],|r|r.get(0)).optional().ok().flatten()
            .or_else(||self.db.query_row("SELECT f.content_hash FROM symbols s JOIN files f ON f.id=s.file_id WHERE s.qualified_name=?1 LIMIT 1",params![place],|r|r.get(0)).optional().ok().flatten())
    }
    #[allow(clippy::too_many_arguments)]
    fn row_to_note(&self,kind:String,phrase:String,body:String,target:Option<String>,target_hash:Option<String>,commit_sha:Option<String>,created_at:String)->Note{
        // stale = the file this note points at changed since the note was written
        let stale=match (&target,&target_hash) { (Some(t),Some(h))=>self.hash_of_place(t).map(|n|&n!=h).unwrap_or(false), _=>false };
        Note{kind,phrase,body,target,commit_sha,created_at,stale}
    }
    /// Notes whose wording overlaps the query — the same lexical rule as marks.
    pub fn notes_for_query(&self,tokens:&[String])->std::result::Result<Vec<Note>,String>{
        let mut st=self.db.prepare("SELECT kind,query_key,phrase,body,target,target_hash,commit_sha,created_at FROM notes").map_err(|e|e.to_string())?;
        let rows:Vec<(String,String,String,String,Option<String>,Option<String>,Option<String>,String)>=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        let mut out=Vec::new(); let mut seen=HashSet::new();
        for (kind,key,phrase,body,target,hash,commit,at) in rows {
            if key_overlap(&key,tokens)<0.5 { continue }
            if !seen.insert(format!("{kind}|{body}")) { continue }
            out.push(self.row_to_note(kind,phrase,body,target,hash,commit,at));
        }
        Ok(out)
    }
    /// Notes anchored to a symbol, for inline surfacing next to its search hit.
    pub fn notes_for_symbol(&self,qn:&str)->std::result::Result<Vec<Note>,String>{
        let mut st=self.db.prepare("SELECT kind,phrase,body,target,target_hash,commit_sha,created_at FROM notes WHERE target=?1 OR target LIKE ?1||':%' GROUP BY body ORDER BY created_at DESC LIMIT 3").map_err(|e|e.to_string())?;
        let rows:Vec<(String,String,String,Option<String>,Option<String>,Option<String>,String)>=st.query_map(params![qn],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        Ok(rows.into_iter().map(|(k,p,b,t,h,c,a)|self.row_to_note(k,p,b,t,h,c,a)).collect())
    }
    /// Remove notes by phrase (all phrasings of the same body go together).
    pub fn delete_notes(&self,phrase:&str)->std::result::Result<usize,String>{
        let key=mark_key(phrase);
        let mut st=self.db.prepare("SELECT DISTINCT body FROM notes WHERE query_key=?1 OR phrase=?2").map_err(|e|e.to_string())?;
        let rows:rusqlite::Result<Vec<String>>=st.query_map(params![key,phrase],|r|r.get(0)).map_err(|e|e.to_string())?.collect();
        let bodies=rows.map_err(|e|e.to_string())?;
        drop(st);
        let mut removed=0;
        for body in bodies { removed+=self.db.execute("DELETE FROM notes WHERE body=?1",params![body]).map_err(|e|e.to_string())?; }
        Ok(removed)
    }
    pub fn list_notes(&self,kind:Option<&str>,limit:usize)->std::result::Result<Vec<Note>,String>{
        let sql=match kind { Some(_)=>"SELECT kind,phrase,body,target,target_hash,commit_sha,created_at FROM notes WHERE kind=?2 GROUP BY body ORDER BY id DESC LIMIT ?1",
                             None=>"SELECT kind,phrase,body,target,target_hash,commit_sha,created_at FROM notes GROUP BY body ORDER BY id DESC LIMIT ?1" };
        let mut st=self.db.prepare(sql).map_err(|e|e.to_string())?;
        let map=|r:&rusqlite::Row<'_>|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?));
        let rows:Vec<_>=match kind { Some(k)=>st.query_map(params![limit as i64,k],map).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?,
                                     None=>st.query_map(params![limit as i64],map).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())? };
        Ok(rows.into_iter().map(|(k,p,b,t,h,c,a)|self.row_to_note(k,p,b,t,h,c,a)).collect())
    }

    /// Assemble the dossier for a commit: which symbols it anchored, how execution
    /// reaches them, what invariants and numbers were recorded, what was rejected,
    /// how it was verified, and which symptoms name it. Reports what is MISSING too —
    /// a card with holes is more useful than a card that pretends to be complete.
    pub fn dossier(&self,commit:&str)->std::result::Result<Dossier,String>{
        let mut st=self.db.prepare("SELECT kind,phrase,body,target,target_hash,commit_sha,created_at FROM notes WHERE coalesce(commit_sha,'')=?1 ORDER BY id").map_err(|e|e.to_string())?;
        let notes:Vec<Note>=st.query_map(params![commit],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?)))
            .map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?
            .into_iter().map(|(k,p,b,t,h,c,a)|self.row_to_note(k,p,b,t,h,c,a)).collect();
        // aliases store the same body under several phrasings — a card shows each fact once
        let of=|kind:&str|{ let mut seen=HashSet::new();
            notes.iter().filter(|n|n.kind==kind).filter(|n|seen.insert(n.body.clone())).cloned().collect::<Vec<_>>() };
        // anchors: every symbol these notes point at, plus marks aiming at the same places
        let mut anchors:Vec<String>=Vec::new();
        for t in notes.iter().filter_map(|n|n.target.clone()) { if !anchors.contains(&t) { anchors.push(t) } }
        // flow: the call chain of the first anchored symbol — computed, never stored
        let flow=anchors.iter().find_map(|a|self.flows(a).ok().and_then(|f|f.into_iter().next()))
            .map(|f|{let mut c=f.chain.clone(); c.extend(f.next.iter().take(2).cloned()); c}).unwrap_or_default();
        // symptoms are the opposite: every phrasing matters, it is how people will ask
        let symptoms=notes.iter().filter(|n|n.kind=="postmortem"||n.kind=="repro").map(|n|n.phrase.clone()).collect::<Vec<_>>();
        let mut missing=Vec::new();
        if anchors.is_empty() { missing.push("ANCHOR — no note is attached to a symbol (`--at`)".into()) }
        if of("invariant").is_empty() { missing.push("STATE/NUMBERS — no invariant recorded (`--kind invariant`)".into()) }
        if of("pending").is_empty()&&of("rejected").is_empty() { missing.push("REJECTED — nothing recorded as tried-and-wrong (`--kind rejected`)".into()) }
        if of("method").is_empty() { missing.push("VERIFY — no check recorded (`--kind method`)".into()) }
        if symptoms.is_empty() { missing.push("SYMPTOMS — no postmortem or repro (`--kind postmortem`)".into()) }
        Ok(Dossier{commit:commit.to_string(),anchors,flow,state:of("invariant"),numbers:of("note"),rejected:of("rejected"),verify:of("method"),pending:of("pending"),links:of("link"),symptoms,missing})
    }

    /// Turn accumulated search failures into permanent marks: for each unresolved
    /// failure the evidence picker proposes a location; `apply` writes the mark.
    pub fn heal(&self,host:&str,model:&str,apply:bool,limit:usize,min_fails:usize)->std::result::Result<Vec<HealProposal>,String>{
        // One failure is noise — a benchmark, a typo, a dead end. A REPEATED failure is
        // a pattern worth teaching; healing single-shot failures polluted marks.
        let mut st=self.db.prepare("SELECT query FROM query_log WHERE useful=0 AND command='search' GROUP BY query HAVING count(*)>=?2 ORDER BY MAX(id) DESC LIMIT ?1").map_err(|e|e.to_string())?;
        let fails:Vec<String>=st.query_map(params![limit as i64,min_fails as i64],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        let mut out=Vec::new();
        for query in fails {
            if self.search(&query)?.first().is_some_and(|r|r.score>=USEFUL_SCORE) { continue }
            let picks=self.llm_fallback(&query,host,model)?;
            let Some(top)=picks.first() else { continue };
            let target=format!("{}:{}",top.file,top.start_line);
            let reason=top.anchor.clone().unwrap_or_default();
            let applied=apply && self.mark_association(&query,&target).is_ok();
            out.push(HealProposal{query,target,reason,applied});
        }
        Ok(out)
    }

    /// Associate a natural-language query with a symbol or file location.
    /// Marks are stored by name (not row id) so they survive re-indexing.
    pub fn mark_association(&self,query:&str,target:&str)->std::result::Result<String,String>{
        let key=mark_key(query); if key.is_empty(){return Err("mark query is empty".into())}
        let resolved=self.resolve_place(target)?;
        self.db.execute("INSERT INTO marks(query_key,query,target) VALUES(?1,?2,?3) ON CONFLICT(query_key,target) DO UPDATE SET query=excluded.query",params![key,query.trim(),resolved]).map_err(|e|e.to_string())?;
        Ok(resolved)
    }

    /// Symbols reached through a remembered association. Exact query match scores
    /// 1.0; a query that merely overlaps a mark's wording scores 0.9.
    fn marked_hits(&self,tokens:&[String])->std::result::Result<Vec<SearchResult>,String>{
        let mut stmt=self.db.prepare("SELECT query_key,target FROM marks").map_err(|e|e.to_string())?;
        let marks:Vec<(String,String)>=stmt.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        let want:HashSet<&str>=tokens.iter().map(String::as_str).collect();
        let mut out=Vec::new(); let mut seen=HashSet::new();
        for (key,target) in marks {
            let overlap=key_overlap(&key,tokens);
            // half the key present is enough: people ask with spare words, not with the
            // exact phrase they once recorded
            if overlap<0.5 { continue }
            let score=if overlap>=1.0 {1.0} else {0.75+0.15*overlap};
            for mut hit in self.resolve_target(&target)? { hit.score=score; hit.marked=true; if seen.insert(format!("{}:{}",hit.file,hit.start_line)) { out.push(hit); } }
        }
        Ok(out)
    }

    /// A mark target is a qualified name, a file path, or `path:line`.
    fn resolve_target(&self,target:&str)->std::result::Result<Vec<SearchResult>,String>{
        let row=|r:&rusqlite::Row<'_>|Ok(SearchResult{symbol:r.get(0)?,qualified_name:r.get(1)?,kind:r.get(2)?,file:r.get(3)?,start_line:r.get::<_,i64>(4)? as usize,end_line:r.get::<_,i64>(5)? as usize,score:1.0,marked:true,body_lines:0,oversized:false,anchor:None,notes:Vec::new(),window:[0,0]});
        let select="SELECT s.name,s.qualified_name,s.kind,f.path,s.start_line,s.end_line FROM symbols s JOIN files f ON f.id=s.file_id";
        let line=target.split_once(':').and_then(|(p,l)|l.parse::<i64>().ok().map(|n|(p.to_string(),n)));
        let (sql,args):(String,Vec<String>)=match &line {
            // narrowest symbol whose range contains the marked line
            Some((path,line))=>(format!("{select} WHERE f.path=?1 AND s.start_line<=?2 AND s.end_line>=?2 ORDER BY s.end_line-s.start_line LIMIT 1"),vec![path.clone(),line.to_string()]),
            None=>(format!("{select} WHERE s.qualified_name=?1 OR (f.path=?1 AND s.kind='module') ORDER BY s.kind='module',s.start_line LIMIT 5"),vec![target.to_string()]),
        };
        let mut st=self.db.prepare(&sql).map_err(|e|e.to_string())?;
        let mut hits:Vec<SearchResult>=st.query_map(rusqlite::params_from_iter(args.iter()),row).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        // an explicit line is the answer; the containing symbol is only context
        if let Some((_,l))=line { for h in &mut hits { h.start_line=l as usize; h.end_line=h.end_line.max(l as usize); } }
        Ok(hits)
    }
    pub fn get(&self,q:&str)->std::result::Result<SymbolSource,String>{ let (id,_)=self.find(q)?; let (n,qn,p,sl,el,sig,desc,input,process,output):(String,String,String,i64,i64,Option<String>,Option<String>,Option<String>,Option<String>,Option<String>)=self.db.query_row("SELECT s.name,s.qualified_name,f.path,s.start_line,s.end_line,s.signature,s.description,s.input_summary,s.process_summary,s.output_summary FROM symbols s JOIN files f ON f.id=s.file_id WHERE s.id=?1",params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?))).map_err(|e|e.to_string())?; let all=fs::read_to_string(self.root.join(&p)).map_err(|e|e.to_string())?; let src=all.lines().skip((sl-1).max(0) as usize).take((el-sl+1).max(1) as usize).collect::<Vec<_>>().join("\n"); let full=all.len()/4;let got=src.len()/4;Ok(SymbolSource{symbol:n,qualified_name:qn,file:p,start_line:sl as usize,end_line:el as usize,signature:sig,description:desc,input_summary:input,process_summary:process,output_summary:output,source:src,source_file_tokens_approx:full,retrieved_tokens_approx:got,tokens_saved_approx:full.saturating_sub(got),reduction_ratio:if full==0{0.0}else{1.0-got as f64/full as f64}}) }
    pub fn impact(&self,q:&str)->std::result::Result<ImpactGraph,String>{let(id,qn)=self.find(q)?;let names=|sql:&str|->std::result::Result<Vec<String>,String>{let mut s=self.db.prepare(sql).map_err(|e|e.to_string())?;let rows=s.query_map(params![id],|r|r.get(0)).map_err(|e|e.to_string())?;rows.map(|x|x.map_err(|e|e.to_string())).collect()};Ok(ImpactGraph{symbol:qn,callers:names("SELECT s.qualified_name FROM references_idx r JOIN symbols s ON s.id=r.source_symbol_id JOIN files f ON f.id=s.file_id WHERE r.target_symbol_id=?1 AND r.kind IN ('call','reference') AND f.path NOT LIKE '%test%' AND f.path NOT LIKE '%spec%' ORDER BY 1")?,callees:names("SELECT s.qualified_name FROM references_idx r JOIN symbols s ON s.id=r.target_symbol_id WHERE r.source_symbol_id=?1 AND r.kind IN ('call','reference','inheritance') ORDER BY 1")?,imports:names("SELECT coalesce(t.qualified_name,e.target_name) FROM relation_edges e LEFT JOIN symbols s ON s.qualified_name=e.source_qn LEFT JOIN symbols t ON t.name=e.target_name WHERE s.id=?1 AND e.kind='import' ORDER BY 1")?,implementations:names("SELECT s.qualified_name FROM references_idx r JOIN symbols s ON s.id=r.source_symbol_id WHERE r.target_symbol_id=?1 AND r.kind IN ('implementation','inheritance') ORDER BY 1")?,tests:names("SELECT s.qualified_name FROM references_idx r JOIN symbols s ON s.id=r.source_symbol_id JOIN files f ON f.id=s.file_id WHERE r.target_symbol_id=?1 AND (f.path LIKE '%test%' OR f.path LIKE '%spec%') ORDER BY 1")?})}
    /// Call chains a symbol takes part in: a bounded walk up to the entry points that
    /// reach it, plus what it calls next. Deterministic — the same answer the
    /// `fdml-flows` pass gives, computed straight from the call edges already indexed.
    /// Call edges contributed by an external engine. Our own graph resolves names
    /// syntactically; an engine that understands function pointers or indirect calls
    /// can add hops we cannot see, so flows are the union of both — with provenance.
    fn provider_call_edges(&self)->std::result::Result<Vec<(String,String,String)>,String>{
        let mut st=self.db.prepare("SELECT provider,payload FROM facts WHERE fact_kind='calls'").map_err(|e|e.to_string())?;
        let rows:Vec<(String,String)>=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        let mut out=Vec::new();
        for (provider,payload) in rows {
            let value:serde_json::Value=match serde_json::from_str(&payload) { Ok(v)=>v, Err(_)=>continue };
            let items=match value { serde_json::Value::Array(a)=>a, other=>vec![other] };
            for item in items {
                if let (Some(from),Some(to))=(item.get("caller").and_then(|v|v.as_str()),item.get("callee").and_then(|v|v.as_str())) {
                    out.push((from.to_string(),to.to_string(),provider.clone()));
                }
            }
        }
        Ok(out)
    }

    pub fn flows(&self,q:&str)->std::result::Result<Vec<Flow>,String>{
        let (id,qn)=self.find(q)?;
        let external=self.provider_call_edges()?;
        let short=|name:&str|name.rsplit('.').next().unwrap_or(name).to_string();
        // ordered by first call site, not alphabetically: the sequence is the answer
        let next:Vec<String>=self.linked(id,"SELECT s.id,s.qualified_name FROM references_idx r JOIN symbols s ON s.id=r.target_symbol_id WHERE r.source_symbol_id=?1 AND r.kind='call' AND s.kind!='module' AND s.id!=?1 GROUP BY s.id,s.qualified_name ORDER BY MIN(r.line),s.qualified_name LIMIT 6")?.into_iter().map(|(_,n)|n).collect();
        // providers that widened the *callee* list apply to every chain of this symbol
        let mut next=next; let mut next_providers:Vec<String>=Vec::new();
        for (from,to,provider) in &external {
            if short(from)==short(&qn) && !next.iter().any(|n|short(n)==short(to)) {
                next.push(to.clone());
                if !next_providers.contains(provider) { next_providers.push(provider.clone()); }
            }
        }
        let mut out=Vec::new(); let mut entries=HashSet::new(); let mut visited=HashSet::new(); visited.insert(id);
        // provenance travels with its own chain: a flow claims a provider only when that
        // provider actually supplied one of its hops
        let mut queue=std::collections::VecDeque::from([(id,vec![qn],next_providers.clone())]);
        while let Some((node,chain,used))=queue.pop_front() {
            if out.len()>=FLOW_MAX_CHAINS { break }
            let mut callers=if chain.len()>FLOW_MAX_DEPTH { Vec::new() } else { self.linked(node,"SELECT DISTINCT s.id,s.qualified_name FROM references_idx r JOIN symbols s ON s.id=r.source_symbol_id WHERE r.target_symbol_id=?1 AND r.kind IN ('call','reference') AND s.kind!='module' AND s.id!=?1 ORDER BY 2")?};
            let mut from_provider:std::collections::HashMap<String,String>=std::collections::HashMap::new();
            if chain.len()<=FLOW_MAX_DEPTH {
                let here=short(&chain[0]);
                for (from,to,provider) in &external {
                    if short(to)==here && !callers.iter().any(|(_,n)|short(n)==short(from)) {
                        // resolve the engine's name against our symbols; skip what we cannot place
                        if let Ok((cid,cqn))=self.find(&short(from)) { from_provider.insert(cqn_key(&cqn),provider.clone()); callers.push((cid,cqn)); }
                    }
                }
            }
            let mut extended=false;
            for (cid,cqn) in callers {
                if visited.insert(cid) {
                    let mut up=chain.clone(); let mut carried=used.clone();
                    if let Some(p)=from_provider.get(&cqn_key(&cqn)) { if !carried.contains(p) { carried.push(p.clone()); } }
                    up.insert(0,cqn); queue.push_back((cid,up,carried)); extended=true;
                }
            }
            // nothing left to climb: an entry point, the depth cap, or a cycle we already walked
            if !extended { let entry=chain[0].clone(); if entries.insert(entry.clone()) { out.push(Flow{entry,depth:chain.len()-1,chain,next:next.clone(),augmented_by:used}); } }
        }
        Ok(out)
    }
    /// Import external analyser output. The contract is a file, not a plugin API: any
    /// engine (Frama-C Eva, Joern, a language-specific tool) writes the unified schema
    /// and we merge it. Nothing about the engine's runtime enters this binary, so the
    /// deterministic core stays local-first and swapping engines costs one adapter.
    ///
    /// Facts are stored by symbol *name*, so they survive re-indexing, and every fact
    /// keeps its `confidence` — a static over-approximation must never be read back as
    /// a fact about an actual run.
    pub fn import_facts(&self,doc:&serde_json::Value,provider:&str)->std::result::Result<(usize,usize),String>{
        let provider=doc.get("provider").and_then(|p|p.as_str()).unwrap_or(provider).to_string();
        let default_conf=doc.get("confidence").and_then(|c|c.as_str()).unwrap_or("static-overapproximation");
        let (mut kept,mut skipped)=(0usize,0usize);
        let obj=doc.as_object().ok_or("facts document must be a JSON object")?;
        for (fact_kind,value) in obj {
            if matches!(fact_kind.as_str(),"provider"|"confidence"|"schema"|"version") { continue }
            match value {
                // map form: { "fields": { "State.flags": {...} } } — the key is the target
                serde_json::Value::Object(entries)=>{
                    for (target,payload) in entries {
                        let conf=payload.get("kind").or_else(||payload.get("confidence")).and_then(|c|c.as_str()).unwrap_or(default_conf);
                        self.put_fact(&provider,target,fact_kind,payload,conf)?; kept+=1;
                    }
                }
                // list form: [ { "caller": "main", "callee": "init" } ] — owner names the target
                serde_json::Value::Array(items)=>{
                    for item in items {
                        match ["target","caller","function","from","symbol"].iter().find_map(|k|item.get(*k).and_then(|v|v.as_str())) {
                            Some(target)=>{ let conf=item.get("kind").and_then(|c|c.as_str()).unwrap_or(default_conf); self.put_fact(&provider,target,fact_kind,item,conf)?; kept+=1; }
                            None=>skipped+=1,
                        }
                    }
                }
                _=>skipped+=1,
            }
        }
        Ok((kept,skipped))
    }
    fn put_fact(&self,provider:&str,target:&str,fact_kind:&str,payload:&serde_json::Value,confidence:&str)->std::result::Result<(),String>{
        // one row per (provider, target, kind): a re-run replaces, never duplicates
        let merged=match self.db.query_row("SELECT payload FROM facts WHERE provider=?1 AND target=?2 AND fact_kind=?3",params![provider,target,fact_kind],|r|r.get::<_,String>(0)).optional().map_err(|e|e.to_string())? {
            Some(prior)=>{
                let mut list=match serde_json::from_str::<serde_json::Value>(&prior) { Ok(serde_json::Value::Array(a))=>a, Ok(v)=>vec![v], Err(_)=>Vec::new() };
                if !list.contains(payload) { list.push(payload.clone()); }
                serde_json::Value::Array(list)
            }
            None=>payload.clone(),
        };
        self.db.execute("INSERT INTO facts(provider,target,fact_kind,payload,confidence) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(provider,target,fact_kind) DO UPDATE SET payload=excluded.payload,confidence=excluded.confidence",params![provider,target,fact_kind,merged.to_string(),confidence]).map_err(|e|e.to_string())?;
        Ok(())
    }
    /// Facts attached to a symbol, whichever engine produced them.
    pub fn facts_for(&self,target:&str)->std::result::Result<Vec<SymbolFact>,String>{
        // the symbol itself, the symbol as a member of something, and facts about what
        // lives inside it (`collide_walls.g_cw_last` answers "what does Eva know here?")
        let mut st=self.db.prepare("SELECT provider,target,fact_kind,payload,confidence FROM facts WHERE target=?1 OR target LIKE '%.'||?1 OR target LIKE ?1||'.%' ORDER BY provider,fact_kind,target").map_err(|e|e.to_string())?;
        let rows:rusqlite::Result<Vec<SymbolFact>>=st.query_map(params![target],|r|Ok(SymbolFact{provider:r.get(0)?,target:r.get(1)?,fact_kind:r.get(2)?,payload:serde_json::from_str(&r.get::<_,String>(3)?).unwrap_or(serde_json::Value::Null),confidence:r.get(4)?})).map_err(|e|e.to_string())?.collect();
        rows.map_err(|e|e.to_string())
    }

    /// Segment a body into phases so a caller can fetch one, not all of it. Phases are
    /// runs of project-internal call sites (external/libc noise excluded) split on a
    /// line gap, each labelled with the nearest comment above it. Deterministic.
    pub fn outline(&self,q:&str)->std::result::Result<Outline,String>{
        let (id,qn)=self.find(q)?;
        let (path,start,end):(String,i64,i64)=self.db.query_row("SELECT f.path,s.start_line,s.end_line FROM symbols s JOIN files f ON f.id=s.file_id WHERE s.id=?1",params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
        let mut st=self.db.prepare("SELECT r.line,t.qualified_name FROM references_idx r JOIN symbols t ON t.id=r.target_symbol_id WHERE r.source_symbol_id=?1 AND r.kind='call' AND t.kind!='module' AND t.id!=?1 ORDER BY r.line").map_err(|e|e.to_string())?;
        let sites:Vec<(i64,String)>=st.query_map(params![id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        let lines:Vec<String>=fs::read_to_string(self.root.join(&path)).map(|t|t.lines().map(str::to_string).collect()).unwrap_or_default();
        let mut phases:Vec<Phase>=Vec::new(); let mut last=i64::MIN;
        for (line,target) in &sites {
            if line.saturating_sub(last)>PHASE_GAP_LINES { phases.push(Phase{line:*line as usize,end_line:*line as usize,label:comment_above(&lines,*line as usize),calls:Vec::new()}); }
            let phase=phases.last_mut().expect("a phase is pushed before use");
            let short=target.rsplit('.').next().unwrap_or(target).to_string();
            if !phase.calls.contains(&short) { phase.calls.push(short); }
            phase.end_line=*line as usize; last=*line;
        }
        // each phase is a retrievable segment: it runs until the next one begins
        for i in 0..phases.len() { let stop=phases.get(i+1).map(|n|n.line-1).unwrap_or(end as usize); phases[i].end_line=phases[i].end_line.max(stop.min(end as usize)); }
        Ok(Outline{symbol:qn,file:path,start_line:start as usize,end_line:end as usize,body_lines:(end-start+1).max(0) as usize,call_sites:sites.len(),phases})
    }
    fn linked(&self,id:i64,sql:&str)->std::result::Result<Vec<(i64,String)>,String>{
        let mut st=self.db.prepare(sql).map_err(|e|e.to_string())?;
        let rows:rusqlite::Result<Vec<(i64,String)>>=st.query_map(params![id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.collect();
        rows.map_err(|e|e.to_string())
    }
    /// Deterministic grep over indexed files: the auxiliary data the LLM fallback
    /// reasons over. Every line is scored by how many query tokens it carries and
    /// annotated with its enclosing symbol, so the model picks from real places.
    pub fn grep_evidence(&self,tokens:&[String])->std::result::Result<Vec<Evidence>,String>{
        let mut st=self.db.prepare("SELECT id,path FROM files").map_err(|e|e.to_string())?;
        let files:Vec<(i64,String)>=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
        let stems:Vec<String>=tokens.iter().map(|t|if t.chars().count()>=6 { t.chars().take(STEM_LEN).collect() } else { t.clone() }).collect();
        let mut scored:Vec<(i64,Evidence)>=Vec::new();
        for (file_id,path) in files {
            let Ok(body)=fs::read_to_string(self.root.join(&path)) else { continue };
            if body.len()>512*1024 { continue }
            // score every matching line first, THEN keep the best per file — taking the
            // first N biased all evidence toward the top of each file and starved the
            // fallback of anything deep inside a large one
            let mut file_hits:Vec<(i64,usize,String)>=Vec::new();
            for (idx,line) in body.lines().enumerate() {
                let low=line.to_lowercase();
                let words:HashSet<&str>=low.split(|c:char|!c.is_alphanumeric()&&c!='_').filter(|w|!w.is_empty()).collect();
                let mut score=0i64;
                for (t,st_) in tokens.iter().zip(&stems) {
                    if words.contains(t.as_str()) { score+=3 } else if low.contains(t.as_str()) { score+=2 } else if low.contains(st_.as_str()) { score+=1 }
                }
                if score>0 { file_hits.push((score,idx+1,line.trim().chars().take(160).collect())); }
            }
            file_hits.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            for (score,lineno,snippet) in file_hits.into_iter().take(3) {
                let symbol:Option<String>=self.db.query_row("SELECT qualified_name FROM symbols WHERE file_id=?1 AND start_line<=?2 AND end_line>=?2 AND kind!='module' ORDER BY end_line-start_line LIMIT 1",params![file_id,lineno as i64],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
                scored.push((score,Evidence{file:path.clone(),line:lineno,text:snippet,symbol}));
            }
        }
        scored.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.file.cmp(&b.1.file)));
        Ok(scored.into_iter().take(EVIDENCE_CAP).map(|(_,e)|e).collect())
    }

    /// Experimental: when deterministic search honestly failed, let a small local
    /// model choose among grep evidence. It picks NUMBERED evidence entries — it
    /// cannot invent a path — and every result is labelled with its provenance.
    pub fn llm_fallback(&self,query:&str,host:&str,model:&str)->std::result::Result<Vec<SearchResult>,String>{
        let tokens:Vec<String>=query.to_lowercase().split(|c:char|!c.is_alphanumeric()&&c!='_').filter(|t|!t.is_empty()).map(str::to_string).collect();
        if tokens.is_empty() { return Ok(vec![]) }
        let evidence=self.grep_evidence(&tokens)?;
        if evidence.is_empty() { return Ok(vec![]) }
        let listing=evidence.iter().enumerate().map(|(i,e)|format!("{}. {}:{} {} | {}",i+1,e.file,e.line,e.symbol.as_deref().map(|s|format!("[{s}]")).unwrap_or_default(),e.text)).collect::<Vec<_>>().join("\n");
        let prompt=format!("An agent searched a codebase for: \"{query}\". Symbol search found nothing useful. Below are grep evidence lines. Pick at most 3 evidence NUMBERS that most likely answer the query; if none fit, useful=false and empty picks. Return only JSON.\n{listing}");
        let schema=json!({"type":"object","properties":{"useful":{"type":"boolean"},"picks":{"type":"array","items":{"type":"object","properties":{"evidence":{"type":"integer"},"reason":{"type":"string"}},"required":["evidence","reason"]}}},"required":["useful","picks"]});
        let url=format!("{}/api/generate",host.trim_end_matches('/'));
        let client=reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(60)).build().map_err(|e|e.to_string())?;
        let response=client.post(url).json(&json!({"model":model,"prompt":prompt,"stream":false,"format":schema,"options":{"temperature":0,"num_predict":300}})).send().map_err(|e|format!("Ollama is unavailable: {e}"))?;
        if !response.status().is_success() { return Err(format!("Ollama returned {}",response.status())); }
        let body:serde_json::Value=response.json().map_err(|e|e.to_string())?;
        let raw=body["response"].as_str().ok_or("Ollama response has no text")?;
        let verdict:serde_json::Value=serde_json::from_str(raw).map_err(|e|format!("invalid fallback JSON: {e}"))?;
        if !verdict["useful"].as_bool().unwrap_or(false) { return Ok(vec![]) }
        let mut out=Vec::new();
        for pick in verdict["picks"].as_array().cloned().unwrap_or_default().iter().take(3) {
            let Some(i)=pick["evidence"].as_u64().map(|v|v as usize).filter(|v|*v>=1&&*v<=evidence.len()) else { continue };
            let e=&evidence[i-1];
            let reason:String=pick["reason"].as_str().unwrap_or("").chars().take(90).collect();
            out.push(SearchResult{symbol:e.symbol.clone().unwrap_or_else(||e.file.clone()),qualified_name:e.symbol.clone().unwrap_or_else(||e.file.clone()),kind:"llm-fallback".into(),file:e.file.clone(),start_line:e.line,end_line:e.line,score:0.5,marked:false,body_lines:1,oversized:false,anchor:Some(format!("{model}: {reason}")),notes:Vec::new(),window:[e.line.saturating_sub(READ_WINDOW/2).max(1),e.line+READ_WINDOW/2]});
        }
        Ok(out)
    }

    /// Telemetry of the tool itself: every search records whether it earned its keep.
    /// Failures are the food of the eternal-tooling-improvement loop — each one is a
    /// case showing where THIS project needs the tool bent toward it.
    pub fn log_query(&self,query:&str,top_score:Option<f64>,results:usize,useful:bool,llm_used:bool,llm_rescued:bool)->std::result::Result<(),String>{
        self.log_command("search",query,top_score,results,useful,llm_used,llm_rescued)
    }
    /// Every command reports through here, so `fdml log` shows the whole loop —
    /// what was searched, what was taught (marks, notes), what was read (outline,
    /// flow) — not just the searches.
    #[allow(clippy::too_many_arguments)]
    pub fn log_command(&self,command:&str,query:&str,top_score:Option<f64>,results:usize,useful:bool,llm_used:bool,llm_rescued:bool)->std::result::Result<(),String>{
        self.db.execute("INSERT INTO query_log(command,query,top_score,results,useful,llm_used,llm_rescued) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![command,query,top_score,results as i64,useful as i64,llm_used as i64,llm_rescued as i64]).map_err(|e|e.to_string())?; Ok(())
    }
    /// The accumulated cases: overall hit-rate, recent failures grouped, LLM-fallback
    /// stats, and retry episodes. All passive — no agent feedback required: the verdict
    /// comes from the score threshold, and a burst of failed reformulations in the log
    /// IS the agent telling us it struggled, whether it knows it or not.
    #[allow(clippy::type_complexity)]
    pub fn query_report(&self,limit:usize)->std::result::Result<(i64,i64,i64,i64,Vec<(String,i64,String)>,Vec<Vec<String>>),String>{
        let count=|sql:&str|self.db.query_row(sql,[],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string());
        let total=count("SELECT count(*) FROM query_log WHERE command='search'")?;
        let failed=count("SELECT count(*) FROM query_log WHERE command='search' AND useful=0")?;
        let llm_used=count("SELECT count(*) FROM query_log WHERE llm_used=1")?;
        let llm_rescued=count("SELECT count(*) FROM query_log WHERE llm_rescued=1")?;
        let mut st=self.db.prepare("SELECT query,count(*),max(created_at) FROM query_log WHERE useful=0 AND command='search' GROUP BY query ORDER BY max(id) DESC LIMIT ?1").map_err(|e|e.to_string())?;
        let rows:rusqlite::Result<Vec<(String,i64,String)>>=st.query_map(params![limit as i64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?.collect();
        // retry episodes: consecutive failed queries within 90s of each other
        let mut st=self.db.prepare("SELECT query,useful,strftime('%s',created_at) FROM query_log WHERE command='search' ORDER BY id DESC LIMIT 200").map_err(|e|e.to_string())?;
        let recent:rusqlite::Result<Vec<(String,i64,i64)>>=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get::<_,String>(2)?.parse::<i64>().unwrap_or(0)))).map_err(|e|e.to_string())?.collect();
        let mut recent=recent.map_err(|e|e.to_string())?; recent.reverse();
        let mut episodes:Vec<Vec<String>>=Vec::new(); let mut chain:Vec<String>=Vec::new(); let mut last_ts=0i64;
        for (query,useful,ts) in recent {
            if useful==0 && (chain.is_empty()||ts-last_ts<=90) { if chain.last()!=Some(&query) { chain.push(query); } last_ts=ts; }
            else { if chain.len()>=2 { episodes.push(std::mem::take(&mut chain)); } else { chain.clear(); } if useful==0 { chain.push(query); last_ts=ts; } }
        }
        if chain.len()>=2 { episodes.push(chain); }
        episodes.reverse(); episodes.truncate(limit);
        Ok((total,failed,llm_used,llm_rescued,rows.map_err(|e|e.to_string())?,episodes))
    }

    /// Which commands actually get used — the adoption half of the picture.
    /// Files whose mtime/size no longer match the index. Cheap (a stat per file) and
    /// worth doing on every search: an index that silently predates the code turns a
    /// correct "no useful result" into a lie.
    pub fn stale_files(&self)->usize{
        let mut st=match self.db.prepare("SELECT path,mtime,size FROM files") { Ok(s)=>s, Err(_)=>return 0 };
        let rows=match st.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?))) { Ok(r)=>r, Err(_)=>return 0 };
        rows.filter_map(|r|r.ok()).filter(|(path,mtime,size)|{
            match fs::metadata(self.root.join(path)) {
                Ok(m)=>{
                    let now=m.modified().ok().and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|d|d.as_nanos() as i64).unwrap_or(0);
                    now!=*mtime||m.len() as i64!=*size
                }
                Err(_)=>true,
            }
        }).count()
    }
    pub fn command_usage(&self)->std::result::Result<Vec<(String,i64)>,String>{
        let mut st=self.db.prepare("SELECT command,count(*) FROM query_log GROUP BY command ORDER BY 2 DESC").map_err(|e|e.to_string())?;
        let rows:rusqlite::Result<Vec<(String,i64)>>=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.collect();
        rows.map_err(|e|e.to_string())
    }
    pub fn status(&self)->std::result::Result<IndexStatus,String>{let count=|t|self.db.query_row(&format!("SELECT count(*) FROM {t}"),[],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string());Ok(IndexStatus{root_path:self.root.display().to_string(),files:count("files")? as usize,symbols:count("symbols")? as usize,references:count("references_idx")? as usize,marks:count("marks")? as usize,last_indexed:self.db.query_row("SELECT value FROM metadata WHERE key='last_indexed'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?,index_size:fs::metadata(self.root.join(".fdml/index.sqlite")).map(|m|m.len()).unwrap_or(0)})}
    pub fn mark(&self,model:&str,host:&str,limit:usize,force:bool)->std::result::Result<usize,String>{
        let where_clause=if force { "" } else { "AND (description_hash IS NULL OR description_model != ?1 OR description_version != '2')" };
        let sql=format!("SELECT s.id,s.qualified_name,s.kind,coalesce(s.signature,''),f.path,s.start_line,s.end_line FROM symbols s JOIN files f ON f.id=s.file_id WHERE s.kind != 'module' {where_clause} ORDER BY s.id");
        let mut stmt=self.db.prepare(&sql).map_err(|e|e.to_string())?;
        let max=if limit==0{usize::MAX}else{limit};
        let map_row=|r: &rusqlite::Row<'_>| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?));
        let items:Vec<(i64,String,String,String,String,i64,i64)>=if force { stmt.query_map([],map_row).map_err(|e|e.to_string())?.take(max).collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())? } else { stmt.query_map(params![model],map_row).map_err(|e|e.to_string())?.take(max).collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())? };
        let mut done=0; for (id,qn,kind,sig,path,start,end) in items { let source=fs::read_to_string(self.root.join(&path)).unwrap_or_default(); let snippet=source.lines().skip(start.saturating_sub(1) as usize).take((end-start+1).clamp(1,120) as usize).collect::<Vec<_>>().join("\n"); let hash=content_hash(&format!("{model}\n{snippet}")); let mark=ollama_mark(host,model,&qn,&kind,&sig,&snippet)?; self.db.execute("UPDATE symbols SET description=?1,input_summary=?2,process_summary=?3,output_summary=?4,tags=?5,description_model=?6,description_version='2',description_hash=?7 WHERE id=?8",params![mark.description,mark.input,mark.process,mark.output,mark.tags.join(" "),model,hash,id]).map_err(|e|e.to_string())?; done+=1; }
        Ok(done)
    }
    /// A deliberately small FDML 1.4 companion document. SQLite contains the
    /// complete symbol map; this file is human-readable and FDML-parser-friendly.
    fn write_fdml_map(&self)->std::result::Result<(),String>{
        let project=self.root.file_name().and_then(|n|n.to_str()).unwrap_or("repository");
        let id=fdml_id(project); let mut out=format!("metadata:\n  version: \"1.4\"\n  author: \"FDML indexer\"\n  description: \"Deterministic structural map; detailed navigation is in index.sqlite.\"\nsystem:\n  id: {}\n  name: \"{}\"\n  description: \"Local codebase index\"\n  components:\n",id,yaml(project));
        let mut stmt=self.db.prepare("SELECT DISTINCT qualified_name FROM symbols WHERE kind='module' ORDER BY qualified_name").map_err(|e|e.to_string())?;
        let modules:Vec<String>=stmt.query_map([],|r|r.get(0)).map_err(|e|e.to_string())?.map(|r|r.map_err(|e|e.to_string())).collect::<std::result::Result<_,_>>()?;
        for module in &modules { out.push_str(&format!("    - \"{}\"\n",yaml(module))); }
        out.push_str("  relationships:\n");
        let mut rels=self.db.prepare("SELECT DISTINCT sf.path,tf.path,r.kind FROM references_idx r JOIN files sf ON sf.id=r.file_id LEFT JOIN symbols t ON t.id=r.target_symbol_id LEFT JOIN files tf ON tf.id=t.file_id WHERE tf.path IS NOT NULL ORDER BY 1,2,3 LIMIT 500").map_err(|e|e.to_string())?;
        let rows=rels.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(|e|e.to_string())?;
        let mut any=false; for row in rows { let (from,to,kind)=row.map_err(|e|e.to_string())?; any=true; out.push_str(&format!("    - from: \"{}\"\n      to: \"{}\"\n      type: \"{}\"\n",yaml(&from),yaml(&to),yaml(&kind))); }
        if !any { out.push_str("    []\n"); }
        fs::write(self.root.join(".fdml/index.fdml"),out).map_err(|e|e.to_string())
    }
}
fn parse(lang:Language,text:&str,path:&str)->std::result::Result<FileAnalysis,String>{match lang{Language::C|Language::Cpp=>scanner::c::CScanner::parse_file(text,path,lang),Language::Python=>scanner::python::PythonScanner::parse_file(text,path),Language::Java=>scanner::java::JavaScanner::parse_file(text,path),Language::CSharp=>scanner::csharp::CSharpScanner::parse_file(text,path),Language::JavaScript=>scanner::javascript::JavaScriptScanner::parse_file(text,path),Language::TypeScript=>scanner::typescript::TypeScriptScanner::parse_file(text,path),Language::Go=>scanner::go_lang::GoScanner::parse_file(text,path),Language::Rust=>scanner::rust_lang::RustScanner::parse_file(text,path)}.map_err(|e|e.to_string())}
fn language(p:&Path)->Option<Language>{p.extension().and_then(|x|x.to_str()).and_then(Language::from_extension)}
fn kind(e:&ElementType)->&'static str{match e{ElementType::Class=>"class",ElementType::Function=>"function",ElementType::Method=>"method",ElementType::Interface=>"interface",ElementType::Enum=>"enum",ElementType::Field=>"variable",ElementType::Property=>"variable",ElementType::Macro=>"macro",ElementType::TypeAlias=>"typedef",ElementType::Module=>"module"}}
fn module_name(path:&str,lang:Language)->String{let p=path.rsplit_once('.').map(|x|x.0).unwrap_or(path).replace(['/', '\\'],".");match lang{Language::Python=>p.trim_end_matches(".__init__").to_string(),_=>p.trim_start_matches("src.").trim_end_matches(".index").to_string()}}
fn rel(root:&Path,p:&Path)->String{p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\',"/")}
/// The nearest comment above a line — a free, human-written phase label. Big functions
/// in real code are commented at exactly the boundaries we segment on.
fn comment_above(lines:&[String],line:usize)->Option<String>{
    let start=line.saturating_sub(1);
    for i in (start.saturating_sub(12)..start).rev() {
        let t=lines.get(i)?.trim();
        let body=t.trim_start_matches(['/','*','#',' ','\t']).trim_end_matches(['*','/']).trim();
        if (t.starts_with("/*")||t.starts_with("//")||t.starts_with('*')||t.starts_with('#')) && body.chars().filter(|c|c.is_alphabetic()).count()>=8 {
            return Some(body.chars().take(90).collect());
        }
    }
    None
}
/// Normalized mark lookup key: lowercase, de-duplicated, order-insensitive tokens,
/// so "world mesh" and "Mesh, world" remember the same thing.
/// An identifier split by the naming conventions it was written with —
/// `collide_walls`, `GpuMesh` and `VH_DAMP_ANG` all become space-separated words,
/// padded so a whole-word match is a plain LIKE. Deterministic; no model involved.
fn name_tokens(name:&str)->String{
    let mut parts:Vec<String>=Vec::new(); let mut current=String::new();
    let chars:Vec<char>=name.chars().collect();
    for (i,c) in chars.iter().enumerate() {
        if *c=='_' || *c=='-' || *c=='.' { if !current.is_empty(){parts.push(std::mem::take(&mut current))} continue }
        // camelCase / PascalCase boundary, and the ACRONYMWord boundary
        let boundary = i>0 && (
            (c.is_uppercase() && chars[i-1].is_lowercase()) ||
            (c.is_lowercase() && i>=2 && chars[i-1].is_uppercase() && chars[i-2].is_uppercase()));
        if boundary && !current.is_empty() {
            if c.is_lowercase() { let last=current.pop().unwrap(); parts.push(std::mem::take(&mut current)); current.push(last); }
            else { parts.push(std::mem::take(&mut current)); }
        }
        current.push(c.to_ascii_lowercase());
    }
    if !current.is_empty(){parts.push(current)}
    parts.retain(|p|!p.is_empty());
    format!(" {} ",parts.join(" "))
}
/// `"world2: %ld instances"` and the runtime line `world2: 1234 instances` both reduce
/// to `world2: {} instances`, so one can find the other. Deliberately not a printf
/// grammar: every conversion becomes one wildcard token.
fn normalize_format(value:&str)->String{
    let mut out=String::new(); let chars:Vec<char>=value.chars().collect(); let mut i=0;
    while i<chars.len() {
        if chars[i]=='%' {
            if chars.get(i+1)==Some(&'%') { out.push('%'); i+=2; continue }
            let mut j=i+1;
            while j<chars.len() && !chars[j].is_ascii_alphabetic() { j+=1 }
            while j<chars.len() && matches!(chars[j],'l'|'h'|'z'|'j'|'t'|'L') { j+=1 }
            if j<chars.len() { out.push_str("{}"); i=j+1; continue }
        }
        out.push(chars[i]); i+=1;
    }
    out
}
/// The contiguous comment block directly above a declaration, any style
/// (`//`, `/* */`, `*`, `#`), joined and capped. Language-agnostic on purpose:
/// one implementation covers every scanner.
fn doc_above(lines:&[&str],decl_line:usize)->Option<String>{
    let mut collected:Vec<&str>=Vec::new();
    let mut i=decl_line.saturating_sub(1);
    while i>0 {
        let t=lines.get(i-1)?.trim();
        let is_comment=t.starts_with("//")||t.starts_with("/*")||t.starts_with('*')||t.starts_with('#')||t.ends_with("*/");
        if !is_comment { break }
        collected.push(t);
        if collected.len()>=12 { break }
        i-=1;
    }
    if collected.is_empty() { return None }
    collected.reverse();
    let joined=collected.iter().map(|t|t.trim_start_matches(['/','*','#',' ','\t']).trim_end_matches("*/").trim()).filter(|t|!t.is_empty()).collect::<Vec<_>>().join(" ");
    if joined.chars().filter(|c|c.is_alphabetic()).count()<8 { return None }
    Some(joined.chars().take(400).collect())
}
fn cqn_key(q:&str)->String{ q.rsplit('.').next().unwrap_or(q).to_string() }
/// How well a query overlaps a stored key. Strict subset matching broke the moment
/// a question word was added, so this is proportional: shared tokens over the
/// smaller side, with connectives ignored.
fn key_overlap(key:&str,tokens:&[String])->f64{
    let have:HashSet<&str>=key.split(' ').filter(|t|!t.is_empty()).collect();
    let want:HashSet<&str>=tokens.iter().map(String::as_str).filter(|t|!STOPWORDS.contains(t)).collect();
    if have.is_empty()||want.is_empty() { return 0.0 }
    let shared=have.intersection(&want).count();
    if shared==0 { return 0.0 }
    if have==want { return 1.0 }
    shared as f64/have.len().min(want.len()) as f64
}
fn mark_key(q:&str)->String{let mut t:Vec<String>=q.to_lowercase().split(|c:char|!c.is_alphanumeric()&&c!='_').filter(|s|!s.is_empty()).map(str::to_string).collect();t.sort();t.dedup();t.join(" ")}
fn content_hash(s:&str)->String{let mut h=std::collections::hash_map::DefaultHasher::new();s.hash(&mut h);format!("{:x}",h.finish())}
fn yaml(s:&str)->String{s.replace('\\',"\\\\").replace('"',"\\\"")}
fn fdml_id(s:&str)->String{let id:String=s.chars().map(|c|if c.is_ascii_alphanumeric(){c.to_ascii_lowercase()}else{'_'}).collect();if id.is_empty(){"repository".into()}else{id}}
fn call_names(line:&str)->Vec<String>{
    let mut out=Vec::new(); let bytes=line.as_bytes();
    for i in 0..bytes.len() { if bytes[i]==b'(' { let mut j=i; while j>0 && (bytes[j-1].is_ascii_alphanumeric() || bytes[j-1]==b'_') {j-=1;} if j<i { let name=&line[j..i]; if !matches!(name,"if"|"for"|"while"|"switch"|"catch"|"function"|"def") {out.push(name.to_string());} } } }
    out
}
fn source_files(root:&Path)->std::result::Result<Vec<PathBuf>,String>{fn walk(d:&Path,out:&mut Vec<PathBuf>)->std::result::Result<(),String>{for e in fs::read_dir(d).map_err(|e|e.to_string())?{let p=e.map_err(|e|e.to_string())?.path();if p.is_dir(){if !SKIP_DIRS.contains(&p.file_name().and_then(|x|x.to_str()).unwrap_or("")){walk(&p,out)?}}else if language(&p).is_some(){out.push(p)}}Ok(())}let mut v=vec![];walk(root,&mut v)?;v.sort();Ok(v)}

#[derive(Deserialize)]
struct LocalMark { description: String, input: String, process: String, output: String, #[serde(default)] tags: Vec<String> }

fn ollama_mark(host:&str, model:&str, qualified_name:&str, kind:&str, signature:&str, source:&str)->std::result::Result<LocalMark,String>{
    let prompt=format!("You label source symbols for local code search. Inspect the complete bounded source range below. Return only JSON. Be factual; do not invent behavior. description max 20 words; input/process/output max 18 words each; 3-8 lowercase tags.\nSymbol: {qualified_name}\nKind: {kind}\nSignature: {signature}\nSource range:\n{source}");
    let schema=json!({"type":"object","properties":{"description":{"type":"string"},"input":{"type":"string"},"process":{"type":"string"},"output":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}}},"required":["description","input","process","output","tags"]});
    let url=format!("{}/api/generate",host.trim_end_matches('/'));
    let client=reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(180)).build().map_err(|e|e.to_string())?;
    let response=client.post(url).json(&json!({"model":model,"prompt":prompt,"stream":false,"format":schema,"options":{"temperature":0}})).send().map_err(|e|format!("Ollama is unavailable: {e}. Start `ollama serve` and ensure `{model}` is installed."))?;
    if !response.status().is_success() { return Err(format!("Ollama returned {}: {}",response.status(),response.text().unwrap_or_default())); }
    let body:serde_json::Value=response.json().map_err(|e|e.to_string())?;
    let raw=body["response"].as_str().ok_or("Ollama response has no text")?;
    serde_json::from_str(raw).map_err(|e|format!("Ollama returned invalid marker JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn indexes_incrementally_searches_gets_and_finds_call_impact() {
        let repo = TempDir::new().unwrap();
        fs::write(repo.path().join("a.ts"), "function a() {\n  b();\n}\n").unwrap();
        fs::write(repo.path().join("b.ts"), "function b() {\n  c();\n}\n").unwrap();
        fs::write(repo.path().join("c.ts"), "function c() {\n  return 1;\n}\n").unwrap();
        let first = Indexer::index(repo.path()).unwrap();
        assert_eq!(first.parsed_files, 3);
        assert!(repo.path().join(".fdml/index.fdml").exists());
        let index = RepositoryIndex::open(repo.path()).unwrap();
        assert_eq!(index.search("b").unwrap()[0].symbol, "b");
        assert!(index.get("b").unwrap().source.contains("c();"));
        let impact = index.impact("b").unwrap();
        assert!(impact.callers.iter().any(|n| n.ends_with(".a")));
        assert!(impact.callees.iter().any(|n| n.ends_with(".c")));

        fs::write(repo.path().join("b.ts"), "function renamed() {\n  c();\n}\n").unwrap();
        let second = Indexer::index(repo.path()).unwrap();
        assert_eq!(second.parsed_files, 1);
        assert_eq!(second.unchanged_files, 2);
        let index = RepositoryIndex::open(repo.path()).unwrap();
        assert!(index.search("renamed").unwrap().iter().any(|s| s.symbol == "renamed"));
        assert!(!index.search("b").unwrap().iter().any(|s| s.symbol == "b" && s.kind != "module"));
    }

    #[test]
    fn marks_rank_first_survive_reindex_and_leave_plain_search_intact() {
        let repo = TempDir::new().unwrap();
        fs::write(repo.path().join("draw.ts"), "function uploadBatches() {\n  return 1;\n}\n").unwrap();
        fs::write(repo.path().join("other.ts"), "function unrelated() {\n  return 2;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();

        // the vocabulary gap: a natural-language query nothing in the code spells out
        assert!(index.search("world mesh").unwrap().is_empty());

        let target = index.mark_association("world mesh", "uploadBatches").unwrap();
        assert!(target.ends_with("uploadBatches"));
        let hits = index.search("world mesh").unwrap();
        assert!(hits[0].marked && hits[0].symbol == "uploadBatches" && hits[0].score == 1.0);
        // order-insensitive wording, and a wider query still reaches the mark
        assert_eq!(index.search("Mesh, world").unwrap()[0].symbol, "uploadBatches");
        assert!(index.search("world mesh upload").unwrap().iter().any(|r| r.marked));

        // marks outrank plain matches but never hide them
        index.mark_association("scenery", "unrelated").unwrap();
        let mixed = index.search("scenery unrelated").unwrap();
        assert!(mixed[0].marked);
        assert!(index.search("uploadBatches").unwrap().iter().any(|r| !r.marked && r.symbol == "uploadBatches"));

        // marks key on names, not row ids, so re-indexing keeps them
        fs::write(repo.path().join("other.ts"), "function unrelated() {\n  return 3;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();
        assert!(index.search("world mesh").unwrap()[0].marked);
        assert_eq!(index.status().unwrap().marks, 2);
    }

    #[test]
    fn flows_reach_entry_points_and_terminate_on_cycles() {
        let repo = TempDir::new().unwrap();
        fs::write(repo.path().join("app.ts"), "function main() {\n  handle();\n}\n").unwrap();
        fs::write(repo.path().join("mid.ts"), "function handle() {\n  target();\n}\n").unwrap();
        fs::write(repo.path().join("leaf.ts"), "function target() {\n  sink();\n}\nfunction sink() {\n  return 1;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();

        let flows = index.flows("target").unwrap();
        assert_eq!(flows.len(), 1);
        let chain = flows[0].chain.iter().map(|q| q.rsplit('.').next().unwrap()).collect::<Vec<_>>();
        assert_eq!(chain, vec!["main", "handle", "target"]);
        assert_eq!(flows[0].depth, 2);
        assert!(flows[0].next.iter().any(|n| n.ends_with(".sink")));

        // an engine can contribute a hop our syntactic graph cannot see (a call through a
        // function pointer); the flow is the union, and says which provider supplied it
        fs::write(repo.path().join("indirect.ts"), "function dispatch() {\n  return 0;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();
        assert!(index.flows("target").unwrap()[0].augmented_by.is_empty());
        index.import_facts(&serde_json::json!({ "calls": [ { "caller": "dispatch", "callee": "target" } ] }), "joern").unwrap();
        let augmented = index.flows("target").unwrap();
        assert!(augmented.iter().any(|f| f.entry.ends_with(".dispatch")), "provider hop must appear: {augmented:?}");
        assert!(augmented.iter().any(|f| f.augmented_by == vec!["joern".to_string()]));

        // a cycle must still terminate, bounded by the visited set and the depth cap
        fs::write(repo.path().join("mid.ts"), "function handle() {\n  target();\n}\nfunction loop_a() {\n  loop_b();\n}\nfunction loop_b() {\n  loop_a();\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();
        assert!(!index.flows("loop_a").unwrap().is_empty());
    }

    #[test]
    fn dossier_gathers_a_commit_and_names_its_holes() {
        let repo = TempDir::new().unwrap();
        fs::write(repo.path().join("a.ts"), "function entry() {\n  body_contact();\n}\nfunction body_contact() {\n  return 1;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();

        // an empty dossier must still be useful: it says what is missing
        let empty = index.dossier("deadbee").unwrap();
        assert!(empty.anchors.is_empty());
        assert_eq!(empty.missing.len(), 5, "every unfilled section is reported: {:?}", empty.missing);

        // record a change the way a session actually would
        index.add_note("объект проваливается sinks through ground", "ground contact used 8 body corners", "postmortem", Some("body_contact"), &[]).unwrap();
        index.add_note("vel is derived", "pos is pushed out; impulse via lin_mom; vel untouched", "invariant", Some("body_contact"), &[]).unwrap();
        index.add_note("spring clamp", "clamping spring force made the car float — rejected", "rejected", None, &[]).unwrap();
        index.add_note("regression check", "run the physics regression: same two metrics miss as on HEAD", "method", None, &[]).unwrap();

        // notes carry the commit they were written on; the dossier keys on it
        let commit: String = index.db.query_row("SELECT coalesce(commit_sha,'') FROM notes LIMIT 1", [], |r| r.get(0)).unwrap();
        let d = index.dossier(&commit).unwrap();
        assert!(d.anchors.iter().any(|a| a.ends_with("body_contact")));
        assert!(d.flow.iter().any(|f| f.ends_with("entry")), "flow is computed, not stored: {:?}", d.flow);
        assert_eq!(d.state.len(), 1);
        assert_eq!(d.rejected.len(), 1);
        assert_eq!(d.verify.len(), 1);
        assert_eq!(d.symptoms.len(), 1);
        assert!(d.missing.is_empty(), "a complete card reports no holes: {:?}", d.missing);
    }

    #[test]
    fn doc_comments_above_symbols_are_searchable() {
        let repo = TempDir::new().unwrap();
        // the author already explained the concept — search must see it
        fs::write(repo.path().join("render.ts"),
            "// headlight projection: the beam texture is projected from the car lamps\nfunction hl_apply() {\n  return 1;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();
        let hits = index.search("headlight beam projection").unwrap();
        assert!(hits.iter().any(|r| r.symbol == "hl_apply"), "comment text must resolve the concept: {hits:?}");
        assert!(hits.iter().find(|r| r.symbol == "hl_apply").unwrap().score > 0.5);
    }

    #[test]
    fn notes_record_knowledge_that_has_no_address_and_surface_with_their_symbol() {
        let repo = TempDir::new().unwrap();
        fs::write(repo.path().join("a.ts"), "function integrate() {\n  return 1;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();

        // a repro recipe: pure text, no place in the code at all
        index.add_note("объект дрожит object jitters", "repro: run the sim headless with a fixed seed\ncause: two solvers writing one body", "repro", None, &[]).unwrap();
        let hits = index.search("object jitters").unwrap();
        assert_eq!(hits[0].kind, "note:repro");
        assert!(hits[0].notes[0].body.contains("two solvers"));
        // the other phrasing finds it too — that is what aliases and bilingual keys are for
        assert!(!index.search("объект дрожит").unwrap().is_empty());

        // an invariant anchored to a symbol rides along when that symbol is a hit
        index.add_note("vel derived from momentum", "rb->vel is recomputed each tick; clamping it is dead code", "invariant", Some("integrate"), &["скорость производная".into()]).unwrap();
        let top = index.search("integrate").unwrap().into_iter().find(|r| r.symbol == "integrate").expect("symbol hit");
        assert!(top.notes.iter().any(|n| n.kind == "invariant"), "invariant must surface with its symbol");
        assert!(!top.notes[0].stale, "an unchanged file is not stale");

        // edit the file: the note now warns that it predates the change
        fs::write(repo.path().join("a.ts"), "function integrate() {\n  return 2;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();
        let top = index.search("integrate").unwrap().into_iter().find(|r| r.symbol == "integrate").unwrap();
        assert!(top.notes.iter().any(|n| n.stale), "a changed file must mark its notes stale");
        assert_eq!(index.list_notes(Some("repro"), 10).unwrap().len(), 1);
    }

    #[test]
    fn identifiers_split_by_their_naming_convention() {
        assert_eq!(name_tokens("collide_walls"), " collide walls ");
        assert_eq!(name_tokens("GpuMesh"), " gpu mesh ");
        assert_eq!(name_tokens("VH_DAMP_ANG"), " vh damp ang ");
        assert_eq!(name_tokens("n2_texpack_decode"), " n2 texpack decode ");
        assert_eq!(name_tokens("HTTPServer"), " http server ", "acronym then word");
        assert_eq!(name_tokens("world2_on"), " world2 on ", "digits stay attached: n2, world2");
    }

    #[test]
    fn facts_import_is_engine_agnostic_and_keeps_confidence() {
        let repo = TempDir::new().unwrap();
        fs::write(repo.path().join("a.ts"), "function process() {\n  parse();\n}\nfunction parse() {\n  return 1;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();

        // map form (Frama-C Eva shape) and list form (Joern shape) in one document
        let doc = serde_json::json!({
            "provider": "frama-c-eva",
            "fields": { "State.flags": { "possible_values": "0..3", "writes": ["init"] } },
            "calls": [ { "caller": "process", "callee": "parse" }, { "callee": "orphan" } ]
        });
        let (kept, skipped) = index.import_facts(&doc, "ignored-when-doc-names-provider").unwrap();
        assert_eq!((kept, skipped), (2, 1), "one call entry has no owner to attach to");

        let flags = index.facts_for("State.flags").unwrap();
        assert_eq!(flags[0].provider, "frama-c-eva");
        assert_eq!(flags[0].confidence, "static-overapproximation", "must not read back as a runtime fact");
        assert_eq!(flags[0].payload["possible_values"], "0..3");
        assert_eq!(index.facts_for("process").unwrap()[0].fact_kind, "calls");

        // a second engine attaches to the same symbol without clobbering the first
        let joern = serde_json::json!({ "provider": "joern", "confidence": "cpg", "data_flow": [ { "from": "process", "to": "parse" } ] });
        index.import_facts(&joern, "joern").unwrap();
        let on_process = index.facts_for("process").unwrap();
        assert_eq!(on_process.len(), 2);
        assert!(on_process.iter().any(|f| f.provider == "joern" && f.confidence == "cpg"));

        // re-running an engine replaces its own rows rather than duplicating them
        index.import_facts(&joern, "joern").unwrap();
        assert_eq!(index.facts_for("process").unwrap().len(), 2);
    }

    #[test]
    fn outline_segments_a_large_body_into_labelled_phases() {
        let repo = TempDir::new().unwrap();
        fs::write(repo.path().join("lib.ts"), "function boot() {\n  return 1;\n}\nfunction render() {\n  return 2;\n}\n").unwrap();
        // one body, two clusters of calls separated by a wide gap
        let mut big = String::from("function god() {\n  // start up the world\n  boot();\n");
        big.push_str(&"  const filler = 0;\n".repeat(80));
        big.push_str("  // draw a frame\n  render();\n}\n");
        fs::write(repo.path().join("god.ts"), big).unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();

        let outline = index.outline("god").unwrap();
        assert_eq!(outline.phases.len(), 2, "a wide gap must split phases");
        assert_eq!(outline.phases[0].calls, vec!["boot"]);
        assert_eq!(outline.phases[0].label.as_deref(), Some("start up the world"));
        assert_eq!(outline.phases[1].calls, vec!["render"]);
        assert_eq!(outline.phases[1].label.as_deref(), Some("draw a frame"));
        // phases are retrievable segments: the first ends where the second begins
        assert!(outline.phases[0].end_line < outline.phases[1].line);
        assert!(outline.body_lines > 80);
    }

    #[test]
    fn marks_accept_file_and_line_targets_and_reject_unknown_ones() {
        let repo = TempDir::new().unwrap();
        fs::write(repo.path().join("world.ts"), "function alpha() {\n  return 1;\n}\nfunction beta() {\n  return 2;\n}\n").unwrap();
        Indexer::index(repo.path()).unwrap();
        let index = RepositoryIndex::open(repo.path()).unwrap();

        index.mark_association("streaming entry point", "world.ts:5").unwrap();
        let hit = &index.search("streaming entry point").unwrap()[0];
        assert!(hit.marked && hit.symbol == "beta");

        index.mark_association("whole module", "world.ts").unwrap();
        assert_eq!(index.search("whole module").unwrap()[0].kind, "module");

        assert!(index.mark_association("typo", "does_not_exist").is_err());
        assert!(index.mark_association("   ", "alpha").is_err());
    }
}
