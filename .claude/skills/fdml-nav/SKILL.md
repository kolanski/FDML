---
name: fdml-nav
description: Code navigation via the FDML index — use BEFORE grep whenever you need to find a symbol, concept, file, call chain, or "who calls what" in this repository. Trigger on any navigation intent ("where is X", "find the function that…", "как устроен…", "что вызывает…"). Costs ~13ms per query and tells you honestly when it has nothing — only then fall back to grep.
---

# fdml-nav: index-first navigation

Binary: `fdml` (install into any repo with `fdml skill --install`, or globally with
`fdml skill --install --global`; check with `fdml skill`).

## The flow (index first, grep second)

1. **Ensure the index exists** (once per repo; a re-run costs ~10ms):
   ```bash
   fdml index <repo-root>
   ```
2. **Search instead of grepping**:
   ```bash
   fdml search "<your query>" --path <repo-root> --json
   ```
   - Query can be a symbol name, several guesses at one name, or a concept
     ("wall collision", "audio callback"). Word order and case do not matter.
   - Doc comments above declarations are indexed, so prose from the author's own
     comment resolves too.
   - Each result carries `file`, `start_line`, and `window: [lo, hi]`.
   - `"marked": true` results are curated associations — trust them first.
   - `⚠ god function` means: navigate it by anchors (`fdml outline <symbol>`),
     never retrieve it whole.
3. **Honest failure — then grep.** If the output says
   `no useful result — fall back to grep` (JSON: empty, or top `score < 0.55`),
   grep as you normally would. Optionally retry once with `--llm`. Failures are
   auto-logged; never report them anywhere, telemetry is passive.

## Reading a result — use Read, never sed/awk

When a `window` comes back, **Read that range**. Do not pull lines with
`sed -n 'A,Bp'`, `awk NR>=`, `head`/`tail`, and do not read the whole file.
This is the single most common way this skill gets bypassed: line numbers linger
in context from an earlier grep and the shell feels faster. It is not — it skips
the index, produces no telemetry, and costs the same tokens.

## When you need more than a location

| Need | Command |
|---|---|
| call chains: how execution reaches a symbol | `fdml search "<q>" --flow` |
| inside a huge function: phases + what each calls | `fdml outline <symbol>` |
| callers / callees / tests of a symbol | `fdml impact <symbol>` |
| smallest source range of a symbol | `fdml get <symbol>` |
| external analyser facts (value ranges, etc.) | `fdml facts <symbol>` |

## Teach it what you learned

When you resolved something the index missed (via grep), mark it — the next
session gets it instantly:

```bash
fdml mark "<query>" <symbol-or-file:line> --alias "<other phrasing>" --alias "<another>"
```

Three rules, learned the expensive way:

1. **Prefer a symbol over a file:line.** `world.walls.world_walls_collect` survives
   edits; `walls.c:214` rots the moment lines shift. Use `file:line` only when the
   place is not a symbol (a block inside a huge function, a literal, a comment).
2. **Mark the wording that actually failed, not the correct term.** Mark keys are
   lexical: matching is over words, not meaning. A mark named with polished
   terminology will miss the way people really ask. Take the failed query verbatim
   from `fdml log` and add live phrasings as `--alias` — including другой язык, if
   that is how the team asks.
3. **Only mark things that have an address in this repo.** External facts — binary
   offsets in a foreign executable, measured constants from an experiment, a
   recipe for another tool, methodology notes — have no location here. They belong
   in memory, notes, or reports. The index cannot hold them and will only return
   confusing near-misses.

## Rules

- Never read a whole file when a `window` was returned.
- One query, at most one `--llm` retry, then grep. No retry loops against the index.
- Do not "verify" a ≥0.9 result with grep — verify by Reading the window.
