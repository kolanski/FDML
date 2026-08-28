---
name: fdml-visionary
description: Evaluate ideas, improvements, and feature proposals against FDML's product vision and architecture strategy. Use when the user proposes a change/improvement ("а что если добавить...", "прилетела идея", "оцени идею", "вписывается ли это"), asks why FDML exists or who it's for, or wants a strategy check before implementing something.
---

## Step 1 — Ground yourself (read before judging)

Read in this order, skim what you already know:

1. `research/repo-studies/00-CORE-REWORK.md` — **single source of truth** for the current architecture (deterministic core, workspace of isolated passes). Overrides anything older.
2. `README.md` — what FDML is and the promise to users.
3. `ROADMAP.md`, `PLAN.md` — where it's heading.
4. `FDML-1.3-en.md` — the spec language itself (only if the idea touches the language).

Current implementation lives in `src/`, `core/`, `web/` (viewer). Check the actual code for the area the idea touches — don't judge from docs alone.

## Step 2 — The vision in one paragraph

FDML bridges "I have an idea" → "it's deployed": features as structured, machine-readable specs, plus a CLI that can *reverse* the direction — scan an existing codebase into an FDML spec (`scan → graph → flows → assemble`). Audience: solo devs and small teams who can't afford translation layers of meetings and tickets. It must run locally, fast, on ordinary machines.

## Step 3 — Non-negotiables (a proposal that breaks these needs extraordinary justification)

1. **Deterministic core, no ML in the hot path.** LLM/enrich is optional, on-demand, outside `scan → assemble`. Proven convergent by three independent tools (see 00-CORE-REWORK.md §0).
2. **Dependency fan, not web.** Passes depend only on `fdml-types`; only the CLI wires them. A pass's public API is one `run(input) -> Output` function.
3. **Local-first, seconds not minutes.** No cloud requirement, no heavy runtimes (LSP servers, Python/Node toolchains) dragged into the core.
4. **Spec is the product.** The YAML/FDML output must stay human-readable and diffable; tooling serves the spec, not the reverse.

## Step 4 — Verdict format

Answer with:

- **Verdict:** `aligns` / `conflicts` / `orthogonal` (nice but not on the critical path).
- **Why:** which vision point or non-negotiable it supports or violates — cite the doc/section.
- **Where it lands:** which pass/crate/module would own it, and whether it fits the one-function-per-pass rule.
- **Lazy version:** the smallest slice of the idea that delivers the value without violating anything. If the full idea conflicts, propose the fitting subset.
- **Cost of yes:** what maintenance/complexity it adds forever (new dependency? new runtime? new user-facing concept?).

Be a skeptical product owner, not a cheerleader: the default answer to scope growth is "not yet". An idea that's good *in general* but wrong *for FDML's audience* is a `conflicts`.
