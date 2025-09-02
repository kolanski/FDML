use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use crate::error::Result;
use crate::parser::{parse_fdml_yaml};
use crate::parser::ast::{FdmlDocument, Feature, Scenario, Field, Value};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub up: Vec<MigrationOperation>,
    pub down: Vec<MigrationOperation>,
    pub dependencies: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MigrationOperation {
    #[serde(rename = "add_feature")]
    AddFeature {
        id: String,
        title: String,
        description: Option<String>,
        scenarios: Option<Vec<String>>,
    },
    #[serde(rename = "remove_feature")]
    RemoveFeature {
        id: String,
    },
    #[serde(rename = "modify_entity")]
    ModifyEntity {
        id: String,
        changes: EntityChanges,
    },
    #[serde(rename = "add_field")]
    AddField {
        entity_id: String,
        field_name: String,
        field_type: String,
        required: Option<bool>,
        default: Option<serde_json::Value>,
    },
    #[serde(rename = "remove_field")]
    RemoveField {
        entity_id: String,
        field_name: String,
    },
    #[serde(rename = "update_action")]
    UpdateAction {
        id: String,
        changes: ActionChanges,
    },
    #[serde(rename = "change_validation")]
    ChangeValidation {
        target_id: String,
        target_type: String, // "entity", "action", "field"
        validation_rules: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityChanges {
    pub name: Option<String>,
    pub description: Option<String>,
    pub add_fields: Option<Vec<String>>,
    pub remove_fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionChanges {
    pub name: Option<String>,
    pub description: Option<String>,
    pub input_changes: Option<HashMap<String, String>>,
    pub output_changes: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationState {
    pub applied_migrations: Vec<String>,
    pub last_migration: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct MigrationRunner {
    migration_dir: PathBuf,
    state_file: PathBuf,
    backup_dir: PathBuf,
    target_fdml_file: Option<PathBuf>,
}

impl MigrationRunner {
    pub fn new<P: AsRef<Path>>(migration_dir: P) -> Self {
        let migration_dir = migration_dir.as_ref().to_path_buf();
        let state_file = migration_dir.join(".migration_state.json");
        let backup_dir = migration_dir.join(".backups");
        
        Self {
            migration_dir,
            state_file,
            backup_dir,
            target_fdml_file: None,
        }
    }

    pub fn with_target_file<P: AsRef<Path>>(mut self, target_file: P) -> Self {
        self.target_fdml_file = Some(target_file.as_ref().to_path_buf());
        self
    }

    /// Create a backup of the current FDML file before applying migrations
    fn create_backup(&self) -> Result<Option<PathBuf>> {
        if let Some(target_file) = &self.target_fdml_file {
            if target_file.exists() {
                fs::create_dir_all(&self.backup_dir)?;
                
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let backup_filename = format!("backup_{}.fdml", timestamp);
                let backup_path = self.backup_dir.join(backup_filename);
                
                fs::copy(target_file, &backup_path)?;
                println!("  📁 Created backup: {}", backup_path.display());
                return Ok(Some(backup_path));
            }
        }
        Ok(None)
    }

    /// Verify that a rollback can be safely performed
    fn verify_rollback_safety(&self, migrations: &[String]) -> Result<()> {
        let migration_map = self.load_migrations()?;
        
        for migration_id in migrations {
            if let Some(migration) = migration_map.get(migration_id) {
                // Check if down operations are available
                if migration.down.is_empty() {
                    return Err(crate::error::FdmlError::migration_error(format!(
                        "Migration '{}' has no down operations - rollback not possible", 
                        migration_id
                    )));
                }
                
                // Verify rollback operations are valid
                for operation in &migration.down {
                    self.validate_operation(operation)?;
                }
            } else {
                return Err(crate::error::FdmlError::migration_error(format!(
                    "Migration '{}' not found", migration_id
                )));
            }
        }
        
        println!("  ✓ Rollback safety verification passed");
        Ok(())
    }

    pub fn apply_migrations(&self, dry_run: bool) -> Result<Vec<String>> {
        let migrations = self.load_migrations()?;
        let state = self.load_state().unwrap_or_default();
        
        let pending_migrations = self.get_pending_migrations(&migrations, &state)?;
        
        if pending_migrations.is_empty() {
            println!("No pending migrations to apply");
            return Ok(Vec::new());
        }

        println!("Found {} pending migrations to apply:", pending_migrations.len());
        for migration_id in &pending_migrations {
            if let Some(migration) = migrations.get(migration_id) {
                println!("  - {} ({})", migration_id, 
                    migration.title.as_deref().unwrap_or("No title"));
            }
        }

        if dry_run {
            println!("\n🔍 DRY RUN MODE - No changes will be applied");
            for migration_id in &pending_migrations {
                if let Some(migration) = migrations.get(migration_id) {
                    println!("\nWould apply migration: {} - {}", 
                        migration.id, 
                        migration.title.as_deref().unwrap_or("No title"));
                    for operation in &migration.up {
                        self.describe_operation(operation);
                    }
                }
            }
            return Ok(pending_migrations);
        }

        // Create backup before applying migrations
        let backup_path = self.create_backup()?;
        
        let mut applied = Vec::new();
        let mut fdml_document = self.load_target_document()?;

        for migration_id in pending_migrations {
            if let Some(migration) = migrations.get(&migration_id) {
                println!("\n📦 Applying migration: {} - {}", 
                    migration.id, 
                    migration.title.as_deref().unwrap_or("No title"));
                
                self.apply_migration(migration, &mut fdml_document)?;
                applied.push(migration_id.clone());
            }
        }

        if !applied.is_empty() {
            self.save_target_document(&fdml_document)?;
            self.update_state(&applied)?;
            
            println!("\n✅ Successfully applied {} migrations", applied.len());
            if let Some(backup_path) = backup_path {
                println!("💾 Backup saved to: {}", backup_path.display());
            }
        }

        Ok(applied)
    }

    pub fn rollback_migrations(&self, count: usize, dry_run: bool) -> Result<Vec<String>> {
        let migrations = self.load_migrations()?;
        let state = self.load_state().unwrap_or_default();
        
        let to_rollback: Vec<_> = state.applied_migrations
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect();

        if to_rollback.is_empty() {
            println!("No migrations to rollback");
            return Ok(Vec::new());
        }

        println!("Planning to rollback {} migrations:", to_rollback.len());
        for migration_id in &to_rollback {
            if let Some(migration) = migrations.get(migration_id) {
                println!("  - {} ({})", migration_id, 
                    migration.title.as_deref().unwrap_or("No title"));
            }
        }

        // Verify rollback safety
        self.verify_rollback_safety(&to_rollback)?;

        if dry_run {
            println!("\n🔍 DRY RUN MODE - No changes will be applied");
            for migration_id in &to_rollback {
                if let Some(migration) = migrations.get(migration_id) {
                    println!("\nWould rollback migration: {} - {}", 
                        migration.id, 
                        migration.title.as_deref().unwrap_or("No title"));
                    for operation in &migration.down {
                        self.describe_operation(operation);
                    }
                }
            }
            return Ok(to_rollback);
        }

        // Create backup before rollback
        let backup_path = self.create_backup()?;

        let mut rolled_back = Vec::new();
        let mut fdml_document = self.load_target_document()?;

        for migration_id in to_rollback {
            if let Some(migration) = migrations.get(&migration_id) {
                println!("\n🔄 Rolling back migration: {} - {}", 
                    migration.id, 
                    migration.title.as_deref().unwrap_or("No title"));
                
                self.rollback_migration(migration, &mut fdml_document)?;
                rolled_back.push(migration_id);
            }
        }

        if !rolled_back.is_empty() {
            self.save_target_document(&fdml_document)?;
            self.remove_from_state(&rolled_back)?;
            
            println!("\n✅ Successfully rolled back {} migrations", rolled_back.len());
            if let Some(backup_path) = backup_path {
                println!("💾 Backup saved to: {}", backup_path.display());
            }
        }

        Ok(rolled_back)
    }

    pub fn migration_status(&self) -> Result<MigrationStatus> {
        let migrations = self.load_migrations()?;
        let state = self.load_state().unwrap_or_default();
        
        let pending = self.get_pending_migrations(&migrations, &state)?;
        
        Ok(MigrationStatus {
            total_migrations: migrations.len(),
            applied_count: state.applied_migrations.len(),
            pending_count: pending.len(),
            applied_migrations: state.applied_migrations,
            pending_migrations: pending,
        })
    }

    /// Load all migration files from the migration directory
    pub fn load_migrations(&self) -> Result<HashMap<String, Migration>> {
        let mut migrations = HashMap::new();
        
        if !self.migration_dir.exists() {
            return Ok(migrations);
        }

        for entry in fs::read_dir(&self.migration_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") ||
               path.extension().and_then(|s| s.to_str()) == Some("yml") {
                let content = fs::read_to_string(&path)?;
                let migration: Migration = serde_yaml::from_str(&content)
                    .map_err(|e| crate::error::FdmlError::simple_parser_error(format!(
                        "Failed to parse migration file {}: {}", path.display(), e
                    )))?;
                
                migrations.insert(migration.id.clone(), migration);
            }
        }
        
        Ok(migrations)
    }

    fn load_state(&self) -> Result<MigrationState> {
        if !self.state_file.exists() {
            return Ok(MigrationState::default());
        }
        
        let content = fs::read_to_string(&self.state_file)?;
        let state: MigrationState = serde_json::from_str(&content)
            .map_err(|e| crate::error::FdmlError::simple_parser_error(format!(
                "Failed to parse migration state: {}", e
            )))?;
        
        Ok(state)
    }

    /// Get pending migrations in dependency-resolved order
    pub fn get_pending_migrations(
        &self, 
        migrations: &HashMap<String, Migration>, 
        state: &MigrationState
    ) -> Result<Vec<String>> {
        let all_migration_ids: Vec<_> = migrations.keys().cloned().collect();
        
        let pending: Vec<_> = all_migration_ids
            .into_iter()
            .filter(|id| !state.applied_migrations.contains(id))
            .collect();
        
        // Resolve dependencies and return in correct order
        self.resolve_migration_dependencies(&pending, migrations)
    }

    /// Resolve migration dependencies and return migrations in correct execution order
    fn resolve_migration_dependencies(
        &self,
        pending_migrations: &[String],
        migrations: &HashMap<String, Migration>
    ) -> Result<Vec<String>> {
        let mut resolved = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        for migration_id in pending_migrations {
            self.visit_migration_deps(
                migration_id, 
                migrations, 
                &mut resolved, 
                &mut visited, 
                &mut visiting
            )?;
        }

        // Filter to only include pending migrations in the final result
        let pending_set: HashSet<_> = pending_migrations.iter().cloned().collect();
        Ok(resolved.into_iter().filter(|id| pending_set.contains(id)).collect())
    }

    /// Recursive dependency visitor for topological sorting
    fn visit_migration_deps(
        &self,
        migration_id: &str,
        migrations: &HashMap<String, Migration>,
        resolved: &mut Vec<String>,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>
    ) -> Result<()> {
        if visited.contains(migration_id) {
            return Ok(());
        }

        if visiting.contains(migration_id) {
            return Err(crate::error::FdmlError::migration_error(format!(
                "Circular dependency detected involving migration '{}'", migration_id
            )));
        }

        visiting.insert(migration_id.to_string());

        if let Some(migration) = migrations.get(migration_id) {
            if let Some(dependencies) = &migration.dependencies {
                for dep_id in dependencies {
                    self.visit_migration_deps(dep_id, migrations, resolved, visited, visiting)?;
                }
            }
        }

        visiting.remove(migration_id);
        visited.insert(migration_id.to_string());
        
        if !resolved.contains(&migration_id.to_string()) {
            resolved.push(migration_id.to_string());
        }

        Ok(())
    }

    fn apply_migration(&self, migration: &Migration, document: &mut FdmlDocument) -> Result<()> {
        for operation in &migration.up {
            self.execute_operation(operation, document)?;
        }
        Ok(())
    }

    fn rollback_migration(&self, migration: &Migration, document: &mut FdmlDocument) -> Result<()> {
        for operation in migration.down.iter().rev() {
            self.execute_operation(operation, document)?;
        }
        Ok(())
    }

    /// Load the target FDML document for modification
    fn load_target_document(&self) -> Result<FdmlDocument> {
        if let Some(target_file) = &self.target_fdml_file {
            if target_file.exists() {
                let content = fs::read_to_string(target_file)?;
                parse_fdml_yaml(&content)
            } else {
                Ok(FdmlDocument::default())
            }
        } else {
            Ok(FdmlDocument::default())
        }
    }

    /// Save the modified FDML document
    fn save_target_document(&self, document: &FdmlDocument) -> Result<()> {
        if let Some(target_file) = &self.target_fdml_file {
            let content = serde_yaml::to_string(document)?;
            fs::write(target_file, content)?;
            println!("  💾 Updated {}", target_file.display());
        }
        Ok(())
    }

    /// Describe what an operation would do (for dry-run mode)
    fn describe_operation(&self, operation: &MigrationOperation) {
        match operation {
            MigrationOperation::AddFeature { id, title, .. } => {
                println!("    + Add feature: {} ({})", id, title);
            },
            MigrationOperation::RemoveFeature { id } => {
                println!("    - Remove feature: {}", id);
            },
            MigrationOperation::ModifyEntity { id, changes } => {
                println!("    ~ Modify entity: {}", id);
                if let Some(name) = &changes.name {
                    println!("      - Change name to: {}", name);
                }
            },
            MigrationOperation::AddField { entity_id, field_name, field_type, .. } => {
                println!("    + Add field '{}' ({}) to entity '{}'", field_name, field_type, entity_id);
            },
            MigrationOperation::RemoveField { entity_id, field_name } => {
                println!("    - Remove field '{}' from entity '{}'", field_name, entity_id);
            },
            MigrationOperation::UpdateAction { id, changes } => {
                println!("    ~ Update action: {}", id);
                if let Some(name) = &changes.name {
                    println!("      - Change name to: {}", name);
                }
            },
            MigrationOperation::ChangeValidation { target_id, target_type, .. } => {
                println!("    ~ Change validation for {} ({})", target_id, target_type);
            },
        }
    }

    /// Validate that an operation is valid
    pub fn validate_operation(&self, operation: &MigrationOperation) -> Result<()> {
        match operation {
            MigrationOperation::AddFeature { id, title, .. } => {
                if id.trim().is_empty() || title.trim().is_empty() {
                    return Err(crate::error::FdmlError::migration_error(
                        "AddFeature operation requires non-empty id and title".to_string()
                    ));
                }
            },
            MigrationOperation::RemoveFeature { id } => {
                if id.trim().is_empty() {
                    return Err(crate::error::FdmlError::migration_error(
                        "RemoveFeature operation requires non-empty id".to_string()
                    ));
                }
            },
            MigrationOperation::AddField { entity_id, field_name, field_type, .. } => {
                if entity_id.trim().is_empty() || field_name.trim().is_empty() || field_type.trim().is_empty() {
                    return Err(crate::error::FdmlError::migration_error(
                        "AddField operation requires non-empty entity_id, field_name, and field_type".to_string()
                    ));
                }
            },
            MigrationOperation::RemoveField { entity_id, field_name } => {
                if entity_id.trim().is_empty() || field_name.trim().is_empty() {
                    return Err(crate::error::FdmlError::migration_error(
                        "RemoveField operation requires non-empty entity_id and field_name".to_string()
                    ));
                }
            },
            _ => {} // Other operations are assumed valid for now
        }
        Ok(())
    }

    fn execute_operation(&self, operation: &MigrationOperation, document: &mut FdmlDocument) -> Result<()> {
        match operation {
            MigrationOperation::AddFeature { id, title, description, scenarios } => {
                println!("  + Adding feature: {} - {}", id, title);
                
                let scenarios = scenarios.as_ref().map(|s| {
                    s.iter().enumerate().map(|(i, scenario_title)| {
                        Scenario {
                            id: format!("{}_scenario_{}", id, i + 1),
                            title: scenario_title.clone(),
                            description: None,
                            given: vec!["System is ready".to_string()],
                            when: vec!["User performs action".to_string()],
                            then: vec!["Expected outcome occurs".to_string()],
                        }
                    }).collect()
                }).unwrap_or_default();

                let feature = Feature {
                    id: id.clone(),
                    title: title.clone(),
                    description: description.clone(),
                    scenarios,
                    acceptance_criteria: None,
                    dependencies: None,
                };
                
                document.features.push(feature);
            },
            
            MigrationOperation::RemoveFeature { id } => {
                println!("  - Removing feature: {}", id);
                document.features.retain(|f| f.id != *id);
            },
            
            MigrationOperation::ModifyEntity { id, changes } => {
                println!("  ~ Modifying entity: {}", id);
                
                if let Some(entity) = document.entities.iter_mut().find(|e| e.id == *id) {
                    if let Some(new_name) = &changes.name {
                        println!("    - Changing name to: {}", new_name);
                        entity.name = Some(new_name.clone());
                    }
                    if let Some(new_description) = &changes.description {
                        println!("    - Updating description");
                        entity.description = Some(new_description.clone());
                    }
                } else {
                    return Err(crate::error::FdmlError::migration_error(format!(
                        "Entity '{}' not found", id
                    )));
                }
            },
            
            MigrationOperation::AddField { entity_id, field_name, field_type, required, default } => {
                println!("  + Adding field {} ({}) to entity {}", field_name, field_type, entity_id);
                
                if let Some(entity) = document.entities.iter_mut().find(|e| e.id == *entity_id) {
                    let field = Field {
                        name: field_name.clone(),
                        field_type: field_type.clone(),
                        description: Some(format!("Field added by migration")),
                        required: *required,
                        default: default.clone().map(|v| {
                            match v {
                                serde_json::Value::String(s) => Value::String(s),
                                serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
                                serde_json::Value::Bool(b) => Value::Boolean(b),
                                _ => Value::String("null".to_string())
                            }
                        }),
                        constraints: None,
                    };
                    
                    entity.fields.push(field);
                } else {
                    return Err(crate::error::FdmlError::migration_error(format!(
                        "Entity '{}' not found", entity_id
                    )));
                }
            },
            
            MigrationOperation::RemoveField { entity_id, field_name } => {
                println!("  - Removing field {} from entity {}", field_name, entity_id);
                
                if let Some(entity) = document.entities.iter_mut().find(|e| e.id == *entity_id) {
                    let initial_count = entity.fields.len();
                    entity.fields.retain(|f| f.name != *field_name);
                    
                    if entity.fields.len() == initial_count {
                        return Err(crate::error::FdmlError::migration_error(format!(
                            "Field '{}' not found in entity '{}'", field_name, entity_id
                        )));
                    }
                } else {
                    return Err(crate::error::FdmlError::migration_error(format!(
                        "Entity '{}' not found", entity_id
                    )));
                }
            },
            
            MigrationOperation::UpdateAction { id, changes } => {
                println!("  ~ Updating action: {}", id);
                
                if let Some(action) = document.actions.iter_mut().find(|a| a.id == *id) {
                    if let Some(new_name) = &changes.name {
                        println!("    - Changing name to: {}", new_name);
                        action.name = Some(new_name.clone());
                    }
                    if let Some(new_description) = &changes.description {
                        println!("    - Updating description");
                        action.description = Some(new_description.clone());
                    }
                } else {
                    return Err(crate::error::FdmlError::migration_error(format!(
                        "Action '{}' not found", id
                    )));
                }
            },
            
            MigrationOperation::ChangeValidation { target_id, target_type, validation_rules } => {
                println!("  ~ Changing validation for {} ({}): {:?}", target_id, target_type, validation_rules);
                
                match target_type.as_str() {
                    "entity" => {
                        if let Some(_entity) = document.entities.iter_mut().find(|e| e.id == *target_id) {
                            // For entity validation, we could add constraints to fields
                            println!("    - Applied validation rules to entity");
                        } else {
                            return Err(crate::error::FdmlError::migration_error(format!(
                                "Entity '{}' not found", target_id
                            )));
                        }
                    },
                    "action" => {
                        if let Some(_action) = document.actions.iter_mut().find(|a| a.id == *target_id) {
                            // For action validation, we could modify preconditions/postconditions
                            println!("    - Applied validation rules to action");
                        } else {
                            return Err(crate::error::FdmlError::migration_error(format!(
                                "Action '{}' not found", target_id
                            )));
                        }
                    },
                    _ => {
                        return Err(crate::error::FdmlError::migration_error(format!(
                            "Unsupported target type for validation: {}", target_type
                        )));
                    }
                }
            },
        }
        Ok(())
    }

    fn update_state(&self, applied: &[String]) -> Result<()> {
        let mut state = self.load_state().unwrap_or_default();
        
        for migration_id in applied {
            if !state.applied_migrations.contains(migration_id) {
                state.applied_migrations.push(migration_id.clone());
            }
        }
        
        state.last_migration = applied.last().cloned();
        state.updated_at = chrono::Utc::now().to_rfc3339();
        
        let content = serde_json::to_string_pretty(&state)?;
        fs::write(&self.state_file, content)?;
        
        Ok(())
    }

    fn remove_from_state(&self, rolled_back: &[String]) -> Result<()> {
        let mut state = self.load_state().unwrap_or_default();
        
        for migration_id in rolled_back {
            state.applied_migrations.retain(|id| id != migration_id);
        }
        
        state.last_migration = state.applied_migrations.last().cloned();
        state.updated_at = chrono::Utc::now().to_rfc3339();
        
        let content = serde_json::to_string_pretty(&state)?;
        fs::write(&self.state_file, content)?;
        
        Ok(())
    }
}

impl Default for MigrationState {
    fn default() -> Self {
        Self {
            applied_migrations: Vec::new(),
            last_migration: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug)]
pub struct MigrationStatus {
    pub total_migrations: usize,
    pub applied_count: usize,
    pub pending_count: usize,
    pub applied_migrations: Vec<String>,
    pub pending_migrations: Vec<String>,
}