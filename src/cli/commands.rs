use crate::cli::args::{Cli, Commands, MigrateCommands, TraceCommands, AddCommands, ListCommands};
use crate::error::{print_error, print_info, print_success, print_warning, Result};
use crate::parser::{parse_fdml_yaml, parse_fdml};
use crate::project::ProjectInitializer;
use crate::validator::Validator;
use crate::generators::{create_generator, GeneratorConfig};
use crate::generators::test_gen::TestGenerator;
use crate::migration::{MigrationRunner, Migration, MigrationOperation};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CommandRunner {
    verbose: bool,
}

impl CommandRunner {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
    
    pub fn run(&self, cli: Cli) -> Result<()> {
        match cli.command {
            Commands::Init { name, force } => self.run_init(name, force),
            Commands::Parse { file, output, debug } => self.run_parse(file, output, debug),
            Commands::Validate { file, strict, output } => self.run_validate(file, strict, output),
            Commands::Generate { input, language, output, template, with_tests } => {
                self.run_generate(input, language, output, template, with_tests)
            },
            Commands::Add { operation } => self.run_add(operation),
            Commands::List { operation } => self.run_list(operation),
            Commands::Migrate { operation } => self.run_migrate(operation),
            Commands::Trace { operation } => self.run_trace(operation),
        }
    }
    
    fn run_init(&self, name: String, force: bool) -> Result<()> {
        if self.verbose {
            print_info(&format!("Initializing FDML project: {}", name));
        }
        
        if force {
            print_warning("Force flag is not yet implemented - directory must not exist");
        }
        
        let initializer = ProjectInitializer::new(name.clone());
        initializer.initialize()?;
        
        print_success(&format!("Successfully initialized FDML project: {}", name));
        print_info("Next steps:");
        println!("  1. cd {}", name);
        println!("  2. fdml validate specs/example.fdml");
        println!("  3. Edit specs/example.fdml to match your needs");
        
        Ok(())
    }
    
    fn run_validate(&self, file: String, strict: bool, output: String) -> Result<()> {
        if self.verbose {
            print_info(&format!("Validating FDML file: {}", file));
        }
        
        // Read the file
        let content = fs::read_to_string(&file).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", file, e))
        })?;
        
        // Parse the FDML document
        let document = parse_fdml_yaml(&content)?;
        
        if self.verbose {
            print_info("Parsing completed successfully");
        }
        
        // Validate the document
        let validator = Validator::new();
        let validation_errors = validator.validate(&document)?;
        
        // Output results
        match output.as_str() {
            "json" => self.output_json_results(&validation_errors)?,
            "text" | _ => self.output_text_results(&file, &validation_errors, strict)?,
        }
        
        // Return error if strict mode and there are validation errors
        if strict && !validation_errors.is_empty() {
            return Err(crate::error::FdmlError::validation_error(
                "Validation failed in strict mode"
            ));
        }
        
        Ok(())
    }
    
    fn output_text_results(&self, file: &str, errors: &[String], strict: bool) -> Result<()> {
        if errors.is_empty() {
            print_success(&format!("✓ {} is valid", file));
        } else {
            if strict {
                print_error(&crate::error::FdmlError::validation_error(
                    format!("Validation failed for {}", file)
                ));
            } else {
                print_warning(&format!("Validation warnings for {}", file));
            }
            
            for (i, error) in errors.iter().enumerate() {
                println!("  {}. {}", i + 1, error);
            }
            
            if !strict {
                print_info(&format!("Found {} validation warnings", errors.len()));
                print_info("Use --strict flag to treat warnings as errors");
            }
        }
        
        Ok(())
    }
    
    fn output_json_results(&self, errors: &[String]) -> Result<()> {
        let result = serde_json::json!({
            "valid": errors.is_empty(),
            "error_count": errors.len(),
            "errors": errors
        });
        
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        Ok(())
    }
    
    fn run_parse(&self, file: String, output: String, debug: bool) -> Result<()> {
        if self.verbose {
            print_info(&format!("Parsing FDML file: {}", file));
        }
        
        // Read the file
        let content = fs::read_to_string(&file).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", file, e))
        })?;
        
        // Parse the FDML document
        let document = if file.ends_with(".fdml") || file.ends_with(".yaml") || file.ends_with(".yml") {
            parse_fdml_yaml(&content)?
        } else {
            parse_fdml(&content)?
        };
        
        if debug && self.verbose {
            print_info("Parsing completed successfully");
            print_info(&format!("Found {} entities, {} actions, {} features", 
                document.entities.len(), 
                document.actions.len(), 
                document.features.len()));
        }
        
        // Output results
        match output.as_str() {
            "yaml" => {
                let yaml_output = serde_yaml::to_string(&document)?;
                println!("{}", yaml_output);
            },
            "json" | _ => {
                let json_output = serde_json::to_string_pretty(&document)?;
                println!("{}", json_output);
            }
        }
        
        Ok(())
    }
    
    fn run_generate(&self, input: String, language: String, output: String, template: Option<String>, with_tests: bool) -> Result<()> {
        if self.verbose {
            print_info(&format!("Generating {} code from: {}", language, input));
        }
        
        // Read and parse the FDML file
        let content = fs::read_to_string(&input).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", input, e))
        })?;
        
        let document = if input.ends_with(".fdml") || input.ends_with(".yaml") || input.ends_with(".yml") {
            parse_fdml_yaml(&content)?
        } else {
            parse_fdml(&content)?
        };
        
        // Create generator configuration
        let config = GeneratorConfig {
            language: language.clone(),
            output_dir: output.clone(),
            template_dir: template,
            with_tests,
        };
        
        // Create and run generator
        let generator = create_generator(&config)?;
        let output_path = Path::new(&output);
        let generated_files = generator.generate(&document, output_path)?;
        
        print_success(&format!("Successfully generated {} files:", generated_files.len()));
        for file in &generated_files {
            println!("  - {}", file);
        }
        
        // Generate tests if requested
        if with_tests {
            let test_generator = TestGenerator::new(&config)?;
            let test_files = test_generator.generate_tests(&document, output_path)?;
            
            if !test_files.is_empty() {
                print_success(&format!("Generated {} test files:", test_files.len()));
                for file in &test_files {
                    println!("  - {}", file);
                }
            }
        }
        
        print_info(&format!("Generated code in: {}", output));
        match language.as_str() {
            "typescript" | "ts" => {
                print_info("Next steps:");
                println!("  1. cd {}", output);
                println!("  2. npm install");
                println!("  3. npm run build");
            },
            "python" | "py" => {
                print_info("Next steps:");
                println!("  1. cd {}", output);
                println!("  2. pip install -r requirements.txt");
                println!("  3. python main.py");
            },
            "go" => {
                print_info("Next steps:");
                println!("  1. cd {}", output);
                println!("  2. go mod tidy");
                println!("  3. go run .");
            },
            _ => {}
        }
        
        Ok(())
    }
    
    fn run_migrate(&self, operation: MigrateCommands) -> Result<()> {
        match operation {
            MigrateCommands::Apply { path, target, dry_run } => {
                if self.verbose {
                    print_info(&format!("Applying migrations from: {}", path));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let mut runner = MigrationRunner::new(&path);
                if let Some(target_file) = target {
                    runner = runner.with_target_file(&target_file);
                }
                
                let applied = runner.apply_migrations(dry_run)?;
                
                if applied.is_empty() && !dry_run {
                    print_info("No pending migrations to apply");
                } else if !dry_run {
                    print_success(&format!("Applied {} migrations", applied.len()));
                }
            },
            MigrateCommands::Rollback { path, target, count, dry_run } => {
                if self.verbose {
                    print_info(&format!("Rolling back {} migrations from: {}", count, path));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let mut runner = MigrationRunner::new(&path);
                if let Some(target_file) = target {
                    runner = runner.with_target_file(&target_file);
                }
                
                let rolled_back = runner.rollback_migrations(count, dry_run)?;
                
                if rolled_back.is_empty() && !dry_run {
                    print_info("No migrations to rollback");
                } else if !dry_run {
                    print_success(&format!("Rolled back {} migrations", rolled_back.len()));
                }
            },
            MigrateCommands::Status { path } => {
                if self.verbose {
                    print_info(&format!("Checking migration status in: {}", path));
                }
                
                let runner = MigrationRunner::new(&path);
                let status = runner.migration_status()?;
                
                println!("Migration Status:");
                println!("  Total migrations: {}", status.total_migrations);
                println!("  Applied: {}", status.applied_count);
                println!("  Pending: {}", status.pending_count);
                
                if !status.applied_migrations.is_empty() {
                    println!("\nApplied migrations:");
                    for migration in &status.applied_migrations {
                        println!("  ✓ {}", migration);
                    }
                }
                
                if !status.pending_migrations.is_empty() {
                    println!("\nPending migrations:");
                    for migration in &status.pending_migrations {
                        println!("  - {}", migration);
                    }
                }
            }
        }
        Ok(())
    }
    
    fn run_trace(&self, operation: TraceCommands) -> Result<()> {
        match operation {
            TraceCommands::Validate { path } => {
                if self.verbose {
                    print_info(&format!("Validating traceability in: {}", path));
                }
                
                // TODO: Implement traceability validation
                print_warning("Traceability validation is not yet implemented");
                print_info("This feature will validate:");
                println!("  - All traceability links exist");
                println!("  - No circular dependencies");
                println!("  - All required relationships are present");
            },
            TraceCommands::Graph { path, format, output } => {
                if self.verbose {
                    print_info(&format!("Generating traceability graph from: {}", path));
                }
                
                // TODO: Implement traceability graph generation
                print_warning("Traceability graph generation is not yet implemented");
                print_info(&format!("Would generate {} graph in {}", format, output));
            },
            TraceCommands::Matrix { path, format, output } => {
                if self.verbose {
                    print_info(&format!("Generating traceability matrix from: {}", path));
                }
                
                // TODO: Implement traceability matrix generation
                print_warning("Traceability matrix generation is not yet implemented");
                print_info(&format!("Would generate {} matrix in {}", format, output));
            }
        }
        Ok(())
    }
    
    fn run_add(&self, operation: AddCommands) -> Result<()> {
        match operation {
            AddCommands::Feature { id, title, description, target } => {
                if self.verbose {
                    print_info(&format!("Adding feature: {} - {}", id, title));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let op = MigrationOperation::AddFeature {
                    id: id.clone(),
                    title: title.clone(),
                    description,
                    scenarios: None,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added feature: {}", id));
            },
            
            AddCommands::Entity { id, name, description, target } => {
                if self.verbose {
                    print_info(&format!("Adding entity: {} - {}", id, name));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let op = MigrationOperation::AddEntity {
                    id: id.clone(),
                    name: name.clone(),
                    description,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added entity: {}", id));
            },
            
            AddCommands::Action { id, name, description, target } => {
                if self.verbose {
                    print_info(&format!("Adding action: {} - {}", id, name));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let op = MigrationOperation::AddAction {
                    id: id.clone(),
                    name: name.clone(),
                    description,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added action: {}", id));
            },
            
            AddCommands::Constraint { id, name, condition, applies_to, description, message, target } => {
                if self.verbose {
                    print_info(&format!("Adding constraint: {} - {}", id, name));
                    print_info(&format!("Condition: {} (applies to: {})", condition, applies_to));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let op = MigrationOperation::AddConstraint {
                    id: id.clone(),
                    name: name.clone(),
                    description,
                    condition: condition.clone(),
                    applies_to: applies_to.clone(),
                    message,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added constraint: {}", id));
            },
            
            AddCommands::Field { entity_id, field_name, field_type, required, default, target } => {
                if self.verbose {
                    print_info(&format!("Adding field: {} ({}) to entity: {}", field_name, field_type, entity_id));
                    if let Some(ref target_file) = target {
                        print_info(&format!("Target FDML file: {}", target_file));
                    }
                }
                
                let default_value = default.map(|d| {
                    // Try to parse as different types
                    if let Ok(b) = d.parse::<bool>() {
                        serde_json::Value::Bool(b)
                    } else if let Ok(n) = d.parse::<f64>() {
                        serde_json::Value::Number(serde_json::Number::from_f64(n).unwrap())
                    } else {
                        serde_json::Value::String(d)
                    }
                });
                
                let op = MigrationOperation::AddField {
                    entity_id: entity_id.clone(),
                    field_name: field_name.clone(),
                    field_type: field_type.clone(),
                    required: Some(required),
                    default: default_value,
                };
                
                self.apply_single_operation(op, target)?;
                print_success(&format!("Successfully added field {} to entity {}", field_name, entity_id));
            },
        }
        Ok(())
    }
    
    fn run_list(&self, operation: ListCommands) -> Result<()> {
        match operation {
            ListCommands::Features { target } => {
                if self.verbose {
                    print_info("Listing features");
                    if let Some(ref target_file) = target {
                        print_info(&format!("From FDML file: {}", target_file));
                    }
                }
                
                let document = self.load_fdml_document(target)?;
                
                if document.features.is_empty() {
                    print_info("No features found");
                } else {
                    println!("Features ({}):", document.features.len());
                    for feature in &document.features {
                        println!("  • {} - {}", feature.id, feature.title);
                        if let Some(ref desc) = feature.description {
                            println!("    Description: {}", desc);
                        }
                        if !feature.scenarios.is_empty() {
                            println!("    Scenarios: {}", feature.scenarios.len());
                        }
                    }
                }
            },
            
            ListCommands::Entities { target } => {
                if self.verbose {
                    print_info("Listing entities");
                    if let Some(ref target_file) = target {
                        print_info(&format!("From FDML file: {}", target_file));
                    }
                }
                
                let document = self.load_fdml_document(target)?;
                
                if document.entities.is_empty() {
                    print_info("No entities found");
                } else {
                    println!("Entities ({}):", document.entities.len());
                    for entity in &document.entities {
                        let name = entity.name.as_deref().unwrap_or(&entity.id);
                        println!("  • {} - {}", entity.id, name);
                        if let Some(ref desc) = entity.description {
                            println!("    Description: {}", desc);
                        }
                        if !entity.fields.is_empty() {
                            println!("    Fields: {}", entity.fields.len());
                        }
                    }
                }
            },
            
            ListCommands::Actions { target } => {
                if self.verbose {
                    print_info("Listing actions");
                    if let Some(ref target_file) = target {
                        print_info(&format!("From FDML file: {}", target_file));
                    }
                }
                
                let document = self.load_fdml_document(target)?;
                
                if document.actions.is_empty() {
                    print_info("No actions found");
                } else {
                    println!("Actions ({}):", document.actions.len());
                    for action in &document.actions {
                        let name = action.name.as_deref().unwrap_or(&action.id);
                        println!("  • {} - {}", action.id, name);
                        if let Some(ref desc) = action.description {
                            println!("    Description: {}", desc);
                        }
                    }
                }
            },
            
            ListCommands::Constraints { target } => {
                if self.verbose {
                    print_info("Listing constraints");
                    if let Some(ref target_file) = target {
                        print_info(&format!("From FDML file: {}", target_file));
                    }
                }
                
                let document = self.load_fdml_document(target)?;
                
                if document.constraints.is_empty() {
                    print_info("No constraints found");
                } else {
                    println!("Constraints ({}):", document.constraints.len());
                    for constraint in &document.constraints {
                        println!("  • {} - {}", constraint.id, constraint.name);
                        if let Some(ref desc) = constraint.description {
                            println!("    Description: {}", desc);
                        }
                        println!("    Rule: {}", constraint.rule);
                    }
                }
            },
        }
        Ok(())
    }
    
    /// Apply a single migration operation directly (used for add commands)
    fn apply_single_operation(&self, operation: MigrationOperation, target: Option<String>) -> Result<()> {
        // Create a temporary migration directory for this operation
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let temp_dir = std::env::temp_dir().join(format!("fdml_direct_{}", timestamp));
        std::fs::create_dir_all(&temp_dir)?;
        
        // Create temporary migration runner
        let mut runner = MigrationRunner::new(&temp_dir);
        if let Some(target_file) = target {
            runner = runner.with_target_file(&target_file);
        } else {
            // Find default FDML file in current directory
            let current_dir = std::env::current_dir()?;
            if let Some(default_file) = self.find_default_fdml_file(&current_dir)? {
                runner = runner.with_target_file(&default_file);
            } else {
                return Err(crate::error::FdmlError::project_error(
                    "No target file specified and no FDML file found in current directory. Use --target to specify a file.".to_string()
                ));
            }
        }
        
        // Validate the operation
        runner.validate_operation(&operation)?;
        
        // Create a temporary migration file
        let migration_id = format!("direct_{}", timestamp);
        let migration = Migration {
            id: migration_id.clone(),
            title: Some("Direct CLI operation".to_string()),
            description: Some("Migration created by direct CLI command".to_string()),
            up: vec![operation],
            down: vec![], // We don't need rollback for direct operations
            dependencies: None,
        };
        
        // Write the temporary migration file
        let migration_file = temp_dir.join(format!("{}.yaml", migration_id));
        let migration_content = serde_yaml::to_string(&migration)?;
        std::fs::write(&migration_file, migration_content)?;
        
        // Apply the migration using the existing apply_migrations method
        let applied = runner.apply_migrations(false)?;
        
        if applied.is_empty() {
            print_warning("No operations were applied");
        }
        
        // Clean up temporary directory
        std::fs::remove_dir_all(&temp_dir).ok();
        
        Ok(())
    }
    
    /// Load FDML document from target file or find default
    fn load_fdml_document(&self, target: Option<String>) -> Result<crate::parser::ast::FdmlDocument> {
        let file_path = if let Some(target_file) = target {
            PathBuf::from(target_file)
        } else {
            let current_dir = std::env::current_dir()?;
            self.find_default_fdml_file(&current_dir)?.ok_or_else(|| {
                crate::error::FdmlError::project_error(
                    "No target file specified and no FDML file found in current directory. Use --target to specify a file.".to_string()
                )
            })?
        };
        
        let content = fs::read_to_string(&file_path).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read file '{}': {}", file_path.display(), e))
        })?;
        
        parse_fdml_yaml(&content)
    }
    
    /// Find the default FDML file in a directory
    fn find_default_fdml_file(&self, dir: &Path) -> Result<Option<PathBuf>> {
        let possible_files = [
            "spec.fdml",
            "specification.fdml", 
            "main.fdml",
            "app.fdml"
        ];
        
        for filename in &possible_files {
            let path = dir.join(filename);
            if path.exists() {
                return Ok(Some(path));
            }
        }
        
        // Look for any .fdml file
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("fdml") {
                        return Ok(Some(path));
                    }
                }
            }
        }
        
        Ok(None)
    }
}

impl Default for CommandRunner {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::env;
    
    #[test]
    fn test_init_command() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        
        // Change to temp directory
        env::set_current_dir(temp_dir.path()).unwrap();
        
        let runner = CommandRunner::new(false);
        let result = runner.run_init("test-project".to_string(), false);
        
        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
        
        assert!(result.is_ok());
    }
}