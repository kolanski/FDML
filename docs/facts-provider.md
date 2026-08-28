# Facts providers — plugging an external analyser into FDML

FDML's core stays deterministic, local and fast. Heavier engines (Frama-C Eva, Joern,
a language-specific analyser) attach through **a file, not a plugin API**: an adapter runs
the engine, writes one JSON document in the schema below, and `fdml facts --import` merges
it into the existing index. Nothing about the engine's runtime — OCaml, a JVM, a container —
enters the FDML binary, and swapping the engine for another language costs one adapter.

```
engine  →  adapter  →  facts.json  →  fdml facts --import  →  .fdml/index.sqlite
```

## The document

One JSON object. Every top-level key is a **fact kind** — the names are yours; nothing is
hard-coded. Two shapes are recognised:

```jsonc
{
  "provider": "frama-c-eva",              // optional; --provider is the fallback
  "confidence": "static-overapproximation", // optional default for every fact below

  // map form — the key IS the target symbol
  "fields": {
    "State.flags": { "possible_values": "0..3", "reads": ["process"], "writes": ["init"] }
  },
  "values": {
    "g_stream_busy": { "possible_values": [0, 1] }
  },

  // list form — the target is taken from target | caller | function | from | symbol
  "calls": [
    { "caller": "main", "callee": "init" }
  ],
  "data_flow": [
    { "from": "process", "to": "parse", "via": "buf" }
  ]
}
```

Entries in a list with none of those owner keys are skipped and counted, never guessed at.

## Confidence is part of the fact

A value analysis returns an over-approximation: `possible_values: [0,1,2]` means *the
analyser could not rule these out*, *not* that the program took all three states. FDML
stores `confidence` with every fact and prints it, so a static bound is never read back as
an observed one. Set it per document (`"confidence": …`) or per entry (`"kind"` /
`"confidence"` inside the entry). The default is `static-overapproximation` — the safe
reading.

## Merging rules

- Facts are keyed by **symbol name**, not row id, so they survive re-indexing.
- One row per `(provider, target, fact_kind)`. Re-running an engine **replaces** its own
  rows; it never duplicates them and never touches another provider's.
- Several providers can describe the same symbol; `fdml facts <symbol>` returns all of
  them, labelled.

## Using it

```bash
adapters/analyze_c.py src/ > facts.json          # your adapter, any language
fdml facts --import facts.json --provider frama-c-eva
fdml facts collide_walls                          # everything known about the symbol
fdml facts collide_walls --json                   # for an agent
```

## Writing an adapter

An adapter is any executable that emits the document above. It should:

1. Run the engine on a **bounded** input (a directory or a file list), not the whole repo
   by default — Eva's cost grows sharply with `-eva-slevel`, and Joern's import is minutes.
2. Map engine identifiers to plain symbol names (`collide_walls`, `State.flags`). FDML
   resolves `Type.field` and bare names against the index.
3. Set `confidence` honestly for what the engine actually proves.
4. Exit non-zero on engine failure so the caller does not import a truncated document.

Suggested layout, one file per engine:

```
adapters/
  frama_c_eva.py     # values, pointers, memory, alarms
  joern.py           # calls, control_flow, data_flow, struct members
  <your_engine>.py
```

## What FDML already knows without an engine

Check before integrating — the deterministic index covers more than it looks:

| Fact | Source today |
|---|---|
| functions, prototypes, typedefs, structs, enums, macros, globals | tree-sitter scan (`fdml index`) |
| call graph, callers/callees | `references_idx` (`fdml impact`, `fdml search --flow`) |
| call chains to entry points | `fdml search --flow` |
| phases inside a large function | `fdml outline` |
| struct **members** | not indexed — cheap to add to the scanner |
| control-flow graph, data flow | not covered — this is where Joern earns its cost |
| possible values, pointer targets, reachability | not covered — this is where Eva earns its cost |
