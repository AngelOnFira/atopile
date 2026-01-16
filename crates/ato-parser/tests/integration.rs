//! Integration tests that verify the parser works on real .ato files.

use ato_parser::{parse, BlockKind, Statement};
use std::fs;
use std::path::PathBuf;

/// Get the project root directory.
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Parse a file and return the AST.
fn parse_file(path: &PathBuf) -> ato_parser::File {
    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("Failed to read file {}: {}", path.display(), e)
    });

    match parse(&source) {
        Ok(file) => file,
        Err(e) => {
            panic!("Parse error in {}: {:?}", path.display(), e);
        }
    }
}

#[test]
fn test_parse_led_badge() {
    let path = project_root().join("examples/led_badge/led_badge.ato");
    if !path.exists() {
        eprintln!("Skipping test_parse_led_badge: file not found at {:?}", path);
        return;
    }

    let file = parse_file(&path);

    // Should have multiple top-level statements
    assert!(!file.statements.is_empty(), "File should have statements");

    // Count pragmas, imports, and modules
    let pragmas: Vec<_> = file
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::Pragma(_)))
        .collect();
    let imports: Vec<_> = file
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::Import(_)))
        .collect();
    let modules: Vec<_> = file
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::BlockDef(b) if b.kind == BlockKind::Module))
        .collect();

    assert!(!pragmas.is_empty(), "Should have pragma statements");
    assert!(!imports.is_empty(), "Should have import statements");
    assert!(!modules.is_empty(), "Should have module definitions");

    // Verify LED_BADGE module exists
    let led_badge = modules.iter().find(|s| {
        if let Statement::BlockDef(b) = s {
            b.name.name == "LED_BADGE"
        } else {
            false
        }
    });
    assert!(led_badge.is_some(), "Should have LED_BADGE module");
}

#[test]
fn test_parse_quickstart() {
    let path = project_root().join("examples/quickstart/quickstart.ato");
    if !path.exists() {
        eprintln!("Skipping test_parse_quickstart: file not found at {:?}", path);
        return;
    }

    let file = parse_file(&path);
    assert!(!file.statements.is_empty(), "File should have statements");
}

#[test]
fn test_parse_i2c() {
    let path = project_root().join("examples/i2c/i2c.ato");
    if !path.exists() {
        eprintln!("Skipping test_parse_i2c: file not found at {:?}", path);
        return;
    }

    let file = parse_file(&path);
    assert!(!file.statements.is_empty(), "File should have statements");
}

#[test]
fn test_parse_layout_reuse() {
    let path = project_root().join("examples/layout_reuse/layout_reuse.ato");
    if !path.exists() {
        eprintln!("Skipping test_parse_layout_reuse: file not found at {:?}", path);
        return;
    }

    let file = parse_file(&path);
    assert!(!file.statements.is_empty(), "File should have statements");
}

#[test]
fn test_parse_pick_parts() {
    let path = project_root().join("examples/pick_parts/pick_parts.ato");
    if !path.exists() {
        eprintln!("Skipping test_parse_pick_parts: file not found at {:?}", path);
        return;
    }

    let file = parse_file(&path);
    assert!(!file.statements.is_empty(), "File should have statements");
}

#[test]
fn test_parse_equations() {
    let path = project_root().join("examples/equations/equations.ato");
    if !path.exists() {
        eprintln!("Skipping test_parse_equations: file not found at {:?}", path);
        return;
    }

    let file = parse_file(&path);
    assert!(!file.statements.is_empty(), "File should have statements");
}

#[test]
fn test_ast_structure() {
    // Test that AST is correctly structured for a known input
    let source = r#"#pragma experiment("FOR_LOOP")
import ElectricPower
from "path/to/file.ato" import Module

module LED_BADGE:
    """LED Badge with ESP32"""
    power = new ElectricPower
    leds = new LED[10]

    # Connect power to all LEDs
    for led in leds:
        power ~ led.power

    assert power.voltage within 3.0V to 3.6V

    trait has_power<required=True>
"#;

    let file = parse(source).expect("Should parse successfully");

    // Check statements
    assert_eq!(file.statements.len(), 4);

    // Check pragma
    assert!(matches!(&file.statements[0], Statement::Pragma(_)));

    // Check import
    assert!(matches!(&file.statements[1], Statement::Import(_)));

    // Check from import
    if let Statement::Import(import) = &file.statements[2] {
        assert!(import.from_path.is_some());
    } else {
        panic!("Expected import statement");
    }

    // Check module
    if let Statement::BlockDef(block) = &file.statements[3] {
        assert_eq!(block.kind, BlockKind::Module);
        assert_eq!(block.name.name, "LED_BADGE");

        // Should have: docstring, 2 assignments, 1 for loop, 1 assert, 1 trait
        assert!(block.body.len() >= 5, "Module should have at least 5 statements");

        // Check for docstring
        assert!(
            matches!(&block.body[0], Statement::StringStmt(_)),
            "First statement should be docstring"
        );

        // Check for for loop
        let has_for = block.body.iter().any(|s| matches!(s, Statement::For(_)));
        assert!(has_for, "Should have for loop");

        // Check for assert
        let has_assert = block.body.iter().any(|s| matches!(s, Statement::Assert(_)));
        assert!(has_assert, "Should have assert");

        // Check for trait
        let has_trait = block.body.iter().any(|s| matches!(s, Statement::Trait(_)));
        assert!(has_trait, "Should have trait");
    } else {
        panic!("Expected block definition");
    }
}

#[test]
fn test_connection_types() {
    let source = r#"module M:
    # Simple connection
    a ~ b

    # Forward chain
    x ~> y ~> z

    # Backward chain
    p <~ q <~ r
"#;

    let file = parse(source).expect("Should parse successfully");

    if let Statement::BlockDef(block) = &file.statements[0] {
        // Check for simple connection
        let has_connection = block
            .body
            .iter()
            .any(|s| matches!(s, Statement::Connection(_)));
        assert!(has_connection, "Should have simple connection");

        // Check for directed connections
        let directed: Vec<_> = block
            .body
            .iter()
            .filter(|s| matches!(s, Statement::DirectedConnection(_)))
            .collect();
        assert_eq!(directed.len(), 2, "Should have 2 directed connections");
    } else {
        panic!("Expected block definition");
    }
}

#[test]
fn test_physical_quantities() {
    let source = r#"module M:
    resistance = 10kohm +/- 5%
    voltage = 3.3V to 5V
    capacitance = 100nF
    current = 500mA
"#;

    let file = parse(source).expect("Should parse successfully");

    if let Statement::BlockDef(block) = &file.statements[0] {
        assert_eq!(block.body.len(), 4, "Should have 4 assignments");

        // All should be assignments with physical values
        for stmt in &block.body {
            assert!(
                matches!(stmt, Statement::Assignment(_)),
                "All should be assignments"
            );
        }
    } else {
        panic!("Expected block definition");
    }
}

#[test]
fn test_new_expressions() {
    let source = r#"module M:
    simple = new Type
    array = new Type[10]
    templated = new Type<param=1>
    both = new Type[5]<param="value">
"#;

    let file = parse(source).expect("Should parse successfully");

    if let Statement::BlockDef(block) = &file.statements[0] {
        assert_eq!(block.body.len(), 4, "Should have 4 assignments");
    } else {
        panic!("Expected block definition");
    }
}

#[test]
fn test_assert_comparisons() {
    let source = r#"module M:
    assert x > 5
    assert y < 10
    assert z >= 0
    assert w <= 100
    assert v within 1 to 10
    assert u is 5
"#;

    let file = parse(source).expect("Should parse successfully");

    if let Statement::BlockDef(block) = &file.statements[0] {
        assert_eq!(block.body.len(), 6, "Should have 6 assertions");

        for stmt in &block.body {
            assert!(matches!(stmt, Statement::Assert(_)), "All should be assertions");
        }
    } else {
        panic!("Expected block definition");
    }
}

#[test]
fn test_trait_variations() {
    let source = r#"module M:
    trait simple_trait
    trait with_constructor::ctor
    trait with_template<arg=1>
    trait full::ctor<arg="value", num=42>
"#;

    let file = parse(source).expect("Should parse successfully");

    if let Statement::BlockDef(block) = &file.statements[0] {
        assert_eq!(block.body.len(), 4, "Should have 4 trait statements");

        for stmt in &block.body {
            assert!(matches!(stmt, Statement::Trait(_)), "All should be traits");
        }
    } else {
        panic!("Expected block definition");
    }
}

#[test]
fn test_json_serialization() {
    let source = r#"module M:
    x = 5
"#;

    let file = parse(source).expect("Should parse successfully");

    // Should be serializable to JSON
    let json = serde_json::to_string_pretty(&file).expect("Should serialize to JSON");

    // Check that it contains expected content (BlockKind serializes as the variant name)
    assert!(json.contains("BlockDef"), "Should contain BlockDef: {}", json);
    assert!(json.contains("Module"), "Should contain Module: {}", json);
    assert!(json.contains("\"name\""), "Should contain name field");
}
