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
        let spec_content = r#"# Simple FDML Specification - Working Example
metadata:
  version: "1.3"
  author: "Your Name"
  description: "Simple working example for FDML"

# Define a basic user entity
entity:
  id: user
  name: "User Entity"
  description: "Represents a user in the system"

# Define user registration feature
feature:
  id: user_registration
  title: "User Registration"
  description: "Allow new users to create accounts"

# Define user creation action
action:
  id: create_user
  name: "Create User"
  description: "Creates a new user account in the system"

# Link feature to action
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