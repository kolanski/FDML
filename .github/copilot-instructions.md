# Copilot Instructions for FDML Repository

## Repository Overview

FDML (Feature-Driven Modeling Language) is a domain-specific language and CLI toolset that bridges the gap between business requirements and production code. It transforms structured feature specifications into working APIs across multiple programming languages.

## Project Architecture

### Core Components

1. **FDML Parser** (`src/parser/`) - Parses FDML specifications into Abstract Syntax Trees (AST)
2. **CLI Tools** (`src/cli/`) - Command-line interface for all FDML operations
3. **Code Generators** (`src/generators/`) - Multi-language code generation (TypeScript, Python, Go)
4. **Migration Engine** (`src/migration/`) - Database-like migration system for FDML specifications
5. **Validator** (`src/validator/`) - Specification validation and consistency checking
6. **Project Management** (`src/project/`) - Project initialization and template management

### Key Technologies

- **Language**: Rust (Edition 2021)
- **CLI Framework**: clap v4 with derive features
- **Serialization**: serde with YAML/JSON support
- **Error Handling**: anyhow and thiserror
- **Testing**: Standard Rust testing with tempfile, assert_cmd, predicates

## FDML Language Specification

FDML v1.3 supports these core concepts:

- **System**: High-level system definitions and component relationships
- **Entity**: Data structures with type constraints and validation rules
- **Action**: Operations with input/output specifications and business logic
- **Feature**: BDD-style scenarios with Given-When-Then syntax
- **Flow**: Action sequences and workflows
- **Constraint**: Business rules and validation constraints
- **Traceability**: Links between features, actions, and entities
- **Migration**: Version-controlled changes to specifications

## Development Guidelines

### Code Style

- Follow standard Rust conventions and `cargo fmt` formatting
- Use descriptive error messages with user-friendly output
- Implement comprehensive error handling with `anyhow::Result`
- Add colored terminal output using the `colored` crate for CLI feedback
- Include thorough documentation for public APIs

### Testing Approach

- Unit tests for core parser and validation logic
- Integration tests for CLI commands using `assert_cmd`
- Example-driven testing using the e-commerce specification
- Test both success and error scenarios thoroughly
- Use `tempfile` for temporary file operations in tests

### CLI Design Principles

- Provide rich feedback with emoji indicators and progress reports
- Support both verbose and quiet modes
- Include helpful error messages with suggestions for fixes
- Use consistent command structure: `fdml <verb> [options] <file>`
- Support dry-run mode for destructive operations

## Key Features to Understand

### Migration System

The migration engine works like database migrations but for FDML specifications:

- **Dependencies**: Migrations can depend on other migrations (topological sorting)
- **Safety**: Automatic backups before operations, rollback validation
- **Operations**: add_feature, remove_feature, modify_entity, add_field, etc.
- **Target Support**: Can specify which FDML file to modify

### Code Generation

Multi-language support with consistent patterns:

- **TypeScript**: Express.js APIs with TypeScript interfaces
- **Python**: FastAPI with Pydantic models
- **Go**: Gin framework with proper struct definitions
- **Testing**: Automated test generation for all languages
- **Templates**: Customizable generation templates

### Traceability System

Links business requirements to implementation:

- **Feature-to-Action**: Maps scenarios to API operations  
- **Action-to-Entity**: Maps operations to data structures
- **Validation**: Ensures all links are valid and complete
- **Visualization**: Future support for dependency graphs

## Working with Examples

The `examples/e-commerce/` directory contains a comprehensive specification demonstrating:

- 4 entities with field constraints and validation
- 6 actions with proper input/output specifications  
- 3 features with complete BDD scenarios
- Migration files showing specification evolution
- Generated code examples in all supported languages

Use this as a reference for understanding expected output and behavior.

## Common Development Tasks

### Adding New Migration Operations

1. Add operation type to `MigrationOperation` enum
2. Implement operation logic in `migration/engine.rs`
3. Add validation in `validate_operation()`
4. Include dry-run description
5. Add tests covering the new operation

### Adding New Code Generators

1. Create new module in `src/generators/`
2. Implement the `Generator` trait
3. Add language support to CLI parsing
4. Create templates for the new language
5. Add comprehensive tests with example generation

### Extending FDML Specification

1. Update AST structures in `src/parser/ast.rs`
2. Modify parser logic in `src/parser/mod.rs`  
3. Update validation rules in `src/validator/`
4. Add examples to test specifications
5. Update documentation and specification files

## Error Handling Best Practices

- Use `anyhow::Result` for functions that can fail
- Provide context with `.with_context()` for better error messages
- Use `thiserror` for custom error types when appropriate
- Include suggestions for fixing common errors
- Test error scenarios as thoroughly as success scenarios

## Performance Considerations

- Parser uses streaming for large files
- Code generation writes directly to filesystem
- Migration operations create backups efficiently
- CLI operations provide progress feedback for long-running tasks

## Dependencies and External Tools

- Avoid adding new dependencies unless absolutely necessary
- Prefer standard library solutions when possible
- Use well-maintained crates with stable APIs
- Consider impact on compilation time and binary size

## Future Architecture Considerations

The roadmap includes Language Server Protocol (LSP) support and VSCode integration. When working on core components, consider:

- Clean separation between parsing and analysis
- Structured diagnostic information for IDE integration
- Performance optimizations for real-time validation
- API design that supports incremental parsing and validation