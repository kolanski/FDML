---
name: fdml-nav
description: Code navigation via the FDML index — use BEFORE grep whenever you need to find a symbol, concept, file, call chain, or "who calls what" in this repository. Trigger on any navigation intent ("where is X", "find the function that…", "как устроен…", "что вызывает…"). Costs ~13ms per query and tells you honestly when it has nothing — only then fall back to grep.
---

# fdml-nav: index-first navigation

Binary: `fdml` (install into any repo with `fdml skill --install`, or globally with
`fdml skill --install --global`; check with `fdml skill`).

## The flow (index first, grep second)

0. **Picking up work that was already under way?** `fdml dossier` first — it is the
   restore point: invariants, the PR recipe, `PENDING` (finished but not merged) and
   the symptoms recorded against this change. Falls back to the last recorded card if
   HEAD has none. Leave one behind when you stop mid-task:
   `fdml note "на чём остановились <тема>" "<done / left>" --kind pending`.
1. **Ensure the index exists** (once per repo; a re-run costs ~10ms):
   ```bash
   fdml index <repo-root>
   ```
2. **Search instead of grepping**. The default output is grep-shaped —
   `path:line: source line`, one line per hit — so it drops into the same habit:
   ```bash
   fdml search "<your query>" --path <repo-root>          # 5 hits, grep-shaped
   fdml search "<query>" --limit 3                        # fewer
   fdml search "<query>" --long                           # read windows, kinds, scores
   ```
   Do **not** add `--json` unless a program parses the output: measured on the same
   query it is ~4x the bytes of the default view, for a human or an agent reading it.
   - Query can be a symbol name, several guesses at one name, or a concept
     ("wall collision", "audio callback"). Word order and case do not matter.
   - Doc comments above declarations are indexed, so prose from the author's own
     comment resolves too.
   - `--long` adds `window: [lo, hi]` — the lines worth reading around the hit.
   - `"marked": true` results are curated associations — trust them first.
   - `⚠ god function` means: navigate it by anchors (`fdml outline <symbol>`),
     never retrieve it whole.
3. **Honest failure — then grep.** If the output says
   `no useful result — fall back to grep` (JSON: empty, or top `score < 0.55`),
   grep as you normally would. Optionally retry once with `--llm`. Failures are
   auto-logged; never report them anywhere, telemetry is passive.
4. **The question this tool does not answer — go straight to grep.** Ranked top-N
   cannot express *absence*: "every place that filters by category, to find the one
   that omits `STATUS_CANCELLED`" needs the complete set, and the index will hand you a
   confident, well-scored, incomplete one. This is worse than an honest miss — the
   answer looks finished. Any query of the form *all sites / everywhere / the one
   that is missing* is grep's, not the index's, and afterwards there is nothing to
   `mark`: an exhaustive set has no single address.

## Reading a result — use Read, never sed/awk

When a `window` comes back, **Read that range**. Do not carve source out of a file
with `sed -n 'A,Bp'`, `awk NR>=`, `head`/`tail` — this is the single most common
way this skill gets bypassed: line numbers linger in context from an earlier grep
and the shell feels faster. It is not. It skips the index, produces no telemetry,
and costs the same tokens.

Trimming a *command's output* is a different thing and is fine: `--limit 3` is the
first choice, `| head -3` works on anything. The rule is about reading files, not
about keeping output short — keeping output short is encouraged.

## When you need more than a location

| Need | Command |
|---|---|
| call chains: how execution reaches a symbol | `fdml search "<q>" --flow` |
| inside a huge function: phases + what each calls | `fdml outline <symbol>` |
| callers / callees / tests of a symbol | `fdml impact <symbol>` |
| show me the code of a symbol (competes with Read, **not** with grep — it returns the whole body) | `fdml get <symbol>` |
| external analyser facts (value ranges, etc.) | `fdml facts <symbol>` |
| knowledge with no address in the code — how to reproduce it, why it broke, an invariant, a hypothesis already disproved | `fdml note "<symptom>"` to read, see below to write |
| everything recorded against one commit, as one card | `fdml dossier [<commit>]` |

## Teach it what you learned

When you resolved something the index missed (via grep), mark it — the next
session gets it instantly. If what you learned was *why* rather than *where*,
record a note instead (below):

```bash
fdml mark "<query>" <symbol-or-file:line> --alias "<other phrasing>" --alias "<another>"
```

### Notes — what marks cannot hold

A symptom has no address. «после подгрузки всё белое», "the car jitters", "how do I
even reproduce this headless" — none of these is a place in the code, so they are
notes, not marks:

```bash
fdml note "<symptom in the words people use>" "<3-5 lines: cause, where, how to check>" \
    --kind repro|postmortem|invariant|rejected|method \
    --at <symbol>            # optional: then it surfaces with that symbol's hits
    --alias "<other phrasing>"
```

Write one **at the moment you fix something**, while the cause is still in your head:
the repro command you had to invent, the cause behind a symptom, an invariant you
just learned, a hypothesis you burned an hour disproving. `fdml search` returns
matching notes on their own, and a note anchored with `--at` appears inline under
that symbol's hit as `↳ [invariant] …`.

### Choosing a kind — the distinctions that actually matter

| kind | it holds | test |
|---|---|---|
| `postmortem` | the **symptom** and what turned out to cause it | would someone hit this and be confused? |
| `invariant` | the **fact underneath** — a rule about the code or data that will bite again | anchor it with `--at`; it must surface on its own |
| `rejected` | a hypothesis that was **wrong** | do not put true-but-unapplied knowledge here |
| `pending` | knowledge that is **true and measured, but the change was reverted** | say what it is and what it must return with |
| `repro` | how to make a problem observable — including how to build and run | a person with a clean checkout can follow it |
| `method` | how to verify a claim — **must carry the expected number** | without a number it is not a check |
| `playbook` | the scenario for a *kind of work*: where to start a feature, how a bug gets fixed here | the entry query is a task («добавить туман»), not a symptom |
| `link` | a decision that lives outside the code | roadmap, PR, ADR, ticket |

Split symptom from fact: "the thing gets pushed out of place" is a `postmortem`,
while "the bounding box includes the wheels, so its floor sits below zero" is an
`invariant` — the second will bite in any task touching those dimensions, so it
needs `--at` and a life of its own.

`rejected` and `pending` are opposites, and confusing them is expensive: `rejected`
means *do not do this*, `pending` means *this is right, put it back together with X*.

A `playbook` is written by hand — nothing assembles it. Two rules keep it alive:
its steps **point at existing notes by their phrase instead of restating them** (a
copied step rots separately from the original), and the ways people start that work
go in `--alias`, not stuffed into the phrase. `--kind method` is not the place for
it: a scenario answers *what do I do now*, a method answers *is my claim true*.

```bash
fdml note "новая фича с чего начать" \
    "1) формулу берём из отчётов реверса, не выдумываем
     2) место в коде: fdml search «<эффект>»
     3) проверка числом — см. заметку «как проверить что не сломал физику»" \
    --kind playbook --alias "добавить туман" --alias "как переносить эффект из игры"
```

**When to use `--at`:** anchor when the rule matters to whoever opens that code —
invariants, traps, gotchas — so it appears inline with the symbol. Leave it off when
the entry point is the symptom itself (`repro`, `method`, `link`): those are reached
by words, not by a symbol.

Use `--kind link` for anything that lives outside the code and explains a
decision — a roadmap section, a PR, an ADR, a design doc, a ticket:

```bash
fdml note "куда идёт индекс роадмап" "docs/navigator.md §8 — очередь работ" --kind link
fdml note "why the value graph is shaped this way" "PR #20" --kind link
```

Then "what's next here?" returns the pointer instead of a guess, and the links
show up in `fdml dossier` alongside the change they belong to.

Name a note by the **symptom itself** («всё пересвечено», "the car jitters"), not by
a question ("почему всё пересвечено"). Question words are ignored when matching, so
both forms find it — but the symptom is what other people will type. Wrong wording is
not permanent: `fdml note --delete "<phrase>"` removes a note and all its phrasings.

**`⚠ stale` means the file changed since the note was written.** It is not noise and
not a reason to ignore the note — it is a signal to re-check the claim against the
current code, and to re-record it if it still holds.

Three rules, learned the expensive way:

1. **Prefer a symbol over a file:line.** `world.walls.world_walls_collect` survives
   edits; `walls.c:214` rots the moment lines shift. Use `file:line` only when the
   place is not a symbol (a block inside a huge function, a literal, a comment).
2. **Mark the wording that actually failed, not the correct term.** Matching is over
   words, not meaning: connective words are ignored and half the key is enough to
   hit, but a mark named with polished terminology still misses the way people
   really ask. Take the failed query verbatim from `fdml log` and add live phrasings
   as `--alias` — including другой язык, if that is how the team asks.
3. **Only *mark* things that have an address in this repo — everything else is a
   note, not memory.** A mark points at a place, so it needs one. Knowledge about
   *this* repo that has no place — repro steps, postmortems, invariants, rejected
   hypotheses, verification methods — goes to `fdml note`, where the next session
   will find it. Only facts about the outside world (offsets inside a foreign
   binary, constants measured from another program, a recipe for a different tool)
   belong in memory or a report: the index has nothing to attach them to.

## What is actually cheaper — measured, not assumed

Both tools are instant (~0.01s); the cost that matters is bytes and round trips.

| situation | grep | fdml |
|---|---|---|
| you know the exact symbol name | ~300 B | **~270 B**, and marks/notes ride along |
| you do **not** know the name (a concept, a symptom) | 3 attempts, may still miss | **~100 B**, one shot |
| a 7,881-line file you must understand | reading it in chunks | **`fdml outline`** — every phase and its calls for the price of ~1.5 window reads |

So: reach for `fdml search` first in both cases, and use `fdml outline` — never
chunked reading — on anything the tool flags as a god function.

## Rules

- Never read a whole file when a `window` was returned.
- One query, at most one `--llm` retry, then grep. No retry loops against the index.
- Do not "verify" a ≥0.9 result with grep — verify by Reading the window.
