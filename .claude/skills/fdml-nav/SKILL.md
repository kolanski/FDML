---
name: fdml-nav
description: Code navigation via the FDML index — use BEFORE grep whenever you need to find a symbol, concept, file, call chain, or "who calls what" in this repository. Trigger on any navigation intent ("where is X", "find the function that…", "как устроен…", "что вызывает…"). Costs ~13ms per query and tells you honestly when it has nothing — only then fall back to grep.
---

# fdml-nav: index-first navigation

Binary: `fdml` (in this repo: `target/release/fdml`). Copy this skill to
`~/.claude/skills/` to use it in any repository.

## The flow (index first, grep second)

1. **Ensure the index exists** (once per repo; rebuild is incremental and takes seconds):
   ```bash
   fdml index <repo-root>
   ```
2. **Search instead of grepping**:
   ```bash
   fdml search "<your query>" --path <repo-root> --json
   ```
   - Query can be a symbol name, several guesses at one name, or a concept
     ("wall collision", "audio callback"). Word order and case do not matter.
   - Each result carries `file`, `start_line`, and `window: [lo, hi]` — **Read that
     window**, not the whole file.
   - `"marked": true` results are curated associations — trust them first.
   - `⚠ god function` on a result means: navigate it by line anchors
     (`fdml outline <symbol>`), never retrieve it whole.
3. **Honest failure — then grep.** If the output says
   `no useful result — fall back to grep` (JSON: empty list or top `score < 0.55`):
   use grep as you normally would. Optionally retry once with `--llm`
   (local Ollama picks from grep evidence, ~8s). Every failure is auto-logged —
   do NOT report it anywhere, telemetry is passive.

## When you need more than a location

| Need | Command |
|---|---|
| call chains: how execution reaches a symbol | `fdml search "<q>" --flow` |
| inside a huge function: phases + what each calls | `fdml outline <symbol>` |
| callers / callees / tests of a symbol | `fdml impact <symbol>` |
| smallest source range of a symbol | `fdml get <symbol>` |
| external analyser facts (value ranges, etc.) | `fdml facts <symbol>` |

## Teach it what you learned

When you resolved a concept the index missed (via grep), spend one call so the next
session gets it instantly:
```bash
fdml mark "<the query that failed>" <file:line-or-symbol>
```

## Rules

- Never read a whole file when a `window` was returned.
- Never loop retries against the index: one query, at most one `--llm` retry, then grep.
- Do not "verify" index results with grep when the score is ≥ 0.9 — that defeats
  the purpose; verify by Reading the window.
