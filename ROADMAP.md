# FDML CLI Tools Roadmap

## 🎯 Minimum Viable CLI (MVP)

The core CLI functionality will include:

```bash
fdml init                    # Create directory structure
fdml validate spec.fdml      # Check syntax and structure  
fdml feature add auth        # Add new feature via CLI
fdml feature list           # Show all features
fdml check                  # Check specification integrity
```

## 🔧 VSCode Integration Preparation

1. **Language Server Protocol (LSP)** - for syntax highlighting
2. **Linter** - FDML rules validation
3. **Syntax Validator** - real-time error display
4. **Autocomplete** - suggestions when writing features
5. **Go to Definition** - navigation between related features

## 📋 Detailed Phase Breakdown

### **Phase 1: Foundation & Core Parser (Weeks 1-4)**
- ✅ Basic project architecture creation
- ✅ FDML parser implementation
- ✅ AST (Abstract Syntax Tree) construction
- ✅ Basic CLI commands (`init`, `validate`)
- ✅ Error handling and user messages

### **Phase 2: Validation & Project Structure (Weeks 5-8)**
- ✅ FDML specification validation system
- ✅ Feature management commands (`feature add`, `feature list`)
- ✅ Directory structure creation and management
- ✅ Project integrity checking (`check`)
- ✅ Enhanced error handling

### **Phase 3: LSP Foundation (Weeks 9-12)**
- ✅ Language Server architecture preparation
- ✅ Basic diagnostics and real-time validation
- ✅ FDML syntax highlighting
- ✅ Feature navigation (Go to Definition)
- ✅ Testing and documentation

### **Phase 4: VSCode Integration (Weeks 13-16)**
- ✅ VSCode extension creation
- ✅ LSP server integration
- ✅ Autocomplete and suggestions
- ✅ Project structure visualization
- ✅ Debugging and optimization

## 🏗️ Technical Architecture

### Core Modules
```
fdml-cli/
├── parser/          # FDML → AST parser
├── validator/       # Rules validation and checking
├── project/         # Project structure management
├── features/        # Feature operations
├── lsp/            # Language Server Protocol
└── cli/            # Command line interface
```

### Technology Stack
- **Primary Language**: Rust (performance, memory safety)
- **Alternative**: TypeScript (rapid ecosystem development)
- **LSP**: Using tower-lsp for Rust or vscode-languageserver for TS
- **CLI**: clap for Rust or commander for TS

## ✅ Success Metrics
- ✅ Parse basic FDML specifications
- ✅ Create project structure with one command
- ✅ Validation with clear error messages
- ✅ Working VSCode extension with syntax highlighting
- ✅ Navigate between related features

## ❌ Excluded from Current Roadmap

The following capabilities are deferred to later versions:
- Code generation (will be added after LLM integration)
- Multiple programming language support
- Complex templates and migrations
- Test generation

## 🚀 Future Development

This updated roadmap creates a solid foundation for future development and LLM integration for code generation in later versions.

After completing Phase 4, the following is planned:
- LLM integration for code generation
- Multiple programming language support
- Advanced generation capabilities and templates
- CI/CD system integration