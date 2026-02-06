//! Parser for the Ato hardware description language.
//!
//! This crate provides a complete parser for the Ato DSL, producing a typed AST
//! with error recovery using the chumsky parser combinator library.
//!
//! # Example
//!
//! ```
//! use ato_parser::parse;
//!
//! let source = r#"
//! module MyModule:
//!     pin p1
//!     signal sig
//!     p1 ~ sig
//! "#;
//!
//! match parse(source) {
//!     Ok(file) => {
//!         println!("Parsed {} statements", file.statements.len());
//!     }
//!     Err(errors) => {
//!         for e in errors {
//!             eprintln!("Parse error: {}", e);
//!         }
//!     }
//! }
//! ```
//!
//! # Error Recovery
//!
//! The parser supports error recovery, allowing it to continue parsing after
//! encountering errors. Use `parse_with_recovery` to get both the AST and errors:
//!
//! ```
//! use ato_parser::parse_with_recovery;
//!
//! let source = "module M:\n    pass\n";
//! let (ast, errors) = parse_with_recovery(source);
//!
//! if let Some(file) = ast {
//!     println!("Parsed {} statements", file.statements.len());
//! }
//! for error in &errors {
//!     eprintln!("Error: {}", error);
//! }
//! ```

pub mod ast;
pub mod error;
mod chumsky;

pub use ast::*;
pub use error::{ParseError, ParseResult};
pub use chumsky::{format_errors, ParseError as ChumskyParseError};

/// Parse Ato source code into an AST.
///
/// This is the main entry point for parsing Ato source code.
/// Returns an error if parsing fails completely.
///
/// For error recovery (getting partial AST even with errors), use `parse_with_recovery`.
pub fn parse(source: &str) -> Result<File, Vec<ChumskyParseError>> {
    let (ast, errors) = chumsky::parse(source);

    if let Some(file) = ast {
        if errors.is_empty() {
            Ok(file)
        } else {
            // We got an AST but also had errors - return the errors
            // (the AST might be incomplete or have placeholder values)
            Err(errors)
        }
    } else {
        // No AST at all - definitely an error
        if errors.is_empty() {
            Err(vec![ChumskyParseError {
                span: ato_lexer::Span::new(0, 0, 1, 1),
                message: "failed to parse".to_string(),
                expected: vec![],
                found: None,
                help: None,
            }])
        } else {
            Err(errors)
        }
    }
}

/// Parse Ato source code with error recovery.
///
/// Returns both the AST (if any could be parsed) and all errors encountered.
/// This is useful when you want to continue processing even with errors,
/// or when you want to collect all errors for display.
pub fn parse_with_recovery(source: &str) -> (Option<File>, Vec<ChumskyParseError>) {
    chumsky::parse(source)
}

/// Parse Ato source code and format any errors using ariadne.
///
/// Returns the AST if parsing succeeded (possibly with recovered errors),
/// and a formatted error string if there were any errors.
pub fn parse_with_formatted_errors(source: &str, filename: &str) -> (Option<File>, Option<String>) {
    chumsky::parse_with_errors(source, filename)
}

/// Parse Ato source code, returning both the AST and a source code wrapper for error display.
///
/// This is useful when you want to display nice error messages with miette.
pub fn parse_with_source(source: &str) -> (Result<File, Vec<ChumskyParseError>>, miette::NamedSource<String>) {
    let named_source = miette::NamedSource::new("<input>", source.to_string());
    let result = parse(source);
    (result, named_source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let source = "";
        let file = parse(source).unwrap();
        assert!(file.statements.is_empty());
    }

    #[test]
    fn test_parse_pragma() {
        let source = "#pragma experiment(\"FOR_LOOP\")\n";
        let file = parse(source).unwrap();
        assert_eq!(file.statements.len(), 1);
        assert!(matches!(file.statements[0], Statement::Pragma(_)));
    }

    #[test]
    fn test_parse_import() {
        let source = "import ElectricPower\n";
        let file = parse(source).unwrap();
        assert_eq!(file.statements.len(), 1);
        assert!(matches!(file.statements[0], Statement::Import(_)));
    }

    #[test]
    fn test_parse_from_import() {
        let source = "from \"path/to/file.ato\" import Module\n";
        let file = parse(source).unwrap();
        assert_eq!(file.statements.len(), 1);
        if let Statement::Import(import) = &file.statements[0] {
            assert!(import.from_path.is_some());
        } else {
            panic!("Expected import statement");
        }
    }

    #[test]
    fn test_parse_simple_module() {
        let source = "module M:\n    pass\n";
        let file = parse(source).unwrap();
        assert_eq!(file.statements.len(), 1);
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert_eq!(block.kind, BlockKind::Module);
            assert_eq!(block.name.name, "M");
            assert_eq!(block.body.len(), 1);
        } else {
            panic!("Expected block definition");
        }
    }

    #[test]
    fn test_parse_module_with_super() {
        let source = "module Child from Parent:\n    pass\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(block.super_type.is_some());
            assert_eq!(block.super_type.as_ref().unwrap().parts[0].name, "Parent");
        } else {
            panic!("Expected block definition");
        }
    }

    #[test]
    fn test_parse_component() {
        let source = "component C:\n    pin p1\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert_eq!(block.kind, BlockKind::Component);
        } else {
            panic!("Expected component");
        }
    }

    #[test]
    fn test_parse_interface() {
        let source = "interface I:\n    pass\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert_eq!(block.kind, BlockKind::Interface);
        } else {
            panic!("Expected interface");
        }
    }

    #[test]
    fn test_parse_pin() {
        let source = "module M:\n    pin p1\n    pin 1\n    pin \"GND\"\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert_eq!(block.body.len(), 3);
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_signal() {
        let source = "module M:\n    signal sig\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::SignalDef(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_assignment() {
        let source = "module M:\n    x = 5\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::Assignment(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_new_expression() {
        let source = "module M:\n    x = new SomeType\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                assert!(matches!(assign.value, Assignable::New(_)));
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_new_with_count() {
        let source = "module M:\n    x = new SomeType[10]\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                if let Assignable::New(new_expr) = &assign.value {
                    assert!(new_expr.count.is_some());
                } else {
                    panic!("Expected new expression");
                }
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_new_with_template() {
        let source = "module M:\n    x = new SomeType<param=1>\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                if let Assignable::New(new_expr) = &assign.value {
                    assert!(new_expr.template.is_some());
                } else {
                    panic!("Expected new expression");
                }
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_connection() {
        let source = "module M:\n    a ~ b\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::Connection(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_directed_connection() {
        let source = "module M:\n    a ~> b ~> c\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::DirectedConnection(conn) = &block.body[0] {
                assert_eq!(conn.direction, ConnectionDirection::Forward);
                assert_eq!(conn.elements.len(), 3);
            } else {
                panic!("Expected directed connection");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_backward_connection() {
        let source = "module M:\n    a <~ b <~ c\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::DirectedConnection(conn) = &block.body[0] {
                assert_eq!(conn.direction, ConnectionDirection::Backward);
            } else {
                panic!("Expected directed connection");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_retype() {
        let source = "module M:\n    x -> NewType\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::Retype(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_assert() {
        let source = "module M:\n    assert x > 5\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::Assert(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_assert_within() {
        let source = "module M:\n    assert x within 1 to 10\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assert(assert_stmt) = &block.body[0] {
                assert_eq!(assert_stmt.comparison.operations[0].kind, CompareOpKind::Within);
            } else {
                panic!("Expected assert");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_trait() {
        let source = "module M:\n    trait some_trait\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::Trait(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_trait_with_constructor() {
        let source = "module M:\n    trait some_trait::constructor\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Trait(trait_stmt) = &block.body[0] {
                assert!(trait_stmt.constructor.is_some());
            } else {
                panic!("Expected trait");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_trait_with_template() {
        let source = "module M:\n    trait some_trait<arg=1>\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Trait(trait_stmt) = &block.body[0] {
                assert!(trait_stmt.template.is_some());
            } else {
                panic!("Expected trait");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_for_loop() {
        let source = "module M:\n    for item in container:\n        pass\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::For(for_stmt) = &block.body[0] {
                assert_eq!(for_stmt.variable.name, "item");
            } else {
                panic!("Expected for loop");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_declaration() {
        let source = "module M:\n    field: ohm\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::Declaration(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_declaration_with_assignment() {
        let source = "module M:\n    field: ohm = 100\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                assert!(matches!(assign.target, AssignTarget::Declaration(_)));
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_physical_quantity() {
        let source = "module M:\n    x = 10kohm\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                assert!(matches!(assign.value, Assignable::Physical(_)));
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_range() {
        let source = "module M:\n    x = 1 to 10\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                if let Assignable::Physical(PhysicalLiteral::Range(_)) = &assign.value {
                    // OK
                } else {
                    panic!("Expected range");
                }
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_bilateral() {
        let source = "module M:\n    x = 10 +/- 5%\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                if let Assignable::Physical(PhysicalLiteral::Bilateral(_)) = &assign.value {
                    // OK
                } else {
                    panic!("Expected bilateral");
                }
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_arithmetic() {
        let source = "module M:\n    x = a + b * c\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                assert!(matches!(assign.value, Assignable::Arithmetic(_)));
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_string_stmt() {
        let source = "module M:\n    \"docstring\"\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::StringStmt(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_semicolon_separated() {
        let source = "module M:\n    pass; pass; pass\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert_eq!(block.body.len(), 3);
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_single_line_block() {
        let source = "module M: pass\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert_eq!(block.body.len(), 1);
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_nested_modules() {
        let source = "module A:\n    module B:\n        pass\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(outer) = &file.statements[0] {
            if let Statement::BlockDef(inner) = &outer.body[0] {
                assert_eq!(inner.name.name, "B");
            } else {
                panic!("Expected inner block");
            }
        } else {
            panic!("Expected outer block");
        }
    }

    #[test]
    fn test_parse_field_reference() {
        let source = "module M:\n    a.b.c = 1\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                if let AssignTarget::FieldRef(field_ref) = &assign.target {
                    assert_eq!(field_ref.parts.len(), 3);
                } else {
                    panic!("Expected field ref");
                }
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_field_with_index() {
        let source = "module M:\n    a[0].b = 1\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                if let AssignTarget::FieldRef(field_ref) = &assign.target {
                    assert!(field_ref.parts[0].index.is_some());
                } else {
                    panic!("Expected field ref");
                }
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_assert_multiply() {
        let source = "module M:\n    assert x >= y * 1.5\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::Assert(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_assign_multiply() {
        // Basic: x = a * b
        let source = "module M:\n    x = 300 * y\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                assert!(matches!(assign.value, Assignable::Arithmetic(_)));
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_assign_chained_multiply() {
        // k_iset = 300 * k_i * k_r (from BQ25185)
        let source = "module M:\n    k_iset = 300 * k_i * k_r\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                assert!(matches!(assign.value, Assignable::Arithmetic(_)));
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_assert_multiply_rhs() {
        // assert x >= y * 1.5 (from BQ25185)
        let source = "module M:\n    assert x >= y * 1.5\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            assert!(matches!(block.body[0], Statement::Assert(_)));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_physical_literal_still_assignable() {
        // Plain physical literals should still be Assignable::Physical
        let source = "module M:\n    x = 10kohm +/- 10%\n";
        let file = parse(source).unwrap();
        if let Statement::BlockDef(block) = &file.statements[0] {
            if let Statement::Assignment(assign) = &block.body[0] {
                assert!(matches!(assign.value, Assignable::Physical(PhysicalLiteral::Bilateral(_))));
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_parse_from_py_import() {
        // BQ25185 has: from "ResistanceMapper.py" import ResistanceMapper
        // This should parse (even though .py files won't be loaded)
        let source = r#"from "ResistanceMapper.py" import ResistanceMapper
module M:
    pass
"#;
        let file = parse(source).unwrap();
        assert!(!file.statements.is_empty());
    }

    #[test]
    fn test_error_recovery() {
        // This has an error (missing colon after module name)
        let source = "module Bad\n    pass\n";
        let (ast, errors) = parse_with_recovery(source);

        // Should have errors
        assert!(!errors.is_empty(), "Should have parse errors");
        // With error recovery, we may or may not get an AST
        let _ = ast;
    }
}
