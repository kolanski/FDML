use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
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

// Integration tests using real e-commerce example
#[test]
fn test_parse_ecommerce_example() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    // Skip test if example file doesn't exist (in case of different test environments)
    if !ecommerce_path.exists() {
        return;
    }
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("parse")
        .arg(ecommerce_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("metadata"))
        .stdout(predicate::str::contains("entities"))
        .stdout(predicate::str::contains("actions"))
        .stdout(predicate::str::contains("features"));
}

#[test]
fn test_parse_ecommerce_yaml_output() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("parse")
        .arg("--output")
        .arg("yaml")
        .arg(ecommerce_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("metadata:"))
        .stdout(predicate::str::contains("entities:"))
        .stdout(predicate::str::contains("version: '1.3'"));
}

#[test]
fn test_validate_ecommerce_example() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("validate")
        .arg(ecommerce_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("is valid"));
}

#[test]
fn test_generate_typescript_ecommerce() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("generated");
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("generate")
        .arg("--language")
        .arg("typescript")
        .arg("--output")
        .arg(&output_dir)
        .arg("--with-tests")
        .arg(ecommerce_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully generated"))
        .stdout(predicate::str::contains("types.ts"))
        .stdout(predicate::str::contains("routes.ts"))
        .stdout(predicate::str::contains("package.json"));
    
    // Verify generated files exist
    assert!(output_dir.join("types.ts").exists());
    assert!(output_dir.join("routes.ts").exists());
    assert!(output_dir.join("package.json").exists());
    assert!(output_dir.join("tests").exists());
    
    // Verify package.json contains expected content
    let package_json = fs::read_to_string(output_dir.join("package.json")).unwrap();
    assert!(package_json.contains("\"name\": \"fdml-generated-api\""));
    assert!(package_json.contains("\"express\""));
    assert!(package_json.contains("\"typescript\""));
}

#[test]
fn test_generate_python_ecommerce() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("generated-python");
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("generate")
        .arg("--language")
        .arg("python")
        .arg("--output")
        .arg(&output_dir)
        .arg("--with-tests")
        .arg(ecommerce_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully generated"))
        .stdout(predicate::str::contains("models.py"))
        .stdout(predicate::str::contains("routes.py"));
    
    // Verify generated files exist
    assert!(output_dir.join("models.py").exists());
    assert!(output_dir.join("routes.py").exists());
    assert!(output_dir.join("requirements.txt").exists());
    
    // Verify models.py contains expected entities
    let models_py = fs::read_to_string(output_dir.join("models.py")).unwrap();
    assert!(models_py.contains("class User(BaseModel)"));
    assert!(models_py.contains("class Product(BaseModel)"));
    assert!(models_py.contains("class Order(BaseModel)"));
}

#[test]
fn test_generate_go_ecommerce() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("generated-go");
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("generate")
        .arg("--language")
        .arg("go")
        .arg("--output")
        .arg(&output_dir)
        .arg("--with-tests")
        .arg(ecommerce_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully generated"))
        .stdout(predicate::str::contains("types.go"))
        .stdout(predicate::str::contains("handlers.go"));
    
    // Verify generated files exist
    assert!(output_dir.join("types.go").exists());
    assert!(output_dir.join("handlers.go").exists());
    assert!(output_dir.join("go.mod").exists());
    
    // Verify types.go contains expected structs
    let types_go = fs::read_to_string(output_dir.join("types.go")).unwrap();
    assert!(types_go.contains("type User struct"));
    assert!(types_go.contains("type Product struct"));
    assert!(types_go.contains("type Order struct"));
}

#[test]
fn test_migrate_status_ecommerce() {
    let migrations_dir = Path::new("examples/e-commerce");
    
    if !migrations_dir.exists() || !migrations_dir.join("migrations").exists() {
        return;
    }
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.current_dir(migrations_dir)
        .arg("migrate")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Migration Status:"))
        .stdout(predicate::str::contains("add_user_preferences"))
        .stdout(predicate::str::contains("add_product_reviews"));
}

#[test]
fn test_trace_validate_ecommerce() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("trace")
        .arg("validate")
        .arg(ecommerce_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Traceability validation"));
}

#[test]
fn test_complex_parsing_with_all_features() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    // Test parsing with verbose output to ensure all components are parsed
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    let output = cmd.arg("parse")
        .arg("--verbose")
        .arg(ecommerce_path)
        .output()
        .unwrap();
    
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    
    // Verify all major sections are present in parsed output
    assert!(stdout.contains("\"entities\""));
    assert!(stdout.contains("\"actions\""));
    assert!(stdout.contains("\"features\""));
    assert!(stdout.contains("\"flows\""));
    assert!(stdout.contains("\"constraints\""));
    assert!(stdout.contains("\"traceability\""));
    
    // Verify specific entities are parsed
    assert!(stdout.contains("\"id\": \"user\""));
    assert!(stdout.contains("\"id\": \"product\""));
    assert!(stdout.contains("\"id\": \"order\""));
    assert!(stdout.contains("\"id\": \"order_item\""));
    
    // Verify actions are parsed
    assert!(stdout.contains("\"create_user\""));
    assert!(stdout.contains("\"list_products\""));
    assert!(stdout.contains("\"create_order\""));
}

#[test]
fn test_end_to_end_typescript_workflow() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("e2e-test");
    
    // Step 1: Parse the FDML file
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("parse")
        .arg(ecommerce_path)
        .assert()
        .success();
    
    // Step 2: Generate TypeScript code
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("generate")
        .arg("--language")
        .arg("typescript")
        .arg("--output")
        .arg(&output_dir)
        .arg("--with-tests")
        .arg(ecommerce_path)
        .assert()
        .success();
    
    // Step 3: Verify all expected files are generated
    let expected_files = vec![
        "types.ts",
        "routes.ts", 
        "package.json",
        "tests/user.test.ts",
        "tests/product.test.ts",
        "tests/order.test.ts",
        "tests/order_item.test.ts",
        "tests/user_registration.feature.test.ts",
        "tests/product_catalog.feature.test.ts",
        "tests/order_management.feature.test.ts",
    ];
    
    for file in expected_files {
        assert!(output_dir.join(file).exists(), "Missing generated file: {}", file);
    }
    
    // Step 4: Verify generated TypeScript content has proper structure
    let types_content = fs::read_to_string(output_dir.join("types.ts")).unwrap();
    assert!(types_content.contains("export interface User"));
    assert!(types_content.contains("export interface Product"));
    assert!(types_content.contains("export interface Order"));
    assert!(types_content.contains("export interface OrderItem"));
    
    let routes_content = fs::read_to_string(output_dir.join("routes.ts")).unwrap();
    assert!(routes_content.contains("import express"));
    assert!(routes_content.contains("router.post"));
    assert!(routes_content.contains("create-user"));
    assert!(routes_content.contains("list-products"));
    
    // Step 5: Verify test files have proper structure
    let user_test = fs::read_to_string(output_dir.join("tests/user.test.ts")).unwrap();
    assert!(user_test.contains("describe('User'"));
    assert!(user_test.contains("should create valid entity"));
    
    let feature_test = fs::read_to_string(output_dir.join("tests/user_registration.feature.test.ts")).unwrap();
    assert!(feature_test.contains("describe('Feature: User Registration'"));
    assert!(feature_test.contains("it('should pass scenario steps'"));
}

#[test]
fn test_end_to_end_python_workflow() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    
    if !ecommerce_path.exists() {
        return;
    }
    
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("e2e-python");
    
    // Generate Python code and validate structure
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("generate")
        .arg("--language")
        .arg("python")
        .arg("--output")
        .arg(&output_dir)
        .arg("--with-tests")
        .arg(ecommerce_path)
        .assert()
        .success();
    
    // Verify Python-specific files
    assert!(output_dir.join("models.py").exists());
    assert!(output_dir.join("routes.py").exists());
    assert!(output_dir.join("requirements.txt").exists());
    assert!(output_dir.join("tests/test_user.py").exists());
    
    // Verify content structure
    let models_content = fs::read_to_string(output_dir.join("models.py")).unwrap();
    assert!(models_content.contains("from pydantic import BaseModel"));
    assert!(models_content.contains("class User(BaseModel)"));
    assert!(models_content.contains("email: str"));
    
    let routes_content = fs::read_to_string(output_dir.join("routes.py")).unwrap();
    assert!(routes_content.contains("from fastapi import APIRouter"));
    assert!(routes_content.contains("@router.post"));
    
    let requirements = fs::read_to_string(output_dir.join("requirements.txt")).unwrap();  
    assert!(requirements.contains("fastapi"));
    assert!(requirements.contains("pydantic"));
}

#[test]
fn test_full_project_lifecycle() {
    let ecommerce_path = Path::new("examples/e-commerce/ecommerce.fdml");
    let migrations_dir = Path::new("examples/e-commerce/migrations");
    
    if !ecommerce_path.exists() || !migrations_dir.exists() {
        return;
    }
    
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("full-test");
    
    // 1. Validate the FDML specification
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("validate")
        .arg(ecommerce_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("is valid"));
    
    // 2. Parse and verify structure
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    let output = cmd.arg("parse")
        .arg("--output")
        .arg("json")
        .arg(ecommerce_path)
        .output()
        .unwrap();
    
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    
    // Parse as JSON to validate it's well-formed
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["entities"].is_array());
    assert!(parsed["actions"].is_array());
    assert!(parsed["features"].is_array());
    
    // 3. Generate code for all supported languages  
    for language in &["typescript", "python", "go"] {
        let lang_dir = project_dir.join(language);
        
        let mut cmd = Command::cargo_bin("fdml").unwrap();
        cmd.arg("generate")
            .arg("--language")
            .arg(language)
            .arg("--output")
            .arg(&lang_dir)
            .arg("--with-tests")
            .arg(ecommerce_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("Successfully generated"));
        
        // Verify basic file structure for each language
        match *language {
            "typescript" => {
                assert!(lang_dir.join("types.ts").exists());
                assert!(lang_dir.join("package.json").exists());
            }
            "python" => {
                assert!(lang_dir.join("models.py").exists());
                assert!(lang_dir.join("requirements.txt").exists());
            }
            "go" => {
                assert!(lang_dir.join("types.go").exists());
                assert!(lang_dir.join("go.mod").exists());
            }
            _ => {}
        }
    }
    
    // 4. Test migration status (from the original directory since it has .fdml-state)
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.current_dir("examples/e-commerce")
        .arg("migrate")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Migration Status"));
    
    // 5. Validate traceability
    let mut cmd = Command::cargo_bin("fdml").unwrap();
    cmd.arg("trace")
        .arg("validate")
        .arg(ecommerce_path)
        .assert()
        .success();
}