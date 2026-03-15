# FDML 1.4 Specification

**Feature-Driven Modeling Language — Platform Architecture Level**

Version: 1.4
Status: Stable

---

## Overview

FDML 1.4 extends the single-system FDML spec with **platform-level constructs** for modeling multi-system architectures. It introduces contours, system entries, integrations, cross-system flows, and shared entities — allowing a single `.fdml` file to describe an entire platform while linking to per-system specs.

FDML 1.4 is a **superset** of the single-system spec. All existing keys (`entities`, `actions`, `features`, `flows`, `constraints`, `traceability`, `generation_rules`) remain valid.

---

## Document Structure

A FDML 1.4 platform spec is a YAML document with these top-level keys:

```yaml
metadata:         # Required
contours:         # Platform-level: trust boundaries
systems:          # Platform-level: system registry
integrations:     # Platform-level: connections between systems
cross_flows:      # Platform-level: flows spanning systems
shared_entities:  # Platform-level: entities replicated across systems

# Single-system keys (also valid at top level):
system:           # System identity
entities:         # Domain entities
actions:          # Domain actions
features:         # BDD features with scenarios
flows:            # Intra-system flows
constraints:      # Business rules
traceability:     # Links between artifacts
generation_rules: # Code generation rules
```

---

## metadata

Required. Identifies the spec version and document.

```yaml
metadata:
  version: "1.4"
  name: "My Platform"
  author: "Team Name"               # optional
  description: "Platform overview"   # optional
  created: "2026-01-15"             # optional, ISO 8601
  updated: "2026-03-15"             # optional, ISO 8601
```

| Field       | Type   | Required | Description                      |
|-------------|--------|----------|----------------------------------|
| version     | string | yes      | Spec version, `"1.4"`           |
| name        | string | no       | Human-readable name              |
| author      | string | no       | Author or team                   |
| description | string | no       | Brief description                |
| created     | string | no       | Creation date (ISO 8601)         |
| updated     | string | no       | Last update date (ISO 8601)      |

---

## contours

Trust/exposure boundaries that group systems by security posture.

```yaml
contours:
  - id: external
    name: External
    description: User-facing frontends and public APIs
    trust_level: public

  - id: integration
    name: Integration
    description: API gateways and BFFs mediating between frontends and core services
    trust_level: internal

  - id: core
    name: Core
    description: Business logic microservices
    trust_level: restricted

  - id: infrastructure
    name: Infrastructure
    description: Databases, message brokers, caches
    trust_level: critical
```

| Field       | Type   | Required | Description                                          |
|-------------|--------|----------|------------------------------------------------------|
| id          | string | yes      | Unique identifier                                    |
| name        | string | yes      | Display name                                         |
| description | string | no       | What this contour represents                         |
| trust_level | string | no       | `public` \| `internal` \| `restricted` \| `critical` |

Standard contour IDs: `external`, `integration`, `core`, `infrastructure`.

---

## systems

Registry of all systems in the platform.

```yaml
systems:
  - id: web_app
    name: Web Application
    type: frontend
    technology: React + TypeScript + Vite
    contour: external
    owner: frontend-team
    description: Main user-facing SPA
    spec: web_app.fdml
    components:
      - dashboard
      - settings
    relationships:
      - from: web_app
        to: api_gateway
        type: http
```

| Field         | Type              | Required | Description                                        |
|---------------|-------------------|----------|----------------------------------------------------|
| id            | string            | yes      | Unique system identifier                           |
| name          | string            | yes      | Display name                                       |
| type          | string            | yes      | `frontend` \| `service` \| `worker` \| `gateway`  |
| technology    | string            | no       | Tech stack summary                                 |
| contour       | string            | no       | ID of the contour this system belongs to           |
| owner         | string            | no       | Team or person responsible                         |
| description   | string            | no       | What this system does                              |
| spec          | string            | no       | Path to per-system `.fdml` file (relative to platform spec) |
| components    | string[]          | no       | Sub-components within this system                  |
| relationships | Relationship[]    | no       | Direct relationships to other systems              |

### System Types

| Type       | Description                                          |
|------------|------------------------------------------------------|
| `frontend` | User-facing application (web, mobile, desktop)       |
| `service`  | Backend service with API (REST, GraphQL, gRPC)       |
| `worker`   | Background processor, job runner, orchestrator       |
| `gateway`  | API gateway, BFF, reverse proxy                      |

### Relationship (inline)

```yaml
relationships:
  - from: web_app
    to: api_gateway
    type: http
    description: REST API calls
```

| Field       | Type   | Required | Description        |
|-------------|--------|----------|--------------------|
| from        | string | yes      | Source system ID    |
| to          | string | yes      | Target system ID   |
| type        | string | yes      | Connection type    |
| description | string | no       | What flows between |

---

## integrations

Explicit connections between systems with protocol details and endpoints.

```yaml
integrations:
  - id: web_to_gateway
    from: web_app
    to: api_gateway
    type: http
    protocol: REST/JSON
    description: Frontend API calls
    async: false
    endpoints:
      - method: GET
        path: /api/users
        description: List users with pagination
      - method: POST
        path: /api/users
        description: Create new user
    data_entities:
      - User
      - Profile

  - id: gateway_to_events
    from: api_gateway
    to: event_service
    type: event
    protocol: Redis pub/sub
    async: true
    channels:
      - user.created
      - user.updated
```

| Field         | Type                   | Required | Description                                                                  |
|---------------|------------------------|----------|------------------------------------------------------------------------------|
| id            | string                 | yes      | Unique integration identifier                                               |
| from          | string                 | yes      | Source system ID                                                             |
| to            | string                 | yes      | Target system ID                                                             |
| type          | string                 | yes      | `http` \| `grpc` \| `event` \| `queue` \| `shared_db` \| `websocket`       |
| protocol      | string                 | no       | Specific protocol (e.g., `REST/JSON`, `Redis pub/sub`, `PostgreSQL`)        |
| description   | string                 | no       | What this integration does                                                   |
| async         | bool                   | no       | Whether communication is asynchronous (default: `false`)                     |
| endpoints     | IntegrationEndpoint[]  | no       | HTTP/gRPC endpoints                                                          |
| channels      | string[]               | no       | Event/message channels or topics                                             |
| data_entities | string[]               | no       | Entity names that flow through this integration                              |

### Integration Types

| Type        | Description                                      | Typical protocols              |
|-------------|--------------------------------------------------|--------------------------------|
| `http`      | Synchronous HTTP API calls                       | REST/JSON, REST/XML            |
| `grpc`      | gRPC remote procedure calls                      | Protocol Buffers               |
| `event`     | Pub/sub event streaming                          | Redis pub/sub, Kafka, NATS     |
| `queue`     | Message queue                                    | RabbitMQ, SQS, Celery          |
| `shared_db` | Direct database access from multiple systems    | PostgreSQL, MongoDB, Redis     |
| `websocket` | Persistent bidirectional connection              | WebSocket, Socket.IO, SSE      |

### IntegrationEndpoint

```yaml
endpoints:
  - method: GET
    path: /api/resource/{id}
    description: Fetch resource by ID
```

| Field       | Type   | Required | Description            |
|-------------|--------|----------|------------------------|
| method      | string | yes      | HTTP method or RPC name |
| path        | string | yes      | URL path or RPC path    |
| description | string | no       | What this endpoint does  |

---

## cross_flows

Business processes that span multiple systems. Each flow is a sequence of steps executed across different systems, connected by integrations.

```yaml
cross_flows:
  - id: user_registration
    name: User Registration
    description: End-to-end flow from signup to welcome email
    trigger: User submits registration form
    steps:
      - id: submit_form
        system: web_app
        action: submit_registration
        description: User fills and submits the registration form

      - id: validate_and_create
        system: user_service
        action: create_user
        description: Validate input, create user record, hash password
        integration: web_to_user_service
        on_success: send_welcome
        on_failure: return_error

      - id: send_welcome
        system: notification_service
        action: send_email
        description: Send welcome email with verification link
        integration: user_to_notifications
```

| Field       | Type            | Required | Description                              |
|-------------|-----------------|----------|------------------------------------------|
| id          | string          | yes      | Unique flow identifier                   |
| name        | string          | yes      | Display name                             |
| description | string          | no       | What this flow accomplishes              |
| trigger     | string          | no       | What initiates this flow                 |
| steps       | CrossFlowStep[] | yes      | Ordered sequence of steps                |

### CrossFlowStep

| Field       | Type   | Required | Description                                    |
|-------------|--------|----------|------------------------------------------------|
| id          | string | yes      | Unique step identifier                         |
| system      | string | yes      | System ID that handles this step               |
| action      | string | no       | Action ID within that system                   |
| description | string | no       | What happens in this step                      |
| integration | string | no       | Integration ID used to reach this system       |
| on_success  | string | no       | Step ID to proceed to on success               |
| on_failure  | string | no       | Step ID to proceed to on failure               |

---

## shared_entities

Entities that exist across multiple systems with different representations. Tracks the canonical source and how each system projects or caches the entity.

```yaml
shared_entities:
  - entity: User
    name: User
    description: Core user identity shared across the platform
    canonical_system: user_service
    fields:
      - name: id
        type: uuid
      - name: email
        type: string
      - name: name
        type: string
      - name: role
        type: enum
    contexts:
      - system: user_service
        entity_id: user
        role: source
        fields: [id, email, name, role, password_hash, created_at, updated_at]
        notes: Full user record with auth data

      - system: web_app
        entity_id: user_profile
        role: projection
        fields: [id, email, name, role]
        notes: Read-only profile view, no sensitive fields

      - system: notification_service
        entity_id: recipient
        role: cache
        fields: [id, email, name]
        notes: Cached for email delivery
```

| Field            | Type                  | Required | Description                                  |
|------------------|-----------------------|----------|----------------------------------------------|
| entity           | string                | yes      | Canonical entity name                        |
| name             | string                | no       | Display name                                 |
| description      | string                | no       | What this entity represents                  |
| canonical_system | string                | no       | System ID that owns the source of truth      |
| fields           | SharedEntityField[]   | no       | Canonical field definitions                  |
| contexts         | SharedEntityContext[] | no       | How each system uses this entity             |

### SharedEntityField

| Field | Type   | Required | Description    |
|-------|--------|----------|----------------|
| name  | string | yes      | Field name     |
| type  | string | yes      | Field type     |

### SharedEntityContext

| Field     | Type     | Required | Description                                       |
|-----------|----------|----------|---------------------------------------------------|
| system    | string   | yes      | System ID                                         |
| entity_id | string  | yes      | Local entity ID within that system                |
| role      | string   | no       | `source` \| `projection` \| `cache`              |
| fields    | string[] | no       | Which fields this system uses                     |
| notes     | string   | no       | Additional context                                |

### Shared Entity Roles

| Role         | Description                                              |
|--------------|----------------------------------------------------------|
| `source`     | System of record — owns writes, full field set           |
| `projection` | Read-only view with subset of fields                     |
| `cache`      | Cached copy for performance, may be stale                |

---

## Single-System Keys

These keys are part of the base FDML spec (pre-1.4) and remain fully supported. In a platform spec, they can appear at the top level for platform-wide definitions, or inline within each system's per-system `.fdml` file.

### system

```yaml
system:
  id: my_service
  name: My Service
  description: Service description
  components: [api, workers, scheduler]
  relationships:
    - from: my_service
      to: database
      type: shared_db
```

### entities

```yaml
entities:
  - id: user
    name: User
    description: Registered user account
    fields:
      - name: id
        type: uuid
        required: true
      - name: email
        type: string
        required: true
        constraints:
          - type: format
            value: email
            message: Must be a valid email
      - name: status
        type: enum
        default: active
    relationships:
      - entity: order
        type: has_many
        description: User's orders
```

### actions

```yaml
actions:
  - id: create_user
    name: Create User
    description: Register a new user
    input:
      entity: user
      fields: [email, name, password]
    output:
      entity: user
      fields: [id, email, name, status]
    preconditions:
      - Email must not already exist
    postconditions:
      - User record created with status=active
      - Welcome email queued
    side_effects:
      - Send welcome email
      - Emit user.created event
```

### features

```yaml
features:
  - id: user_registration
    title: User Registration
    description: Allow new users to create an account
    scenarios:
      - id: successful_registration
        title: Successful registration with valid data
        given:
          - User is on the registration page
          - Email "test@example.com" is not taken
        when:
          - User fills in name, email, and password
          - User clicks Register
        then:
          - Account is created with status active
          - Welcome email is sent
          - User is redirected to dashboard
    acceptance_criteria:
      - Email must be unique
      - Password must be at least 8 characters
    dependencies:
      - create_user
```

### flows

```yaml
flows:
  - id: checkout_flow
    name: Checkout Flow
    description: From cart to order confirmation
    steps:
      - id: validate_cart
        action: validate_cart
        description: Check stock and pricing
      - id: process_payment
        action: charge_payment
        description: Charge payment method
        conditions:
          - Cart is valid
      - id: create_order
        action: create_order
        description: Create order record
```

### constraints

```yaml
constraints:
  - id: unique_email
    name: Unique Email
    type: uniqueness
    rule: "entity.User.email must be unique across all records"
    entities: [user]
    actions: [create_user, update_user]
```

### traceability

```yaml
traceability:
  - from: feature:user_registration
    to: action:create_user
    relation: implements
  - from: action:create_user
    to: entity:user
    relation: modifies
```

### generation_rules

```yaml
generation_rules:
  - id: crud_api
    name: CRUD API Generator
    description: Generate REST endpoints for entity CRUD
    triggers: [entity:*]
    generates: [api_endpoint, model, migration]
    template: templates/crud.hbs
```

---

## Per-System Specs

A platform spec can reference external per-system `.fdml` files via the `spec` field on system entries:

```yaml
systems:
  - id: user_service
    name: User Service
    type: service
    spec: user_service.fdml    # relative path
```

The per-system file is a standard FDML document with `entities`, `actions`, `features`, etc. The viewer loads it on drill-down.

File resolution order:
1. Exact path: `<base_dir>/<spec>`
2. Platform prefix: `<platform_name>.<spec>`
3. System ID: `<system_id>.fdml`
4. Platform + system: `<platform_name>.<system_id>.fdml`

---

## Validation Rules

The FDML validator enforces:

| Rule | Description |
|------|-------------|
| Contour IDs unique | No duplicate contour IDs |
| System IDs unique | No duplicate system IDs |
| Integration IDs unique | No duplicate integration IDs |
| Integration references valid | `from` and `to` must reference existing system IDs |
| Integration type valid | Must be one of: `http`, `grpc`, `event`, `queue`, `shared_db`, `websocket` |
| Cross-flow step systems valid | Each step's `system` must reference an existing system ID |
| Shared entity systems valid | Each context's `system` must reference an existing system ID |

---

## CLI Support

### Generate a platform spec from code

```bash
# Scan a multi-system project and output detection results
fdml scan-platform ./my-platform --format yaml

# Generate via LLM
fdml scan-platform ./my-platform --llm --output platform.fdml

# Use fast model
fdml scan-platform ./my-platform --llm --fast

# Specify model
fdml scan-platform ./my-platform --llm --model claude-sonnet-4-20250514
```

### Serve and visualize

```bash
# View platform spec with Architecture view
fdml serve platform.fdml

# Generate from code and view live
fdml serve platform.fdml --generate ./my-platform

# Parallel LLM calls for faster generation
fdml serve platform.fdml --generate ./my-platform --parallel 4
```

---

## Full Example

```yaml
metadata:
  version: "1.4"
  name: E-Commerce Platform

contours:
  - id: external
    name: External
    description: User-facing applications
    trust_level: public
  - id: core
    name: Core
    description: Business logic services
    trust_level: restricted

systems:
  - id: storefront
    name: Storefront
    type: frontend
    technology: React + TypeScript
    contour: external
    spec: storefront.fdml

  - id: order_service
    name: Order Service
    type: service
    technology: Python + FastAPI
    contour: core
    spec: order_service.fdml

  - id: payment_service
    name: Payment Service
    type: service
    technology: Go
    contour: core

integrations:
  - id: storefront_to_orders
    from: storefront
    to: order_service
    type: http
    protocol: REST/JSON
    endpoints:
      - method: POST
        path: /api/orders
        description: Place a new order
      - method: GET
        path: /api/orders/{id}
        description: Get order status

  - id: orders_to_payments
    from: order_service
    to: payment_service
    type: http
    protocol: REST/JSON
    async: false
    endpoints:
      - method: POST
        path: /api/charges
        description: Charge customer payment method

cross_flows:
  - id: place_order
    name: Place Order
    trigger: Customer clicks "Buy Now"
    steps:
      - id: submit
        system: storefront
        action: submit_order
        description: Collect cart items and shipping info
      - id: create
        system: order_service
        action: create_order
        description: Validate and persist order
        integration: storefront_to_orders
        on_success: charge
      - id: charge
        system: payment_service
        action: process_payment
        description: Charge payment method
        integration: orders_to_payments

shared_entities:
  - entity: Order
    canonical_system: order_service
    fields:
      - name: id
        type: uuid
      - name: status
        type: enum
      - name: total
        type: decimal
      - name: items
        type: array
    contexts:
      - system: order_service
        entity_id: order
        role: source
        fields: [id, status, total, items, customer_id, shipping, created_at]
      - system: storefront
        entity_id: order_summary
        role: projection
        fields: [id, status, total]
      - system: payment_service
        entity_id: payment_order
        role: projection
        fields: [id, total]
        notes: Only needs ID and amount for charging
```
