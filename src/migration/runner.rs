use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use crate::error::Result;

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
}

impl MigrationRunner {
    pub fn new<P: AsRef<Path>>(migration_dir: P) -> Self {
        let migration_dir = migration_dir.as_ref().to_path_buf();
        let state_file = migration_dir.join(".migration_state.json");
        
        Self {
            migration_dir,
            state_file,
        }
    }

    pub fn apply_migrations(&self, dry_run: bool) -> Result<Vec<String>> {
        let migrations = self.load_migrations()?;
        let state = self.load_state().unwrap_or_default();
        
        let pending_migrations = self.get_pending_migrations(&migrations, &state)?;
        let mut applied = Vec::new();

        for migration_id in pending_migrations {
            if let Some(migration) = migrations.get(&migration_id) {
                if dry_run {
                    println!("Would apply migration: {} - {}", 
                        migration.id, 
                        migration.title.as_deref().unwrap_or("No title"));
                } else {
                    self.apply_migration(migration)?;
                    println!("Applied migration: {} - {}", 
                        migration.id, 
                        migration.title.as_deref().unwrap_or("No title"));
                }
                applied.push(migration_id.clone());
            }
        }

        if !dry_run && !applied.is_empty() {
            self.update_state(&applied)?;
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

        let mut rolled_back = Vec::new();

        for migration_id in to_rollback {
            if let Some(migration) = migrations.get(&migration_id) {
                if dry_run {
                    println!("Would rollback migration: {} - {}", 
                        migration.id, 
                        migration.title.as_deref().unwrap_or("No title"));
                } else {
                    self.rollback_migration(migration)?;
                    println!("Rolled back migration: {} - {}", 
                        migration.id, 
                        migration.title.as_deref().unwrap_or("No title"));
                }
                rolled_back.push(migration_id);
            }
        }

        if !dry_run && !rolled_back.is_empty() {
            self.remove_from_state(&rolled_back)?;
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

    fn load_migrations(&self) -> Result<HashMap<String, Migration>> {
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

    fn get_pending_migrations(
        &self, 
        migrations: &HashMap<String, Migration>, 
        state: &MigrationState
    ) -> Result<Vec<String>> {
        let mut all_migration_ids: Vec<_> = migrations.keys().cloned().collect();
        all_migration_ids.sort();
        
        let pending: Vec<_> = all_migration_ids
            .into_iter()
            .filter(|id| !state.applied_migrations.contains(id))
            .collect();
            
        Ok(pending)
    }

    fn apply_migration(&self, migration: &Migration) -> Result<()> {
        for operation in &migration.up {
            self.execute_operation(operation)?;
        }
        Ok(())
    }

    fn rollback_migration(&self, migration: &Migration) -> Result<()> {
        for operation in migration.down.iter().rev() {
            self.execute_operation(operation)?;
        }
        Ok(())
    }

    fn execute_operation(&self, operation: &MigrationOperation) -> Result<()> {
        match operation {
            MigrationOperation::AddFeature { id, title, .. } => {
                println!("  + Adding feature: {} - {}", id, title);
                // TODO: Implement actual feature addition logic
            },
            MigrationOperation::RemoveFeature { id } => {
                println!("  - Removing feature: {}", id);
                // TODO: Implement actual feature removal logic
            },
            MigrationOperation::ModifyEntity { id, changes } => {
                println!("  ~ Modifying entity: {}", id);
                if let Some(name) = &changes.name {
                    println!("    - Changing name to: {}", name);
                }
                // TODO: Implement actual entity modification logic
            },
            MigrationOperation::AddField { entity_id, field_name, field_type, .. } => {
                println!("  + Adding field {} ({}) to entity {}", field_name, field_type, entity_id);
                // TODO: Implement actual field addition logic
            },
            MigrationOperation::RemoveField { entity_id, field_name } => {
                println!("  - Removing field {} from entity {}", field_name, entity_id);
                // TODO: Implement actual field removal logic
            },
            MigrationOperation::UpdateAction { id, changes } => {
                println!("  ~ Updating action: {}", id);
                if let Some(name) = &changes.name {
                    println!("    - Changing name to: {}", name);
                }
                // TODO: Implement actual action update logic
            },
            MigrationOperation::ChangeValidation { target_id, target_type, validation_rules } => {
                println!("  ~ Changing validation for {} ({}): {:?}", target_id, target_type, validation_rules);
                // TODO: Implement actual validation change logic
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