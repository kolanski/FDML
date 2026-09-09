# FDML Extension 1.4.2: Decisions and Provenance

Status: Draft
Scope: conceptual extension over FDML 1.4 and extension 1.4.1; no implementation
requirements yet. Every element here is added only because a real project already
produced the need for it. What has not yet earned its place is listed in §11 and stays
there until it does.

---

## 1. Purpose

FDML already models the path from human value to verified behavior:

```
vision.fdml (human)   →  feature  →  scenario  →  test  →  runner result  →  evidence
generated spec (scan) →  entity / action / flow  ──realizes──▶  feature
```

Two things on that path have no address in the model today:

- **decisions** — why the implementation is the way it is, and which decision is
  currently the one in force when several were made over time;
- **provenance** — where a statement came from and how far it can be trusted: a human
  wrote it, a scanner observed it, a model guessed it, or a run proved it.

Both showed up before this document did. A working session on FDML itself produced six
architectural decisions and recorded them as navigator notes, outside the spec, because the
spec had no place for them. The code graph already carries a two-value provenance marker
(`Matched` / `Suggested`) and a `confidence` that is always `0.0`. The evidence work of
extension 1.3 §9 already had to say "assertion-level evidence: unknown". This extension
gives those three facts a spec-level form and nothing more.

This is not an ADR format. A conventional ADR is a *projection* of a decision node
(§3.3). This is not a solution graph either; see §11 for why not yet.

---

## 2. Core rules

1. **A decision is an element.** It has an id, lives in the spec, and can be linked to like
   any other element. What it decides *about* is expressed with links to existing elements,
   never by duplicating them.
2. **Decisions are historical.** A later decision that materially changes the outcome does
   not edit the earlier one; it `supersedes` it. The one in force is the one nothing
   supersedes.
3. **Every statement has a provenance status**, from a closed set (§4). It is inherited
   from the document that carries the statement and overridden only for exceptions.
4. **Status is never promoted silently.** Only a human edit produces `declared`; only an
   evidence run produces `verified`. A scanner cannot emit `declared`; an enrichment step
   cannot emit `verified`.
5. **Reconstruction has a ceiling.** A scan may recover structure and behavior. It must not
   recover value, consumer, intent, or rationale (§5). Anything of that kind that comes out
   of a machine is `inferred` and produced by enrich, never by scan.
6. **Evidence proves only what its source establishes** (§6). FDML reports the granularity
   the runner adapter reports and does not invent finer detail.

---

## 3. Decision element

### 3.1 Format

```yaml
decisions:
  - id: stable_ids
    title: "Graph node identity is the location, not the name"
    context: >
      Two scans of the same tree produced different feature ids: the group name was taken
      from whichever member iteration reached first. 348 bare-name collisions in one
      generated spec. Human-authored links cannot stand on that.
    outcome: >
      Ids are `type:path:name`. Feature ids use the module path, not its last word.
    consequences: >
      Ids are longer and carry a path; a file move changes the id (accepted: a move is a
      change). Rename inside a file no longer moves anything else.
    status: declared
```

| field | required | form |
|---|---|---|
| `id` | yes | element id, unique across the document set |
| `title` | yes | one line |
| `context` | yes | prose: what problem and which constraints were in force |
| `outcome` | yes | prose: what was decided |
| `consequences` | no | prose: what follows, good and bad |
| `status` | no | provenance (§4); defaults to the document's status |

That is the whole element. Drivers, candidate options, scoring and alternatives are
deliberately absent (§11). A rejected alternative is either prose in `context` or a
`rejects` link to an element that exists.

### 3.2 Relations

Expressed through the existing `traceability` mechanism (1.3 §9); no new link syntax.

| relation | from | to | meaning |
|---|---|---|---|
| `selects` | decision | any element | the decision commits to this: an action, entity, feature, constraint, test |
| `rejects` | decision | any element | the decision rules this out; the element may be a feature, another decision, a constraint |
| `supersedes` | decision | decision | this decision replaces that one; the target is no longer in force |

`selects` and `rejects` may point at elements in the generated spec (via the
cross-document rules the project adopts; see §8). `supersedes` must point at a decision.

Reading the graph answers, without prose search: what is currently decided (decisions with
no incoming `supersedes`), what a given decision replaced (chain of `supersedes`), and
which implementation elements a decision committed to (`selects` targets).

### 3.3 ADR as a projection

A MADR-style record maps onto a decision node without loss of the parts that matter:

```
Context and problem statement  →  context
Considered options             →  prose in context, or `rejects` links
Decision outcome               →  outcome
Consequences                   →  consequences
Confirmation                   →  `verifies` links from tests to the scenarios the decision selected
```

An existing ADR directory can therefore be imported as decisions with `status: declared`
and `supersedes` links taken from the ADRs' own "superseded by" lines. Links reconstructed
from anything else are `inferred` (§5).

---

## 4. Provenance status

A closed enum, on elements and on traceability links:

| status | means | who may produce it |
|---|---|---|
| `declared` | a human wrote it | a human, in a human-owned file |
| `observed` | a deterministic pass read it off the code | `scan` → `graph` → `assemble` |
| `inferred` | a model or heuristic proposed it | `enrich`, archaeology (§5) |
| `verified` | a run established it | an evidence run against a declared test (§6) |
| `unknown` | nobody knows, and the model says so | anyone; it is the honest default for missing knowledge |

### 4.1 Inheritance

Status is carried by the document and inherited by everything in it:

| document | default status |
|---|---|
| `vision.fdml`, hand-written specs, imported ADRs | `declared` |
| generated spec (`fdml scan` → `assemble`) | `observed` |
| enrich overlays (`.fdml/enrich/`) | `inferred` |
| evidence reports | `verified` / `unknown` per item |

Per-element `status:` is written only for exceptions. A generated spec with `status:
observed` on every one of two thousand lines is noise and breaks diffability, which
extension 1.4 treats as a property of the product.

### 4.2 Why not `derived`

An earlier draft had `observed → derived → inferred`. Its own example, "OrderController
depends on search_index", is a dependency-graph edge, which is `observed`. No case has
yet needed a third rung. It is deferred (§11), not rejected.

### 4.3 Relation to existing markers

`LinkSource { Matched, Suggested }` and `confidence` in the code graph are this field's
ancestors. `Matched` ≈ `declared` confirmed against code; `Suggested` = `observed`.
They fold into `status` when the extension lands in the type layer; no second mechanism.

---

## 5. Archaeology boundary

A scan of an existing repository **may** recover: entities, actions, flows, dependencies,
call structure, module boundaries, candidate capabilities, and the observed shape of
behavior.

A scan **must not** claim to recover: business or user value, the consumer of a value,
the original product intent, or the rationale behind a decision.

```
CODE → STRUCTURE → CAPABILITY          deterministic; status observed
CAPABILITY → ?VALUE → ?CONSUMER        human declaration, or inferred and marked as such
```

Archaeology of *decisions* (from git history, diffs, dependency changes, existing ADRs)
produces observations and hypotheses:

```
observed:   commit 8f31 changed aggregation to exclude CANCELLED
inferred:   CANCELLED transactions should not participate in aggregation
never, without external evidence:
            the product owner requested exclusion of CANCELLED transactions
```

Such output is produced by `enrich`, outside the deterministic core, and carries
`inferred`. The core invariant of extension 1.4.1 §3, "the scan pipeline cannot derive
value from code", is the same rule stated for one node type; this section states it for all
of them.

---

## 6. Evidence semantics

Extension 1.3 §9 binds a declared `test` to a scenario with `verifies`. This section fixes
what a result of that test may claim.

1. **Granularity is the adapter's.** A runner adapter declares what it can see:

   | granularity | example | what one result attributes to |
   |---|---|---|
   | `test` | cargo: `test T ... ok` | every scenario `T` verifies, as one unit |
   | `assertion` | a runner that names failed assertions | one `then` line |
   | `metric` | a regression harness: `scenario.metric` vs baseline | one `then` line that names the metric |

   FDML must not synthesize a finer granularity than the adapter reports. When one test
   verifies three scenarios and passes, all three are `proven` **by that test**, and the
   assertion level is `unknown`. That is a valid, honest state of the model.

2. **Silence is not evidence.** If a runner prints only failures, the adapter must obtain the
   set of what was checked from elsewhere (a baseline file, a manifest). "Passed" and "not
   in this run" must remain distinguishable.

3. **Base vocabulary** for a scenario's evidence state: `proven`, `failed`,
   `not_verified` (no test claims to verify it), `unknown` (a test claims it, the run did
   not include it). A feature is `PROVEN` only when every scenario is `proven`.

4. **Adapter-specific states are named, not folded.** A baseline-relative harness may
   report `known_baseline_failure`, `regression`, `unchanged`. These must not be collapsed
   into `failed` merely because the process exits non-zero. Which of them counts as
   "the feature still works" is a per-project declaration, not a runner default.

---

## 7. Closed enums

Unknown values are parse errors, following extension 1.4.1 §5.

| field | values |
|---|---|
| `status` | `declared \| observed \| inferred \| verified \| unknown` |
| decision relations (normative) | `selects \| rejects \| supersedes` |
| evidence `granularity` | `test \| assertion \| metric` |
| evidence state (base) | `proven \| failed \| not_verified \| unknown` |

The general `relation` field on `traceability` is **not** closed by this document; see the
amendment in §10.2.

---

## 8. Validation rules

When implemented, the validator adds:

- decision ids join the id set (`collect_ids`), so links to and from decisions resolve;
- `supersedes` must target a decision, must not target itself, and the `supersedes`
  graph must be acyclic;
- `verifies` must originate at a `tests` element (already in 1.3 §9);
- a `status: declared` inside a generated document is an error: a machine wrote that file;
- a `status: verified` on anything other than an evidence item is an error: nothing but a
  run may say that.

Reporting, not validation: `fdml trace validate` lists decisions currently in force and
decisions that `select` elements no longer present in the document set (orphaned decisions;
the "stale ADR" problem, as a report line, not a failure).

---

## 9. CLI

No new command. `fdml trace validate` gains the report lines above. `fdml assemble`
writes the document-level `status: observed`. `fdml enrich` writes `inferred`. An
evidence command, when it exists, writes `verified` / `unknown` per scenario and never
touches the spec files themselves.

---

## 10. Amendments proposed to existing documents

These are not made by this file. They are recorded here so the change set is one document.

### 10.1 Extension 1.4.1 §3 — archaeology boundary

After "the scan pipeline cannot derive value from code", add:

> The same ceiling applies to consumer, intent and rationale. Anything of that kind that a
> machine produces carries `status: inferred` and is produced by enrich, never by scan
> (extension 1.4.2 §5).

### 10.2 Extension 1.4.1 §5 — relation vocabulary

Replace "unknown values are parse errors" for `relation` with:

> Canonical relations (`realizes`, `requires`, `verifies`, and the decision relations of
> 1.4.2 §3.2) are closed **semantically**: the validator checks their endpoints and meaning.
> Project-specific relations are accepted **syntactically** and reported as advisory. This
> is the escape hatch §5 promised "when a real user hits the closed set": the repository's
> own shipped example uses `operates_on` and `creates`, and closing the set broke its tests.

### 10.3 FDML 1.3 §9 — evidence

Append to the `tests` paragraph the two sentences of §6.1 and §6.2 of this document:
FDML must not invent assertion-level evidence; silence is not evidence.

---

## 11. Deferred (add only on demonstrated need from real usage)

Each of these defines its own closed enum when it lands, not before.

- **Solution graph**: decision drivers, candidate solutions, scoring, elimination as
  nodes. The nearest thing this repository keeps today is a note kind `rejected`, and it
  holds zero entries. A model of structured alternatives is not earned by a project that
  does not yet write down what it rejected in prose.
- **Consequences as nodes** (`decision --causes--> consequence`, later requirements
  `--depends_on--> consequence`). Prose until a traversal is actually asked for.
- **Decision archaeology from git** as a pass: which commits look like decisions. The
  join commit → session already exists (`fdml history`); inferring decisions from diffs is
  enrich work and stays there.
- **`derived`** as a fourth machine status (§4.2).
- **Per-decision evidence**: `decision --verified_by--> test`. Today evidence attaches to
  scenarios; a decision reaches evidence through what it `selects`.

---

## 12. Non-goals

This extension does not:

- replace ADRs with another document format; a MADR record is a valid way to author a
  decision and imports as one;
- require a decision for every implementation detail;
- require every alternative to be preserved;
- make a model's reasoning authoritative; a model may propose, the graph keeps the explicit
  decision and its evidence independently of how it was reached;
- treat a passing test as proof of anything the test does not verify;
- close the general `relation` vocabulary.

---

## 13. Invariants

Two sentences the whole extension reduces to:

> **FDML may reconstruct what a system does. It must not silently reconstruct why a human
> wanted it.**

> **Evidence may prove only what its source actually establishes.**

Read together with the authority each layer already has:

```
VALUE           human authority           declared
STRUCTURE       machine reconstruction    observed
DECISION        explicit rationale        declared (or inferred, and marked)
IMPLEMENTATION  code                      observed
EVIDENCE        a run                     verified / unknown
```

The graph connects these without collapsing their different sources of authority. That is
what "provenance" names.
