# FDML Extension 1.4.1: EARS Requirements

> **Status: Proposal / Draft.** This document describes how the Easy Approach to
> Requirements Syntax (EARS) can be adopted by FDML. Nothing here is implemented yet;
> the intent is to agree on the schema and the validation surface before writing code.

## 18. Extension 1.4.1: EARS Requirements

### 18.1 Extension Purpose

FDML today describes **behaviour** (`features` → `scenarios`, BDD style) and
**business rules** (`constraints`), but it has no first-class place for a
**normative requirement** — a single, atomic, testable statement of what the system
shall do. The closest thing is `features[].acceptance_criteria`, which is an
unconstrained `Vec<String>`: nothing validates it, nothing links to it, nothing is
generated from it.

EARS closes that gap. It constrains natural-language requirements to a handful of
templates built from four keywords (`While`, `When`, `Where`, `If/Then`) plus `shall`.
The result is still readable prose — no formal logic to learn — but it is regular
enough for a machine to parse, classify, lint, and trace.

This matters more for FDML than for a classic requirements tool, because FDML
specs are increasingly produced by LLMs (`scan-platform`, `link-code --llm`).
A constrained grammar gives us a hard target for JSON-Schema-driven generation and a
deterministic checker for whatever the model writes back.

Goals of this extension:

- Add a `requirements:` block whose items are EARS-shaped and machine-checkable.
- Classify each requirement into an EARS pattern and validate its well-formedness.
- Link requirements to systems, actions, features, scenarios and code through the
  existing `traceability` mechanism, so coverage becomes computable.
- Generate BDD scenario skeletons and test stubs *from* requirements.
- Keep every existing spec valid — the block is optional and additive.

### 18.2 EARS in One Page

EARS was developed at Rolls-Royce (Mavin et al., 2009) and constrains every
requirement to one canonical shape:

```
<optional preconditions> <optional trigger> the <system name> shall <system response>
```

Clauses always appear in the same order. Which keywords are present determines the
pattern:

| Pattern | Keywords | Template | Example |
|---|---|---|---|
| Ubiquitous | — | `The <system> shall <response>.` | The billing service shall store all invoices for 7 years. |
| State-driven | `While` | `While <state>, the <system> shall <response>.` | While the cart is empty, the storefront shall hide the checkout button. |
| Event-driven | `When` | `When <trigger>, the <system> shall <response>.` | When the user submits the registration form, the auth service shall create an account with status `active`. |
| Optional feature | `Where` | `Where <feature is included>, the <system> shall <response>.` | Where two-factor authentication is enabled, the auth service shall require a TOTP code. |
| Unwanted behaviour | `If … then` | `If <unwanted condition>, then the <system> shall <response>.` | If the payment provider returns a timeout, then the checkout service shall retry once and mark the order as `payment_pending`. |
| Complex | ≥ 2 keywords | `Where <feature>, while <state>, when <trigger>, the <system> shall <response>.` | While the account is locked, when the user submits valid credentials, the auth service shall reject the attempt and log an audit event. |

Two things make this useful to us:

1. **Ubiquitous ≠ "no trigger by accident".** Forcing the author to pick a pattern
   surfaces the missing trigger or missing state that vague criteria hide.
2. **Unwanted behaviour is a first-class pattern.** Most FDML specs today describe
   only happy paths; `If … then` requirements make the error surface explicit, and
   those are exactly the scenarios generators forget.

### 18.3 How EARS Fits FDML's Feature Concept

This is the crux of the proposal, so it deserves more than a diagram. FDML's model of
a feature today is:

```
feature = title + description + scenarios (Given/When/Then) + acceptance_criteria (free text) + dependencies
```

EARS overlaps that structure almost clause for clause, which is precisely why it must
be integrated deliberately rather than bolted on:

| EARS clause | Gherkin equivalent in FDML | Note |
|---|---|---|
| `While <state>` | `given:` | Same idea: precondition that holds. |
| `When <trigger>` | `when:` | Direct match. |
| `shall <response>` | `then:` | Direct match. |
| `If <cond>, then` | — | No FDML analogue. Error paths today are just another scenario, indistinguishable from a happy path. |
| `Where <feature>` | — | No FDML analogue. Optionality/feature-flags are unexpressible. |
| — | `and:` chains in `then:` | Gherkin allows a scenario to assert 3 things; EARS forbids it. |

So EARS is **not a second way to write scenarios**. The difference is granularity and
intent:

- A **scenario is an example** — one concrete walk-through, non-atomic, illustrative.
- A **requirement is a norm** — atomic, always true, individually testable and traceable.

FDML currently has only the example, and that is the root of three concrete gaps in
the toolchain as it stands:

1. **`acceptance_criteria` is a dead field.** It is `Option<Vec<String>>` in
   `ast.rs`, parsed, and then never validated, referenced, or generated from. It is
   where the normative statements actually live today, informally.
2. **Traceability is coarse.** A trace link can only point at a whole scenario, but a
   scenario asserts several things at once, so "which code implements this rule" has
   no precise answer. Requirements are the missing atom.
3. **Generated tests are empty.** `test_gen.rs` emits `expect(true).toBe(true)` with
   the Given/When/Then lines as comments, because free-text steps carry no structure.
   Clause-structured requirements give the generator something to name and assert on.

There are three ways to place EARS in the model. They differ in how much duplication
they create.

**Option A — parallel top-level `requirements:` block, features verify them.**
Classic requirements-engineering layering. Downside for FDML: you state the rule once
in EARS and again as Given/When/Then, and nothing keeps the two in sync. Double
bookkeeping in a language whose selling point is a single source of truth.

**Option B — EARS constrains `acceptance_criteria` in place.** No new concept, no new
block: existing criteria strings must simply parse as EARS. Cheapest possible change,
immediate lint value, zero migration. Downside: criteria have no ids, so they cannot
be traced or verified individually — gaps 2 and 3 above stay open.

**Option C (recommended) — requirements live *inside* the feature, scenarios verify
them.** The feature stays the unit of work, exactly as today, but gains a normative
core:

```yaml
features:
  - id: user_registration
    title: User Registration
    requirements:                       # WHAT the system shall do — atomic, EARS-shaped
      - id: req_welcome_mail
        text: "When a user completes registration, the auth service shall send a welcome email within 60 seconds."
      - id: req_unique_email
        text: "The auth service shall reject a registration whose email is already registered."
    scenarios:                          # HOW we demonstrate it — examples, may cover several requirements
      - id: successful_registration
        verifies: [req_welcome_mail]
        given: [...]
        when:  [...]
        then:  [...]
```

Read plainly: **a feature is a named set of requirements plus the scenarios that
demonstrate them.** That is very close to how features are reviewed in practice
("what must it do" / "show me"), and it keeps every existing block in place.
`acceptance_criteria` becomes the legacy spelling of `requirements` — same slot in the
feature, now with ids and a grammar.

Because scenarios are mechanically derivable from clauses (§18.8), Option C also opens
the door to the direction FDML already takes with code: the requirement is the source,
the scenario is a generated artifact you edit only when you need extra concrete
examples. That is a later step, not part of this extension — but Option C is what
makes it reachable, and Option A is what makes it awkward.

A top-level `requirements:` block still exists for what genuinely does not belong to a
single feature: platform-wide and non-functional requirements ("The gateway shall
respond to /health within 100 ms"), and requirements owned by a system rather than a
feature in multi-system 1.4 specs. Feature-scoped and top-level requirements share one
id namespace and one validator.

The resulting layering:

```
systems / contours          — who is responsible
      ↓  system:
requirements (EARS)         — WHAT shall happen        ← new; inside a feature, or top-level for NFRs
      ↓  verified_by                ↓  implemented_by
scenarios (BDD)             — HOW we show it      actions / entities — HOW it is modelled
      ↓  traceability                                    ↓  traceability
code elements                                        code elements
```

Relationship to `constraints`: they stay as they are — invariants over entities and
actions, expressed as a `rule` expression, always true, machine-evaluable. A
requirement is a statement of obligation, usually conditional, written in prose.
Where they overlap (`unique_email`), the constraint remains the executable form and
the requirement references it via `implemented_by`. Rule of thumb: if it can be
written as an expression over fields, it is a constraint; if it needs a trigger or a
state to make sense, it is a requirement.

### 18.4 Schema

Per §18.3 (Option C), a `requirements:` list may appear in two places with identical
item shape: inside a feature (the normal case) and at top level (platform-wide and
non-functional requirements). Both share one id namespace and one validator; a
feature-scoped requirement inherits `system:` from its feature's context when omitted.

```yaml
requirements:
  - id: req_login_lockout
    pattern: unwanted                 # optional; inferred when omitted, checked when present
    system: auth_service              # ref → systems[].id or system.id
    text: >
      If the user submits an invalid password 5 times within 15 minutes,
      then the auth service shall lock the account for 15 minutes.
    priority: must                    # must | should | could  (optional)
    rationale: "Brute-force protection, security review AR-114"
    applies_to: [user, session]       # refs → entity / action / feature ids
    implemented_by: [lock_account]    # refs → action ids (or code refs via traceability)
    verified_by: [scenario_lockout_after_5_attempts]   # refs → scenario / feature ids
    tags: [security, auth]
    source: "SEC-114"                 # external tracker id (optional)
```

`text` is the canonical, human-authored form. The parser derives the clause
structure from it. Authors who prefer explicit structure may write `clauses:`
instead of (or in addition to) `text:`:

```yaml
  - id: req_reverse_thrust
    system: engine_control
    clauses:
      where: "the thrust reverser option is installed"
      while: "the aircraft is on the ground"
      when: "reverse thrust is commanded"
      shall: "enable reverse thrust"
    # text: rendered from clauses when absent
```

Rules of the dual form:

- At least one of `text` / `clauses` must be present.
- If both are present they must agree after normalization; disagreement is an error
  (`EARS012`), because a silently diverging pair is worse than either alone.
- `clauses.if` and `clauses.when` are mutually exclusive (a requirement has one
  trigger; the choice of keyword is what distinguishes wanted from unwanted).
- `clauses.shall` is mandatory and holds the response only — without the leading
  `the <system> shall`.

Cross-references added to existing blocks (all optional):

```yaml
features:
  - id: user_registration
    requirements:                                          # inline definitions (Option C), or …
      - id: req_welcome_mail
        text: "When a user completes registration, the auth service shall send a welcome email within 60 seconds."
      - req_platform_audit_log                             # … a bare id, referencing a top-level requirement
    scenarios:
      - id: successful_registration
        verifies: [req_welcome_mail]                       # scenario verifies requirement(s)
```

`features[].requirements` accepts both full requirement objects and plain id strings,
so a feature can define its own rules and point at shared platform ones in the same
list. `features[].acceptance_criteria` remains valid and is read as a list of
requirement `text` values without ids (lintable by §18.7, but not traceable — which is
the incentive to migrate via `fdml ears convert`).

`traceability` gains three relation values: `verifies`, `implements`, `derives_from`.
The inline fields above are sugar; the validator expands them into the same graph the
`traceability` block produces, so `fdml trace` keeps working unchanged.

### 18.5 Grammar

```ebnf
requirement   = [ where_clause "," SP ] [ while_clause "," SP ] [ trigger ] response "."
where_clause  = "Where" SP condition
while_clause  = "While" SP state
trigger       = ( "When" SP event "," SP ) | ( "If" SP condition "," SP "then" SP )
response      = "the" SP system SP "shall" SP action
condition     = text ; state = text ; event = text ; action = text
system        = text ; matched case-insensitively against systems[].name / id
```

Keyword matching is case-insensitive at the start of a clause and anchored: a `when`
inside the response body is not a trigger. Parsing is a small hand-written scanner
over the sentence — no dependency needed, ~200 lines.

Pattern classification (`pattern` is inferred, not required):

| Keywords found | Inferred pattern |
|---|---|
| none | `ubiquitous` |
| `While` only | `state` |
| `When` only | `event` |
| `Where` only | `optional` |
| `If … then` only | `unwanted` |
| two or more | `complex` |

When the author declares `pattern:` and the inferred value differs, that is an error
(`EARS005`) — it is the cheapest way to catch "I meant this to be an error path but
wrote a happy path".

### 18.6 Localization

FDML ships Russian documentation, and specs in this repo are already written by
Russian-speaking authors. The checker therefore reads a keyword table rather than
hard-coded English literals:

| Role | `en` | `ru` |
|---|---|---|
| state | `While` | `Пока` |
| trigger | `When` | `Когда` |
| optional feature | `Where` | `Если предусмотрено` / `При наличии` |
| unwanted | `If … then` | `Если … то` |
| obligation | `shall` | `должен` / `должна` / `должно` |

The language is taken from `metadata.language` (new optional field, default `en`), or
per requirement via `lang: ru`. Generation and linting stay language-aware; ids,
pattern names and error codes stay English. Mixed-language documents are allowed but
each requirement must be internally consistent (`EARS013`).

### 18.7 Validation Rules

Added to `Validator::new()` as regular `ValidationRule`s, so `fdml validate` picks
them up with no CLI change. Errors fail `--strict`; warnings do not.

| Code | Severity | Rule |
|---|---|---|
| `EARS001` | error | Requirement has no obligation keyword (`shall`). |
| `EARS002` | error | More than one `shall` — split into atomic requirements. |
| `EARS003` | error | Clause order violates `Where → While → When/If → shall`. |
| `EARS004` | error | `If` without matching `then`. |
| `EARS005` | error | Declared `pattern` does not match the inferred pattern. |
| `EARS006` | error | Missing both `text` and `clauses`. |
| `EARS007` | error | Duplicate requirement `id`. |
| `EARS008` | error | `system` / `applies_to` / `verified_by` / `implemented_by` reference an unknown id. |
| `EARS009` | error | Both `clauses.when` and `clauses.if` present. |
| `EARS012` | error | `text` and `clauses` disagree after normalization. |
| `EARS013` | error | Mixed-language keywords within one requirement. |
| `EARS020` | warning | Vague term in the response (`fast`, `user-friendly`, `efficient`, `appropriate`, `etc.`, `быстро`, `удобно`, `и т.д.`). |
| `EARS021` | warning | Conjunction in the response (`and`, `or`, `и`, `или`) — probably two requirements. |
| `EARS022` | warning | `must` / `should` / `will` used instead of `shall`. |
| `EARS023` | warning | Passive or capability phrasing (`shall be able to`, `shall support`). |
| `EARS024` | warning | Requirement has no `verified_by` — no scenario proves it. |
| `EARS025` | warning | Requirement has no `system` — ownership is undefined. |
| `EARS026` | warning | Two requirements normalize to the same text (likely duplicate). |
| `EARS027` | info | Feature has scenarios but no requirements (coverage gap in the other direction). |

`EARS020`–`EARS023` are the linting layer that gives EARS most of its practical value;
they are the reason to run this in CI even before the generation work lands.

### 18.8 Generation

Once requirements are structured, three generators fall out almost for free.

**Requirement → Gherkin skeleton.** The clause mapping is mechanical:

| EARS clause | Gherkin |
|---|---|
| `where` | `Given <feature> is enabled` |
| `while` | `Given <state>` |
| `when` | `When <event>` |
| `if` | `When <condition>` (scenario titled `… (unwanted)`) |
| `shall` | `Then <response>` |

`fdml ears scaffold spec.fdml --write` appends a draft scenario per uncovered
requirement, with `verifies: [<req_id>]` already filled in. Authors edit prose; the
link is never hand-maintained.

**Requirement → test stub.** `TestGenerator` gains a per-requirement stub in each
target language, named after the requirement and tagged `@req:<id>`, so a coverage
tool can map failing tests back to requirements:

```typescript
// req_login_lockout — If the user submits an invalid password 5 times …
describe('@req:req_login_lockout', () => {
  it('locks the account for 15 minutes', async () => {
    // TODO: implement
  });
});
```

**Requirement → constraint hint.** Ubiquitous requirements over an entity field
(`shall store …`, `shall reject …`) are reported as candidate `constraints` by
`fdml ears suggest`. Suggestion only — no automatic writes.

### 18.9 LLM Integration

`src/linker/llm_classify.rs` already drives Ollama with JSON Schema
(`scenario_schema()`). An analogous `requirement_schema()` constrains the model to the
EARS shape at decode time:

```json
{
  "type": "object",
  "properties": {
    "requirements": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id":      { "type": "string" },
          "system":  { "type": "string" },
          "pattern": { "enum": ["ubiquitous","state","event","optional","unwanted","complex"] },
          "clauses": {
            "type": "object",
            "properties": {
              "where": {"type": "string"}, "while": {"type": "string"},
              "when":  {"type": "string"}, "if":    {"type": "string"},
              "shall": {"type": "string"}
            },
            "required": ["shall"]
          }
        },
        "required": ["id", "system", "clauses"]
      }
    }
  },
  "required": ["requirements"]
}
```

Generating `clauses` rather than free `text` is the point: the model cannot produce an
unparseable requirement, and the deterministic checker (§18.7) still audits what comes
back. `scan-platform` then emits requirements alongside features, and
`--skip-requirements` mirrors the existing `--skip-scenarios` flag.

### 18.10 CLI Surface

Follows the existing `Migrate` / `Trace` subcommand shape in `src/cli/args.rs`:

```bash
fdml ears check spec.fdml [--strict] [--format text|json]   # grammar + lint rules
fdml ears list spec.fdml [--pattern unwanted] [--system auth_service]
fdml ears coverage spec.fdml [--format text|json]           # req → scenario → code matrix
fdml ears convert spec.fdml [--write]                       # acceptance_criteria → requirements draft
fdml ears scaffold spec.fdml [--write]                      # requirements → scenario skeletons
fdml ears suggest spec.fdml                                 # candidate constraints
```

`fdml validate` runs the §18.7 rules automatically. `fdml ears check` exists so CI can
gate on requirement quality alone, and so the exit code is meaningful in a pre-commit
hook.

### 18.11 Implementation Plan

Ordered so each phase is independently shippable and useful.

**Phase 1 — model + checker (~1–2 days).**
- `src/parser/ast.rs`: `Requirement`, `EarsClauses`, `EarsPattern`;
  `requirements: Vec<RequirementRef>` on both `FdmlDocument` and `Feature`, with
  `#[serde(default)]`. `RequirementRef` is an untagged enum of `Id(String)` and
  `Inline(Requirement)` (§18.4), and `Scenario` gains `verifies: Vec<String>`.
  Rust keywords need renames: `#[serde(rename = "where")] pub where_clause`, likewise
  `while_state`, `if_condition`.
- New `src/ears/` module: `grammar.rs` (scanner + classifier + normalizer),
  `keywords.rs` (en/ru tables), `lint.rs` (`EARS020`–`EARS027`).
- `src/validator/rules.rs`: register `ears_wellformed`, `ears_unique_ids`,
  `ears_references`, `ears_lint`.
- Unit tests per pattern, per error code, plus round-trip `text ⇄ clauses`.

**Phase 2 — CLI + traceability (~1 day).**
- `src/cli/args.rs` / `commands.rs`: `Ears` subcommand with `check`, `list`,
  `coverage`, `convert`.
- Expand inline `requirements:` / `verifies:` refs into the traceability graph;
  extend `fdml trace` output with the new relations.

**Phase 3 — generation (~1–2 days).**
- `src/generators/test_gen.rs`: per-requirement stubs for ts/py/go with `@req:` tags.
- `fdml ears scaffold` writing scenario skeletons.

**Phase 4 — LLM + viewer (~2 days).**
- `requirement_schema()` and prompt in `src/linker/llm_classify.rs`; wire into
  `link-code` / `scan-platform` with `--skip-requirements`.
- `web/`: requirements panel and a coverage matrix view (requirement × scenario ×
  code), reusing the existing spec-viewer data flow.

**Phase 5 — docs and spec bump.**
- Fold this document into the main spec as FDML 1.5, update `README.md`,
  `USAGE.md`, `CLI_TOOLS_QUICK_REFERENCE.md`, and add a worked example under
  `examples/e-commerce/`.

Note on the hand-written parser: `src/parser/parser.rs` currently stubs out
`parse_action`, `parse_flow`, `parse_constraint` and friends, while the real path for
`.fdml` files is `parse_fdml_yaml` via serde. Phase 1 therefore targets the serde path
only; a `TokenType::Requirement` arm should be added to the lexer/parser at the same
time the other stubs are finished, not before.

### 18.12 Backward Compatibility

- `requirements:` is optional and defaults to empty — every existing spec parses
  unchanged.
- `acceptance_criteria` is untouched and stays valid indefinitely.
- All new validation on existing constructs is warning-level (`EARS027`), so no
  currently-passing spec starts failing `fdml validate --strict`.
- `fdml ears convert` is opt-in and never rewrites a file without `--write`.

### 18.13 Open Questions

1. **Requirement ids.** Prefix convention (`req_*`) enforced, or free-form with
   uniqueness only? Enforcing a prefix makes traceability output readable at a glance
   but breaks imports from external trackers.
2. **Requirements per system.** In multi-system 1.4 specs, do requirements live in the
   platform file or in each per-system spec? Proposal: both, with `system:` mandatory
   in the platform file and inferred in a per-system file.
3. **Should `constraints` be re-expressed as ubiquitous requirements** in a later
   version, or do the two blocks stay separate permanently? Keeping them separate is
   simpler now; merging is a 1.6 conversation.
4. **`shall` vs `должен` in generated artifacts** — generated test names in Russian
   specs: transliterate, translate, or keep the source clause verbatim?

### 18.14 Full Example

```yaml
metadata:
  version: "1.5"
  language: en

systems:
  - id: auth_service
    name: Authentication Service
    type: service
    technology: rust

entities:
  - id: user
    fields:
      - name: email
        type: string
        required: true
      - name: failed_attempts
        type: int
        default: 0

actions:
  - id: lock_account
    name: Lock account
    input:  { entity: user, fields: [id] }
    output: { entity: user, fields: [id, locked_until] }

# Top level: platform-wide / non-functional, not owned by one feature
requirements:
  - id: req_audit_retention
    system: auth_service
    text: "The authentication service shall retain authentication audit records for 12 months."
    pattern: ubiquitous
    priority: must
    tags: [compliance]

features:
  - id: user_registration
    title: User Registration
    requirements:
      - id: req_unique_email
        text: "The authentication service shall reject a registration whose email is already registered."
        pattern: ubiquitous
        priority: must
        applies_to: [user]
        verified_by: [scenario_duplicate_email]

      - id: req_welcome_mail
        text: "When a user completes registration, the authentication service shall send a welcome email within 60 seconds."
        pattern: event
        priority: should
        verified_by: [successful_registration]
    scenarios:
      - id: successful_registration
        title: Successful registration with valid data
        verifies: [req_welcome_mail]
        given: ["Email test@example.com is not registered"]
        when:  ["User submits the registration form"]
        then:  ["Account is created with status active", "Welcome email is sent"]

  - id: user_login
    title: User Login
    requirements:
      - id: req_login_lockout
        pattern: unwanted
        priority: must
        rationale: "Brute-force protection (SEC-114)"
        clauses:
          if: "a user submits an invalid password 5 times within 15 minutes"
          shall: "lock the account for 15 minutes and write an audit record"
        implemented_by: [lock_account]
        verified_by: [scenario_lockout_after_5_attempts]

      - id: req_totp_prompt
        pattern: complex
        clauses:
          where: "two-factor authentication is enabled for the account"
          while: "the account is not locked"
          when: "the user submits valid credentials"
          shall: "request a TOTP code before issuing a session token"
        verified_by: [scenario_totp_challenge]

      - req_audit_retention          # shared platform requirement, referenced by id
    scenarios: []                    # to be filled by `fdml ears scaffold`
```

`fdml ears check` on this document reports: 5 requirements (2 ubiquitous, 1 event,
1 unwanted, 1 complex), 0 errors, and warnings for the requirements in `user_login`
whose `verified_by` scenarios do not exist yet (`EARS008`/`EARS024`) — which is exactly
the backlog `fdml ears scaffold` then fills in.

### 18.15 References

- [EARS official guide — Alistair Mavin](https://alistairmavin.com/ears/)
- [Easy Approach to Requirements Syntax — Wikipedia](https://en.wikipedia.org/wiki/Easy_Approach_to_Requirements_Syntax)
- [Mavin et al., "Easy Approach to Requirements Syntax (EARS)", RE'09 (PDF)](https://ccy05327.github.io/SDD/08-PDF/Easy%20Approach%20to%20Requirements%20Syntax%20(EARS).pdf)
- [Adopting EARS Notation for Requirements Specification — Visure](https://visuresolutions.com/alm-guide/adopting-ears-notation/)
- [EARS integration request in github/spec-kit (prior art in an AI-spec toolchain)](https://github.com/github/spec-kit/issues/1356)
