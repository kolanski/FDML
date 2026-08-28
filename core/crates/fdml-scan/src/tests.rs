//! Parity + determinism tests for the ported scanner.

use super::*;
use std::fs;

/// Build a tiny mixed Python/TypeScript fixture in a fresh temp dir.
fn write_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();

    fs::write(
        dir.path().join("mod_a.py"),
        "import os\n\
         \n\
         def top_level():\n\
         \x20\x20\x20\x20return 1\n\
         \n\
         class Greeter:\n\
         \x20\x20\x20\x20def greet(self, name):\n\
         \x20\x20\x20\x20\x20\x20\x20\x20return name\n",
    )
    .unwrap();

    fs::write(
        dir.path().join("widget.ts"),
        "import { Foo } from './foo';\n\
         \n\
         export interface Shape {\n\
         \x20\x20area(): number;\n\
         }\n\
         \n\
         export class Circle {\n\
         \x20\x20radius: number;\n\
         \x20\x20area(): number {\n\
         \x20\x20\x20\x20return this.radius;\n\
         \x20\x20}\n\
         }\n",
    )
    .unwrap();

    dir
}

/// Recursively collect every element name in a file analysis.
fn element_names(elements: &[CodeElement], out: &mut Vec<String>) {
    for el in elements {
        out.push(el.name.clone());
        element_names(&el.children, out);
    }
}

fn all_names(scan: &ScanResult) -> Vec<String> {
    let mut names = Vec::new();
    for file in &scan.files {
        element_names(&file.elements, &mut names);
    }
    names
}

#[test]
fn parity_python_and_typescript() {
    let dir = write_fixture();
    let scan = run(dir.path(), &[]).unwrap();

    // Two source files, both languages detected.
    assert_eq!(scan.files.len(), 2, "expected 2 scanned files");
    assert!(scan.metadata.languages_detected.contains(&Language::Python));
    assert!(scan.metadata.languages_detected.contains(&Language::TypeScript));

    let names = all_names(&scan);
    // Python: top-level function + class + method.
    assert!(names.contains(&"top_level".to_string()), "names={names:?}");
    assert!(names.contains(&"Greeter".to_string()), "names={names:?}");
    assert!(names.contains(&"greet".to_string()), "names={names:?}");
    // TypeScript: class + interface.
    assert!(names.contains(&"Circle".to_string()), "names={names:?}");
    assert!(names.contains(&"Shape".to_string()), "names={names:?}");

    // Imports: the Python `os` import and the relative TS `./foo` import.
    let modules: Vec<&str> = scan
        .files
        .iter()
        .flat_map(|f| f.imports.iter().map(|i| i.module.as_str()))
        .collect();
    assert!(modules.contains(&"os"), "imports={modules:?}");
    assert!(modules.contains(&"./foo"), "imports={modules:?}");

    // At least one relationship (imports/inheritance) was derived.
    assert!(!scan.relationships.is_empty(), "expected >=1 relationship");

    // Statistics sanity.
    assert!(scan.statistics.classes >= 1);
    assert!(scan.statistics.functions >= 1);
    assert!(scan.statistics.methods >= 1);
    assert!(scan.statistics.interfaces >= 1);
}

#[test]
fn determinism_same_fixture_identical_json() {
    let dir = write_fixture();
    let a = run(dir.path(), &[]).unwrap();
    let b = run(dir.path(), &[]).unwrap();

    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb, "two scans of the same fixture must serialize identically");
}
