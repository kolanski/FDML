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
            Commands::ParseCode { input, output, format, exclude } => {
                self.run_parse_code(input, output, format, exclude)
            },
            Commands::LinkCode { code, fdml, output, format, llm, fast, model } => {
                self.run_link_code(code, fdml, output, format, llm, fast, model)
            },
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
    
    fn run_parse_code(&self, input: String, output: Option<String>, format: String, exclude: Vec<String>) -> Result<()> {
        if self.verbose {
            print_info(&format!("Scanning source code in: {}", input));
        }

        let result = crate::scanner::scan_project(&input, &exclude)?;

        // Format output
        let output_str = match format.as_str() {
            "json" => serde_json::to_string_pretty(&result)
                .map_err(|e| crate::error::FdmlError::project_error(format!("JSON serialization error: {}", e)))?,
            "yaml" | _ => serde_yaml::to_string(&result)
                .map_err(|e| crate::error::FdmlError::project_error(format!("YAML serialization error: {}", e)))?,
        };

        // Write to file or stdout
        if let Some(output_path) = output {
            fs::write(&output_path, &output_str).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to write output to {}: {}", output_path, e))
            })?;
            print_success(&format!("Code analysis written to: {}", output_path));
        } else {
            println!("{}", output_str);
        }

        // Print summary
        let stats = &result.statistics;
        print_success(&format!(
            "Scan complete: {} files, {} classes, {} functions, {} methods, {} fields, {} relationships",
            result.metadata.total_files,
            stats.classes,
            stats.functions,
            stats.methods,
            stats.fields,
            stats.relationships,
        ));
        print_info(&format!(
            "Languages: {:?} | External imports: {} | Internal imports: {}",
            result.metadata.languages_detected.iter().map(|l| l.name()).collect::<Vec<_>>(),
            stats.imports_external,
            stats.imports_internal,
        ));

        Ok(())
    }

    fn run_link_code(
        &self,
        code: String,
        fdml: Option<String>,
        output: Option<String>,
        format: String,
        llm: bool,
        fast: bool,
        model: Option<String>,
    ) -> Result<()> {
        if self.verbose {
            print_info(&format!("Linking code inventory: {}", code));
            if let Some(ref spec) = fdml {
                print_info(&format!("FDML spec: {}", spec));
            }
        }

        // Load inventory
        let inventory_content = fs::read_to_string(&code).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read inventory file '{}': {}", code, e))
        })?;
        let scan: crate::scanner::types::ScanResult = serde_yaml::from_str(&inventory_content)
            .or_else(|_| serde_json::from_str(&inventory_content).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to parse inventory (tried YAML and JSON): {}", e))
            }))?;

        // Load spec if provided
        let spec_doc = if let Some(ref spec_path) = fdml {
            let content = fs::read_to_string(spec_path).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to read spec file '{}': {}", spec_path, e))
            })?;
            Some(crate::parser::parse_fdml_yaml(&content)?)
        } else {
            None
        };

        // Run linking
        let report = crate::linker::link_code(
            &scan,
            spec_doc.as_ref(),
            &code,
            fdml.as_deref(),
        );

        // Generate metaprompt
        let metaprompt = crate::linker::generate_metaprompt(&report, &scan);

        // If --llm flag, send to claude CLI
        if llm {
            let llm_result = self.call_llm(&metaprompt, fast, model.as_deref())?;

            // Write LLM result
            if let Some(ref output_path) = output {
                fs::write(output_path, &llm_result).map_err(|e| {
                    crate::error::FdmlError::project_error(format!("Failed to write LLM output to {}: {}", output_path, e))
                })?;
                print_success(&format!("LLM-generated FDML spec written to: {}", output_path));

                // Also save the prompt used
                let prompt_path = format!("{}.prompt.md", output_path.trim_end_matches(".yaml").trim_end_matches(".fdml"));
                fs::write(&prompt_path, &metaprompt).map_err(|e| {
                    crate::error::FdmlError::project_error(format!("Failed to write prompt to {}: {}", prompt_path, e))
                })?;
                print_info(&format!("Prompt saved to: {}", prompt_path));
            } else {
                println!("{}", llm_result);
            }

            return Ok(());
        }

        // Format output — include both report and metaprompt
        let output_str = match format.as_str() {
            "json" => serde_json::to_string_pretty(&report)
                .map_err(|e| crate::error::FdmlError::project_error(format!("JSON serialization error: {}", e)))?,
            "prompt" => metaprompt.clone(),
            "yaml" | _ => {
                let yaml = serde_yaml::to_string(&report)
                    .map_err(|e| crate::error::FdmlError::project_error(format!("YAML serialization error: {}", e)))?;
                yaml
            }
        };

        // Write to file or stdout
        if let Some(ref output_path) = output {
            fs::write(output_path, &output_str).map_err(|e| {
                crate::error::FdmlError::project_error(format!("Failed to write output to {}: {}", output_path, e))
            })?;
            print_success(&format!("Linking report written to: {}", output_path));

            // Also write metaprompt alongside if format is not "prompt"
            if format != "prompt" {
                let prompt_path = format!("{}.prompt.md", output_path.trim_end_matches(".yaml").trim_end_matches(".json"));
                fs::write(&prompt_path, &metaprompt).map_err(|e| {
                    crate::error::FdmlError::project_error(format!("Failed to write prompt to {}: {}", prompt_path, e))
                })?;
                print_success(&format!("LLM metaprompt written to: {}", prompt_path));
            }
        } else {
            println!("{}", output_str);
        }

        // Print summary
        let matched_entities = report.entities.iter().filter(|e| matches!(e.source, crate::linker::types::LinkSource::Matched)).count();
        let matched_actions = report.actions.iter().filter(|a| matches!(a.source, crate::linker::types::LinkSource::Matched)).count();
        print_success(&format!(
            "Link complete: {} entity candidates ({} matched), {} action candidates ({} matched), {} feature suggestions",
            report.entities.len(), matched_entities,
            report.actions.len(), matched_actions,
            report.features.len(),
        ));
        print_info(&format!(
            "Traceability links: {} | Unlinked code: {} | Unlinked spec: {}",
            report.traceability.len(),
            report.unlinked_code.len(),
            report.unlinked_spec.len(),
        ));
        print_info(&format!(
            "Coverage — spec: {:.0}% | code: {:.0}%",
            report.coverage.spec_coverage.percentage,
            report.coverage.code_coverage.percentage,
        ));

        Ok(())
    }

    /// Call LLM — tries Anthropic API first (if key set), falls back to claude CLI
    fn call_llm(&self, prompt: &str, fast: bool, model: Option<&str>) -> Result<String> {
        let prompt_lines = prompt.lines().count();
        let prompt_bytes = prompt.len();
        eprintln!("  ℹ Prompt: {} lines, {:.1} KB", prompt_lines, prompt_bytes as f64 / 1024.0);

        // Try ANTHROPIC_API_KEY first (fast, no Node.js overhead)
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            return self.call_anthropic_api(&api_key, prompt, fast, model);
        }

        // Fall back to claude CLI
        self.call_claude_cli(prompt, fast, model)
    }

    /// Call Anthropic API directly via curl (requires ANTHROPIC_API_KEY)
    fn call_anthropic_api(&self, api_key: &str, prompt: &str, fast: bool, model: Option<&str>) -> Result<String> {
        use std::process::Command;

        let model_name = match model {
            Some("haiku") => "claude-haiku-4-5-20251001".to_string(),
            Some("sonnet") => "claude-sonnet-4-5-20250929".to_string(),
            Some("opus") => "claude-opus-4-6".to_string(),
            Some(m) => m.to_string(),
            None if fast => "claude-haiku-4-5-20251001".to_string(),
            None => "claude-sonnet-4-5-20250929".to_string(),
        };

        eprintln!("  ℹ Model: {}", model_name);
        eprintln!("  ℹ Provider: Anthropic API (curl)");

        let system_msg = "You are an FDML specification generator. Output ONLY valid YAML — no markdown fences, no explanations. Start directly with YAML content.";

        // Use serde_json to build the request (safe escaping)
        let request = serde_json::json!({
            "model": model_name,
            "max_tokens": 8192,
            "system": system_msg,
            "messages": [{"role": "user", "content": prompt}]
        });

        let tmp_body = std::env::temp_dir().join("fdml_llm_request.json");
        fs::write(&tmp_body, request.to_string()).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to write request: {}", e))
        })?;

        eprintln!("  ℹ Sending request...");

        let output = Command::new("curl")
            .arg("-s")
            .arg("-X").arg("POST")
            .arg("https://api.anthropic.com/v1/messages")
            .arg("-H").arg(format!("x-api-key: {}", api_key))
            .arg("-H").arg("anthropic-version: 2023-06-01")
            .arg("-H").arg("content-type: application/json")
            .arg("-d").arg(format!("@{}", tmp_body.display()))
            .output()
            .map_err(|e| {
                crate::error::FdmlError::project_error(format!("curl failed: {}", e))
            })?;

        let _ = fs::remove_file(&tmp_body);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::FdmlError::project_error(format!("curl error: {}", stderr)));
        }

        let response_str = String::from_utf8_lossy(&output.stdout).to_string();
        let json: serde_json::Value = serde_json::from_str(&response_str).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Bad API response: {}", e))
        })?;

        if let Some(error) = json.get("error") {
            let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown");
            return Err(crate::error::FdmlError::project_error(format!("API error: {}", msg)));
        }

        let text = json.get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| crate::error::FdmlError::project_error("No text in API response".to_string()))?;

        if let Some(usage) = json.get("usage") {
            let inp = usage.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
            let out = usage.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
            eprintln!("  ℹ Tokens: {} in, {} out", inp, out);
        }
        eprintln!("  ℹ Response: {} lines", text.lines().count());

        print_success("LLM response received (API)");
        Ok(Self::strip_yaml_fences(text))
    }

    /// Call claude CLI subprocess
    /// Usage: cat prompt.md | claude -p "instruction" --output-format text --max-turns 1
    /// See: https://code.claude.com/docs/en/cli-reference
    fn call_claude_cli(&self, prompt: &str, fast: bool, model: Option<&str>) -> Result<String> {
        use std::process::Command;

        let model_name = if let Some(m) = model {
            m.to_string()
        } else if fast {
            "haiku".to_string()
        } else {
            "sonnet".to_string()
        };

        eprintln!("  ℹ Provider: claude CLI");
        eprintln!("  ℹ Model: {}", model_name);

        // Write prompt to temp file for piping via stdin
        let tmp_prompt = std::env::temp_dir().join("fdml_link_prompt.md");
        fs::write(&tmp_prompt, prompt).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to write prompt: {}", e))
        })?;

        let system_prompt = "You are an FDML specification generator. Output ONLY valid YAML — no markdown fences, no explanations. Start directly with YAML content.";

        eprintln!("  ℹ Running: cat prompt | claude -p ... --max-turns 1 --output-format text");

        // cat file | claude -p "query" --model X --output-format text --max-turns 1 --no-session-persistence
        let shell_cmd = format!(
            "cat {} | claude -p \"Generate a complete FDML YAML specification from this analysis\" \
             --model {} \
             --system-prompt \"{}\" \
             --output-format text \
             --max-turns 1 \
             --no-session-persistence",
            tmp_prompt.display(),
            model_name,
            system_prompt.replace('"', "\\\""),
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(&shell_cmd)
            .output()
            .map_err(|e| {
                crate::error::FdmlError::project_error(format!("claude CLI failed to start: {}", e))
            })?;

        let _ = fs::remove_file(&tmp_prompt);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::FdmlError::project_error(
                format!("claude CLI error (exit {}): {}", output.status, stderr)
            ));
        }

        let response = String::from_utf8_lossy(&output.stdout).to_string();
        if response.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::FdmlError::project_error(
                format!("claude CLI empty response. stderr: {}", stderr)
            ));
        }

        eprintln!("  ℹ Response: {} lines", response.lines().count());
        print_success("LLM response received (CLI)");
        Ok(Self::strip_yaml_fences(&response))
    }

    /// Strip markdown YAML fences from LLM output
    fn strip_yaml_fences(text: &str) -> String {
        let cleaned = text.trim();
        let cleaned = if cleaned.starts_with("```yaml") || cleaned.starts_with("```yml") {
            let start = cleaned.find('\n').unwrap_or(0) + 1;
            let end = cleaned.rfind("```").unwrap_or(cleaned.len());
            &cleaned[start..end]
        } else if cleaned.starts_with("```") {
            let start = cleaned.find('\n').unwrap_or(0) + 1;
            let end = cleaned.rfind("```").unwrap_or(cleaned.len());
            &cleaned[start..end]
        } else {
            cleaned
        };
        cleaned.trim().to_string()
    }

    /// Apply a single migration operation directly (used for add commands)
    fn apply_single_operation(&self, operation: MigrationOperation, target: Option<String>) -> Result<()> {
        // Determine target file
        let target_file = if let Some(target_file) = target {
            let path = PathBuf::from(target_file);
            // Check if the target file exists for direct operations
            if !path.exists() {
                return Err(crate::error::FdmlError::project_error(
                    format!("Target file '{}' does not exist. Use 'fdml init' to create a new project first.", path.display())
                ));
            }
            path
        } else {
            // Find default FDML file in current directory
            let current_dir = std::env::current_dir()?;
            self.find_default_fdml_file(&current_dir)?.ok_or_else(|| {
                crate::error::FdmlError::project_error(
                    "No target file specified and no FDML file found in current directory. Use --target to specify a file.".to_string()
                )
            })?
        };
        
        // Create a temporary migration directory for this operation with a unique name
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%f");
        let temp_dir = std::env::temp_dir().join(format!("fdml_direct_{}", timestamp));
        std::fs::create_dir_all(&temp_dir)?;
        
        // Create temporary migration runner
        let runner = MigrationRunner::new(&temp_dir).with_target_file(&target_file);
        
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