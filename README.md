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

This isn't just documentation. It's **executable specification** that generates:
- ✅ API endpoints
- ✅ Database migrations  
- ✅ Test suites
- ✅ Type definitions
- ✅ Documentation

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

- **Indie Hackers**: Ship features in hours, not weeks. No team required.
- **AI-First Teams**: Let LLMs understand and generate features directly
- **Remote Teams**: Async-friendly specifications that work across timezones
- **Solo Founders**: Build like a team of 10 with automated generation

## 🗺️ Roadmap

### Phase 1: Documentation & Specification ✅
- [x] Core FDML v1.3 specification
- [x] Traceability extension (v1.3.1)
- [x] Migration system (v1.3.2)
- [ ] Real-world examples and case studies
- [ ] Best practices guide

### Phase 2: Core Toolset 🚧
- [ ] FDML Parser (TypeScript/Rust)
- [ ] CLI tool for validation
- [ ] Code generators:
  - [ ] TypeScript/JavaScript
  - [ ] Python
  - [ ] Go
- [ ] Test suite generator
- [ ] Migration runner

📋 **Detailed Implementation Plan**: [CLI Tools Roadmap →](CLI_TOOLS_ROADMAP.md)

### Phase 3: VS Code Extension 🔮
- [ ] Syntax highlighting & IntelliSense
- [ ] Real-time validation
- [ ] Feature preview
- [ ] One-click generation
- [ ] Integrated debugging

### Phase 4: LLM-First Integration 🤖
- [ ] AI copilot for feature writing
- [ ] Natural language → FDML conversion
- [ ] Smart suggestions based on existing features
- [ ] Automated test scenario generation
- [ ] GPT-4/Claude native support

### Phase 5: Ecosystem 🌍
- [ ] GitHub Actions integration
- [ ] Package registry for shared features
- [ ] Cloud playground
- [ ] Community templates

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

## 📊 Why FDML Will Win

- **Clear Specifications**: No more "what did the PM mean by this?"
- **Automated Generation**: Write features once, get code everywhere  
- **Built-in Testing**: Every feature comes with tests by design
- **AI-Ready**: LLMs can understand and extend your features

## 🚦 Getting Started

1. **Read the Spec**: [FDML Specification v1.3](FDML-1.3-en.md)
2. **CLI Tools Development**: [CLI Tools Roadmap](CLI_TOOLS_ROADMAP.md) | [Technical Spec](CLI_TOOLS_TECHNICAL_SPEC.md) | [Quick Reference](CLI_TOOLS_QUICK_REFERENCE.md)
3. **Follow Development**: [Twitter](https://twitter.com/KolanskiNik)

## 💰 Build Your Competitive Moat

While others are stuck in meetings debating requirements, you're shipping features. While they're fixing bugs from miscommunication, you're acquiring users.

FDML isn't just a language. It's your **unfair advantage**.

## 🤝 Contributing

FDML is open source and we love contributors! Check out our [Contributing Guide](CONTRIBUTING.md).

## 📝 License

MIT License - go build something amazing!

---

<p align="center">
  <b>Stop translating. Start shipping.</b><br>
  <a href="https://fdml.dev">fdml.dev</a>
</p>