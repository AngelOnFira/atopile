//! Syntax coverage tests - verify all language features work.
//!
//! Each test verifies a specific syntax construct from the Ato language.

/// Parse a source string and verify success.
fn parse_ok(source: &str) -> Result<ato_parser::File, String> {
    ato_parser::parse(source)
        .map_err(|e| format!("Parse error: {:?}", e))
}

/// Parse a source string and verify it fails.
fn parse_err(source: &str) -> bool {
    ato_parser::parse(source).is_err()
}

// ============================================================================
// Imports
// ============================================================================

#[test]
fn test_syntax_import_simple() {
    let result = parse_ok("import ModuleName");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_import_multiple() {
    let result = parse_ok("import Module1, Module2, Module3");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_import_qualified() {
    let result = parse_ok("import Module1.Submodule");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_from_import() {
    let result = parse_ok("from \"path/to/file.ato\" import Module");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_deprecated_import() {
    // Deprecated form: import X from "path"
    let result = parse_ok("import Module from \"path/to/file.ato\"");
    assert!(result.is_ok());
}

// ============================================================================
// Pragmas
// ============================================================================

#[test]
fn test_syntax_pragma_simple() {
    let result = parse_ok("#pragma text");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_pragma_with_args() {
    let result = parse_ok("#pragma func(\"X\")");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_pragma_experiment() {
    let result = parse_ok("#pragma experiment(\"FOR_LOOP\")");
    assert!(result.is_ok());
}

// ============================================================================
// Block Definitions
// ============================================================================

#[test]
fn test_syntax_module() {
    let result = parse_ok("module Test:\n    pass");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_component() {
    let result = parse_ok("component Test:\n    pass");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_interface() {
    let result = parse_ok("interface Test:\n    pass");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_module_inheritance() {
    let result = parse_ok("module Base:\n    pass\n\nmodule Derived from Base:\n    pass");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_nested_component() {
    let result = parse_ok("module Outer:\n    component Inner:\n        pass");
    assert!(result.is_ok());
}

// ============================================================================
// Declarations
// ============================================================================

#[test]
fn test_syntax_pin_name() {
    let result = parse_ok("module Test:\n    pin my_pin");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_pin_number() {
    let result = parse_ok("module Test:\n    pin 1");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_pin_string() {
    let result = parse_ok("module Test:\n    pin \"GND\"");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_signal() {
    let result = parse_ok("module Test:\n    signal my_signal");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_field_declaration() {
    let result = parse_ok("module Test:\n    field: SomeType");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_parameter_declaration() {
    let result = parse_ok("module Test:\n    resistance: ohm");
    assert!(result.is_ok());
}

// ============================================================================
// Assignments
// ============================================================================

#[test]
fn test_syntax_assignment_int() {
    let result = parse_ok("module Test:\n    x = 123");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assignment_negative() {
    let result = parse_ok("module Test:\n    x = -50");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assignment_float() {
    let result = parse_ok("module Test:\n    x = 3.14");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assignment_string() {
    let result = parse_ok("module Test:\n    x = \"hello\"");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assignment_bool_true() {
    let result = parse_ok("module Test:\n    x = True");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assignment_bool_false() {
    let result = parse_ok("module Test:\n    x = False");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assignment_hex() {
    let result = parse_ok("module Test:\n    x = 0xFF");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assignment_binary() {
    let result = parse_ok("module Test:\n    x = 0b1010");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assignment_octal() {
    let result = parse_ok("module Test:\n    x = 0o777");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_cumulative_add() {
    let result = parse_ok("module Test:\n    x = 0\n    x += 1");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_cumulative_sub() {
    let result = parse_ok("module Test:\n    x = 0\n    x -= 1");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_set_or() {
    let result = parse_ok("module Test:\n    x = 0\n    x |= 1");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_set_and() {
    let result = parse_ok("module Test:\n    x = 0\n    x &= 1");
    assert!(result.is_ok());
}

// ============================================================================
// Physical Quantities
// ============================================================================

#[test]
fn test_syntax_quantity_voltage() {
    let result = parse_ok("module Test:\n    v = 5V");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_quantity_millivolt() {
    let result = parse_ok("module Test:\n    v = 100mV");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_quantity_resistance() {
    let result = parse_ok("module Test:\n    r = 10kohm");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_quantity_capacitance() {
    let result = parse_ok("module Test:\n    c = 100nF");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_quantity_current() {
    let result = parse_ok("module Test:\n    i = 1mA");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_bilateral_tolerance_percent() {
    let result = parse_ok("module Test:\n    r = 10kohm +/- 5%");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_bilateral_tolerance_absolute() {
    let result = parse_ok("module Test:\n    v = 5V +/- 100mV");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_range() {
    let result = parse_ok("module Test:\n    v = 3V to 3.6V");
    assert!(result.is_ok());
}

// ============================================================================
// Connections
// ============================================================================

#[test]
fn test_syntax_connection_simple() {
    let result = parse_ok("module Test:\n    pin a\n    pin b\n    a ~ b");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_connection_directed() {
    let result = parse_ok("module Test:\n    pin a\n    pin b\n    a ~> b");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_connection_directed_reverse() {
    let result = parse_ok("module Test:\n    pin a\n    pin b\n    a <~ b");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_connection_chain() {
    let result = parse_ok("module Test:\n    pin a\n    pin b\n    pin c\n    a ~> b ~> c");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_connection_field_access() {
    let result = parse_ok("module Test:\n    x.pin1 ~ y.pin2");
    assert!(result.is_ok());
}

// ============================================================================
// Instantiation
// ============================================================================

#[test]
fn test_syntax_new_simple() {
    let result = parse_ok("module Test:\n    x = new SomeModule");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_new_array() {
    let result = parse_ok("module Test:\n    x = new SomeModule[10]");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_new_template_int() {
    let result = parse_ok("module Test:\n    x = new SomeModule<param=1>");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_new_template_float() {
    let result = parse_ok("module Test:\n    x = new SomeModule<param=2.5>");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_new_template_string() {
    let result = parse_ok("module Test:\n    x = new SomeModule<param=\"hello\">");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_new_template_bool() {
    let result = parse_ok("module Test:\n    x = new SomeModule<param=True>");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_new_template_multiple() {
    let result = parse_ok("module Test:\n    x = new SomeModule<a=1, b=2.5, c=\"hello\">");
    assert!(result.is_ok());
}

// ============================================================================
// Traits
// ============================================================================

#[test]
fn test_syntax_trait_simple() {
    let result = parse_ok("module Test:\n    trait my_trait");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_trait_with_template() {
    let result = parse_ok("module Test:\n    trait my_trait<param=1>");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_trait_constructor() {
    let result = parse_ok("module Test:\n    trait my_trait::constructor");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_trait_constructor_template() {
    let result = parse_ok("module Test:\n    trait my_trait::constructor<param=1>");
    assert!(result.is_ok());
}

// ============================================================================
// Assertions
// ============================================================================

#[test]
fn test_syntax_assert_greater() {
    let result = parse_ok("module Test:\n    x: V\n    assert x > 5V");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assert_less() {
    let result = parse_ok("module Test:\n    x: V\n    assert x < 10V");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assert_greater_equal() {
    let result = parse_ok("module Test:\n    x: V\n    assert x >= 5V");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assert_less_equal() {
    let result = parse_ok("module Test:\n    x: V\n    assert x <= 10V");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assert_chained() {
    let result = parse_ok("module Test:\n    x: V\n    assert 3V < x < 5V");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assert_within() {
    let result = parse_ok("module Test:\n    x: V\n    assert x within 5V +/- 10%");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_assert_is_range() {
    let result = parse_ok("module Test:\n    x: V\n    assert x is 3V to 5V");
    assert!(result.is_ok());
}

// ============================================================================
// For Loops
// ============================================================================

#[test]
fn test_syntax_for_loop() {
    let result = parse_ok("module Test:\n    arr = new X[5]\n    for item in arr:\n        pass");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_for_loop_slice() {
    let result = parse_ok("module Test:\n    arr = new X[10]\n    for item in arr[0:5]:\n        pass");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_for_loop_list() {
    let result = parse_ok("module Test:\n    pin a\n    pin b\n    for item in [a, b]:\n        pass");
    assert!(result.is_ok());
}

// ============================================================================
// Arithmetic Expressions (only in assertions, not assignments per grammar)
// ============================================================================

#[test]
fn test_syntax_expr_add_in_assert() {
    // Note: Arithmetic is allowed in assertions, not plain assignments
    let result = parse_ok("module Test:\n    x: V\n    assert x is (1V + 2V)");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_expr_sub_in_assert() {
    let result = parse_ok("module Test:\n    x: V\n    assert x is (5V - 3V)");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_expr_mul_in_assert() {
    let result = parse_ok("module Test:\n    x: V\n    assert x is (2V * 3)");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_expr_div_in_assert() {
    let result = parse_ok("module Test:\n    x: A\n    assert x is (10V / 2kohm)");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_expr_grouped_in_assert() {
    let result = parse_ok("module Test:\n    x: V\n    assert x is ((1V + 2V) * 3)");
    assert!(result.is_ok());
}

// ============================================================================
// Retyping
// ============================================================================

#[test]
fn test_syntax_retype() {
    let result = parse_ok("module Test:\n    x.field -> NewType");
    assert!(result.is_ok());
}

// ============================================================================
// Array Indexing
// ============================================================================

#[test]
fn test_syntax_array_index() {
    let result = parse_ok("module Test:\n    arr = new X[10]\n    y = arr[5]");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_field_array_index() {
    let result = parse_ok("module Test:\n    x.arr[0] ~ y.arr[1]");
    assert!(result.is_ok());
}

// ============================================================================
// Docstrings
// ============================================================================

#[test]
fn test_syntax_docstring() {
    let result = parse_ok("\"This is a docstring\"\nmodule Test:\n    pass");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_docstring_in_module() {
    let result = parse_ok("module Test:\n    \"Module docstring\"\n    pass");
    assert!(result.is_ok());
}

// ============================================================================
// Pass Statement
// ============================================================================

#[test]
fn test_syntax_pass() {
    let result = parse_ok("module Test:\n    pass");
    assert!(result.is_ok());
}

// ============================================================================
// Top-level Statements
// ============================================================================

#[test]
fn test_syntax_toplevel_pass() {
    let result = parse_ok("pass");
    assert!(result.is_ok());
}

#[test]
fn test_syntax_toplevel_assignment() {
    let result = parse_ok("x = 123");
    assert!(result.is_ok());
}

// ============================================================================
// Known Limitations / Gaps
// ============================================================================

#[test]
fn test_gap_semicolon_separated_toplevel() {
    // KNOWN GAP: Semicolon-separated statements at top level
    // The grammar allows this but Rust parser doesn't
    let result = parse_ok("import X; import Y");
    // This is expected to fail
    assert!(result.is_err(), "Semicolon-separated top-level statements not yet supported");
}

#[test]
fn test_gap_in_as_identifier() {
    // 'in' keyword can be used as identifier (e.g., signal names, pin names)
    // The parser's identifier() function accepts keyword tokens including In
    let result = parse_ok("module Test:\n    signal in");
    assert!(result.is_ok(), "Expected 'in' to work as identifier, got error: {:?}", result.err());
}

#[test]
fn test_gap_unicode_ohm() {
    // KNOWN GAP: Unicode Ω not supported
    // Only ASCII 'ohm' works
    let result = parse_ok("module Test:\n    r = 10kΩ");
    // This is expected to fail
    assert!(result.is_err(), "Unicode Ω not yet supported");
}

// ============================================================================
// Summary Test
// ============================================================================

#[test]
fn test_syntax_coverage_summary() {
    // Count passing features
    let features = [
        ("imports", parse_ok("import X").is_ok()),
        ("from imports", parse_ok("from \"x.ato\" import Y").is_ok()),
        ("pragmas", parse_ok("#pragma text").is_ok()),
        ("modules", parse_ok("module X:\n    pass").is_ok()),
        ("components", parse_ok("component X:\n    pass").is_ok()),
        ("interfaces", parse_ok("interface X:\n    pass").is_ok()),
        ("inheritance", parse_ok("module B:\n    pass\nmodule D from B:\n    pass").is_ok()),
        ("pins", parse_ok("module X:\n    pin p").is_ok()),
        ("signals", parse_ok("module X:\n    signal s").is_ok()),
        ("parameters", parse_ok("module X:\n    r: ohm").is_ok()),
        ("assignments", parse_ok("module X:\n    x = 1").is_ok()),
        ("connections", parse_ok("module X:\n    a ~ b").is_ok()),
        ("directed connections", parse_ok("module X:\n    a ~> b").is_ok()),
        ("new instances", parse_ok("module X:\n    x = new Y").is_ok()),
        ("array instances", parse_ok("module X:\n    x = new Y[5]").is_ok()),
        ("templates", parse_ok("module X:\n    x = new Y<a=1>").is_ok()),
        ("traits", parse_ok("module X:\n    trait t").is_ok()),
        ("assertions", parse_ok("module X:\n    x: V\n    assert x > 0V").is_ok()),
        ("for loops", parse_ok("module X:\n    for i in arr:\n        pass").is_ok()),
        ("quantities", parse_ok("module X:\n    x = 5V").is_ok()),
        ("tolerances", parse_ok("module X:\n    x = 5V +/- 5%").is_ok()),
        ("ranges", parse_ok("module X:\n    x = 3V to 5V").is_ok()),
        ("arithmetic (in assert)", parse_ok("module X:\n    x: V\n    assert x is (1V + 2V)").is_ok()),
        ("retype", parse_ok("module X:\n    x -> Y").is_ok()),
        ("docstrings", parse_ok("\"doc\"\nmodule X:\n    pass").is_ok()),
        ("pass", parse_ok("pass").is_ok()),
        ("nested blocks", parse_ok("module X:\n    component Y:\n        pass").is_ok()),
    ];

    let passing = features.iter().filter(|(_, ok)| *ok).count();
    let total = features.len();

    eprintln!("\n=== Syntax Coverage Summary ===");
    for (name, ok) in &features {
        eprintln!("{}: {}", name, if *ok { "✅" } else { "❌" });
    }
    eprintln!("---");
    eprintln!("Coverage: {}/{} ({:.0}%)", passing, total, passing as f64 / total as f64 * 100.0);

    // All core features should work
    assert_eq!(passing, total, "All core syntax features should be supported");
}
