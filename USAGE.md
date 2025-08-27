# FDML CLI Tool - Usage Examples

This document demonstrates the complete functionality of the FDML CLI tool.

## Installation

1. Clone the repository:
```bash
git clone https://github.com/kolanski/FDML.git
cd FDML
```

2. Build the CLI:
```bash
cargo build --release
```

3. The binary will be available at `target/release/fdml`

## Commands

### 1. Getting Help

```bash
# Show general help
fdml --help

# Show version
fdml --version
```

### 2. Initialize a New Project

```bash
# Create a new FDML project
fdml init my-awesome-project

# This creates:
# my-awesome-project/
# ├── fdml.yaml              # Project configuration
# ├── README.md              # Documentation
# ├── .gitignore             # Git ignore file
# ├── specs/                 # FDML specifications
# │   └── example.fdml       # Example specification
# ├── features/              # Feature definitions
# ├── entities/              # Entity definitions
# ├── flows/                 # Flow definitions
# ├── docs/                  # Documentation
# └── generated/             # Generated code output
```

### 3. Validate FDML Files

```bash
# Validate a specification
fdml validate specs/example.fdml

# Validate with verbose output
fdml --verbose validate specs/example.fdml

# Strict validation (treat warnings as errors)
fdml validate --strict specs/example.fdml

# JSON output for machine processing
fdml validate --output json specs/example.fdml
```

## Example Usage Workflow

### Step 1: Create a Project

```bash
$ fdml init user-management-system
✓ Successfully initialized FDML project: user-management-system
ℹ Next steps:
  1. cd user-management-system
  2. fdml validate specs/example.fdml
  3. Edit specs/example.fdml to match your needs

$ cd user-management-system
```

### Step 2: Validate the Example

```bash
$ fdml validate specs/example.fdml
✓ ✓ specs/example.fdml is valid
```

### Step 3: Examine the Generated Structure

```bash
$ ls -la
total 16
drwxr-xr-x  8 user group  256 Oct 25 10:30 .
drwxr-xr-x  3 user group   96 Oct 25 10:30 ..
-rw-r--r--  1 user group  183 Oct 25 10:30 .gitignore
-rw-r--r--  1 user group 1390 Oct 25 10:30 README.md
drwxr-xr-x  2 user group   64 Oct 25 10:30 docs
drwxr-xr-x  2 user group   64 Oct 25 10:30 entities
-rw-r--r--  1 user group  474 Oct 25 10:30 fdml.yaml
drwxr-xr-x  2 user group   64 Oct 25 10:30 features
drwxr-xr-x  2 user group   64 Oct 25 10:30 flows
drwxr-xr-x  2 user group   64 Oct 25 10:30 generated
drwxr-xr-x  2 user group   96 Oct 25 10:30 specs

$ head -20 specs/example.fdml
# Example FDML Specification
metadata:
  version: "1.3"
  author: "Your Name"
  description: "Example specification for learning FDML"
  created: "2024-01-01"

# Define a simple user entity
entity:
  id: user
  name: "User Entity"
  description: "Represents a user in the system"
  fields:
    - name: id
      type: string
      description: "Unique user identifier"
      required: true
    - name: email
      type: string
      description: "User email address"
      required: true
```

### Step 4: Test Different Validation Modes

```bash
# Normal validation
$ fdml validate specs/example.fdml
✓ ✓ specs/example.fdml is valid

# Verbose validation
$ fdml --verbose validate specs/example.fdml
ℹ Validating FDML file: specs/example.fdml
ℹ Parsing completed successfully
✓ ✓ specs/example.fdml is valid

# JSON output
$ fdml validate --output json specs/example.fdml
{
  "error_count": 0,
  "errors": [],
  "valid": true
}
```

### Step 5: Create Your Own Specification

```bash
$ cat > specs/my-spec.fdml << 'EOF'
metadata:
  version: "1.3"
  author: "My Name"

entity:
  id: product
  name: "Product"
  description: "A product in our system"

feature:
  id: product_catalog
  title: "Product Catalog"
  description: "Display products to users"
EOF

$ fdml validate specs/my-spec.fdml
✓ ✓ specs/my-spec.fdml is valid
```

## Error Handling Examples

### Missing Required Fields

```bash
$ cat > invalid.fdml << 'EOF'
entity:
  # Missing required 'id' field
  name: "Invalid Entity"
EOF

$ fdml validate invalid.fdml
⚠ Validation warnings for invalid.fdml
  1. Entity at index 0 is missing required 'id' field
ℹ Found 1 validation warnings
ℹ Use --strict flag to treat warnings as errors

$ fdml validate --strict invalid.fdml
Error: Validation failed in strict mode
```

### File Not Found

```bash
$ fdml validate nonexistent.fdml
Error: Failed to read file 'nonexistent.fdml': No such file or directory (os error 2)
```

## Project Configuration

The generated `fdml.yaml` contains project settings:

```yaml
# FDML Project Configuration
project:
  name: user-management-system
  version: "0.1.0"
  description: "A new FDML project"
  
settings:
  validation:
    strict: true
    rules:
      - required_ids
      - unique_ids
      - valid_references
      - required_fields
  
  generation:
    output_dir: "generated"
    formats:
      - typescript
      - documentation
  
  paths:
    specs: "specs"
    features: "features"
    entities: "entities"
    flows: "flows"
    docs: "docs"
```

## Integration with Development Workflow

### Git Integration

The generated `.gitignore` excludes build artifacts:

```gitignore
# Generated files
generated/

# IDE files
.vscode/
.idea/
*.swp
*.swo

# OS files
.DS_Store
Thumbs.db

# Temporary files
*.tmp
*.temp
.cache/

# Build artifacts
target/
node_modules/
```

### CI/CD Integration

Example GitHub Actions workflow:

```yaml
name: FDML Validation
on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build FDML CLI
        run: cargo build --release
      - name: Validate FDML specs
        run: |
          for file in specs/*.fdml; do
            ./target/release/fdml validate --strict "$file"
          done
```

This completes the Phase 1 implementation with a fully functional CLI tool that provides all the requested features and more.