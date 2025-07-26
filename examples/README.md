# FDML Examples

This directory contains example FDML specifications demonstrating various features and use cases.

## E-commerce Platform Example

The `e-commerce/` directory contains a comprehensive example of an e-commerce platform specification.

### Features Demonstrated

- **Complete Entity Modeling**: Users, products, orders, and order items
- **Action Definitions**: CRUD operations with proper input/output specifications
- **Feature Scenarios**: BDD-style feature definitions with Given-When-Then scenarios
- **Business Constraints**: Validation rules and business logic constraints
- **Traceability Links**: Connections between features, actions, and entities
- **System Architecture**: Component definitions and relationships
- **Migration System**: Evolution of the specification over time

### Quick Start

1. **Validate the specification:**
   ```bash
   fdml validate examples/e-commerce/ecommerce.fdml
   ```

2. **Generate TypeScript code:**
   ```bash
   fdml generate examples/e-commerce/ecommerce.fdml \
     --language typescript \
     --output ./generated-ecommerce-ts \
     --with-tests
   ```

3. **Generate Python FastAPI:**
   ```bash
   fdml generate examples/e-commerce/ecommerce.fdml \
     --language python \
     --output ./generated-ecommerce-py \
     --with-tests
   ```

4. **Generate Go API:**
   ```bash
   fdml generate examples/e-commerce/ecommerce.fdml \
     --language go \
     --output ./generated-ecommerce-go
   ```

5. **Apply migrations:**
   ```bash
   fdml migrate status --path examples/e-commerce/migrations
   fdml migrate apply --path examples/e-commerce/migrations --dry-run
   fdml migrate apply --path examples/e-commerce/migrations
   ```

### Generated Code Structure

**TypeScript Output:**
```
generated-ecommerce-ts/
├── types.ts              # Entity interfaces
├── routes.ts             # Express.js API routes
├── package.json          # Node.js dependencies
└── tests/
    ├── user.test.ts      # Entity tests
    ├── product.test.ts   # Entity tests
    └── *.feature.test.ts # Feature scenario tests
```

**Python Output:**
```
generated-ecommerce-py/
├── models.py             # Pydantic models
├── routes.py             # FastAPI routes
├── main.py               # Application entry point
├── requirements.txt      # Python dependencies
└── tests/
    └── test_*.py         # pytest test files
```

**Go Output:**
```
generated-ecommerce-go/
├── types.go              # Struct definitions
├── handlers.go           # HTTP handlers
├── main.go               # Application entry point
├── go.mod                # Go module
└── *_test.go             # Go test files
```

### Key Concepts Illustrated

1. **Entity Relationships**: How different entities relate to each other
2. **Field Constraints**: Type validation, required fields, default values
3. **Action Patterns**: CRUD operations with proper input/output specifications
4. **Feature Testing**: BDD scenarios that can be converted to automated tests
5. **Business Rules**: Constraints that enforce business logic
6. **System Design**: Component architecture and dependencies
7. **Evolution**: How specifications can evolve using migrations

### Advanced Features

- **Complex Workflows**: Order fulfillment process with multiple steps
- **State Machines**: Order status transitions with validation
- **Business Constraints**: Unique email addresses, positive prices, valid transitions
- **Traceability**: Full traceability from features to implementation
- **Migration Dependencies**: Sequential migrations with dependency management

This example demonstrates how FDML can serve as a single source of truth for complex business applications, generating production-ready code while maintaining clear business requirements and full traceability.