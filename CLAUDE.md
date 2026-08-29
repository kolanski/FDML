# FDML — working notes for agents

## Navigate with the index, not with grep

This repo indexes itself. Before hunting for a symbol, a concept or a symptom:

```bash
fdml dossier                         # ← первым делом: на чём остановились, что не закоммичено
fdml search "<query>" --limit 3      # where is it (grep-shaped output)
fdml outline <symbol>                # a huge function, phase by phase
fdml note "<symptom>"                # why it broke before, how to reproduce
```

`fdml dossier` — это и есть «восстанови контекст»: инварианты, рецепт PR, `PENDING`
(готовое, но не влитое) и симптомы, записанные против этого коммита. Если при HEAD
карточки нет, показывается последняя — работа не теряется на следующем коммите.
Заканчивая сессию с незавершённой работой, оставляйте её там:
`fdml note "на чём остановились <тема>" "<что сделано, что осталось>" --kind pending`.

The index self-heals: if the code moved since the last run, `search` reindexes
itself (~15 ms when current). If it answers `no useful result`, that is honest —
grep then, and afterwards teach it: `fdml mark "<the query that failed>" <symbol>`.

Design and roadmap live in `docs/navigator.md`; ask for them with
`fdml search "куда идёт навигатор"` rather than opening files at random.

## Editing: Edit tool first, ast-grep for sweeps

Do **not** patch files with `python3 - <<'PY' … s.replace(…)`. It has cost us real
bugs this way: heredoc quoting silently mangles arguments, and a replacement that
matches nothing fails quietly.

| task | tool |
|---|---|
| a few known edits | the **Edit** tool — it enforces uniqueness and shows a diff |
| the same change across a codebase | `ast-grep run -p '<pattern>' -r '<rewrite>' -U` (AST-aware, ~10 ms/file) |
| genuinely programmatic rewriting | a python **file**, not a heredoc |

## Git

Never run `git checkout <ref> -- .`, `git restore` or `git reset --hard` while work
is uncommitted — it overwrites the working tree and the changes are unrecoverable.
Commit or stash first. Verify the repository with `pwd` before any `git add`, and
stage explicit paths rather than `-A`.

## Before a PR

`cargo test --release` (5 suites must pass), then audit the staged diff for
absolute paths, personal data and anything belonging to another project.
`research/` and `.fdml/` are intentionally untracked.
