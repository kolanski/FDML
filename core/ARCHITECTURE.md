# FDML Core Rework — Synthesis & Architecture

> Single source of truth for the deterministic-core rework. Synthesizes three studied
> repos into one design, with provenance on every idea.
>
> **Sources** (full studies in this folder):
> - `01-codegraph.md` — [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) — TS, MIT. Local, deterministic, tree-sitter → SQLite graph → heuristic ref-resolution. No ML.
> - `02-codeboarding.md` — [CodeBoarding/CodeBoarding](https://github.com/CodeBoarding/CodeBoarding) — Python, MIT. Deterministic graph clustering → readable high-level map; LLM only names things.
> - `03-understand-anything.md` — [Egonex-AI/Understand-Anything](https://github.com/Egonex-AI/Understand-Anything) — TS+Py, MIT. tree-sitter + import-resolver + Louvain → knowledge-graph.json → React Flow.
>
> Provenance tags used below: `[codegraph]` `[CodeBoarding]` `[U-A]` `[convergent]` (all three independently) `[FDML]` (our own decision).

---

## 0. Why this rework is de-risked

All three tools — built independently — converge on the **same backbone**:

```
tree-sitter extract  →  deterministic import/ref resolution  →  community-detection clustering  →  ML strictly OUTSIDE the structural core
```

That is exactly FDML's intended `scan → graph → flows` pipeline. Three shipping tools prove a
deterministic, no-ML, tree-sitter code-graph is fast and accurate enough for real use. `[convergent]`

**Linking the repos themselves: rejected.** All three are TS/Python. Linking the heavy ones drags in
their runtimes — and CodeBoarding's LSP servers (Pyright/JDTLS/tsserver) are *the* reason it takes
minutes-to-hours, not seconds `[CodeBoarding]`. We take **pieces**: known algorithms via Rust crates,
domain know-how via porting the idea.

---

## 1. Architecture (one binary, isolated passes)

Cargo workspace; passes are crates; only the CLI knows the order. `[FDML]`

```
fdml/
└── crates/
    ├── fdml-types/         # data contracts ONLY (structs between passes). Zero logic.
    ├── fdml-discovery/     # pass 1: (root, patterns) -> Vec<System>
    ├── fdml-scan/          # pass 2: System -> ScanResult         (tree-sitter, parallel)
    ├── fdml-graph/         # pass 3: &[ScanResult] -> CodeGraph    (resolve + cluster)
    ├── fdml-flows/         # pass 4: &CodeGraph -> Vec<Flow>       (perf-bounded)
    ├── fdml-integrations/  # pass 5: &[ScanResult] -> Vec<Integration>
    ├── fdml-assemble/      # pass 6: all of above -> FdmlSpec (YAML)
    ├── fdml-enrich/        # OPTIONAL ML layer, OUTSIDE the hot path. CLI calls it only on demand.
    └── fdml-cli/           # the binary. Wires passes. Single place they meet.
```

**Two rules that ARE the architecture** `[FDML]`:
1. **Dependency fan, not web.** Each pass depends on `fdml-types` and nothing else of ours. No pass
   depends on another pass. Only `fdml-cli` depends on all. Cycles won't compile → can't intertwine.
2. **Public API of a pass = one function** `pub fn run(input: &In) -> Out`. Everything else private.
   `cargo test -p fdml-flows` tests a pass alone.

```rust
// fdml-cli/src/main.rs — the entire orchestration
let systems = discovery::run(&root, &patterns);
let scans:  Vec<_> = systems.par_iter().map(scan::run).collect();   // rayon
let graph   = graph::run(&scans);
let flows   = flows::run(&graph);
let integ   = integrations::run(&scans);
let spec    = assemble::run(&scans, &graph, &flows, &integ);
spec.write(&out)?;
// enrich::run(&mut spec, provider)?;  // opt-in only
```

---

## 2. Provenance map (the cheat sheet)

| Capability | Source | Crate or Port | Pass |
|---|---|---|---|
| Graph structure | `[convergent]` | crate **`petgraph`** | graph |
| Community detection (clusters → map "boxes") | Leiden `[CodeBoarding]`, Louvain `[U-A]` | crate (louvain on petgraph) — **not hand-rolled** | graph |
| Graph store + full-text search | SQLite+FTS5 `[codegraph]` | crates **`rusqlite`** + `tantivy`/`nucleo` | graph / viewer |
| Multi-language import resolver | `[U-A]` (crown jewel) | **port** per-language rules | scan→graph |
| Reference-resolution confidence cascade | `matchReference` `[codegraph]` | **port the idea** | graph |
| Grouping heuristic (readable map) | `coverage × cluster-penalty` `[CodeBoarding]` | **port formula** (~50 lines) | graph |
| Super-clustering to 5–8 boxes | `[CodeBoarding]` | **port** | graph |
| 100% node coverage (orphan cascade) | `[CodeBoarding]` | **port** | graph |
| Flow reconstruction (topo-sort tour) | Kahn, in-degree-0 entry pts `[U-A]` | **port** (replaces our broken all-paths BFS) | flows |
| Layer detection (dir-segment patterns) | `[U-A]` | **port** | graph/flows |
| Route→handler manifest | `[codegraph]` | **port** | integrations |
| Dynamic-dispatch synthesizers (pub/sub, observer, callback) fan-out-capped | `[codegraph]` | **port, high-precision** | integrations |
| Manifest-keyword framework configs + non-code parsers (docker/k8s/sql/graphql/proto/terraform) | `[U-A]` | **port** | discovery/integrations |
| Incremental: structural fingerprint → SKIP/PARTIAL/FULL | SHA-256+signature `[U-A]`; cluster warm-start `[CodeBoarding]` | **port** | cross-cutting |
| Deterministic ordered-commit (parallel but reproducible) | reorder buffer `[codegraph]` | **port the idea** | scan→graph |
| Stable node IDs `type:path:name` | `[U-A]` | **adopt convention** | types |
| Provenance-tagged edges (`tree-sitter` \| `heuristic`) | `[codegraph]` | **adopt convention** | types |
| Health metrics (cohesion/coupling/instability/god-class/circular) | `[CodeBoarding]` | **port** — free viewer feature | assemble/viewer |
| Fuzzy search + lazy ELK layout (viewer) | Fuse.js + ELK `[U-A]` | crate `nucleo` + existing React viewer | viewer |

---

## 3. Per-pass design

### Pass 1 — `fdml-discovery`  `(root, patterns) -> Vec<System>`
Find independent projects/systems in a folder. Pure, no LLM, instant.
- **Manifest-keyword detection** `[U-A]`: classify each project by manifest (`package.json`,
  `*.csproj`, `requirements.txt`, `go.mod`, `Cargo.toml`, `docker-compose.yml`). FDML already has
  `classify_*` in `platform.rs` — lift it here, extend with U-A's keyword configs.
- Replaces the current entanglement: `detect_systems` is buried with `detect_integrations` in one
  839-line `platform.rs`. Carve it out clean.
- **Test**: fixture with two manifests → exactly two systems.

### Pass 2 — `fdml-scan`  `System -> ScanResult`  (parallel)
Tree-sitter extraction (FDML already does this well — keep). Add:
- **Extract-then-link** `[codegraph]`: each file parse emits nodes + an `unresolved_refs` list. No
  cross-file work here — that decouples parallel parsing from linking and keeps this pass embarrassingly
  parallel (rayon).
- **Deterministic ordered-commit** `[codegraph]`: a bounded reorder buffer commits results in file
  order, so the graph is byte-identical regardless of thread timing. Reproducible despite parallelism.

### Pass 3 — `fdml-graph`  `&[ScanResult] -> CodeGraph`  ← the heart
Resolve refs into edges, then cluster into the readable map.
- **Import resolution** `[U-A]`: port the multi-language resolver (tsconfig alias walk-up, NodeNext
  rewrite, Python ancestor-root walk, Go module prefix, JVM suffix-index, PSR-4, Rust crate/super).
  This is the crown jewel — the importMap is treated as **ground truth** that back-fills dropped edges.
- **Ref-resolution cascade** `[codegraph]`: ordered confidence ladder
  `file-path → qualified-name → chained-call → method → exact → fuzzy`, gated by language family,
  with an ambiguous-name ceiling (skip names with >500 candidates — also an O(K²) guard).
  `validate-before-edge`: only emit an edge if the target type checks out. Near-LSP precision, zero ML.
- **Clustering** `[convergent]`: community detection (Leiden/Louvain) on the dependency graph, **fixed
  seed** for reproducibility `[CodeBoarding seed=42]`. Get it from a crate, **do not hand-roll** (see
  §6 — this updates earlier "skip Louvain" advice).
  - count-fallback + size-split + singleton-merge `[U-A]`.
- **Readable-map grouping** `[CodeBoarding]` — the deterministic IP no crate has:
  - score clusters by `coverage × cluster-count-penalty`, ideal band **N/20 … N/5** clusters;
  - **super-cluster**: meta-graph of clusters (edge weight = cross-call count), resolution sweep to
    land **5–8 top boxes**; absorb tiny clusters by graph distance → file overlap;
  - **100% coverage** via 4-tier orphan cascade (cluster → file co-location → nearest-by-graph-distance
    → fallback) + emit a coverage report.
- **Cluster naming WITHOUT LLM** `[FDML, from CodeBoarding's LLM-only list]`: derive from dominant
  module path / most-central node (`getDominantFile` = densest in-file edge subgraph `[codegraph]`).
- **Output** stored in SQLite `[codegraph]`: WAL, composite `(source,kind)`/`(target,kind)` edge
  indexes, FTS synced by triggers (~150 lines via `rusqlite`).

### Pass 4 — `fdml-flows`  `&CodeGraph -> Vec<Flow>`  ← REWRITE for perf
Current `flows.rs::bfs_paths` enumerates **all simple paths** (exponential), clones the visited-set per
branch, then `.truncate(10)` *after* — a textbook hang on a dense graph. **Delete it.** `[FDML, bug found]`
- Replace with **topo-sort tour** `[U-A]`: Kahn's algorithm, in-degree-0 nodes = entry points, walk in
  topological order → one representative path per ingress→sink, **O(V+E)**.
- Ingress/sink heuristics (handler/route/subscribe names; save/write/publish names) — FDML already has
  these in `find_ingress_points`/`find_sink_points`; keep, drop the BFS underneath.
- **Perf test** (mandatory): dense fixture (clique of 200 nodes) must finish < 50 ms. If it explodes,
  the test fails. `[FDML]`

### Pass 5 — `fdml-integrations`  `&[ScanResult] -> Vec<Integration>`
Cross-system edges. Currently tangled into `platform.rs` — carve out.
- **Route→handler manifest** `[codegraph]`: extract route nodes (`@app.post("/x")`, `router.get(...)`)
  → match client calls to handlers → HTTP integration edges.
- **Dynamic-dispatch synthesizers** `[codegraph]`: pub/sub, observer, callback, EventEmitter patterns,
  **fan-out-capped** to stay high-precision. Tag edges `provenance: heuristic`.
- **Non-code parsers** `[U-A]`: docker-compose / k8s / sql / graphql / protobuf / terraform → emit
  service/endpoint/schema nodes for integration matching.
- **Perf**: shared-entity matching must use the HashMap exact-match path (O(n)); the current O(n²) ×
  edit-distance fuzzy pass in `detect_shared_entities` is gated by token-bucket or dropped for v1.
  `[FDML, bug found]`

### Pass 6 — `fdml-assemble`  `... -> FdmlSpec`
Deterministic YAML build from all sections (FDML already has `assemble.rs` — keep, feed it the new
graph/flows). Add **health metrics** `[CodeBoarding]` (cohesion, coupling, instability, god-class,
circular-deps) — cheap to compute from the same graph, a free differentiating feature for the viewer.

### Pass 7 — `fdml-enrich`  (OPTIONAL, OUTSIDE hot path)
The only ML. Takes the assembled spec, adds descriptions / BDD scenarios / cluster prose / relation
verbs `[CodeBoarding's LLM-only list]`. CLI calls it only on `--enrich`. Core never imports it.

---

## 4. Cross-cutting

- **Stable node IDs** `type:path:name` as merge key `[U-A]`; assemble normalize/dedup/drop-dangling,
  importMap back-fills dropped edges. Lives in `fdml-types`.
- **Provenance on every edge** (`tree-sitter` | `heuristic`) `[codegraph]` — lets the viewer show
  confidence and lets us debug false edges.
- **Incremental re-run** `[U-A + CodeBoarding]`: per-file structural fingerprint (SHA-256 of
  signature) → classify change NONE/COSMETIC/STRUCTURAL → SKIP/PARTIAL/FULL re-analysis; cluster
  warm-start with locked memberships. This is the "seconds on re-run" win.

---

## 5. What we deliberately DON'T take (and why)

- **LSP / type-checker dependency** `[CodeBoarding]` — the single cause of its minutes-to-hours runtime.
  Tree-sitter-only is the correct trade. ✗
- **Embeddings / RAG / vector store** — `[U-A]`'s `SemanticSearchEngine` is **dead code; no embeddings
  are ever generated**. Don't be fooled into thinking the map needs a vector DB. Runtime search = fuzzy
  (`nucleo`). ✗
- **LLM in the pipeline** + prompt-as-program + defensive-repair machinery `[U-A, CodeBoarding]` —
  exists only to babysit a model in the hot path. Ours is opt-in and outside. ✗
- **Two-runtime split** (Python + Node) `[U-A]` — do the merge in-process in Rust. One binary. ✗
- **25 framework resolvers + 6 regex synthesizers on day one** `[codegraph]` — long tail of edge-case
  patches. Ship the cascade + top frameworks; add the tail when a real repo needs it. (YAGNI)
- **MCP daemon / watchdog / WASM worker-recycle scaffolding** `[codegraph]` — TS/WASM artifacts; native
  Rust tree-sitter doesn't need them. ✗
- **Budget-driven truncation caps** `[U-A]` — only exist to fit LLM context; irrelevant with no LLM in core.

---

## 6. Decision that changed

Earlier in design I advised "skip community detection, group by directory (YAGNI)." **Updated:** all
three tools independently use community detection because directory grouping isn't good enough for a
*readable map* — grouping quality IS the product. The lazy way to get it is a **crate**, not a
hand-rolled Louvain and not their TS. So: "don't hand-write Louvain" still holds; "get it from a crate"
is the new call. `[FDML]`

---

## 7. Crate shopping list (the real "подключить")

| Need | Crate |
|---|---|
| Graph | `petgraph` |
| Community detection | louvain-on-petgraph (eval crates; thin if none fits) |
| Graph store + FTS | `rusqlite` (bundled), FTS5 |
| Fuzzy search | `nucleo` or `fuzzy-matcher` |
| Parallelism | `rayon` |
| Tree-sitter | already in tree |
| Hashing (fingerprint) | `sha2` / `blake3` |

---

## 8. Open decisions (need your call)

1. **Graph store**: SQLite (`[codegraph]`, queryable, survives runs, enables incremental) vs pure
   in-memory `petgraph` (simpler, faster, rebuild every run). SQLite buys incremental + viewer queries;
   in-memory is lazier. Lean: **in-memory for v1, SQLite when incremental lands.**
2. **Branch base**: `main` (clean) vs `feat/fdml-1.4` (keep WIP). Lean: **`main`**.
3. **First slice to build**: `fdml-types` + `fdml-discovery` (carve `detect_systems` out of
   `platform.rs`) + `fdml-flows` rewrite (kills the all-paths hang). Highest risk-reduction per line.
