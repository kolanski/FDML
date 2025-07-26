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
- [x] **Complete CLI Tool**: All commands working (`init`, `parse`, `validate`, `generate`, `migrate`, `trace`)
- [x] **Multi-Language Code Generators**: Production-ready TypeScript, Python, and Go generators
- [x] **Automated Test Generation**: Comprehensive test suites for entities and BDD scenarios
- [x] **Migration System**: Full lifecycle support with apply/rollback/status and dependency resolution
- [x] **FDML Parser**: Complete v1.3 specification support with JSON/YAML output
- [x] **Error Handling**: User-friendly error messages with detailed diagnostics
- [x] **Template Framework**: Extensible code generation with customizable templates
- [x] **Working Examples**: Comprehensive e-commerce platform specification with all features

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
- **Production-Ready Code**: Generate working APIs in TypeScript, Python, and Go
- **Comprehensive Testing**: Automated test suites for all generated code
- **Evolution Support**: Safe migrations with rollback capabilities
- **Single Source of Truth**: From business requirements to running code

## 🚦 Getting Started

1. **Read the Spec**: [FDML Specification v1.3](FDML-1.3-en.md)  
2. **Build the CLI**: 
   ```bash
   git clone https://github.com/kolanski/FDML
   cd FDML
   cargo build --release
   ```
3. **Try the CLI with the working e-commerce example**: 
   ```bash
   # Parse and validate the example
   ./target/release/fdml parse examples/e-commerce/ecommerce.fdml
   ./target/release/fdml validate examples/e-commerce/ecommerce.fdml
   
   # Generate TypeScript code with tests
   ./target/release/fdml generate examples/e-commerce/ecommerce.fdml \
     --language typescript --output ./generated-ts --with-tests
   
   # Generate Python FastAPI
   ./target/release/fdml generate examples/e-commerce/ecommerce.fdml \
     --language python --output ./generated-py --with-tests
   
   # Generate Go API
   ./target/release/fdml generate examples/e-commerce/ecommerce.fdml \
     --language go --output ./generated-go
   
   # Check migration status
   ./target/release/fdml migrate status --path examples/e-commerce/migrations
   
   # Initialize your own project
   ./target/release/fdml init my-project
   ```

### 🛠️ CLI Commands

The FDML CLI provides a comprehensive toolset for working with FDML specifications:

**Core Commands:**
- `fdml init <name>` - Initialize a new FDML project with templates
- `fdml parse <file>` - Parse and display AST with JSON output
- `fdml validate <file>` - Validate FDML specification files

**Code Generation:**
- `fdml generate <file> --language <ts|py|go>` - Generate production-ready code
- `--with-tests` - Include comprehensive automated tests
- `--output <dir>` - Specify output directory
- `--template <dir>` - Use custom templates

**Migration System:**
- `fdml migrate apply --path <dir>` - Apply pending migrations
- `fdml migrate rollback --count <n> --path <dir>` - Rollback migrations
- `fdml migrate status --path <dir>` - Show migration status

**Traceability:**
- `fdml trace validate` - Validate traceability links (framework ready)
- `fdml trace graph` - Generate dependency graphs (framework ready)
- `fdml trace matrix` - Generate traceability matrices (framework ready)

### 🎯 Real-World Example

The repository includes a **comprehensive e-commerce platform specification** in `examples/e-commerce/` that demonstrates:

- **4 entities** (User, Product, Order, OrderItem) with field constraints and validation
- **6 actions** with proper input/output specifications and business logic
- **3 features** with complete BDD scenarios (Given-When-Then)
- **Business constraints** and validation rules
- **Complete traceability mapping** between features, actions, and entities
- **Sequential migrations** with dependencies for specification evolution

**Generated Output Stats** from the e-commerce example:
- **TypeScript**: 3 code files + 7 test files (ready-to-run Express.js app)
- **Python**: 4 code files + comprehensive pytest tests (ready-to-run FastAPI app)
- **Go**: 4 code files + test files (ready-to-run Gin app)
- **All**: Include proper dependencies and can be built/run immediately

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

Phase 2 transforms FDML from specification to **complete development framework**:

- **Developers** can now generate production-ready APIs from business requirements
- **Teams** have single source of truth with full traceability
- **Projects** can evolve safely using the migration system
- **Multi-language** support enables gradual adoption across tech stacks

The toolchain successfully generates working applications that compile and run, complete with test suites and proper dependency management.

FDML isn't just a language. It's your **clarity advantage** - while others are stuck in meetings debating requirements, you have validated feature definitions generating working code.

## 🤝 Contributing

FDML is open source and we love contributors! Check out our [Contributing Guide](CONTRIBUTING.md).

## 📝 License

MIT License - go build something amazing!

---

<p align="center">
  <b>Stop translating. Start shipping.</b><br>
  <a href="https://fdml.dev">fdml.dev</a>
</p>