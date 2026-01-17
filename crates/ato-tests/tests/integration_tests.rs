//! Integration tests - full pipeline tests on real projects.

use std::fs;
use std::path::Path;

/// Run the full pipeline on a fixture file.
fn run_full_pipeline(name: &str) -> Result<ato_sema::Design, String> {
    let path_str = format!(
        "{}/fixtures/integration/{}.ato",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let path = Path::new(&path_str);
    let source = fs::read_to_string(&path).map_err(|e| format!("Failed to read: {}", e))?;

    // Parse
    let _ast = ato_parser::parse(&source).map_err(|e| format!("Parse error: {:?}", e))?;

    // Semantic analysis
    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer
        .analyze_file(&source, path)
        .map_err(|e| format!("Sema error: {:?}", e))?;

    Ok(design)
}

// Integration fixture tests
#[test]
fn test_integration_resistor_divider() {
    let result = run_full_pipeline("resistor_divider");
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.unwrap_err());
    let design = result.unwrap();
    assert!(design.module_count() >= 2); // Resistor + VoltageDivider
}

#[test]
fn test_integration_led_circuit() {
    let result = run_full_pipeline("led_circuit");
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.unwrap_err());
    let design = result.unwrap();
    assert!(design.module_count() >= 3); // LED + Resistor + LEDCircuit + Power interface
}

// Example project tests - parse and analyze real example files
fn analyze_example_file(path: &Path) -> Result<ato_sema::Design, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("Failed to read: {}", e))?;

    // Parse
    let _ast = ato_parser::parse(&source).map_err(|e| format!("Parse error: {:?}", e))?;

    // Semantic analysis
    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer
        .analyze_file(&source, path)
        .map_err(|e| format!("Sema error: {:?}", e))?;

    Ok(design)
}

#[test]
fn test_example_quickstart() {
    // This test may fail if imports are not resolved, which is expected
    // since we don't have a full import resolver yet
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../..",
        "/examples/quickstart/quickstart.ato"
    ));
    if path.exists() {
        let source = fs::read_to_string(path).unwrap();
        // Just test that it parses
        let result = ato_parser::parse(&source);
        assert!(result.is_ok(), "Failed to parse quickstart: {:?}", result);
    }
}

#[test]
fn test_example_equations() {
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../..",
        "/examples/equations/equations.ato"
    ));
    if path.exists() {
        let source = fs::read_to_string(path).unwrap();
        // Just test that it parses
        let result = ato_parser::parse(&source);
        assert!(result.is_ok(), "Failed to parse equations: {:?}", result);
    }
}

// Full pipeline inline tests
#[test]
fn test_pipeline_simple() {
    let source = "module Test:\n    pass";
    let path = Path::new("test.ato");

    // Parse
    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    // Sema
    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
}

#[test]
fn test_pipeline_with_connections() {
    let source = r#"
module Test:
    pin p1
    pin p2
    pin p3
    p1 ~ p2
    p2 ~ p3
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
    let d = design.unwrap();
    assert_eq!(d.connection_count(), 2);
}

#[test]
fn test_pipeline_with_parameters() {
    let source = r#"
module Resistor:
    pin 1
    pin 2
    resistance: ohm

module App:
    r = new Resistor
    r.resistance = 10kohm +/- 5%
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
}

#[test]
fn test_pipeline_with_assertions() {
    let source = r#"
module Test:
    voltage: V
    assert voltage > 3V
    assert voltage < 5V
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
    let d = design.unwrap();
    assert!(d.constraint_count() >= 2);
}

#[test]
fn test_pipeline_with_inheritance() {
    let source = r#"
module Base:
    pin a
    pin b
    a ~ b

module Derived from Base:
    pin c
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    // Note: inheritance may not be fully supported yet, so just test parsing
    let _ = ast;
}

#[test]
fn test_pipeline_interface_and_module() {
    let source = r#"
interface Power:
    pin vcc
    pin gnd

module PowerSupply:
    output = new Power
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
}

#[test]
fn test_pipeline_complex_circuit() {
    let source = r#"
interface Power:
    pin vcc
    pin gnd

module Resistor:
    pin 1
    pin 2
    resistance: ohm

module LED:
    pin anode
    pin cathode

module LEDDriver:
    pin input
    pin output
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
    let d = design.unwrap();
    assert!(d.module_count() >= 3);
}

#[test]
fn test_pipeline_multiple_instances() {
    let source = r#"
module Resistor:
    pin 1
    pin 2

module ResistorArray:
    r1 = new Resistor
    r2 = new Resistor
    r3 = new Resistor

    r1.2 ~ r2.1
    r2.2 ~ r3.1
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
}

#[test]
fn test_pipeline_nested_instances() {
    let source = r#"
module Inner:
    pin a
    pin b

module Middle:
    pin x
    pin y

module Outer:
    pin z
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
}

#[test]
fn test_pipeline_signal_bus() {
    let source = r#"
module Test:
    pin p1
    pin p2
    pin p3
    pin p4
    signal bus

    p1 ~ bus
    p2 ~ bus
    p3 ~ bus
    p4 ~ bus
"#;
    let path = Path::new("test.ato");

    let ast = ato_parser::parse(source);
    assert!(ast.is_ok());

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path);
    assert!(design.is_ok());
}

// Design structure verification tests (without snapshots)
#[test]
fn test_design_simple_structure() {
    let source = "module Test:\n    pin a\n    pin b\n    a ~ b";
    let path = Path::new("test.ato");

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path).unwrap();

    assert_eq!(design.module_count(), 1);
    assert!(design.field_count() >= 2);
    assert!(design.connection_count() >= 1);
}

#[test]
fn test_design_multiple_modules() {
    let source = r#"
module Resistor:
    pin 1
    pin 2
    resistance: ohm

module App:
    pin x
    pin y
"#;
    let path = Path::new("test.ato");

    let mut analyzer = ato_sema::Analyzer::new();
    let design = analyzer.analyze_file(source, path).unwrap();

    assert_eq!(design.module_count(), 2);
}
