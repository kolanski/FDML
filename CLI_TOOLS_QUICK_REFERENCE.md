# FDML CLI Tools Quick Reference

> **Quick reference guide for developers implementing FDML CLI tools**

## Project Setup Checklist

### Initial Setup
- [ ] Choose technology stack (Rust recommended, TypeScript alternative)
- [ ] Set up project structure with clear module separation
- [ ] Configure build system and dependency management
- [ ] Set up testing framework and CI/CD pipeline
- [ ] Establish coding standards and documentation format

### Core Dependencies (Rust)
```toml
[dependencies]
clap = "4.0"           # CLI argument parsing
serde = "1.0"          # Serialization/deserialization
serde_yaml = "0.9"     # YAML parsing
handlebars = "4.3"     # Template engine
anyhow = "1.0"         # Error handling
tokio = "1.0"          # Async runtime (if needed)
```

### Core Dependencies (TypeScript)
```json
{
  "dependencies": {
    "commander": "^11.0.0",
    "js-yaml": "^4.1.0",
    "handlebars": "^4.7.0",
    "ajv": "^8.12.0",
    "chalk": "^5.0.0"
  }
}
```

## Essential Commands Implementation Order

### Phase 1: MVP Commands
1. `fdml version` - Version information
2. `fdml parse <file>` - Basic parsing and validation
3. `fdml validate <file>` - Comprehensive validation
4. `fdml help` - Help system

### Phase 2: Core Commands
5. `fdml generate typescript <file>` - TypeScript generation
6. `fdml generate python <file>` - Python generation
7. `fdml lint <file>` - Linting and style checking

### Phase 3: Advanced Commands
8. `fdml migrate` - Migration management
9. `fdml test generate` - Test generation
10. `fdml docs generate` - Documentation generation

## Key Implementation Patterns

### Error Handling Pattern
```rust
// Rust example
pub type Result<T> = anyhow::Result<T>;

#[derive(Debug, Error)]
pub enum FDMLError {
    #[error("Parse error at {location}: {message}")]
    ParseError { location: String, message: String },
    
    #[error("Validation failed: {0}")]
    ValidationError(String),
}
```

### Configuration Loading Pattern
```rust
// Load configuration with fallbacks
pub fn load_config() -> Result<Config> {
    let config = Config::builder()
        .add_source(config::File::with_name(".fdmlrc").required(false))
        .add_source(config::Environment::with_prefix("FDML"))
        .build()?;
    
    config.try_deserialize()
}
```

### Template Generation Pattern
```rust
// Template-based code generation
pub fn generate_code(template: &str, data: &Value) -> Result<String> {
    let mut handlebars = Handlebars::new();
    register_helpers(&mut handlebars);
    handlebars.render_template(template, data)
        .map_err(|e| anyhow!("Template generation failed: {}", e))
}
```

## Testing Strategy

### Test File Organization
```
tests/
├── fixtures/           # Test FDML files
│   ├── valid/
│   ├── invalid/
│   └── edge_cases/
├── snapshots/          # Expected outputs
├── integration/        # End-to-end tests
└── unit/              # Component tests
```

### Essential Test Cases
- [ ] Parse valid FDML files successfully
- [ ] Reject invalid FDML files with clear errors
- [ ] Generate valid code for all target languages
- [ ] Handle edge cases (empty files, large files, Unicode)
- [ ] CLI command integration tests
- [ ] Configuration loading tests
- [ ] Template rendering tests

## Code Generation Templates

### TypeScript Template Structure
```
templates/typescript/
├── entity.hbs          # Entity interfaces
├── action.hbs          # Action/service classes
├── validation.hbs      # Validation schemas
├── api.hbs            # API client
└── tests.hbs          # Test templates
```

### Template Helper Functions
```rust
// Essential template helpers
fn pascal_case(name: &str) -> String;
fn camel_case(name: &str) -> String;
fn snake_case(name: &str) -> String;
fn type_mapping(fdml_type: &DataType, target: &str) -> String;
```

## Validation Rules Priority

### Critical Validations (Implement First)
1. YAML syntax validation
2. Required field validation
3. Data type validation
4. ID uniqueness validation
5. Reference integrity validation

### Important Validations (Implement Second)
1. Constraint applicability validation
2. Circular dependency detection
3. Naming convention validation
4. Type consistency validation

### Nice-to-Have Validations (Implement Later)
1. Performance anti-pattern detection
2. Best practice recommendations
3. Style guide enforcement
4. Documentation completeness

## Performance Optimization Tips

### Parsing Optimization
- Use streaming parsers for large files
- Implement lazy loading for unused sections
- Cache parsed AST between operations
- Parallelize validation rules where possible

### Memory Management
- Use borrowing instead of cloning where possible
- Implement pagination for large result sets
- Stream output for large generated files
- Clean up temporary files automatically

### Build Time Optimization
- Embed templates at compile time
- Use release builds for benchmarking
- Optimize dependency tree
- Enable link-time optimization

## Common Pitfalls to Avoid

### Parser Implementation
- ❌ Don't ignore error positions and context
- ❌ Don't assume file encodings (support UTF-8)
- ❌ Don't hardcode file paths or extensions
- ✅ Provide detailed error messages with suggestions
- ✅ Support both files and stdin input
- ✅ Handle large files efficiently

### Code Generation
- ❌ Don't generate code without validation
- ❌ Don't overwrite files without confirmation
- ❌ Don't ignore template compilation errors
- ✅ Generate human-readable code with comments
- ✅ Include generation metadata in output
- ✅ Support custom template overrides

### CLI Design
- ❌ Don't use inconsistent command naming
- ❌ Don't ignore --help and --version flags
- ❌ Don't suppress error details in quiet mode
- ✅ Follow Unix CLI conventions
- ✅ Provide progress indicators for long operations
- ✅ Support both interactive and scripted usage

## Debugging and Troubleshooting

### Debug Output
```rust
// Add debug logging throughout
use log::{debug, info, warn, error};

debug!("Parsing file: {}", file_path);
info!("Generated {} files successfully", count);
warn!("Deprecated syntax found in entity: {}", entity_id);
error!("Failed to generate code: {}", error);
```

### Common Issues and Solutions

| Issue | Symptoms | Solution |
|-------|----------|----------|
| Slow parsing | High CPU usage, timeouts | Profile parser, optimize hotpaths |
| Memory leaks | Growing memory usage | Check for circular references |
| Invalid generation | Compilation errors | Validate templates, add integration tests |
| Unicode issues | Parsing failures | Ensure UTF-8 handling throughout |

## Release Checklist

### Pre-Release
- [ ] All tests passing
- [ ] Documentation updated
- [ ] Performance benchmarks run
- [ ] Cross-platform testing completed
- [ ] Security audit completed
- [ ] Breaking changes documented

### Release Process
- [ ] Version number updated
- [ ] CHANGELOG.md updated
- [ ] Git tags created
- [ ] Binaries built for all platforms
- [ ] Package repositories updated
- [ ] Documentation deployed

### Post-Release
- [ ] Community notifications sent
- [ ] GitHub release created
- [ ] Package registries updated
- [ ] Monitoring and feedback collection set up

## Resources and References

### FDML Specification
- [FDML v1.3 Specification](FDML-1.3-en.md)
- [CLI Tools Roadmap](CLI_TOOLS_ROADMAP.md)
- [Technical Specification](CLI_TOOLS_TECHNICAL_SPEC.md)

### Community and Support
- GitHub Discussions for questions
- Issue templates for bug reports
- Contributing guidelines for new developers
- Code of conduct for community standards

This quick reference provides the essential information needed to start implementing FDML CLI tools efficiently and correctly.