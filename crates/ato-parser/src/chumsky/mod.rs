//! Chumsky-based parser for the Ato language with error recovery.
//!
//! This module provides an alternative parser implementation using the chumsky
//! parser combinator library. It offers better error recovery and more composable
//! parser definitions compared to the hand-written recursive descent parser.

mod error;
mod expr;
mod stmt;

use crate::ast::*;
use ato_lexer::{Lexer, Token};
use chumsky::prelude::*;

pub use error::{format_errors, ParseError};

/// Parse Ato source code into an AST using the chumsky parser.
///
/// This parser provides error recovery, allowing it to continue parsing
/// after encountering errors and report multiple issues at once.
pub fn parse(source: &str) -> (Option<File>, Vec<ParseError>) {
    let lexer = Lexer::new(source);
    let tokens: Vec<Token> = lexer.collect();

    let len = source.len();
    let (ast, errors) = stmt::file_parser().parse_recovery(chumsky::Stream::from_iter(
        len..len,
        tokens.into_iter().map(|t| (t.clone(), t.span.start..t.span.end)),
    ));

    let parse_errors: Vec<ParseError> = errors
        .into_iter()
        .map(|e| ParseError::from_simple(e, source))
        .collect();

    (ast, parse_errors)
}

/// Parse Ato source and format any errors using ariadne.
///
/// Returns the AST if parsing succeeded (possibly with recovered errors),
/// and a formatted error string if there were any errors.
pub fn parse_with_errors(source: &str, filename: &str) -> (Option<File>, Option<String>) {
    let (ast, errors) = parse(source);

    let error_string = if errors.is_empty() {
        None
    } else {
        Some(format_errors(&errors, source, filename))
    };

    (ast, error_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_module() {
        let source = "module M:\n    pass\n";
        let (ast, errors) = parse(source);

        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(ast.is_some());

        let file = ast.unwrap();
        assert_eq!(file.statements.len(), 1);

        if let Statement::BlockDef(block) = &file.statements[0] {
            assert_eq!(block.kind, BlockKind::Module);
            assert_eq!(block.name.name, "M");
        } else {
            panic!("Expected block definition");
        }
    }

    #[test]
    fn test_parse_import() {
        let source = "import ElectricPower\n";
        let (ast, errors) = parse(source);

        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(ast.is_some());
    }

    #[test]
    fn test_parse_from_import() {
        let source = "from \"path/to/file.ato\" import Module\n";
        let (ast, errors) = parse(source);

        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(ast.is_some());
    }

    #[test]
    fn test_error_recovery() {
        // This has an error (missing colon after module name)
        let source = "module Bad\n    pass\n";
        let (ast, errors) = parse(source);

        // Should have errors
        assert!(!errors.is_empty(), "Should have parse errors");
        // With parse_recovery(), we may or may not get an AST depending on how much was parseable
        // The important thing is that errors are collected
        let _ = ast; // AST may be None or Some with partial content
    }

    #[test]
    fn test_parse_connection() {
        let source = "module M:\n    a ~ b\n";
        let (ast, errors) = parse(source);

        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(ast.is_some());
    }

    #[test]
    fn test_parse_assignment() {
        let source = "module M:\n    x = 5\n";
        let (ast, errors) = parse(source);

        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(ast.is_some());
    }

    #[test]
    fn test_parse_physical_quantity() {
        let source = "module M:\n    x = 10kohm +/- 5%\n";
        let (ast, errors) = parse(source);

        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(ast.is_some());
    }
}
