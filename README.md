# FDML: Feature-Driven Modeling Language

> Transform vibes into features. Ship code that matters.

[![Version](https://img.shields.io/badge/version-1.3-blue.svg)](https://github.com/yourusername/fdml)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Indie Hacker Approved](https://img.shields.io/badge/Indie%20Hacker-Approved-orange.svg)](https://www.indiehackers.com)

## 🚀 What is FDML?

FDML (Feature-Driven Modeling Language) is a domain-specific language that bridges the gap between "I have this idea" and "It's deployed to production." 

Instead of translating business requirements through layers of meetings, tickets, and miscommunication, FDML lets you **write features as code** that both humans and machines understand.

```gherkin
Feature: User can monetize their content
  Scenario: Creator enables paid subscriptions
    Given creator has verified account
    When they set subscription price to $9.99/month
    Then subscribers are charged monthly
    And creator receives 90% revenue share
```

This isn't just documentation. It's **structured specification** that enables:
- ✅ Clear feature definitions
- ✅ Validation and consistency checking
- ✅ Team alignment and communication
- ✅ Foundation for future tooling
- ✅ Traceability and documentation

## 💡 Why FDML?

### The Problem

Every startup dies the same death: **the gap between what users want and what gets built**.

- Product managers write requirements in Notion
- Engineers interpret them differently  
- QA tests something else entirely
- Users get... whatever that is

### The Solution

FDML makes your feature definitions the **single source of truth**:

```yaml
entity:
  id: subscription
  fields:
    - name: price
      type: float
      constraints: 
        - min_value: 0.99
        - max_value: 999.99
    - name: creator_revenue_share
      type: float
      default: 0.90
```

From this, FDML generates everything. No ambiguity. No telephone game. Just features.

## 🎯 Perfect For

- **Indie Hackers**: Define and validate features quickly with clear specifications
- **Development Teams**: Maintain single source of truth for feature requirements
- **Remote Teams**: Async-friendly specifications that work across timezones
- **Solo Founders**: Organize and structure your product features systematically

## 🗺️ Roadmap

> **Note**: For detailed technical roadmap and CLI command specifications, see [ROADMAP.md](ROADMAP.md)

### Phase 1: Foundation & Core Parser (Weeks 1-4) ✅
- [x] Core FDML v1.3 specification
- [x] Traceability extension (v1.3.1)
- [x] Migration system (v1.3.2)
- [x] System design principles integration (v1.3.3)
- [ ] FDML Parser implementation (Rust/TypeScript)
- [ ] Abstract Syntax Tree (AST) construction
- [ ] Basic CLI commands (`fdml init`, `fdml validate`)
- [ ] Error handling and user-friendly messages
- [ ] Project architecture foundation

### Phase 2: Validation & Project Structure (Weeks 5-8) 🚧
- [ ] FDML specification validation system
- [ ] Feature management commands (`fdml feature add`, `fdml feature list`)
- [ ] Project directory structure creation and management
- [ ] Integrity checking (`fdml check`)
- [ ] Advanced error handling and diagnostics
- [ ] Real-world examples and case studies

### Phase 3: LSP Foundation (Weeks 9-12) 🔮
- [ ] Language Server Protocol architecture preparation
- [ ] Real-time diagnostics and validation
- [ ] FDML syntax highlighting support
- [ ] Feature navigation (Go to Definition)
- [ ] Basic autocomplete foundations
- [ ] Testing and documentation

### Phase 4: VSCode Integration (Weeks 13-16) 🤖
- [ ] VSCode extension development
- [ ] LSP server integration
- [ ] Advanced autocomplete and IntelliSense
- [ ] Feature structure visualization
- [ ] Debugging and optimization
- [ ] Extension marketplace publishing

### Phase 5: Future Enhancements 🌍
- [ ] LLM integration for code generation
- [ ] AI copilot for feature writing
- [ ] GitHub Actions integration
- [ ] Package registry for shared features
- [ ] Multi-language code generators (when mature)

## 🔥 Killer Features

### 1. **Traceability Built-In**
Every line of code traces back to a business requirement. Know exactly why every function exists.

```yaml
traceability:
  from: user_signup_scenario
  to: POST_/api/auth/signup
  relation: implements
```

### 2. **Migration-First Development**
Changes are migrations, not overwrites. Roll back features as easily as database schemas.

```yaml
migration:
  id: 2024_01_15_add_oauth
  up:
    - add_feature:
        id: oauth_login
        title: Login with Google
  down:
    - remove_feature:
        id: oauth_login
```

### 3. **AI-Native Design**
LLMs can read and write FDML natively. Your AI coding assistant becomes a feature-shipping machine.

### 4. **Type-Safe by Default**
Strong typing from specification to implementation. If it compiles, it works.

### 5. **System Design Principles Built-In** (New in v1.3.3)
Integrate proven architectural patterns directly into your specifications. No more guessing how to implement features efficiently.

```yaml
system:
  id: payment_system
  design_principles:
    efficiency: [Sc, Op]     # Scalable, Optimistic design
    reliability: [Ft, At]    # Fault Tolerant, Atomic operations
    security: [Lp, Ac]       # Least Privilege, Access Control
  
  implementation_strategies:
    - principle: Op  # Optimistic Design
      strategy: "Process payments optimistically, handle failures async"
    - principle: Ft  # Fault Tolerance
      strategy: "Circuit breaker with exponential backoff"
```

## 📊 Why FDML Will Win

- **Clear Specifications**: No more "what did the PM mean by this?"
- **Structured Validation**: Catch inconsistencies and missing requirements early
- **Built-in Traceability**: Every feature links to business requirements
- **Architectural Guidance**: 40+ proven design principles guide implementation
- **Future-Ready**: Foundation for advanced tooling and automation

## 🚦 Getting Started

1. **Read the Spec**: [FDML Specification v1.3](FDML-1.3-en.md)
   - Including new [System Design Principles Extension (v1.3.3)](fmdl-spec/FDML-1.3.3-extension-en.md)
2. **Try the CLI**: Coming soon - `fdml init`, `fdml validate`, `fdml feature add`
3. **Follow Development**: [Twitter](https://twitter.com/KolanskiNik)

## 💰 Build Your Competitive Moat

While others are stuck in meetings debating requirements, you have clear specifications. While they're fixing bugs from miscommunication, you have validated feature definitions.

FDML isn't just a language. It's your **clarity advantage**.

## 🤝 Contributing

FDML is open source and we love contributors! Check out our [Contributing Guide](CONTRIBUTING.md).

## 📝 License

MIT License - go build something amazing!

---

<p align="center">
  <b>Stop translating. Start shipping.</b><br>
  <a href="https://fdml.dev">fdml.dev</a>
</p>