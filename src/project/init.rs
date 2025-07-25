use crate::error::{FdmlError, Result};
use std::fs;
use std::path::Path;

pub struct ProjectInitializer {
    project_name: String,
    project_path: std::path::PathBuf,
}

impl ProjectInitializer {
    pub fn new(project_name: String) -> Self {
        let project_path = std::path::PathBuf::from(&project_name);
        Self {
            project_name,
            project_path,
        }
    }
    
    pub fn initialize(&self) -> Result<()> {
        if self.project_path.exists() {
            return Err(FdmlError::project_error(format!(
                "Directory '{}' already exists",
                self.project_name
            )));
        }
        
        // Create project directory
        fs::create_dir_all(&self.project_path)?;
        
        // Create subdirectories
        self.create_directories()?;
        
        // Create initial files
        self.create_initial_files()?;
        
        Ok(())
    }
    
    fn create_directories(&self) -> Result<()> {
        let dirs = vec![
            "specs",
            "features",
            "entities", 
            "flows",
            "docs",
            "generated",
        ];
        
        for dir in dirs {
            let dir_path = self.project_path.join(dir);
            fs::create_dir_all(dir_path)?;
        }
        
        Ok(())
    }
    
    fn create_initial_files(&self) -> Result<()> {
        // Create project configuration
        self.create_project_config()?;
        
        // Create example specification
        self.create_example_spec()?;
        
        // Create README
        self.create_readme()?;
        
        // Create .gitignore
        self.create_gitignore()?;
        
        Ok(())
    }
    
    fn create_project_config(&self) -> Result<()> {
        let config_content = format!(r#"# FDML Project Configuration
project:
  name: {}
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
"#, self.project_name);

        let config_path = self.project_path.join("fdml.yaml");
        fs::write(config_path, config_content)?;
        Ok(())
    }
    
    fn create_example_spec(&self) -> Result<()> {
        let spec_content = r#"# Example FDML Specification
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
    - name: name
      type: string
      description: "User display name"
      required: true
    - name: created_at
      type: datetime
      description: "Account creation timestamp"
      required: true

# Define a feature for user registration
feature:
  id: user_registration
  title: "User Registration"
  description: "Allow new users to create accounts"
  scenarios:
    - id: successful_registration
      title: "Successful user registration"
      description: "User can successfully create a new account"
      given:
        - "the registration form is displayed"
        - "no user exists with the given email"
      when:
        - "user enters valid email address"
        - "user enters valid name"
        - "user submits the registration form"
      then:
        - "a new user account is created"
        - "user receives confirmation email"
        - "user is redirected to dashboard"
    - id: duplicate_email_registration
      title: "Registration with duplicate email"
      description: "System prevents registration with existing email"
      given:
        - "the registration form is displayed"
        - "a user already exists with the given email"
      when:
        - "user enters existing email address"
        - "user submits the registration form"
      then:
        - "registration is rejected"
        - "error message is displayed"
        - "no new account is created"

# Define an action for user creation
action:
  id: create_user
  name: "Create User"
  description: "Creates a new user account in the system"
  input:
    entity: user
    fields: ["email", "name"]
    description: "User data for account creation"
  output:
    entity: user
    fields: ["id", "email", "name", "created_at"]
    description: "Created user with generated ID and timestamp"
  preconditions:
    - "email must be valid format"
    - "email must not already exist"
    - "name must not be empty"
  postconditions:
    - "user exists in database"
    - "user has unique ID"
    - "created_at is set to current timestamp"

# Define traceability
traceability:
  from: user_registration
  to: create_user
  relation: implements
  description: "The create_user action implements the user registration feature"
"#;

        let spec_path = self.project_path.join("specs").join("example.fdml");
        fs::write(spec_path, spec_content)?;
        Ok(())
    }
    
    fn create_readme(&self) -> Result<()> {
        let readme_content = format!(r#"# {}

A new FDML (Feature-Driven Modeling Language) project.

## Project Structure

```
{}/
├── fdml.yaml           # Project configuration
├── specs/              # FDML specifications
│   └── example.fdml    # Example specification
├── features/           # Feature definitions
├── entities/           # Entity definitions
├── flows/              # Flow definitions
├── docs/               # Documentation
└── generated/          # Generated code (auto-created)
```

## Getting Started

1. **Validate your specifications:**
   ```bash
   fdml validate specs/example.fdml
   ```

2. **Edit the example specification:**
   Open `specs/example.fdml` and modify it according to your needs.

3. **Add more specifications:**
   Create additional `.fdml` files in the `specs/` directory.

## FDML Commands

- `fdml validate <file>` - Validate an FDML specification
- `fdml help` - Show help information
- `fdml --version` - Show version information

## Learn More

- [FDML Specification](https://github.com/kolanski/FDML)
- [Documentation](docs/)

## Example Usage

The `specs/example.fdml` file contains a simple user registration example. You can:

1. Run validation: `fdml validate specs/example.fdml`
2. Extend the example with your own entities and features
3. Add more complex scenarios and flows

Happy modeling! 🚀
"#, self.project_name, self.project_name);

        let readme_path = self.project_path.join("README.md");
        fs::write(readme_path, readme_content)?;
        Ok(())
    }
    
    fn create_gitignore(&self) -> Result<()> {
        let gitignore_content = r#"# Generated files
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
"#;

        let gitignore_path = self.project_path.join(".gitignore");
        fs::write(gitignore_path, gitignore_content)?;
        Ok(())
    }
    
    pub fn project_path(&self) -> &Path {
        &self.project_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_project_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let project_name = "test-project";
        let project_path = temp_dir.path().join(project_name);
        
        let initializer = ProjectInitializer {
            project_name: project_name.to_string(),
            project_path: project_path.clone(),
        };
        
        initializer.initialize().unwrap();
        
        // Check that directories were created
        assert!(project_path.join("specs").exists());
        assert!(project_path.join("features").exists());
        assert!(project_path.join("entities").exists());
        
        // Check that files were created
        assert!(project_path.join("fdml.yaml").exists());
        assert!(project_path.join("specs/example.fdml").exists());
        assert!(project_path.join("README.md").exists());
        assert!(project_path.join(".gitignore").exists());
    }
    
    #[test]
    fn test_existing_directory_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_name = "existing-project";
        let project_path = temp_dir.path().join(project_name);
        
        // Create the directory first
        fs::create_dir_all(&project_path).unwrap();
        
        let initializer = ProjectInitializer {
            project_name: project_name.to_string(),
            project_path: project_path.clone(),
        };
        
        let result = initializer.initialize();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }
}