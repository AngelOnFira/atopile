//! Integration tests that verify the lexer works on real .ato files.

use ato_lexer::{lex, TokenKind};
use std::fs;
use std::path::{Path, PathBuf};

/// Get the project root directory.
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Helper to lex a file and ensure no errors.
fn lex_file(path: &Path) -> Vec<ato_lexer::Token> {
    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("Failed to read file {}: {}", path.display(), e)
    });
    let (tokens, errors) = lex(&source);

    if !errors.is_empty() {
        panic!(
            "Lexer errors in {}: {:?}",
            path.display(),
            errors
        );
    }

    // Should always end with EOF
    assert!(
        tokens.last().map(|t| t.kind) == Some(TokenKind::Eof),
        "Token stream should end with EOF. Last token: {:?}",
        tokens.last()
    );

    tokens
}

#[test]
fn test_lex_led_badge() {
    let path = project_root().join("examples/led_badge/led_badge.ato");
    if !path.exists() {
        eprintln!("Skipping test_lex_led_badge: file not found at {:?}", path);
        return;
    }

    let tokens = lex_file(&path);

    // Should have pragma tokens
    let has_pragma = tokens.iter().any(|t| t.kind == TokenKind::Pragma);
    assert!(has_pragma, "Should have pragma tokens");

    // Should have module keyword
    let has_module = tokens.iter().any(|t| t.kind == TokenKind::Module);
    assert!(has_module, "Should have module keyword");

    // Should have import keyword
    let has_import = tokens.iter().any(|t| t.kind == TokenKind::Import);
    assert!(has_import, "Should have import keyword");

    // Should have new keyword
    let has_new = tokens.iter().any(|t| t.kind == TokenKind::New);
    assert!(has_new, "Should have new keyword");

    // Should have connection operators
    let has_wire = tokens.iter().any(|t| t.kind == TokenKind::Wire);
    let has_sperm = tokens.iter().any(|t| t.kind == TokenKind::Sperm);
    assert!(has_wire, "Should have wire (~) operator");
    assert!(has_sperm, "Should have sperm (~>) operator");

    // Should have physical quantities
    let numbers: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Number)
        .collect();
    assert!(!numbers.is_empty(), "Should have number tokens");

    // Check for specific quantities
    let texts: Vec<_> = numbers.iter().map(|t| t.text.as_str()).collect();
    assert!(
        texts.iter().any(|t| t.contains("kohm") || t.contains("V") || t.contains("mA")),
        "Should have physical quantities with units"
    );

    // Should have INDENT/DEDENT
    let has_indent = tokens.iter().any(|t| t.kind == TokenKind::Indent);
    let has_dedent = tokens.iter().any(|t| t.kind == TokenKind::Dedent);
    assert!(has_indent, "Should have INDENT tokens");
    assert!(has_dedent, "Should have DEDENT tokens");

    // Should have traits
    let has_trait = tokens.iter().any(|t| t.kind == TokenKind::Trait);
    assert!(has_trait, "Should have trait keyword");

    // Should have for loops
    let has_for = tokens.iter().any(|t| t.kind == TokenKind::For);
    let has_in = tokens.iter().any(|t| t.kind == TokenKind::In);
    assert!(has_for, "Should have for keyword");
    assert!(has_in, "Should have in keyword");
}

#[test]
fn test_lex_quickstart() {
    let path = project_root().join("examples/quickstart/quickstart.ato");
    if !path.exists() {
        eprintln!("Skipping test_lex_quickstart: file not found at {:?}", path);
        return;
    }

    let tokens = lex_file(&path);
    assert!(tokens.last().map(|t| t.kind) == Some(TokenKind::Eof));
}

#[test]
fn test_lex_i2c() {
    let path = project_root().join("examples/i2c/i2c.ato");
    if !path.exists() {
        eprintln!("Skipping test_lex_i2c: file not found at {:?}", path);
        return;
    }

    let tokens = lex_file(&path);
    assert!(tokens.last().map(|t| t.kind) == Some(TokenKind::Eof));
}

#[test]
fn test_comprehensive_tokens() {
    // Test all major token types in one source
    let source = r#"#pragma experiment("FOR_LOOP")
# This is a comment
import ElectricPower
from "path/to/file.ato" import Module

component MyComponent:
    """Docstring"""
    pin p1
    signal sig

module MyModule from BaseModule:
    # Declarations
    field: ohm
    power = new ElectricPower
    resistors = new Resistor[10]

    # Assignments
    resistance = 10kohm +/- 5%
    voltage = 3.3V to 5V
    flag = True
    name = "hello"

    # Connections
    p1 ~ p2
    a ~> bridge ~> b
    c <~ other <~ d
    x -> SomeType

    # Assertions
    assert voltage within 3V to 6V
    assert current is 100mA +/- 10%
    assert x >= 0V
    assert y <= 10V

    # For loop
    for item in resistors:
        item ~ power

    # Trait
    trait some_trait<arg = 1>
    trait other::constructor

    # Operators
    sum = a + b - c * d / e ** f
    hex = 0xFF
    bin = 0b1010
    oct = 0o77

    # Expression with parens
    grouped = (x + y)

interface MyInterface:
    pass
"#;

    let (tokens, errors) = lex(source);

    assert!(errors.is_empty(), "Unexpected errors: {:?}", errors);

    // Collect all token kinds
    let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();

    // Verify we got all expected token types
    assert!(kinds.contains(&TokenKind::Pragma));
    assert!(kinds.contains(&TokenKind::Import));
    assert!(kinds.contains(&TokenKind::From));
    assert!(kinds.contains(&TokenKind::Component));
    assert!(kinds.contains(&TokenKind::Module));
    assert!(kinds.contains(&TokenKind::Interface));
    assert!(kinds.contains(&TokenKind::Pin));
    assert!(kinds.contains(&TokenKind::Signal));
    assert!(kinds.contains(&TokenKind::New));
    assert!(kinds.contains(&TokenKind::Assign));
    assert!(kinds.contains(&TokenKind::Wire));
    assert!(kinds.contains(&TokenKind::Sperm));
    assert!(kinds.contains(&TokenKind::LSperm));
    assert!(kinds.contains(&TokenKind::Arrow));
    assert!(kinds.contains(&TokenKind::Assert));
    assert!(kinds.contains(&TokenKind::Within));
    assert!(kinds.contains(&TokenKind::Is));
    assert!(kinds.contains(&TokenKind::To));
    assert!(kinds.contains(&TokenKind::For));
    assert!(kinds.contains(&TokenKind::In));
    assert!(kinds.contains(&TokenKind::Trait));
    assert!(kinds.contains(&TokenKind::DoubleColon));
    assert!(kinds.contains(&TokenKind::Pass));
    assert!(kinds.contains(&TokenKind::True));
    assert!(kinds.contains(&TokenKind::PlusOrMinus));
    assert!(kinds.contains(&TokenKind::Plus));
    assert!(kinds.contains(&TokenKind::Minus));
    assert!(kinds.contains(&TokenKind::Star));
    assert!(kinds.contains(&TokenKind::Div));
    assert!(kinds.contains(&TokenKind::Power));
    assert!(kinds.contains(&TokenKind::GreaterEq));
    assert!(kinds.contains(&TokenKind::LessEq));
    assert!(kinds.contains(&TokenKind::Colon));
    assert!(kinds.contains(&TokenKind::OpenParen));
    assert!(kinds.contains(&TokenKind::CloseParen));
    assert!(kinds.contains(&TokenKind::OpenBracket));
    assert!(kinds.contains(&TokenKind::CloseBracket));
    assert!(kinds.contains(&TokenKind::LessThan));
    assert!(kinds.contains(&TokenKind::GreaterThan));
    assert!(kinds.contains(&TokenKind::Indent));
    assert!(kinds.contains(&TokenKind::Dedent));
    assert!(kinds.contains(&TokenKind::String));
    assert!(kinds.contains(&TokenKind::Number));
    assert!(kinds.contains(&TokenKind::Name));
    assert!(kinds.contains(&TokenKind::Newline));
    assert!(kinds.contains(&TokenKind::Eof));
}

#[test]
fn test_token_spans() {
    let source = "module M:\n    pin p";
    let (tokens, errors) = lex(source);

    assert!(errors.is_empty());

    // Find the 'module' token
    let module_token = tokens.iter().find(|t| t.kind == TokenKind::Module).unwrap();
    assert_eq!(module_token.span.line, 1);
    assert_eq!(module_token.span.column, 1);
    assert_eq!(module_token.span.start, 0);
    assert_eq!(module_token.span.end, 6);

    // Find 'pin' token (should be on line 2)
    let pin_token = tokens.iter().find(|t| t.kind == TokenKind::Pin).unwrap();
    assert_eq!(pin_token.span.line, 2);
}

#[test]
fn test_nested_indentation() {
    let source = r#"module A:
    module B:
        module C:
            pass
        pass
    pass
"#;

    let (tokens, errors) = lex(source);
    assert!(errors.is_empty(), "Errors: {:?}", errors);

    let indent_count = tokens.iter().filter(|t| t.kind == TokenKind::Indent).count();
    let dedent_count = tokens.iter().filter(|t| t.kind == TokenKind::Dedent).count();

    assert_eq!(indent_count, 3, "Should have 3 INDENT tokens");
    assert_eq!(dedent_count, 3, "Should have 3 DEDENT tokens");
}
