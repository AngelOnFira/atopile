//! Python bindings for Ato Rust crates.
//!
//! This crate provides PyO3-based Python bindings for the Ato lexer, parser,
//! domain types, and constraint solver.

use pyo3::prelude::*;

mod lexer;
mod domain;
mod solver;

#[cfg(feature = "parser")]
mod parser;

/// Tokenize Ato source code.
///
/// Args:
///     source: The Ato source code to tokenize.
///
/// Returns:
///     A list of token dictionaries, each containing:
///     - kind: The token type (e.g., "Module", "Name", "Colon")
///     - text: The token text
///     - line: Line number (1-indexed)
///     - column: Column number (1-indexed)
///     - start: Start byte offset
///     - end: End byte offset
///
/// Raises:
///     ValueError: If lexing produces errors.
#[pyfunction]
fn tokenize(source: &str) -> PyResult<Vec<lexer::PyToken>> {
    lexer::tokenize(source)
}

/// Parse Ato source code into an AST.
///
/// Args:
///     source: The Ato source code to parse.
///
/// Returns:
///     A dictionary representing the AST.
///
/// Raises:
///     ValueError: If parsing fails.
#[cfg(feature = "parser")]
#[pyfunction]
fn parse(source: &str) -> PyResult<PyObject> {
    parser::parse_source(source)
}

/// A Python module providing Ato language tools.
#[pymodule]
fn ato_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(tokenize, m)?)?;

    #[cfg(feature = "parser")]
    m.add_function(wrap_pyfunction!(parse, m)?)?;

    // Add domain types
    m.add_class::<domain::PyQuantity>()?;
    m.add_class::<domain::PyInterval>()?;
    m.add_class::<domain::PyQuantityInterval>()?;
    m.add_class::<domain::PyUnit>()?;

    // Add solver types
    m.add_class::<solver::PySolver>()?;
    m.add_class::<solver::PySolverConfig>()?;
    m.add_class::<solver::PyExpression>()?;
    m.add_class::<solver::PyPredicate>()?;

    Ok(())
}
