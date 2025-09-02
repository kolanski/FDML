use crate::cli::args::{Cli, Commands, MigrateCommands, TraceCommands};
use crate::error::{print_error, print_info, print_success, print_warning, Result};
use crate::parser::{parse_fdml_yaml, parse_fdml};
use crate::project::ProjectInitializer;
use crate::validator::Validator;
use crate::generators::{create_generator, GeneratorConfig};
use crate::generators::test_gen::TestGenerator;
use crate::migration::MigrationRunner;
use std::fs;
use std::path::Path;

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