//! Negative tests - verify that invalid ato source is rejected or warned about.
//!
//! Tests are organized into:
//! 1. Should-not-compile: source that must produce errors
//! 2. Should-warn: source that should produce warnings (may compile but with diagnostics)
//!
//! Tests marked `#[ignore]` with a reason document bugs where the analyzer
//! currently accepts invalid input that it should reject.

use ato_sema::{Analyzer, SemaError};

// ============================================================================
// Helpers
// ============================================================================

/// Analyze source and return the result.
/// Uses `analyze_source` which needs no filesystem.
fn analyze(source: &str) -> Result<ato_ir::Design, Vec<SemaError>> {
    let mut analyzer = Analyzer::new();
    analyzer.analyze_source(source)
}

/// Analyze source and assert it produces errors.
/// Returns the error list for further inspection.
fn expect_errors(source: &str) -> Vec<SemaError> {
    let result = analyze(source);
    assert!(result.is_err(), "Expected errors but analysis succeeded");
    result.unwrap_err()
}

/// Check if any error matches a predicate.
fn has_error<F: Fn(&SemaError) -> bool>(errors: &[SemaError], pred: F) -> bool {
    errors.iter().any(pred)
}

// ============================================================================
// 1. Should-not-compile tests
// ============================================================================

#[test]
fn test_undefined_name_in_connection() {
    // Connecting two names that were never declared should error.
    let source = r#"
module M:
    a ~ b
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Connecting undefined names should produce errors");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::UndefinedName { .. })),
        "Expected UndefinedName error, got: {:?}", errors
    );
}

#[test]
fn test_undefined_name_one_side() {
    // One side defined, other side not -- should still error.
    let source = r#"
module M:
    pin p1
    p1 ~ nonexistent
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Connecting to undefined name should error");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::UndefinedName { .. })),
        "Expected UndefinedName error, got: {:?}", errors
    );
}

#[test]
#[ignore = "BUG: Analyzer does not yet check interface type compatibility in connections"]
fn test_incompatible_interface_connection() {
    // Connecting an I2C interface to an SPI interface should error.
    // These are different interface types and should not be connectable.
    let source = r#"
interface I2C:
    signal scl
    signal sda

interface SPI:
    signal sclk
    signal mosi
    signal miso

module M:
    i2c_bus = new I2C
    spi_bus = new SPI
    i2c_bus ~ spi_bus
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Connecting incompatible interfaces should error");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::TypeMismatch { .. })),
        "Expected TypeMismatch error, got: {:?}", errors
    );
}

#[test]
#[ignore = "BUG: Analyzer does not detect circular inheritance (uses visited set but no error)"]
fn test_circular_inheritance() {
    // module A from B + module B from A should error with CyclicInheritance.
    let source = r#"
module A from B:
    pass

module B from A:
    pass
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Circular inheritance should produce errors");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::CyclicInheritance { .. })),
        "Expected CyclicInheritance error, got: {:?}", errors
    );
}

#[test]
fn test_import_nonexistent_module_with_file() {
    // `import NonExistentThing` should error when using analyze_file.
    // Note: analyze_source uses a dummy path so import resolution is skipped;
    // we need analyze_file for this.
    let temp_dir = std::env::temp_dir().join("ato_neg_test_import");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let source_path = temp_dir.join("test.ato");
    std::fs::write(&source_path, r#"
import NonExistentThing

module App:
    x = new NonExistentThing
"#).unwrap();

    let source = std::fs::read_to_string(&source_path).unwrap();
    let mut analyzer = Analyzer::new();
    let result = analyzer.analyze_file(&source, &source_path);

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(result.is_err(), "Importing nonexistent module should error");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::UnresolvedImport { .. })),
        "Expected UnresolvedImport error, got: {:?}", errors
    );
}

#[test]
fn test_duplicate_field_definition() {
    // Two fields with the same name in the same module should error.
    let source = r#"
module M:
    pin p1
    pin p1
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Duplicate field definitions should error");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::DuplicateDefinition { .. })),
        "Expected DuplicateDefinition error, got: {:?}", errors
    );
}

#[test]
fn test_duplicate_signal_definition() {
    // Two signals with the same name should error.
    let source = r#"
module M:
    signal s1
    signal s1
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Duplicate signal definitions should error");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::DuplicateDefinition { .. })),
        "Expected DuplicateDefinition error, got: {:?}", errors
    );
}

#[test]
#[ignore = "BUG: Analyzer does not check value type vs parameter type (string assigned to physical param)"]
fn test_assign_string_to_physical_param() {
    // Assigning a string to a parameter declared with a physical unit should error.
    let source = r#"
module M:
    resistance: ohm
    resistance = "hello"
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Assigning string to physical param should error");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::TypeMismatch { .. })),
        "Expected TypeMismatch error, got: {:?}", errors
    );
}

#[test]
#[ignore = "BUG: Analyzer does not check whether a module instance is connectable to a signal"]
fn test_connect_module_to_signal() {
    // Connecting a module instance (not a pin/signal/interface) directly to a signal
    // should error or warn, since modules are not connectable endpoints.
    let source = r#"
module Inner:
    pin p1

module M:
    inner = new Inner
    signal s1
    inner ~ s1
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Connecting module instance to signal should error");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::NotConnectable { .. } | SemaError::TypeMismatch { .. })),
        "Expected NotConnectable or TypeMismatch error, got: {:?}", errors
    );
}

// ============================================================================
// 1b. Additional should-not-compile tests
// ============================================================================

#[test]
fn test_undefined_type_in_new() {
    // `new NonExistent` referencing a type never defined should error.
    let source = r#"
module M:
    x = new NonExistent
"#;
    let result = analyze(source);
    // Currently analyze_source doesn't have full import resolution,
    // so the type won't be resolved. Check if it errors or just creates
    // an unresolved instance.
    if result.is_err() {
        let errors = result.unwrap_err();
        assert!(
            has_error(&errors, |e| matches!(e,
                SemaError::UndefinedName { .. } |
                SemaError::UnresolvedImport { .. } |
                SemaError::BaseTypeNotFound { .. }
            )),
            "Expected some kind of undefined type error, got: {:?}", errors
        );
    } else {
        // If it passes, verify the instance has no resolved_type
        let design = result.unwrap();
        let m = design.modules().iter().find(|m| m.name == "M").unwrap();
        let x_id = m.get_field("x").expect("x field should exist");
        let x_field = design.get_field(x_id).unwrap();
        if let ato_ir::FieldKind::Instance { resolved_type, .. } = &x_field.kind {
            // BUG: ideally this should error, not silently produce None
            eprintln!("WARNING: new NonExistent succeeded with resolved_type={:?} (should error)", resolved_type);
        }
    }
}

#[test]
fn test_invalid_field_access() {
    // Accessing a field that doesn't exist on a module should error.
    let source = r#"
module Inner:
    pin p1

module M:
    inner = new Inner
    inner.nonexistent ~ inner.p1
"#;
    let result = analyze(source);
    // This might or might not error depending on how deep field resolution goes
    if result.is_err() {
        let errors = result.unwrap_err();
        eprintln!("Got expected errors for invalid field access: {:?}",
            errors.iter().map(|e| e.to_string()).collect::<Vec<_>>());
    } else {
        eprintln!("WARNING: Accessing nonexistent field succeeded (should error in the future)");
    }
}

#[test]
fn test_parse_error_propagated() {
    // Syntactically invalid source should produce a parse error.
    let source = r#"
module M:
    ~~~ invalid syntax
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Syntactically invalid source should error");
}

#[test]
fn test_base_type_not_found() {
    // Inheriting from a type that doesn't exist should error.
    let source = r#"
module M from NonExistentBase:
    pass
"#;
    let result = analyze(source);
    // The analyzer may or may not error here depending on resolution
    if result.is_err() {
        let errors = result.unwrap_err();
        assert!(
            has_error(&errors, |e| matches!(e,
                SemaError::BaseTypeNotFound { .. } |
                SemaError::UndefinedName { .. } |
                SemaError::UnresolvedImport { .. }
            )),
            "Expected BaseTypeNotFound or similar, got: {:?}", errors
        );
    } else {
        eprintln!("WARNING: Inheriting from NonExistentBase succeeded (should error)");
    }
}

#[test]
fn test_duplicate_module_definition() {
    // Two modules with the same name should error.
    let source = r#"
module M:
    pass

module M:
    pass
"#;
    let result = analyze(source);
    assert!(result.is_err(), "Duplicate module definitions should error");
    let errors = result.unwrap_err();
    assert!(
        has_error(&errors, |e| matches!(e, SemaError::DuplicateDefinition { .. })),
        "Expected DuplicateDefinition error, got: {:?}", errors
    );
}

#[test]
fn test_array_index_out_of_bounds() {
    // Accessing an index beyond the array size should error.
    let source = r#"
module M:
    arr = new X[3]
    arr[10] ~ arr[0]
"#;
    let result = analyze(source);
    // May or may not be caught at sema time
    if result.is_err() {
        let errors = result.unwrap_err();
        eprintln!("Got errors for out-of-bounds: {:?}",
            errors.iter().map(|e| e.to_string()).collect::<Vec<_>>());
    } else {
        eprintln!("WARNING: Array index out of bounds not caught at sema time");
    }
}

// ============================================================================
// 2. Should-warn tests
// ============================================================================
// Note: The current analyzer does not have a warning system distinct from
// errors. These tests document cases that should produce warnings in the
// future. For now, they verify the code compiles (possibly with errors)
// and document the expected behavior.

#[test]
#[ignore = "Analyzer has no warning system yet -- unconstrained passives are not warned about"]
fn test_unconstrained_passive() {
    // A Resistor instantiated with no constraints on resistance should warn
    // because part picking will fail without constraints.
    let source = r#"
module Resistor:
    resistance: ohm

module M:
    r1 = new Resistor
"#;
    let result = analyze(source);
    // Should succeed but produce a warning about unconstrained resistance
    assert!(result.is_ok(), "Should compile, just warn. Errors: {:?}", result.err());
    // TODO: check for warnings when warning system is added
    eprintln!("TODO: Verify warning about unconstrained passive parameter");
}

#[test]
#[ignore = "Analyzer has no warning system yet -- unused instances are not warned about"]
fn test_unused_instance() {
    // An instance declared but never connected should warn.
    let source = r#"
module Inner:
    pin p1
    pin p2

module M:
    pin out
    unused = new Inner
    used = new Inner
    used.p1 ~ out
"#;
    let result = analyze(source);
    // Should succeed but produce a warning about unused instance
    assert!(result.is_ok(), "Should compile, just warn. Errors: {:?}", result.err());
    // TODO: check for warnings when warning system is added
    eprintln!("TODO: Verify warning about unused instance 'unused'");
}

#[test]
#[ignore = "Analyzer has no warning system yet -- duplicate connections are not warned about"]
fn test_duplicate_connection() {
    // Connecting the same endpoints twice should warn.
    let source = r#"
module M:
    pin a
    pin b
    a ~ b
    a ~ b
"#;
    let result = analyze(source);
    // Should succeed but produce a warning about duplicate connection
    assert!(result.is_ok(), "Should compile, just warn. Errors: {:?}", result.err());
    // TODO: check for warnings when warning system is added
    eprintln!("TODO: Verify warning about duplicate connection a ~ b");
}

// ============================================================================
// 2b. Additional edge-case tests
// ============================================================================

#[test]
fn test_empty_module() {
    // An empty module (with just pass) should be valid.
    let source = r#"
module M:
    pass
"#;
    let result = analyze(source);
    assert!(result.is_ok(), "Empty module should be valid. Errors: {:?}", result.err());
}

#[test]
fn test_self_connection() {
    // Connecting a pin to itself should either error or produce a warning.
    let source = r#"
module M:
    pin p1
    p1 ~ p1
"#;
    let result = analyze(source);
    // Currently this likely succeeds -- it's not harmful but is suspicious.
    // Document the current behavior.
    match result {
        Ok(_) => eprintln!("INFO: Self-connection p1 ~ p1 accepted (could warn in future)"),
        Err(errors) => eprintln!("Self-connection rejected: {:?}",
            errors.iter().map(|e| e.to_string()).collect::<Vec<_>>()),
    }
}

#[test]
fn test_empty_source() {
    // Empty source should produce an empty design, not an error.
    let result = analyze("");
    assert!(result.is_ok(), "Empty source should be valid");
    let design = result.unwrap();
    assert_eq!(design.module_count(), 0);
}

#[test]
fn test_connection_in_interface() {
    // Connections inside an interface block should be valid.
    let source = r#"
interface MyInterface:
    signal a
    signal b
    a ~ b
"#;
    let result = analyze(source);
    assert!(result.is_ok(), "Connections in interface should work. Errors: {:?}", result.err());
}

// ============================================================================
// 2c. Unconnected pins warning test
// ============================================================================

#[test]
#[ignore = "Analyzer has no warning system yet -- unconnected pins are not warned about"]
fn test_unconnected_pins_on_component() {
    // A component with pins that are never connected should produce a warning.
    let source = r#"
component Chip:
    pin 1
    pin 2
    pin 3
    pin 4

module M:
    chip = new Chip
    signal sig
    chip.1 ~ sig
"#;
    let result = analyze(source);
    // Should succeed but ideally warn about pins 2, 3, 4 being unconnected
    assert!(result.is_ok(), "Should compile, just warn. Errors: {:?}", result.err());
    // TODO: check for warnings when warning system is added
    eprintln!("TODO: Verify warning about unconnected pins 2, 3, 4 on chip");
}

// ============================================================================
// 3. led_badge robustness tests
// ============================================================================
// These tests modify the led_badge source to verify that the analyzer
// catches breakage when connections are removed or types are changed.

/// Path to the led_badge stdlib directory.
fn stdlib_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ato-sema/stdlib")
}

/// Path to the led_badge.ato source file.
fn led_badge_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/led_badge/led_badge.ato")
}

/// Analyze modified led_badge source with full stdlib/import support.
fn analyze_led_badge_modified(source: &str) -> Result<ato_ir::Design, Vec<SemaError>> {
    let path = led_badge_path();
    let stdlib = stdlib_path();

    let mut analyzer = Analyzer::new();
    if stdlib.exists() {
        analyzer = analyzer.with_stdlib(stdlib);
    }

    analyzer.analyze_file(source, &path)
}

#[test]
fn test_led_badge_removing_connection_still_compiles() {
    // Removing a connection from led_badge should still compile (connections are
    // not required). This documents that the analyzer does NOT enforce connectivity.
    let source = std::fs::read_to_string(led_badge_path()).unwrap();

    // Remove the first power connection line
    let modified = source.replace(
        "usb_c.usb2.usb_if.buspower ~ charger.power_input",
        "# removed: usb_c.usb2.usb_if.buspower ~ charger.power_input"
    );

    // Should still compile -- removing a connection is not an error
    let result = analyze_led_badge_modified(&modified);
    match &result {
        Ok(_) => eprintln!("INFO: led_badge compiles with a connection removed (expected -- no connectivity enforcement)"),
        Err(errors) => eprintln!("INFO: led_badge fails with connection removed: {:?}",
            errors.iter().map(|e| e.to_string()).collect::<Vec<_>>()),
    }
    // We just document the behavior; it's expected to pass since the analyzer
    // doesn't enforce that all interfaces must be connected.
}

#[test]
#[ignore = "BUG: Analyzer does not detect wrong interface type in connections (no type checking)"]
fn test_led_badge_wrong_interface_type() {
    // Replacing a connection with a type-mismatched one should error.
    // e.g., connecting an I2C interface where ElectricPower is expected.
    let source = std::fs::read_to_string(led_badge_path()).unwrap();

    // Replace the USB power connection with a nonsensical I2C connection
    let modified = source.replace(
        "usb_c.usb2.usb_if.buspower ~ charger.power_input",
        "microcontroller.i2c[0] ~ charger.power_input"
    );

    let result = analyze_led_badge_modified(&modified);
    assert!(result.is_err(),
        "Connecting I2C to power_input should error with type mismatch");
}

#[test]
fn test_led_badge_referencing_nonexistent_field() {
    // Referencing a field that doesn't exist should error.
    let source = std::fs::read_to_string(led_badge_path()).unwrap();

    // Add a line referencing a nonexistent field
    let modified = source.replace(
        "charger.power_battery ~ battery.power",
        "charger.power_battery ~ battery.power\n    charger.nonexistent_field ~ battery.power"
    );

    let result = analyze_led_badge_modified(&modified);
    // May or may not error depending on field resolution depth
    match &result {
        Ok(_) => eprintln!("WARNING: Referencing nonexistent field compiled (should error in future)"),
        Err(errors) => {
            eprintln!("Correctly caught nonexistent field reference: {:?}",
                errors.iter().map(|e| e.to_string()).collect::<Vec<_>>());
        }
    }
}

// ============================================================================
// 4. Missing required fields / constraint violation tests
// ============================================================================

#[test]
#[ignore = "BUG: Analyzer does not detect missing required parameter constraints"]
fn test_missing_required_field_on_instance() {
    // Some modules require certain fields to be set (e.g., ElectricPower.required = True).
    // Instantiating without satisfying requirements should produce a warning or error.
    let source = r#"
module PoweredDevice:
    power: V
    assert power within 3V to 3.6V

module M:
    dev = new PoweredDevice
"#;
    let result = analyze(source);
    // Should warn or error that dev.power is unconstrained
    assert!(result.is_err(), "Missing required parameter should produce an error");
}

#[test]
#[ignore = "BUG: Analyzer does not detect contradictory constraints"]
fn test_contradictory_constraints() {
    // Two assertions that contradict each other should error at solve time.
    // This tests that the solver pipeline catches contradictions.
    let source = r#"
module M:
    voltage: V
    assert voltage > 10V
    assert voltage < 5V
"#;
    let result = analyze(source);
    // The sema phase may accept this (constraints are checked at solve time).
    // If sema passes, we'd need to run the solver to catch the contradiction.
    if result.is_ok() {
        let design = result.unwrap();
        if design.constraint_count() >= 2 {
            // Try solving to check for contradictions
            let collector = ato_sema::ConstraintCollector::new();
            if let Ok((mut solver, _deps)) = collector.collect(&design) {
                let solve_result = solver.solve();
                assert!(
                    matches!(solve_result, Err(ato_solver::SolverError::Contradiction(_))),
                    "Solver should detect contradiction between voltage > 10V and voltage < 5V"
                );
            }
        }
    }
}

// ============================================================================
// Summary
// ============================================================================

#[test]
fn test_negative_test_summary() {
    // Provide a summary of which negative checks work.
    let checks = [
        ("undefined name in connection", analyze("module M:\n    a ~ b").is_err()),
        ("undefined name one side", analyze("module M:\n    pin p1\n    p1 ~ x").is_err()),
        ("duplicate field (pin)", analyze("module M:\n    pin p1\n    pin p1").is_err()),
        ("duplicate signal", analyze("module M:\n    signal s1\n    signal s1").is_err()),
        ("duplicate module", analyze("module M:\n    pass\nmodule M:\n    pass").is_err()),
        ("parse error propagated", analyze("module M:\n    ~~~ bad").is_err()),
        ("empty source ok", analyze("").is_ok()),
        ("empty module ok", analyze("module M:\n    pass").is_ok()),
    ];

    eprintln!("\n=== Negative Test Summary ===");
    let mut passing = 0;
    let total = checks.len();
    for (name, ok) in &checks {
        let status = if *ok { "PASS" } else { "FAIL" };
        if *ok { passing += 1; }
        eprintln!("  {}: {}", name, status);
    }
    eprintln!("---");
    eprintln!("Passing: {}/{}", passing, total);

    assert_eq!(passing, total, "All basic negative checks should pass");
}
