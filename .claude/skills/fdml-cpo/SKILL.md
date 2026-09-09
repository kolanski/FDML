---
name: fdml-cpo
description: Decide what this repository should build next, from evidence rather than opinion — the gap between the declared queue and what the telemetry, the failed queries and the half-finished work actually show. Use when asked "что дальше", "什么 next", "приоритеты", "составь план", "что важнее", or before opening a planning discussion. Not for judging a single idea — that is fdml-visionary.
---

# fdml-cpo: what to build next, and the number that says so

The roadmap says what we *intended*. The index says what actually happened. Your job
is the **diff between them** — and to refuse to propose anything the evidence does not
already point at.

## Step 1 — Gather (never skip; never substitute memory)

```bash
fdml log --json                 # queries, failures, retry episodes
fdml candidates --json          # shell work rebuilt by hand — a missing tool
fdml history --json --limit 40  # where the work actually went, and from which ask
fdml dossier --json             # pending / rejected / invariants at this commit
fdml note --list "" | head -40  # everything recorded, all kinds
python3 research/grep-bypass.py # greps that never asked the index (if present)
```

Then read the declared queue — `docs/navigator.md §8` — with `fdml search "куда идёт
навигатор роадмап очередь работ"`. That is the plan of record you are diffing against.

## Step 2 — The two guards, before you propose anything

1. **`rejected` is a wall.** A note of kind `rejected` means the hypothesis was tried and
   refuted. Proposing it again wastes the exact hours that were spent disproving it. Read
   them first; if your idea is there, it is dead — say so and move on.
2. **`pending` is not a proposal, it is a debt.** `pending` means *this is right and
   measured, but it was reverted*. It outranks any new idea: finishing it costs less than
   starting anything, and the measurement is already done.

## Step 3 — The rule that makes this a CPO and not a brainstorm

> **Every item carries a number from the evidence, or it is not an item.**

"Стоит улучшить поиск" is not a proposal. "`грep -n` обошёл индекс 13 раз при 62% обхода
в этой репе" is. If you cannot attach a count, a rate, or a date from Step 1, you are
speculating — put it under *Гипотезы* at the bottom and mark it unmeasured.

Two numbers are usually the strongest and are the easiest to miss:

- **retry episodes** — consecutive rephrasings are the agent giving up on the tool, the
  single strongest passive signal there is;
- **`candidates`** — the second time the same shell shape is assembled by hand, a tool
  should have existed. Repetition is the whole argument.

## Step 4 — What you must not do

- **Do not re-derive a label that already exists.** The kind of work is in the
  conventional-commit type (`feat:` / `fix:`), the kind of knowledge is in the note kind.
  Classifying prompts to recover either is rebuilding what the author already wrote.
- **Do not propose a dashboard, a report or a visualisation as a priority item** unless a
  number says a decision is currently being made blind. A map sells the layer; metrics
  justify it. Selling is legitimate — but call it selling, not the critical path.
- **Do not invent a metric name.** Use the ones the commands emit.

## Step 5 — The answer

Ranked, most-justified first. For each:

```
<что> — <одно предложение>
  улика:   <число и откуда: fdml log / candidates / history / bypass>
  цена:    <что усложняется навсегда: зависимость, новое понятие, рантайм>
  почему сейчас: <что разблокирует или что перестанет протухать>
```

Then two short sections that are usually more valuable than the list:

- **Долги** — `pending` заметки, дословно. Они уже измерены.
- **Стена** — `rejected`, чтобы обсуждение не пошло по второму кругу.

Close with what the evidence does **not** cover — the question nobody has measured yet.
An honest hole beats a confident guess, and it is the next thing to instrument.
