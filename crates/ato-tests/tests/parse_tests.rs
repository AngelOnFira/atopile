//! Parse tests - verify parsing of valid/invalid .ato files.

use std::fs;
use std::path::Path;

/// Test that all valid fixture files parse successfully.
fn parse_valid_fixture(name: &str) {
    let path = format!(
        "{}/fixtures/parse/valid/{}.ato",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let source = fs::read_to_string(&path).expect("Failed to read fixture");
    let result = ato_parser::parse(&source);
    assert!(
        result.is_ok(),
        "Expected {} to parse successfully, got errors: {:?}",
        name,
        result.unwrap_err()
    );
}

/// Test that invalid fixture files produce parse errors.
fn parse_invalid_fixture(name: &str) {
    let path = format!(
        "{}/fixtures/parse/invalid/{}.ato",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let source = fs::read_to_string(&path).expect("Failed to read fixture");
    let result = ato_parser::parse(&source);
    assert!(
        result.is_err(),
        "Expected {} to fail parsing, but it succeeded",
        name
    );
}

// Valid parse tests
#[test]
fn test_parse_empty() {
    parse_valid_fixture("empty");
}

#[test]
fn test_parse_simple_module() {
    parse_valid_fixture("simple_module");
}

#[test]
fn test_parse_module_with_pins() {
    parse_valid_fixture("module_with_pins");
}

#[test]
fn test_parse_connections() {
    parse_valid_fixture("connections");
}

#[test]
fn test_parse_interface() {
    parse_valid_fixture("interface");
}

#[test]
fn test_parse_component() {
    parse_valid_fixture("component");
}

#[test]
fn test_parse_imports() {
    parse_valid_fixture("imports");
}

#[test]
fn test_parse_quantities() {
    parse_valid_fixture("quantities");
}

#[test]
fn test_parse_assertions() {
    parse_valid_fixture("assertions");
}

#[test]
fn test_parse_inheritance() {
    parse_valid_fixture("inheritance");
}

#[test]
fn test_parse_arrays() {
    parse_valid_fixture("arrays");
}

#[test]
fn test_parse_docstrings() {
    parse_valid_fixture("docstrings");
}

// Invalid parse tests
#[test]
fn test_parse_missing_colon() {
    parse_invalid_fixture("missing_colon");
}

#[test]
fn test_parse_bad_indent() {
    parse_invalid_fixture("bad_indent");
}

#[test]
fn test_parse_unclosed_string() {
    parse_invalid_fixture("unclosed_string");
}

#[test]
fn test_parse_invalid_operator() {
    parse_invalid_fixture("invalid_operator");
}

#[test]
fn test_parse_missing_name() {
    parse_invalid_fixture("missing_name");
}

// AST structure verification tests (without snapshots)
#[test]
fn test_ast_simple_module_structure() {
    let source = "module Test:\n    pass";
    let ast = ato_parser::parse(source).unwrap();
    assert_eq!(ast.statements.len(), 1);
    // Verify it's a BlockDef of type Module
    if let ato_parser::Statement::BlockDef(block) = &ast.statements[0] {
        assert_eq!(block.kind, ato_parser::BlockKind::Module);
        assert_eq!(block.name.name, "Test");
    } else {
        panic!("Expected BlockDef");
    }
}

#[test]
fn test_ast_module_with_pins_structure() {
    let source = "module Test:\n    pin p1\n    pin p2\n    p1 ~ p2";
    let ast = ato_parser::parse(source).unwrap();
    assert_eq!(ast.statements.len(), 1);
    if let ato_parser::Statement::BlockDef(block) = &ast.statements[0] {
        assert_eq!(block.body.len(), 3);
        assert!(matches!(block.body[0], ato_parser::Statement::PinDeclaration(_)));
        assert!(matches!(block.body[1], ato_parser::Statement::PinDeclaration(_)));
        assert!(matches!(block.body[2], ato_parser::Statement::Connection(_)));
    } else {
        panic!("Expected BlockDef");
    }
}

#[test]
fn test_ast_quantities_structure() {
    let source = "module Test:\n    x: V = 5V\n    y: ohm = 10kohm +/- 5%";
    let ast = ato_parser::parse(source).unwrap();
    assert_eq!(ast.statements.len(), 1);
    if let ato_parser::Statement::BlockDef(block) = &ast.statements[0] {
        assert_eq!(block.body.len(), 2);
        assert!(matches!(block.body[0], ato_parser::Statement::Assignment(_)));
        assert!(matches!(block.body[1], ato_parser::Statement::Assignment(_)));
    } else {
        panic!("Expected BlockDef");
    }
}

#[test]
fn test_ast_inheritance_structure() {
    let source = "module Base:\n    pin a\n\nmodule Derived from Base:\n    pin b";
    let ast = ato_parser::parse(source).unwrap();
    assert_eq!(ast.statements.len(), 2);
    if let ato_parser::Statement::BlockDef(derived) = &ast.statements[1] {
        assert!(derived.super_type.is_some());
        assert_eq!(derived.super_type.as_ref().unwrap().parts[0].name, "Base");
    } else {
        panic!("Expected BlockDef");
    }
}

// Inline tests for common patterns
#[test]
fn test_parse_inline_empty() {
    let result = ato_parser::parse("");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().statements.len(), 0);
}

#[test]
fn test_parse_inline_comment_only() {
    let result = ato_parser::parse("# just a comment\n");
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_multiple_modules() {
    let source = "module A:\n    pass\n\nmodule B:\n    pass\n\nmodule C:\n    pass";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().statements.len(), 3);
}

#[test]
fn test_parse_inline_nested_new() {
    let source = "module Test:\n    r = new Resistor\n    c = new Capacitor";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_signal() {
    let source = "module Test:\n    signal my_signal";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_semicolon_separated() {
    let source = "module Test:\n    pass; pass; pass";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_hex_number() {
    let source = "module Test:\n    val = 0xFF";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_binary_number() {
    let source = "module Test:\n    val = 0b1010";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_octal_number() {
    let source = "module Test:\n    val = 0o777";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_float() {
    let source = "module Test:\n    val = 3.14159";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_negative_number() {
    let source = "module Test:\n    val = -42";
    let result = ato_parser::parse(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_inline_physical_units() {
    let units = vec!["V", "mV", "uV", "kV", "ohm", "kohm", "Mohm", "F", "nF", "pF", "uF", "A", "mA", "uA"];
    for unit in units {
        let source = format!("module Test:\n    val = 10{}", unit);
        let result = ato_parser::parse(&source);
        assert!(result.is_ok(), "Failed to parse unit: {}", unit);
    }
}
