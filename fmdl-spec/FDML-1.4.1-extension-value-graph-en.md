# FDML Extension 1.4.1: Feature Value Graph and Vision Layer

Version: 1.4.1
Status: Draft
Depends on: FDML 1.4 (platform spec), FDML 1.3 §6 (features), §9 (traceability)

---

## 1. Purpose

> Axiom: **a feature is something that delivers final, measurable value.**

This extension makes that axiom checkable. It adds:

1. A **value graph**: features linked by `realizes` edges into a DAG whose roots carry
   a measurable `value` block.
2. A **vision layer**: a hand-written `vision.fdml` file describing the *ideal* set of
   user-reachable features — separate from the generated spec that describes what the
   code actually contains.
3. A **gap report**: the diff between vision and reality.

Layers (business / product / system) are **not structure** — they are depth in the
graph. The `layer` label is optional and exists only for readability.

## 2. Core rules

1. **A feature is a node.**
2. **`realizes` is an edge.** A feature may realize one or more other features,
   forming a DAG of arbitrary depth (a technical feature may be shared between
   business features: a payment-provider integration realizes both subscriptions
   and one-time purchases).
3. **Every root of the graph must carry a `value` block with a metric.** Everything
   below inherits value through its path to a root.

The primary lint follows: *every feature must have a `realizes` path to a
metric-bearing root.* A feature with no path is **dangling** — nobody can say why it
exists. This is the checkable form of "every line of code traces back to a business
requirement."

## 3. Vision layer vs. generated spec

| File | Written by | `value` | Lint mode |
|---|---|---|---|
| `vision.fdml` | human | required at roots | **strict** — dangling features are errors |
| generated spec (`fdml scan` → assemble) | machine | never required | **advisory** — dangling features are report lines, not errors |

Rationale: the scan pipeline cannot derive value from code. Requiring `value` on
generated features would make every scanned spec fail its own lint. Instead, value
lives in the vision file, and generated features link *up* into it via `realizes`.

The bridge produces the **gap report** (`fdml gap`):

- vision feature with no realizing code feature → **gap** (this is the roadmap);
- code feature with a `realizes` path into the vision → covered;
- code feature with no path and not on any user flow → **enabler or removal candidate**.

**User-facing is derived, not declared.** A code feature is user-facing when its code
is reachable from an entrypoint via the flows pass. Internal ("enabler") features are
everything else. No hand labeling.

## 4. Node format (this extension only)

```yaml
feature:
  id: subscription
  title: In-app subscription
  layer: business              # optional label; values are closed (see §5)

  value:                       # required at roots, optional below
    beneficiary: user          # closed enum, see §5
    statement: "Recurring revenue instead of one-time purchases"
    metric:
      name: mrr                # required
      unit: usd                # free-form: units cannot be enumerated
      baseline: 0              # optional
      target: 500000           # optional
    horizon: 90d               # optional

  realizes: []                 # empty = root

  scenarios:                   # unchanged from FDML 1.3 §6
    - given: ...
      when: ...
      then: ...
```

Inside `value`, only `statement` and `metric.name` are required. A six-field
mandatory form would go unfilled; a dead required field is worse than an absent one.

### 4.1 Writing the value statement (root cards)

Structural lints can't catch a value statement that is true but sells nothing.
Four rules, distilled from Value Proposition Canvas [Osterwalder], outcome
statements [Ulwich, ODI], positioning templates [Moore, "Crossing the Chasm";
Dunford, "Obviously Awesome"]:

1. **Value is a delta, not a number.** "15 minutes" says nothing; "15 minutes
   instead of a week" is the product. A user-root metric should carry `baseline`
   = the old way. *Advisory lint: user-beneficiary root metric without
   `baseline`.*
2. **User's verb, not the product's.** Not "the notebook becomes an API"
   (product does something) but "the analyst ships an API in 2 clicks"
   (user gains a power).
3. **Name the enemy.** The statement must contain the alternative being killed:
   the rewrite-by-hand, the ticket queue, the deploy cycle. No enemy — no value.
4. **The uniqueness test [Dunford].** If a competitor could put the same
   sentence on their landing page, it is not a USP — rewrite or demote the root.

The full client-facing pitch of a root needs **no new fields**: the extended
description is FDML 1.3 `description` (written in the client's language — the
client buys his solved task, not the implementation), and the client story is
`scenarios` (Given/When/Then as a day-in-the-life). Viewers render roots
expandable: collapsed = title + hero metric with killed baseline; expanded =
statement, description, full metric block, scenario, and a thin "backed by
N features / M files" rollup derived from the graph.

Non-root features expand too, answering a different question. `realizes` answers
*why* (value direction, upward); the expanded card answers *how* — the feature's
**surfaces**: the user flows, API endpoints, and views through which it touches
the world. Surfaces need no new fields and are never hand-written: they are
derived by the scan from existing constructs (`flows` §7, `actions`, entrypoint
reachability) and joined via canonical traceability (`implements`/`verifies`).
Hand-listed endpoints would rot in a sprint; derived ones cannot. The
enforcement is the existing §3 rule in lint form: *a feature on a
`beneficiary: user` path that no flow reaches is suspect*.

Example of one branch across depths:

```yaml
feature:
  id: paywall_screen
  layer: product
  realizes: [subscription]

feature:
  id: payment_provider_integration
  layer: system
  realizes: [subscription, one_time_purchases]   # DAG: shared between branches
  value:                                          # instrumental metric — allowed, not required
    beneficiary: system
    metric: {name: payment_success_rate, unit: percent, target: 99}
```

## 5. Closed enums

Unknown values are **parse errors**, not warnings. Enforced by the type layer
(Rust enums + serde), not by a separate lint.

| Field | Values | Notes |
|---|---|---|
| `beneficiary` | `user \| business \| process \| system` | `user` = lands on a user flow; `system`/`process` = enablers closed inside the product |
| `layer` | `business \| product \| system` | field optional, values closed |
| `relation` | `realizes \| requires \| verifies` | canonical set; FDML 1.3 `depends_on` → `requires`, `implements` → `realizes` (migration aliases) |

Free-form remains only where enumeration is impossible: `metric.unit`.

Inline `realizes` is sugar over canonical traceability (`relation: realizes`).
Internally there is one link mechanism; `feature_dependency` (1.3 §10.2) is an alias
for `requires` and folds into it.

No escape hatch (`x-` prefixes) yet — add when a real user hits the closed set.

## 6. CLI

```
fdml lint --value    # strict on vision.fdml, advisory on generated specs
fdml gap             # vision vs reality diff (§3)
fdml view --roots    # product overview = graph roots with rolled-up metrics
fdml view --tree <id>  # drill-down along realizes
```

## 7. Deferred (add only on demonstrated need from real phase-1 usage)

- `status` (value lifecycle: hypothesis → … → sunset) + `toggle` with `expiry` lint
- `kano` axis (user-beneficiary features only) + roadmap report
- `parent` (intra-layer hierarchy, orthogonal to `realizes`)
- `requires` / `excludes` variability constraints
- cross-repo `realizes` references (`other-repo/feature_id`)

Each defines its own closed enum when it lands, not before.
