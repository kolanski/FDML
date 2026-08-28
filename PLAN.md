# FDML Core Rework — Work Plan

> Branch `feat/core-rework` **from `feat/fdml-1.4`** (25 commits ahead of main; has `--no-llm`, .NET detect, parallel scan — the code we lift).
> Architecture & provenance: `research/repo-studies/00-CORE-REWORK.md`.
> Size: S / M / L / XL. Provenance: `[codegraph]` `[CodeBoarding]` `[U-A]` `[convergent]` `[FDML]`.
> **Связи:** `← вход` (что ест) · `→ потребитель` (кто ест выход).

---

## Мастер-таблица

| ID | Задача | Выход / артефакт | Размер | Связи |
|----|--------|------------------|--------|-------|
| **0** | **Скелет** | workspace компилируется | **S** | — |
| 0.1 | workspace `Cargo.toml` (`members = crates/*`) | — | S | корень |
| 0.2 | `fdml-types`: контракты + `serde` derive (для артефактов) | crate | S | → **все** |
| 0.3 | node-id `type:path:name` + provenance enum `[U-A][codegraph]` | в types | S | → graph |
| 0.4 | `fdml-cli`: clap, сабкоманды-заглушки, `run` + `--workdir` конвенция | bin | S | → зовёт пассы |
| 0.5 | `ARCHITECTURE.md` в корень | doc | S | — |
| **1** | **discovery** | `systems.json` | **S–M** | ← types |
| 1.1 | лифт `detect_systems`+`classify_*`+compose из `platform.rs` | — | S–M | ← System-правила |
| 1.2 | `run(root, patterns) -> Vec<System>` | `.fdml/systems.json` | S | → scan, integrations |
| 1.3 | discovery-rules: **плоская таблица** marker/dir→kind/tech; встроенный дефолт + оверрайд файлом | `discovery-rules.yaml` (вход) | M | ← discovery |
| 1.4 | CLI: `fdml discover ./proj --workdir .fdml/` пишет артефакт | systems.json | S | self-contained |
| 1.5 | тест: фикстура 2 манифеста → 2 системы | — | S | — |
| 1.⚠ | **НЕ DSL** — только данные; появилась логика → стоп | — | — | — |
| **2** | **scan** (parallel) | `scans/*.json` | **M** | ← System |
| 2.1 | лифт tree-sitter сканеров | — | M | ← types |
| 2.2 | extract-then-link: `unresolved_refs` на файл `[codegraph]` | — | M | → graph |
| 2.3 | rayon + ordered-commit (воспроизводимость) `[codegraph]` | — | M | — |
| 2.4 | `run(System) -> ScanResult` + CLI сабкоманда | `.fdml/scans/*.json` | M | → graph, integrations |
| 2.5 | тест: фикстура → ожидаемые узлы + refs | — | S | — |
| **3** | **graph** ⭐ длинный шест | `graph.json` | **XL** | ← scans |
| 3.1 | import-resolver **TS/Py/Go/C#** (порт `[U-A]`) | — | L | C/C++ → фаза 9 |
| 3.2 | ref-каскад `matchReference` (порт `[codegraph]`) | — | M | ← 2.2 unresolved_refs |
| 3.3 | `petgraph` + provenance-рёбра | — | S | крейт |
| 3.4 | community detection (крейт — **спайк #1**) | — | M | → 3.5 |
| 3.5 | группировка `coverage×штраф` + super-cluster **5–8** + orphan-cascade (порт `[CodeBoarding]`) | — | M | ← 3.4 |
| 3.6 | имена кластеров **без LLM** (dominant file / central node `[codegraph]`) | — | S | — |
| 3.7 | `run(&[ScanResult]) -> CodeGraph` + CLI | `.fdml/graph.json` | — | → flows, assemble |
| 3.8 | тест: детерминизм (fixed seed) + 100% coverage | — | S | — |
| **4** | **flows** (rewrite) | `flows.json` | **S–M** | ← graph |
| 4.1 | topo-sort tour (Kahn, O(V+E)) `[U-A]` — **выкинуть all-paths BFS** `[FDML bug]` | — | S–M | ← CodeGraph |
| 4.2 | сохранить ingress/sink эвристики | — | S | — |
| 4.3 | `run(&CodeGraph) -> Vec<Flow>` + CLI | `.fdml/flows.json` | — | → assemble |
| 4.4 | **перф-тест**: клика 200 узлов < 50мс | — | S | gate |
| **5** | **integrations** (parallel пути) | `integrations.json` | **M** | ← scans |
| 5.1 | route→handler manifest `[codegraph]` | — | M | ← ScanResult |
| 5.2 | dyn-dispatch synthesizers (fan-out-capped) `[codegraph]` | — | M | — |
| 5.3 | non-code парсеры docker/k8s/sql/proto `[U-A]` | — | M | — |
| 5.4 | shared-entities через HashMap — **фикс O(n²)** `[FDML bug]` | — | S | — |
| 5.5 | `run(&[ScanResult]) -> Vec<Integration>` + CLI | `.fdml/integrations.json` | — | → assemble |
| **6** | **assemble** | `spec.fdml` | **S–M** | ← 3,4,5 |
| 6.1 | лифт `assemble.rs` на новые graph/flows | — | S–M | — |
| 6.2 | health-метрики cohesion/coupling/circular `[CodeBoarding]` | — | S | → viewer |
| 6.3 | `run(...) -> FdmlSpec`, проходит `fdml validate` | `.fdml/spec.fdml` | — | → cli, viewer |
| 6.4 | golden-file тест | — | S | — |
| **7** | **CLI + cutover** | один бинарник | **M** | ← все |
| 7.1 | реальный пайплайн вместо `todo!()`, `--no-llm` дефолт | — | M | — |
| 7.2 | прогресс-вывод | — | S | — |
| 7.3 | прогон на реальном проекте, сверка со старым выводом | — | M | gate |
| 7.4 | удалить мигрированный старый `src/` | — | S | — |
| **8** | **позже (не v1)** | — | — | вне hot path |
| 8.1 | `fdml-enrich` (ML, opt-in): описания / BDD / прозвища | — | — | ядро не импортит |
| 8.2 | `discovery --gen-rules`: LLM **разово** генерит `discovery-rules.yaml` | rules-файл | M | → 1.3 |
| 8.3 | SQLite store (инкрементальность, запросы viewer) `[codegraph]` | — | — | — |
| 8.4 | incremental: structural fingerprint → SKIP/PARTIAL/FULL `[U-A]` | — | — | ← 8.3 |
| 8.5 | viewer hookup (fuzzy `nucleo` + ELK) `[U-A]` | — | — | ← 6.3 |
| **9** | **отложено: C/C++ resolver** | — | — | ← 3.1 |
| 9.1 | эвристика `#include` (quoted=relative+basename, angle=external) | — | M | — |
| 9.2 | потом: `compile_commands.json` -I, `#ifdef` | — | — | по кейсу |

---

## Порядок сборки — вертикаль сначала (ленивый)

```
types → discovery → scan → graph(минимальный: только рёбра) → flows → assemble → cli
        = РАБОТАЮЩИЙ end-to-end срез
потом углубляем graph: 3.4 кластеры → 3.5 группировка → 3.6 имена
```
Сначала тонкая вертикаль даёт прогон целиком → интеграция проверена до того, как вложим XL в алгоритмы графа.

## Критический путь и параллель

- **Крит. путь:** `types → scan → graph → flows → assemble → cli`. Длинный шест = **graph (3)**.
- **Параллельно пути:** discovery (1) и integrations (5) — зависят только от types/scan.

## Артефакты (что на выходе каждого шага)

```
.fdml/
  systems.json        ← discovery
  scans/*.json        ← scan
  graph.json          ← graph
  flows.json          ← flows
  integrations.json   ← integrations
  spec.fdml           ← assemble
discovery-rules.yaml  → вход discovery (опц., дефолт встроенный)
```
`fdml run ./proj` — всё в памяти, **без артефактов** (быстрый путь). Сабкоманды + `--workdir` — для дебага/резюма по шагам.

## Захваченные идеи → фаза 3 (graph)

- **Role-теги модулей** (идея Nik + layer-detection `[U-A]`/`[CodeBoarding]`): тег архитектурной
  роли на модуль/кластер, детерминированно из dir-паттернов + import-сигналов + имён. Стартовый
  набор: `api`, `domain`/`business`, `data-access`, `integration`, `infra`/`config`, `ui`,
  `model`/`dto`, `util`, `test`. Многотеговый. Консолидирует ingress/sink-эвристики флоу + метки
  кластеров. Coarse-ось `tech` vs `business` поверх. `kind` отдельный НЕ нужен — `system_type` уже
  системный kind (можно расширить словарь: `cli`/`desktop`/`mobile`).
- **Shared-entities как field-level map + union-find** `[CQL]`: вместо симметричного fuzzy-match по
  имени — типизированное направленное соответствие полей + слияние union-find (= colimit
  детерминированно, без теории категорий) + проверка транзитивности A↔B,B↔C⇒A↔C. Фаза 3
  (shared-entities) / фаза 5 (task 5.4). Деталь: `research/repo-studies/04-cql-categorical.md`.
- **Path-equations → будущий constraints-слой** `[CQL-paper]`: детерминированная модель «два пути
  отношений от одной сущности должны совпадать» (rewrite до нормальной формы, без instances/
  theorem-prover). НО FDML сейчас constraints ниоткуда не берёт (читает код, не авторит инварианты) —
  паркуем как design-note; триггер = появление authoring-поверхности спека. 0 строк кода сейчас.
  Деталь: `research/repo-studies/05-cql-paper.md`.

## Спайки (закрыть при оценке)

1. Louvain/Leiden крейт под Rust? Иначе хенд-ролл ~50–80 строк на `petgraph`. (полчаса разведки)
2. Насколько чисто лифтятся сканеры из `src` (связь со старыми типами)? — размер фазы 2.
3. ✅ **закрыт:** резолвер день-один = TS/Py/Go/C#; C/C++ → фаза 9.
