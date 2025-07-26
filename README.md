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
- [x] FDML Parser implementation (Rust)
- [x] Abstract Syntax Tree (AST) construction
- [x] Basic CLI commands (`fdml init`, `fdml validate`)
- [x] Error handling and user-friendly messages
- [x] Project architecture foundation

### Phase 2: Core Toolset Implementation (Weeks 5-8) ✅
- [x] Complete CLI with `parse`, `generate`, `migrate`, `trace` commands
- [x] Code generators for TypeScript, Python, and Go
- [x] Automated test generation for all supported languages
- [x] Migration system with apply/rollback/status operations
- [x] FDML specification validation system
- [x] Advanced error handling and diagnostics
- [x] Template-based code generation framework

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

## 📊 Why FDML Will Win

- **Clear Specifications**: No more "what did the PM mean by this?"
- **Structured Validation**: Catch inconsistencies and missing requirements early
- **Built-in Traceability**: Every feature links to business requirements
- **Future-Ready**: Foundation for advanced tooling and automation

## 🚦 Getting Started

1. **Read the Spec**: [FDML Specification v1.3](FDML-1.3-en.md)  
2. **Install the CLI**: `cargo install fdml` (coming soon to crates.io)
3. **Try the CLI**: 
   ```bash
   # Initialize a new project
   fdml init my-project
   
   # Validate FDML files
   fdml validate my-spec.fdml
   
   # Generate TypeScript code
   fdml generate my-spec.fdml --language typescript --output ./generated
   
   # Generate with tests
   fdml generate my-spec.fdml --language python --with-tests
   
   # Apply migrations
   fdml migrate apply --path ./migrations
   ```

### 🛠️ CLI Commands

The FDML CLI provides a comprehensive toolset for working with FDML specifications:

**Core Commands:**
- `fdml init <name>` - Initialize a new FDML project
- `fdml parse <file>` - Parse and display AST (JSON/YAML output)
- `fdml validate <file>` - Validate FDML specification files

**Code Generation:**
- `fdml generate <file> --language <ts|py|go>` - Generate production-ready code
- `--with-tests` - Include automated tests
- `--output <dir>` - Specify output directory

**Migration System:**
- `fdml migrate apply` - Apply pending migrations
- `fdml migrate rollback --count <n>` - Rollback migrations
- `fdml migrate status` - Show migration status

**Traceability (Coming Soon):**
- `fdml trace validate` - Validate traceability links
- `fdml trace graph` - Generate dependency graphs
- `fdml trace matrix` - Generate traceability matrices

### 🎯 Code Generation Examples

**From this FDML specification:**
```yaml
entities:
  - id: user
    fields:
      - name: email
        type: string
        required: true
      - name: name
        type: string

actions:
  - id: create_user
    input:
      entity: user
    output:
      entity: user
```

**TypeScript Output:**
```typescript
export interface User {
  email: string;
  name?: string;
}

router.post('/create-user', (req, res) => {
  // TODO: Implement action logic
  res.status(501).json({ error: 'Not implemented' });
});
```

**Python Output:**
```python
class User(BaseModel):
    email: str
    name: Optional[str] = None

@router.post("/create-user")
async def create_user(data: User):
    raise HTTPException(status_code=501, detail="Not implemented")
```

**Go Output:**
```go
type User struct {
    Email string `json:"email"`
    Name  string `json:"name"`
}

func CreateUser(c *gin.Context) {
    c.JSON(http.StatusNotImplemented, gin.H{"error": "Not implemented"})
}
```

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