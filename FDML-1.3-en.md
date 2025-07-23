# FDML Specification v1.3 — Complete Specification

## 1. Introduction

**FDML (Feature-Driven Modeling Language)** is a domain-specific language for formal description of systems, entities, business rules, and user scenarios.  
The goal is to provide a unified platform for communication between business and developers with simultaneous capabilities for test automation and code generation.

FDML combines BDD and DDD ideas, structuring descriptions to:
- be readable for business users;
- be formal and unambiguous for development and automation;
- ensure traceability and extensibility.

---

## 2. Core Concepts and Language Structure

The language is based on a set of key concepts:

| Concept       | Description                                                    |
|---------------|----------------------------------------------------------------|
| system        | Describes high-level system and its components                 |
| entity        | Defines data structure and constraints                          |
| action        | Describes operations and behavior with input and output data   |
| feature       | Defines user requirements and behavioral scenarios             |
| flow          | Defines action sequences (workflow)                            |
| constraint    | Business rules and constraints                                 |
| traceability  | Links and traceability between elements                        |
| generation_rule | Generation rules and dependencies between elements           |

---

## 3. System

### 3.1 Purpose
Defines the context — a set of interconnected components, subsystems, and their interactions.

### 3.2 Structure

```yaml
system:
  id: string             # Unique system identifier
  name: string           # Human-readable system name
  description: string    # Brief description
  components:            # List of components or subsystems
    - string             # Component identifier
  relationships:         # Component relationship definitions
    - from: string       # Source component
      to: string         # Target component
      type: string       # Relationship type (e.g., "dependency", "data flow")
      description: string # Optional — relationship description
```

### 3.3 Allowed Relationship Types (recommended):

* dependency — dependency
* data\_flow — data flow
* control\_flow — control
* event — event

---

## 4. Entities

### 4.1 Purpose

Define data structure and their constraints.

### 4.2 Syntax

```yaml
entity:
  id: string            # Unique identifier
  name: string          # Entity name
  description: string   # Entity description
  fields:               # Field list
    - name: string      # Field name
      type: <data_type> # Data type
      required: bool    # Is field required
      description: string # Field description
      default: any      # Default value (optional)
      constraints:      # Constraint list (optional)
        - string|dict   # E.g., "unique", {"max_length": 255}
  indexes:              # Indexes for search optimization (optional)
    - fields: [string]  # Fields for indexing
      unique: bool      # Unique index
```

### 4.3 Supported Data Types

| Type     | Description                              | Example                                |
| -------- | ---------------------------------------- | -------------------------------------- |
| string   | String                                   | "text"                                 |
| integer  | Integer                                  | 42                                     |
| float    | Floating point number                    | 3.1415                                 |
| boolean  | Boolean type (true/false)                | true                                   |
| datetime | Date and time in ISO 8601 format         | "2025-07-23T14:25:43Z"                 |
| date     | Date only                                | "2025-07-23"                           |
| array    | Array of elements of specific type       | [1,2,3]                                |
| object   | Nested object (dictionary)               | { "key": "value" }                     |
| enum     | Enumeration of values (strings or numbers)| "RED", "GREEN", "BLUE"                 |
| uuid     | Unique identifier (UUID)                 | "550e8400-e29b-41d4-a716-446655440000" |

### 4.4 Field Constraints

* **unique** — value uniqueness within entity
* **max\_length** — maximum string length
* **min\_length** — minimum string length
* **max\_value** — maximum numeric value
* **min\_value** — minimum numeric value
* **pattern** — regular expression for strings
* **nullable** — allows null (false by default)
* **default** — default value

---

## 5. Actions

### 5.1 Purpose

Define behavior and operations on entities or the system.

### 5.2 Structure

```yaml
action:
  id: string
  name: string
  description: string
  input:                   # Input parameters
    - name: string
      type: <data_type>
      required: bool
  output:                  # Output data
    - name: string
      type: <data_type>
  logic: string            # Pseudocode, algorithm description, or implementation reference
  exceptions:              # Exceptions/errors that may occur
    - code: string
      message: string
```

### 5.3 Example

```yaml
action:
  id: create_candidate
  name: "Create candidate"
  description: "Adding a new candidate to the system"
  input:
    - name: candidate_data
      type: object
      required: true
  output:
    - name: success
      type: boolean
  logic: |
    Check email uniqueness
    Save candidate data
    Return success=true
  exceptions:
    - code: "ERR_DUPLICATE_EMAIL"
      message: "Email already in use"
```

---

## 6. Features

### 6.1 Purpose

Define user requirements and expected system behavior.

### 6.2 Structure

BDD-style using keywords:

```gherkin
Feature: <Feature name>
  Description: <Description>

  Scenario: <Scenario name>
    Given <initial state>
    When <action>
    Then <expected result>
    And <additional conditions>
```

### 6.3 Writing Rules

* Use clear, readable language as close to business domain as possible.
* Do not mention technical details or UI elements.
* Each scenario should be independent and focus on single business logic.

---

## 7. Flows

### 7.1 Purpose

Define action sequences and possible transitions.

### 7.2 Syntax

```yaml
flow:
  id: string
  name: string
  steps:
    - action_id: string              # Action identifier to execute
      on_success_action_id: string   # Next action ID on successful execution
      on_failure_action_id: string   # Next action ID on error (optional)
```

---

## 8. Constraints

### 8.1 Purpose

Business rules that apply to entities, actions, or features.

### 8.2 Structure

```yaml
constraint:
  id: string
  name: string
  description: string
  applies_to:                # What it applies to: entity_id, action_id, feature_id
    - string
  condition: string          # Condition in DSL (expression)
  message: string            # Error message on violation
```

### 8.3 Example

```yaml
constraint:
  id: unique_email
  name: "Email uniqueness"
  description: "Candidate email must be unique"
  applies_to:
    - candidate
  condition: "email is unique"
  message: "Email already in use"
```

---

## 9. Traceability

### 9.1 Purpose

Provides links and tracking between requirements, actions, entities, and tests.

### 9.2 Syntax

```yaml
traceability:
  from: string      # Source element ID (e.g., feature)
  to: string        # Linked element ID (e.g., action)
  relation: string  # Relationship type (implements, verifies, depends_on)
```

---

## 10. Generation Rules and Dependencies

### 10.1 Generation Rules

Define automatic transformation or element creation.

```yaml
generation_rule:
  target: string         # Element ID
  rule: string           # Generation rule description
```

### 10.2 Feature Dependencies

Define which features must be implemented first.

```yaml
feature_dependency:
  feature: string        # ID of dependent feature
  depends_on: string     # ID of feature it depends on
```

---

## 11. Format and File Types

* Main format — YAML for structural descriptions.
* Text BDD features — in Gherkin format (.feature).
* Integration with JSON or XML possible when needed.

---

## 12. Extensions

* Multilingual description support.
* Entity and component nesting capability.
* Extensibility through custom types and annotations.

---

## 13. Storage, Versioning, and Implementation Compliance

### 13.1 Specification Storage

FDML specifications are stored in a format supporting:

- Multi-file structure where each entity, feature, system, etc. is a separate file or set of files.  
- Hierarchical organization (folders, namespaces) reflecting the project's business structure.  
- Standardized formats: YAML for structured entities, Gherkin (.feature) for BDD scenarios, additional formats (JSON, XML) for integrations.  

This facilitates maintenance, automatic parsing, and CI/CD system integration.

### 13.2 Versioning

#### 13.2.1 Core Principles

- Each specification version is fixed with a unique identifier (e.g., `v1.3.0`).  
- Versions reflect changes in model structure, business logic, requirements, or implementation.  
- Changes are accompanied by release notes describing affected entities, added or removed fields, new scenarios, etc.  

#### 13.2.2 Types of Changes

- **Patch (fixes)** — description clarifications not affecting structure.  
- **Minor (non-breaking additions)** — new entities or fields without breaking compatibility.  
- **Major (breaking changes)** — incompatible changes, removal or rework of key elements.  

#### 13.2.3 History Management

- Change history is stored in version control system (Git or similar).  
- Standardized commit messages and pull request templates are used for traceability.  
- Code and specification changes are linked through issue ID, feature ID, and other metadata.

### 13.3 Change Tracking by Entity Types

| Entity           | What to Track                                               |
|------------------|-------------------------------------------------------------|
| system           | Changes in composition, relationships, descriptions          |
| entity           | Field additions/removals, type changes, constraints         |
| action           | Parameter, logic, error changes                             |
| feature          | New or modified scenarios, acceptance criteria              |
| flow             | Sequences, branching                                        |
| constraint       | Rules and conditions                                        |
| traceability     | Links between elements                                      |
| generation_rule  | Generation logic                                            |
| feature_dependency | Dependencies                                              |

When fixing a specification version, a list of changed entities with change descriptions is documented.

### 13.4 Specification and Implementation Connection

#### 13.4.1 Concept

- Specification describes behavior and structure that must be implemented in code.  
- Code — set of classes and methods, must comply with specification requirements.  
- Static and dynamic analysis comparing code and specification is used for compliance verification.

#### 13.4.2 Approaches

- Generate code templates from specifications (interfaces, classes).  
- Test automation with BDD frameworks (Cucumber, Behave).  
- Link code and specification versions through version control system.  
- Requirements traceability through `traceability` elements.

### 13.5 Notes and Recommendations

- Versioning is a mandatory element of the model lifecycle.  
- Change history ensures reproducibility and audit.  
- CI/CD is recommended for specification validation and automated test execution.  
- FDML may be extended in the future with attributes supporting implementation verification (e.g., `implementation_reference`, `test_coverage`).

---

## 14. Extension 1.3.1: Traceability Mechanism via Metadata

### 14.1 Purpose

FDML 1.3 describes feature structure in Gherkin format, allowing business functionality description as scenarios. However, this version lacks structured traceability between:

• features
• actions
• implementations (endpoints, workflows, services)
• business requirements / system requirements

This specification's goal is to introduce a structured external traceability mechanism that doesn't disrupt .feature file readability and supplements them with links through YAML metadata files.

### 14.2 Components

#### 14.2.1 *.feature — main Gherkin file

No changes. Contains feature and scenario descriptions.

Example:

```gherkin
Feature: Article management

  Scenario: Create new article
    Given user is authenticated
    When they send valid data
    Then article is created with ID

  Scenario: Delete article
    Given article exists
    When user sends DELETE
    Then article is deleted
```

#### 14.2.2 *.feature.meta.yaml — feature metadata

Metafile describes structure, identifiers, links, and scenarios. It's placed next to .feature.

Structure:

```yaml
feature:
  id: <unique_identifier>
  name: <feature_name>
  description: <description>
  file: <feature_file_name>
  
  scenarios:
    - id: <unique_scenario_identifier>
      title: <human_readable_title>
      line: <line_number_in_feature>
      action: <action_id>
      traceability:
        - to: <target_object_id>
          relation: <relation_type>  # e.g.: implements, verifies, tests, blocks
```

Example:

```yaml
feature:
  id: article_crud
  name: Article management
  description: Create, view and delete articles
  file: article_crud.feature
  
  scenarios:
    - id: article_create
      title: Create new article
      line: 3
      action: create_article
      traceability:
        - to: BR-12
          relation: implements
        - to: API-POST-/articles
          relation: verifies

    - id: article_delete
      title: Delete article
      line: 8
      action: delete_article
      traceability:
        - to: BR-13
          relation: implements
        - to: API-DELETE-/articles/{id}
          relation: verifies
```

#### 14.2.3 traceability.yaml (optional)

Automatically generated. Can be aggregated from all *.meta.yaml files.

```yaml
traceability:
  - from: article_create
    to: BR-12
    relation: implements
  - from: article_create
    to: API-POST-/articles
    relation: verifies
```

### 14.3 Relation Type Specification

| Relation Type | Purpose                                                               |
|---------------|-----------------------------------------------------------------------|
| implements    | Scenario implements business requirement or functionality              |
| verifies      | Verifies/covers API, workflow, UI                                     |
| tests         | Is an autotest for specified entity                                   |
| blocks        | Blocks execution/release until implemented                            |
| depends_on    | Scenario depends on another implementation or resource                 |

### 14.4 Validation Rules

1. Each .meta.yaml must reference existing .feature through file.
2. Each line must point to actually existing Scenario.
3. Feature and scenario IDs are unique within project.
4. All traceability.to must reference existing action/BR/API (if validation enabled).
5. Metafiles are required only for features with business links or autotests.

### 14.5 Usage Example

1. Automatic coverage matrix generation:
• Which business requirements are covered
• Which endpoints are tested

2. CI/CD validation:
• Check .feature and .meta.yaml correspondence
• Display API/BR coverage

3. Documentation generation:
• For analysts, QA, architects

### 14.6 Possible Utilities

• fdml-trace validate: validates *.meta.yaml
• fdml-trace graph: builds graphviz link diagram
• fdml-trace matrix: CSV or Markdown coverage matrix

### 14.7 Conclusion

This FDML 1.3.1 extension provides:
• End-to-end traceability support
• Structure independence from Gherkin syntax
• Complete machine processing and coverage analysis
• Preservation of familiar scenario readability for the team

This extension is compatible with existing .feature files and can be implemented gradually, starting with most critical business features.

---

## 15. Extension 1.3.2: Migration Mechanism (FDML Migrations)

### 15.1 Extension Purpose

Provide systematic, controlled, and formalized change management mechanism (migrations) within FDML models:

- for features,
- for entities,
- for actions,
- and related components.

This enables:

- capturing changes over time,
- ensuring reversibility (rollback),
- verifying state consistency,
- integrating with code generation system and CI/CD.

### 15.2 General Description

#### 15.2.1 Migration Concept

Migration is an atomic set of declaratively described changes that brings FDML model state from one version to another.

Migrations are stored as files with unique identifiers (e.g., timestamp or semantic number) and have two main parts:

- **up** — operations applying changes;
- **down** — operations reverting changes.

### 15.3 Migration Format

#### 15.3.1 YAML Migration Manifest Structure

```yaml
id: string               # Unique migration identifier (e.g., date and sequence number)
author: string           # Migration author
date: ISO8601            # Migration creation date
description: string      # Brief migration description

up:                     # Operations to apply migration
  - <operation>:        # One of operations from list (see below)
      <params>: ...     

down:                   # Operations to rollback migration (reverse of up)
  - <operation>:
      <params>: ...
```

#### 15.3.2 Supported Migration Operations

| Operation          | Description                                                                               | Example Parameters                                      |
|--------------------|-------------------------------------------------------------------------------------------|---------------------------------------------------------|
| `add_feature`      | Add new feature                                                                           | `id`, `title`, `entity`, `operation`, `roles`          |
| `remove_feature`   | Remove existing feature                                                                   | `id`                                                    |
| `modify_feature`   | Modify existing feature attributes                                                        | `id`, `fields` (specify changed fields)                 |
| `add_entity`       | Add new entity                                                                            | `id`, `fields`                                          |
| `remove_entity`    | Remove entity                                                                             | `id`                                                    |
| `modify_entity`    | Modify entity structure                                                                   | `id`, `fields`                                          |
| `add_action`       | Add action                                                                                | `id`, `description`, `input`, `output`                  |
| `remove_action`    | Remove action                                                                             | `id`                                                    |
| `modify_action`    | Modify action                                                                             | `id`, `fields`                                          |
| `add_constraint`   | Add constraint                                                                            | `id`, `condition`, `applies_to`, `message`              |
| `remove_constraint`| Remove constraint                                                                         | `id`                                                    |
| `modify_constraint`| Modify constraint                                                                         | `id`, `fields`                                          |

### 15.4 Migration Example

```yaml
id: 2025_07_24_001_add_crud_user
author: ara
date: 2025-07-24T12:00:00Z
description: Added CRUD features for User entity

up:
  - add_feature:
      id: user.create
      title: Create user
      entity: user
      operation: create
      roles: [admin]

  - add_feature:
      id: user.read
      title: View user
      entity: user
      operation: read
      roles: [admin, support]

down:
  - remove_feature:
      id: user.create

  - remove_feature:
      id: user.read
```

### 15.5 Migration Management

- Migrations are stored in a separate folder (e.g., `fdml_migrations/`).
- Each migration is a separate YAML file with unique `id`.
- FDML model state is tracked using state file (`fdml_migration_state.json`) storing list of applied migrations.
- Migration management tool (CLI/script) supports commands:
  - `upgrade` — apply all new migrations;
  - `downgrade` — rollback last migration;
  - `status` — show current state and available migrations;
  - `validate` — check model integrity and consistency after migrations.

### 15.6 Interaction with Other FDML Components

- When applying migrations, corresponding `.feature`, `.entity` and other definitions are automatically updated.
- Code and test generation is tied to specific model version corresponding to current migration state.
- Versioning and `traceability` system can use migration data to build dynamic roadmap and audit.

### 15.7 Validation and Integrity

- Migrations must be executed in strict order.
- Circular dependencies and migration skips are prohibited.
- Model consistency check runs before applying migration.
- Migration is rolled back on errors.

### 15.8 Summary

FDML 1.3.2 introduces powerful and transparent model evolution management mechanism based on Alembic-style migrations. This extension:

- improves version control and change history,
- facilitates project maintenance and development,
- ensures integration with generation and testing automation.

---

# Appendix: Data Types Table

| Type     | Value Format                      | Description                             |
| -------- | --------------------------------- | --------------------------------------- |
| string   | String                            | Text data                               |
| integer  | Integer                           | For numeric identifiers, counters       |
| float    | Floating point number             | For decimal numbers                     |
| boolean  | true / false                      | Boolean value                           |
| datetime | ISO 8601 "YYYY-MM-DDTHH:MM:SSZ"  | Date and time                           |
| date     | "YYYY-MM-DD"                      | Date only                               |
| array    | List of same-type elements        | Collection of elements                  |
| object   | Key-value                         | Nested object                           |
| enum     | String from fixed set             | Value enumeration                       |
| uuid     | String in UUID format             | Unique identifier                       |

---

# Conclusion

This specification serves as a comprehensive, formal, and practical guide for describing domain, behavior, and business logic using FDML v1.3.
It is focused on maximum readability, executability, and interdisciplinary collaboration.

The specification includes core version 1.3, as well as extensions:
- 1.3.1 — traceability mechanism through external metadata
- 1.3.2 — migration mechanism for model change management

These extensions provide complete development lifecycle support, from requirements to implementation and maintenance.