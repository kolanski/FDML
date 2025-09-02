#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use crate::parser::ast::*;
    use crate::migration::{Migration, MigrationOperation, MigrationRunner, MigrationState};

    fn create_test_migration_file(dir: &std::path::Path, filename: &str, migration: &Migration) {
        let content = serde_yaml::to_string(migration).unwrap();
        fs::write(dir.join(filename), content).unwrap();
    }

    fn create_test_fdml_file(path: &std::path::Path) {
        let content = r#"
metadata:
  version: "1.3"
  author: "Test"

entities:
  - id: user
    name: "User"
    fields:
      - name: id
        type: string
        required: true
      - name: email
        type: string
        required: true

features:
  - id: user_auth
    title: "User Authentication"
    scenarios:
      - id: login
        title: "User can login"
        given: ["User exists"]
        when: ["User provides credentials"]
        then: ["User is authenticated"]
"#;
        fs::write(path, content).unwrap();
    }

    #[test]
    fn test_migration_dependency_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("migrations");
        fs::create_dir_all(&migration_dir).unwrap();

        // Create migrations with dependencies
        let migration1 = Migration {
            id: "001_add_user".to_string(),
            title: Some("Add User".to_string()),
            description: None,
            up: vec![MigrationOperation::AddFeature {
                id: "user_management".to_string(),
                title: "User Management".to_string(),
                description: None,
                scenarios: None,
            }],
            down: vec![MigrationOperation::RemoveFeature {
                id: "user_management".to_string(),
            }],
            dependencies: None,
        };

        let migration2 = Migration {
            id: "002_add_profile".to_string(),
            title: Some("Add Profile".to_string()),
            description: None,
            up: vec![MigrationOperation::AddField {
                entity_id: "user".to_string(),
                field_name: "profile".to_string(),
                field_type: "string".to_string(),
                required: Some(false),
                default: None,
            }],
            down: vec![MigrationOperation::RemoveField {
                entity_id: "user".to_string(),
                field_name: "profile".to_string(),
            }],
            dependencies: Some(vec!["001_add_user".to_string()]),
        };

        create_test_migration_file(&migration_dir, "001_add_user.yaml", &migration1);
        create_test_migration_file(&migration_dir, "002_add_profile.yaml", &migration2);

        let runner = MigrationRunner::new(&migration_dir);
        let migrations = runner.load_migrations().unwrap();
        let state = MigrationState::default();
        
        let pending = runner.get_pending_migrations(&migrations, &state).unwrap();
        
        // Should resolve dependencies correctly: 001 before 002
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0], "001_add_user");
        assert_eq!(pending[1], "002_add_profile");
    }

    #[test]
    fn test_circular_dependency_detection() {
        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("migrations");
        fs::create_dir_all(&migration_dir).unwrap();

        let migration1 = Migration {
            id: "001_circular".to_string(),
            title: Some("Circular 1".to_string()),
            description: None,
            up: vec![],
            down: vec![],
            dependencies: Some(vec!["002_circular".to_string()]),
        };

        let migration2 = Migration {
            id: "002_circular".to_string(),
            title: Some("Circular 2".to_string()),
            description: None,
            up: vec![],
            down: vec![],
            dependencies: Some(vec!["001_circular".to_string()]),
        };

        create_test_migration_file(&migration_dir, "001_circular.yaml", &migration1);
        create_test_migration_file(&migration_dir, "002_circular.yaml", &migration2);

        let runner = MigrationRunner::new(&migration_dir);
        let migrations = runner.load_migrations().unwrap();
        let state = MigrationState::default();
        
        let result = runner.get_pending_migrations(&migrations, &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular dependency"));
    }

    #[test]
    fn test_add_feature_operation() {
        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("migrations");
        let target_file = temp_dir.path().join("test.fdml");
        fs::create_dir_all(&migration_dir).unwrap();

        create_test_fdml_file(&target_file);

        let migration = Migration {
            id: "001_add_feature".to_string(),
            title: Some("Add Feature".to_string()),
            description: None,
            up: vec![MigrationOperation::AddFeature {
                id: "new_feature".to_string(),
                title: "New Feature".to_string(),
                description: Some("A new feature".to_string()),
                scenarios: Some(vec!["Scenario 1".to_string()]),
            }],
            down: vec![MigrationOperation::RemoveFeature {
                id: "new_feature".to_string(),
            }],
            dependencies: None,
        };

        create_test_migration_file(&migration_dir, "001_add_feature.yaml", &migration);

        let runner = MigrationRunner::new(&migration_dir).with_target_file(&target_file);
        let applied = runner.apply_migrations(false).unwrap();

        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0], "001_add_feature");

        // Verify the feature was added
        let content = fs::read_to_string(&target_file).unwrap();
        let document: FdmlDocument = serde_yaml::from_str(&content).unwrap();
        assert_eq!(document.features.len(), 2); // original + new
        
        let new_feature = document.features.iter().find(|f| f.id == "new_feature").unwrap();
        assert_eq!(new_feature.title, "New Feature");
        assert_eq!(new_feature.scenarios.len(), 1);
    }

    #[test]
    fn test_add_field_operation() {
        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("migrations");
        let target_file = temp_dir.path().join("test.fdml");
        fs::create_dir_all(&migration_dir).unwrap();

        create_test_fdml_file(&target_file);

        let migration = Migration {
            id: "001_add_field".to_string(),
            title: Some("Add Field".to_string()),
            description: None,
            up: vec![MigrationOperation::AddField {
                entity_id: "user".to_string(),
                field_name: "age".to_string(),
                field_type: "integer".to_string(),
                required: Some(false),
                default: Some(serde_json::Value::Number(serde_json::Number::from(18))),
            }],
            down: vec![MigrationOperation::RemoveField {
                entity_id: "user".to_string(),
                field_name: "age".to_string(),
            }],
            dependencies: None,
        };

        create_test_migration_file(&migration_dir, "001_add_field.yaml", &migration);

        let runner = MigrationRunner::new(&migration_dir).with_target_file(&target_file);
        let applied = runner.apply_migrations(false).unwrap();

        assert_eq!(applied.len(), 1);

        // Verify the field was added
        let content = fs::read_to_string(&target_file).unwrap();
        let document: FdmlDocument = serde_yaml::from_str(&content).unwrap();
        
        let user_entity = document.entities.iter().find(|e| e.id == "user").unwrap();
        assert_eq!(user_entity.fields.len(), 3); // id, email, age
        
        let age_field = user_entity.fields.iter().find(|f| f.name == "age").unwrap();
        assert_eq!(age_field.field_type, "integer");
        assert_eq!(age_field.required, Some(false));
    }

    #[test]
    fn test_rollback_verification() {
        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("migrations");
        fs::create_dir_all(&migration_dir).unwrap();

        // Migration without down operations
        let migration = Migration {
            id: "001_no_rollback".to_string(),
            title: Some("No Rollback".to_string()),
            description: None,
            up: vec![MigrationOperation::AddFeature {
                id: "feature".to_string(),
                title: "Feature".to_string(),
                description: None,
                scenarios: None,
            }],
            down: vec![], // No down operations
            dependencies: None,
        };

        create_test_migration_file(&migration_dir, "001_no_rollback.yaml", &migration);

        // Simulate applied migration
        let mut state = MigrationState::default();
        state.applied_migrations.push("001_no_rollback".to_string());
        let state_content = serde_json::to_string_pretty(&state).unwrap();
        fs::write(migration_dir.join(".migration_state.json"), state_content).unwrap();

        let runner = MigrationRunner::new(&migration_dir);
        let result = runner.rollback_migrations(1, false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no down operations"));
    }

    #[test]
    fn test_backup_creation() {
        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("migrations");
        let target_file = temp_dir.path().join("test.fdml");
        fs::create_dir_all(&migration_dir).unwrap();

        create_test_fdml_file(&target_file);

        let migration = Migration {
            id: "001_test".to_string(),
            title: Some("Test".to_string()),
            description: None,
            up: vec![MigrationOperation::AddFeature {
                id: "test_feature".to_string(),
                title: "Test Feature".to_string(),
                description: None,
                scenarios: None,
            }],
            down: vec![],
            dependencies: None,
        };

        create_test_migration_file(&migration_dir, "001_test.yaml", &migration);

        let runner = MigrationRunner::new(&migration_dir).with_target_file(&target_file);
        let _applied = runner.apply_migrations(false).unwrap();

        // Check that backup was created
        let backup_dir = migration_dir.join(".backups");
        assert!(backup_dir.exists());
        
        let backup_files: Vec<_> = fs::read_dir(&backup_dir).unwrap().collect();
        assert_eq!(backup_files.len(), 1);
    }

    #[test]
    fn test_dry_run_mode() {
        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("migrations");
        let target_file = temp_dir.path().join("test.fdml");
        fs::create_dir_all(&migration_dir).unwrap();

        create_test_fdml_file(&target_file);
        let original_content = fs::read_to_string(&target_file).unwrap();

        let migration = Migration {
            id: "001_dry_run".to_string(),
            title: Some("Dry Run Test".to_string()),
            description: None,
            up: vec![MigrationOperation::AddFeature {
                id: "dry_feature".to_string(),
                title: "Dry Feature".to_string(),
                description: None,
                scenarios: None,
            }],
            down: vec![],
            dependencies: None,
        };

        create_test_migration_file(&migration_dir, "001_dry_run.yaml", &migration);

        let runner = MigrationRunner::new(&migration_dir).with_target_file(&target_file);
        let applied = runner.apply_migrations(true).unwrap(); // dry_run = true

        assert_eq!(applied.len(), 1);

        // Verify file was not modified
        let current_content = fs::read_to_string(&target_file).unwrap();
        assert_eq!(original_content, current_content);

        // Verify no state was saved
        assert!(!migration_dir.join(".migration_state.json").exists());
    }

    #[test]
    fn test_operation_validation() {
        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("migrations");
        fs::create_dir_all(&migration_dir).unwrap();

        let runner = MigrationRunner::new(&migration_dir);

        // Valid operation
        let valid_op = MigrationOperation::AddFeature {
            id: "valid_feature".to_string(),
            title: "Valid Feature".to_string(),
            description: None,
            scenarios: None,
        };
        assert!(runner.validate_operation(&valid_op).is_ok());

        // Invalid operation (empty id)
        let invalid_op = MigrationOperation::AddFeature {
            id: "".to_string(),
            title: "Invalid Feature".to_string(),
            description: None,
            scenarios: None,
        };
        assert!(runner.validate_operation(&invalid_op).is_err());

        // Invalid field operation
        let invalid_field_op = MigrationOperation::AddField {
            entity_id: "".to_string(),
            field_name: "field".to_string(),
            field_type: "string".to_string(),
            required: None,
            default: None,
        };
        assert!(runner.validate_operation(&invalid_field_op).is_err());
    }
}