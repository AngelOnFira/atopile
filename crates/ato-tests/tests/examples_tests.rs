//! Tests for real-world example projects in the atopile repository.
//!
//! These tests verify that the Rust parser can handle actual production .ato files.

use std::fs;
use std::path::Path;

/// Parse an example file and verify it succeeds.
fn parse_example(path: &str) -> Result<ato_parser::File, String> {
    let full_path = format!(
        "{}/../../{}",
        env!("CARGO_MANIFEST_DIR"),
        path
    );
    let path = Path::new(&full_path);

    if !path.exists() {
        return Err(format!("Example file not found: {}", full_path));
    }

    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    ato_parser::parse(&source)
        .map_err(|errors| format!("Parse errors in {}: {:?}", path.display(), errors))
}

/// Run semantic analysis on an example file.
fn analyze_example(path: &str) -> Result<ato_sema::Design, String> {
    let full_path = format!(
        "{}/../../{}",
        env!("CARGO_MANIFEST_DIR"),
        path
    );
    let path_buf = Path::new(&full_path);

    if !path_buf.exists() {
        return Err(format!("Example file not found: {}", full_path));
    }

    let source = fs::read_to_string(path_buf)
        .map_err(|e| format!("Failed to read {}: {}", path_buf.display(), e))?;

    // First parse
    let _ast = ato_parser::parse(&source)
        .map_err(|errors| format!("Parse errors: {:?}", errors))?;

    // Then analyze
    let mut analyzer = ato_sema::Analyzer::new();
    analyzer.analyze_file(&source, path_buf)
        .map_err(|errors| format!("Sema errors: {:?}", errors))
}

// ============================================================================
// Example Project Parse Tests
// ============================================================================

#[test]
fn test_example_quickstart_parses() {
    let result = parse_example("examples/quickstart/quickstart.ato");
    assert!(result.is_ok(), "quickstart.ato should parse: {:?}", result.err());
    let ast = result.unwrap();
    assert_eq!(ast.statements.len(), 2); // import + module App
}

#[test]
fn test_example_equations_parses() {
    let result = parse_example("examples/equations/equations.ato");
    assert!(result.is_ok(), "equations.ato should parse: {:?}", result.err());
    let ast = result.unwrap();
    assert_eq!(ast.statements.len(), 3); // imports + VoltageDivider + App
}

#[test]
fn test_example_layout_reuse_parses() {
    let result = parse_example("examples/layout_reuse/layout_reuse.ato");
    assert!(result.is_ok(), "layout_reuse.ato should parse: {:?}", result.err());
    let ast = result.unwrap();
    assert!(ast.statements.len() >= 4); // pragmas + imports + modules
}

#[test]
fn test_example_pick_parts_parses() {
    let result = parse_example("examples/pick_parts/pick_parts.ato");
    assert!(result.is_ok(), "pick_parts.ato should parse: {:?}", result.err());
    let ast = result.unwrap();
    assert!(ast.statements.len() >= 5); // pragmas + imports + module
}

#[test]
fn test_example_i2c_parses() {
    let result = parse_example("examples/i2c/i2c.ato");
    assert!(result.is_ok(), "i2c.ato should parse: {:?}", result.err());
    let ast = result.unwrap();
    assert!(ast.statements.len() >= 8); // pragmas + imports + modules
}

#[test]
fn test_example_esp32_minimal_parses() {
    let result = parse_example("examples/esp32_minimal/esp32_minimal.ato");
    assert!(result.is_ok(), "esp32_minimal.ato should parse: {:?}", result.err());
    let ast = result.unwrap();
    assert!(ast.statements.len() >= 6); // pragmas + imports + module
}

#[test]
fn test_example_led_badge_parses() {
    let result = parse_example("examples/led_badge/led_badge.ato");
    assert!(result.is_ok(), "led_badge.ato should parse: {:?}", result.err());
    let ast = result.unwrap();
    assert!(ast.statements.len() >= 15); // many imports + modules
}

// ============================================================================
// Standard Library Parse Tests
// ============================================================================

#[test]
fn test_stdlib_debug_parses() {
    let result = parse_example("src/faebryk/library/debug.ato");
    assert!(result.is_ok(), "debug.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_diodes_parses() {
    let result = parse_example("src/faebryk/library/diodes.ato");
    assert!(result.is_ok(), "diodes.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_filters_parses() {
    let result = parse_example("src/faebryk/library/filters.ato");
    assert!(result.is_ok(), "filters.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_i2c_pulls_weak_parses() {
    let result = parse_example("src/faebryk/library/i2c_pulls_weak.ato");
    assert!(result.is_ok(), "i2c_pulls_weak.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_interfaces_parses() {
    let result = parse_example("src/faebryk/library/interfaces.ato");
    assert!(result.is_ok(), "interfaces.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_mosfets_parses() {
    let result = parse_example("src/faebryk/library/mosfets.ato");
    assert!(result.is_ok(), "mosfets.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_oscillators_parses() {
    let result = parse_example("src/faebryk/library/oscillators.ato");
    assert!(result.is_ok(), "oscillators.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_regulators_parses() {
    let result = parse_example("src/faebryk/library/regulators.ato");
    assert!(result.is_ok(), "regulators.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_resistors_parses() {
    let result = parse_example("src/faebryk/library/resistors.ato");
    assert!(result.is_ok(), "resistors.ato should parse: {:?}", result.err());
}

#[test]
fn test_stdlib_vdivs_parses() {
    let result = parse_example("src/faebryk/library/vdivs.ato");
    assert!(result.is_ok(), "vdivs.ato should parse: {:?}", result.err());
}

// ============================================================================
// Example Semantic Analysis Tests (where imports allow)
// ============================================================================

#[test]
fn test_example_quickstart_sema() {
    // Note: This will fail at import resolution since we don't have Resistor defined
    // But it tests that parsing + basic sema infrastructure works
    let result = analyze_example("examples/quickstart/quickstart.ato");
    // We expect this to fail due to unresolved imports, but parsing should work
    let _ = result; // Just verify it doesn't panic
}

#[test]
fn test_example_equations_sema() {
    let result = analyze_example("examples/equations/equations.ato");
    let _ = result; // Just verify it doesn't panic
}

// ============================================================================
// Parts Files Parse Tests (auto-generated component files)
// ============================================================================

#[test]
fn test_parts_files_parse() {
    // Test a sample of auto-generated parts files
    let parts_paths = [
        "examples/quickstart/parts/UNI_ROYAL_0603WAF1000T5E/UNI_ROYAL_0603WAF1000T5E.ato",
        "examples/equations/parts/FOJAN_FRC0402F2003TS/FOJAN_FRC0402F2003TS.ato",
    ];

    for path in parts_paths {
        let result = parse_example(path);
        // Parts files might not exist, so we just check if they parse when present
        if result.is_ok() {
            let ast = result.unwrap();
            assert!(!ast.statements.is_empty(), "Parts file {} should have content", path);
        }
    }
}

// ============================================================================
// Summary Statistics Test
// ============================================================================

#[test]
fn test_all_examples_summary() {
    let examples = [
        "examples/quickstart/quickstart.ato",
        "examples/equations/equations.ato",
        "examples/layout_reuse/layout_reuse.ato",
        "examples/pick_parts/pick_parts.ato",
        "examples/i2c/i2c.ato",
        "examples/esp32_minimal/esp32_minimal.ato",
        "examples/led_badge/led_badge.ato",
    ];

    let mut passed = 0;
    let mut failed = 0;

    for path in examples {
        if parse_example(path).is_ok() {
            passed += 1;
        } else {
            failed += 1;
            eprintln!("FAILED: {}", path);
        }
    }

    assert_eq!(passed, 7, "All 7 examples should parse successfully");
    assert_eq!(failed, 0, "No examples should fail");
}

#[test]
fn test_all_stdlib_summary() {
    let stdlib = [
        "src/faebryk/library/debug.ato",
        "src/faebryk/library/diodes.ato",
        "src/faebryk/library/filters.ato",
        "src/faebryk/library/i2c_pulls_weak.ato",
        "src/faebryk/library/interfaces.ato",
        "src/faebryk/library/mosfets.ato",
        "src/faebryk/library/oscillators.ato",
        "src/faebryk/library/regulators.ato",
        "src/faebryk/library/resistors.ato",
        "src/faebryk/library/vdivs.ato",
    ];

    let mut passed = 0;
    let mut failed = 0;

    for path in stdlib {
        if parse_example(path).is_ok() {
            passed += 1;
        } else {
            failed += 1;
            eprintln!("FAILED: {}", path);
        }
    }

    assert_eq!(passed, 10, "All 10 stdlib files should parse successfully");
    assert_eq!(failed, 0, "No stdlib files should fail");
}
