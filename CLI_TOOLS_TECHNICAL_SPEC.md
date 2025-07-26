# FDML CLI Tools Technical Specification

> **Technical specification and implementation details for FDML CLI tools**

## Overview

This document provides technical specifications for implementing CLI tools that support the FDML (Feature-Driven Modeling Language) specification v1.3.

## Architecture

### Core Components

```
fdml-cli/
├── src/
│   ├── core/
│   │   ├── parser.rs          # FDML specification parser
│   │   ├── ast.rs             # Abstract Syntax Tree definitions
│   │   ├── validator.rs       # Validation engine
│   │   └── error.rs           # Error handling
│   ├── generators/
│   │   ├── mod.rs             # Generator trait and common logic
│   │   ├── typescript.rs      # TypeScript code generator
│   │   ├── python.rs          # Python code generator
│   │   ├── go.rs              # Go code generator
│   │   └── templates/         # Code generation templates
│   ├── cli/
│   │   ├── commands/          # CLI command implementations
│   │   └── main.rs            # CLI entry point
│   └── utils/
│       ├── file_io.rs         # File operations
│       └── config.rs          # Configuration management
├── tests/
│   ├── fixtures/              # Test FDML files
│   ├── integration/           # Integration tests
│   └── unit/                  # Unit tests
└── examples/
    ├── sample.fdml            # Example FDML files
    └── generated/             # Example generated code
```

## Data Structures

### AST Definitions

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FDMLDocument {
    pub systems: Vec<System>,
    pub entities: Vec<Entity>,
    pub actions: Vec<Action>,
    pub features: Vec<Feature>,
    pub flows: Vec<Flow>,
    pub constraints: Vec<Constraint>,
    pub traceability: Vec<TraceabilityLink>,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<Field>,
    pub indexes: Vec<Index>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub field_type: DataType,
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<Value>,
    pub constraints: Vec<Constraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Date,
    Array(Box<DataType>),
    Object(HashMap<String, DataType>),
    Enum(Vec<String>),
    UUID,
}
```

## CLI Interface Specification

### Command Structure

```bash
fdml <SUBCOMMAND> [OPTIONS] [ARGS]

SUBCOMMANDS:
    parse       Parse and validate FDML files
    validate    Run comprehensive validation
    generate    Generate code from FDML specifications
    migrate     Manage FDML migrations
    test        Generate and run tests
    docs        Generate documentation
    dev         Development tools and server
    version     Show version information
    help        Show help information
```

### Command Details

#### Parse Command
```bash
fdml parse [OPTIONS] <FILE>

OPTIONS:
    -o, --output <FORMAT>    Output format: json, yaml, pretty [default: pretty]
    -q, --quiet             Suppress output except errors
    -v, --verbose           Verbose output with detailed information
    
EXAMPLES:
    fdml parse specification.fdml
    fdml parse --output json specification.fdml
```

#### Generate Command
```bash
fdml generate [OPTIONS] <TARGET> <FILE>

TARGETS:
    typescript              Generate TypeScript code
    python                  Generate Python code
    go                      Generate Go code
    all                     Generate for all supported languages

OPTIONS:
    -o, --output <DIR>      Output directory [default: ./generated]
    -t, --template <DIR>    Custom template directory
    --config <FILE>         Configuration file
    --dry-run              Show what would be generated without creating files

EXAMPLES:
    fdml generate typescript specification.fdml
    fdml generate --output ./src/generated typescript specification.fdml
    fdml generate all specification.fdml
```

## Code Generation

### Template Engine

Using Handlebars template engine with custom helpers:

```handlebars
{{!-- TypeScript entity template --}}
export interface {{pascalCase name}} {
{{#each fields}}
  {{name}}{{#unless required}}?{{/unless}}: {{typeScript type}};
{{/each}}
}

{{!-- Validation schema --}}
export const {{camelCase name}}Schema = z.object({
{{#each fields}}
  {{name}}: {{zodType type}}{{#unless required}}.optional(){{/unless}},
{{/each}}
});
```

### Custom Helpers

```rust
// Template helpers for code generation
pub fn register_helpers(handlebars: &mut Handlebars) {
    handlebars.register_helper("pascalCase", Box::new(pascal_case_helper));
    handlebars.register_helper("camelCase", Box::new(camel_case_helper));
    handlebars.register_helper("typeScript", Box::new(typescript_type_helper));
    handlebars.register_helper("zodType", Box::new(zod_type_helper));
}
```

## Validation Rules

### Validation Categories

1. **Syntax Validation**
   - YAML structure validity
   - Required field presence
   - Data type correctness

2. **Semantic Validation**
   - ID uniqueness within scope
   - Reference integrity
   - Type consistency
   - Constraint applicability

3. **Business Rules**
   - Naming conventions
   - Required metadata
   - Domain-specific rules

### Example Validation Rules

```rust
pub trait ValidationRule {
    fn validate(&self, document: &FDMLDocument) -> Vec<ValidationError>;
}

pub struct UniqueIdRule;
impl ValidationRule for UniqueIdRule {
    fn validate(&self, document: &FDMLDocument) -> Vec<ValidationError> {
        // Implementation for checking ID uniqueness
    }
}

pub struct TypeConsistencyRule;
impl ValidationRule for TypeConsistencyRule {
    fn validate(&self, document: &FDMLDocument) -> Vec<ValidationError> {
        // Implementation for checking type consistency
    }
}
```

## Configuration

### CLI Configuration File

```yaml
# .fdmlrc.yml
output:
  directory: "./generated"
  clean_before_generate: true

generators:
  typescript:
    target: "es2020"
    module: "commonjs"
    include_validation: true
    orm: "prisma"
  
  python:
    target_version: "3.9"
    framework: "fastapi"
    orm: "sqlalchemy"
  
  go:
    module_name: "myapp"
    framework: "gin"
    orm: "gorm"

validation:
  strict_mode: true
  custom_rules: []
  ignore_warnings: false

templates:
  custom_path: "./templates"
  fallback_to_builtin: true
```

## Error Handling

### Error Types

```rust
#[derive(Debug, Error)]
pub enum FDMLError {
    #[error("Parse error at line {line}, column {column}: {message}")]
    ParseError {
        line: usize,
        column: usize,
        message: String,
    },
    
    #[error("Validation error: {message}")]
    ValidationError { message: String },
    
    #[error("Generation error: {message}")]
    GenerationError { message: String },
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

### Error Output Format

```json
{
  "errors": [
    {
      "type": "validation_error",
      "message": "Entity 'user' field 'email' has invalid constraint 'max_length' for type 'integer'",
      "location": {
        "file": "specification.fdml",
        "line": 15,
        "column": 12
      },
      "severity": "error",
      "code": "E001"
    }
  ],
  "warnings": [
    {
      "type": "style_warning",
      "message": "Entity name 'userProfile' should use snake_case: 'user_profile'",
      "location": {
        "file": "specification.fdml",
        "line": 8,
        "column": 1
      },
      "severity": "warning",
      "code": "W001"
    }
  ]
}
```

## Testing Strategy

### Test Categories

1. **Unit Tests**
   - Parser component tests
   - Validator rule tests
   - Generator function tests
   - Utility function tests

2. **Integration Tests**
   - End-to-end CLI command tests
   - Generated code compilation tests
   - Configuration loading tests

3. **Property-Based Tests**
   - Round-trip parsing tests
   - Generator output validity tests

### Test File Structure

```
tests/
├── fixtures/
│   ├── valid/                # Valid FDML files for testing
│   ├── invalid/              # Invalid FDML files for error testing
│   └── edge_cases/           # Edge case scenarios
├── integration/
│   ├── cli_tests.rs          # CLI command integration tests
│   ├── generation_tests.rs   # Code generation integration tests
│   └── validation_tests.rs   # Validation integration tests
└── unit/
    ├── parser_tests.rs       # Parser unit tests
    ├── validator_tests.rs    # Validator unit tests
    └── generator_tests.rs    # Generator unit tests
```

## Performance Requirements

### Benchmarks

- **Parse Performance**: < 100ms for files with 100 entities
- **Validation Performance**: < 500ms for comprehensive validation of 100 entities
- **Generation Performance**: < 2s for generating TypeScript code from 100 entities
- **Memory Usage**: < 50MB for processing files with 1000 entities

### Optimization Strategies

1. **Lazy Loading**: Parse only required sections for specific commands
2. **Caching**: Cache parsed AST for repeated operations
3. **Parallel Processing**: Parallelize validation rules and code generation
4. **Memory Efficiency**: Use streaming for large file processing

## Deployment & Distribution

### Release Artifacts

- **Binaries**: Static binaries for Linux, macOS, Windows
- **NPM Package**: Cross-platform Node.js package
- **Docker Images**: Alpine-based container images
- **Package Managers**: Homebrew, Chocolatey, APT packages

### Version Management

```rust
// Version information embedded in binary
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_HASH: &str = env!("GIT_HASH");
pub const BUILD_DATE: &str = env!("BUILD_DATE");
```

## Future Considerations

### Extensibility

- Plugin system for custom generators
- Custom validation rule plugins
- Template override system
- External tool integrations

### Language Server Protocol

Foundation for IDE integrations:
- Syntax highlighting
- Error reporting
- Auto-completion
- Hover documentation
- Go-to-definition

This technical specification provides the detailed implementation guidance needed to build robust and efficient FDML CLI tools.