use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_fdml_version() {
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("fdml 0.2.0"));
}

#[test]
fn test_fdml_help() {
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("FDML (Feature-Driven Modeling Language) CLI tools"));
}

#[test]
fn test_init_command() {
    let temp_dir = TempDir::new().unwrap();
    let project_name = "test-integration-project";
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.current_dir(&temp_dir)
        .arg("init")
        .arg(project_name)
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully initialized FDML project"));
    
    // Check that expected files and directories were created
    let project_path = temp_dir.path().join(project_name);
    assert!(project_path.exists());
    assert!(project_path.join("specs").exists());
    assert!(project_path.join("fdml.yaml").exists());
    assert!(project_path.join("specs/example.fdml").exists());
    assert!(project_path.join("README.md").exists());
}

#[test]
fn test_validate_valid_file() {
    let temp_dir = TempDir::new().unwrap();
    let valid_fdml = r#"
metadata:
  version: "1.3"
  author: "Test Author"

entity:
  id: test_entity
  name: "Test Entity"
"#;
    
    let fdml_file = temp_dir.path().join("valid.fdml");
    fs::write(&fdml_file, valid_fdml).unwrap();
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("validate")
        .arg(&fdml_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("is valid"));
}

#[test]
fn test_validate_json_output() {
    let temp_dir = TempDir::new().unwrap();
    let valid_fdml = r#"
metadata:
  version: "1.3"

entity:
  id: test_entity
"#;
    
    let fdml_file = temp_dir.path().join("valid.fdml");
    fs::write(&fdml_file, valid_fdml).unwrap();
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("validate")
        .arg("--output")
        .arg("json")
        .arg(&fdml_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"valid\": true"));
}

#[test]
fn test_validate_nonexistent_file() {
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("validate")
        .arg("nonexistent.fdml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Project Error"));
}

#[test]
fn test_init_existing_directory() {
    let temp_dir = TempDir::new().unwrap();
    let project_name = "existing-dir";
    let project_path = temp_dir.path().join(project_name);
    
    // Create the directory first
    fs::create_dir_all(&project_path).unwrap();
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.current_dir(&temp_dir)
        .arg("init")
        .arg(project_name)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_verbose_flag() {
    let temp_dir = TempDir::new().unwrap();
    let valid_fdml = r#"
entity:
  id: test
"#;
    
    let fdml_file = temp_dir.path().join("test.fdml");
    fs::write(&fdml_file, valid_fdml).unwrap();
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("--verbose")
        .arg("validate")
        .arg(&fdml_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Validating FDML file"))
        .stdout(predicate::str::contains("Parsing completed successfully"));
}