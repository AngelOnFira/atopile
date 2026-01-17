//! Error handling for the CLI.
//!
//! This module provides error types and utilities for reporting errors
//! to the user with nice formatting using miette.

use miette::{Diagnostic, NamedSource, Report, SourceSpan};
use std::path::Path;
use thiserror::Error;

/// Result type for CLI operations.
pub type CliResult<T> = Result<T, CliError>;

/// CLI error type that wraps all possible errors.
#[derive(Error, Debug)]
pub enum CliError {
    /// IO error (file not found, permission denied, etc.).
    #[error("IO error: {message}")]
    Io {
        message: String,
        #[source]
        source: Option<std::io::Error>,
    },

    /// Parse error with source information.
    #[error("Parse error in {file}")]
    Parse {
        file: String,
        errors: Vec<ParseErrorInfo>,
        source_code: String,
    },

    /// Semantic analysis error.
    #[error("Semantic error in {file}")]
    Semantic {
        file: String,
        errors: Vec<SemanticErrorInfo>,
        source_code: String,
    },

    /// Constraint solver error.
    #[error("Solver error in {file}")]
    Solver {
        file: String,
        message: String,
        span: Option<(usize, usize)>,
        source_code: String,
    },

    /// Multiple files had errors.
    #[error("Build failed with {count} error(s)")]
    MultipleErrors { count: usize },
}

/// Information about a parse error.
#[derive(Debug, Clone)]
pub struct ParseErrorInfo {
    pub message: String,
    pub span: Option<(usize, usize)>,
    pub help: Option<String>,
}

/// Information about a semantic error.
#[derive(Debug, Clone)]
pub struct SemanticErrorInfo {
    pub message: String,
    pub span: Option<(usize, usize)>,
    pub help: Option<String>,
}

impl CliError {
    /// Create an IO error.
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
            source: None,
        }
    }

    /// Create an IO error with source.
    pub fn io_with_source(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            message: message.into(),
            source: Some(source),
        }
    }

    /// Create a parse error.
    pub fn parse(file: impl Into<String>, errors: Vec<ParseErrorInfo>, source_code: String) -> Self {
        Self::Parse {
            file: file.into(),
            errors,
            source_code,
        }
    }

    /// Create a semantic error.
    pub fn semantic(file: impl Into<String>, errors: Vec<SemanticErrorInfo>, source_code: String) -> Self {
        Self::Semantic {
            file: file.into(),
            errors,
            source_code,
        }
    }

    /// Create a solver error.
    pub fn solver(file: impl Into<String>, message: impl Into<String>, span: Option<(usize, usize)>, source_code: String) -> Self {
        Self::Solver {
            file: file.into(),
            message: message.into(),
            span,
            source_code,
        }
    }

    /// Report the error to stderr with nice formatting.
    pub fn report(&self) {
        match self {
            CliError::Io { message, source } => {
                eprintln!("Error: {}", message);
                if let Some(src) = source {
                    eprintln!("  Caused by: {}", src);
                }
            }
            CliError::Parse { file, errors, source_code } => {
                for err in errors {
                    let report = create_parse_report(file, err, source_code);
                    eprintln!("{:?}", report);
                }
            }
            CliError::Semantic { file, errors, source_code } => {
                for err in errors {
                    let report = create_semantic_report(file, err, source_code);
                    eprintln!("{:?}", report);
                }
            }
            CliError::Solver { file, message, span, source_code } => {
                let report = create_solver_report(file, message, *span, source_code);
                eprintln!("{:?}", report);
            }
            CliError::MultipleErrors { count } => {
                eprintln!("Error: Build failed with {} error(s)", count);
            }
        }
    }
}

/// A diagnostic error for parse failures.
#[derive(Error, Debug, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(ato::parse))]
struct ParseDiagnostic {
    message: String,

    #[source_code]
    src: NamedSource<String>,

    #[label("here")]
    span: Option<SourceSpan>,

    #[help]
    help: Option<String>,
}

/// A diagnostic error for semantic failures.
#[derive(Error, Debug, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(ato::semantic))]
struct SemanticDiagnostic {
    message: String,

    #[source_code]
    src: NamedSource<String>,

    #[label("here")]
    span: Option<SourceSpan>,

    #[help]
    help: Option<String>,
}

/// A diagnostic error for solver failures.
#[derive(Error, Debug, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(ato::solver))]
struct SolverDiagnostic {
    message: String,

    #[source_code]
    src: NamedSource<String>,

    #[label("constraint")]
    span: Option<SourceSpan>,

    #[help]
    help: Option<String>,
}

fn create_parse_report(file: &str, err: &ParseErrorInfo, source_code: &str) -> Report {
    let span = err.span.map(|(start, len)| SourceSpan::from((start, len)));

    ParseDiagnostic {
        message: err.message.clone(),
        src: NamedSource::new(file, source_code.to_string()),
        span,
        help: err.help.clone(),
    }
    .into()
}

fn create_semantic_report(file: &str, err: &SemanticErrorInfo, source_code: &str) -> Report {
    let span = err.span.map(|(start, len)| SourceSpan::from((start, len)));

    SemanticDiagnostic {
        message: err.message.clone(),
        src: NamedSource::new(file, source_code.to_string()),
        span,
        help: err.help.clone(),
    }
    .into()
}

fn create_solver_report(file: &str, message: &str, span: Option<(usize, usize)>, source_code: &str) -> Report {
    let span = span.map(|(start, len)| SourceSpan::from((start, len)));

    SolverDiagnostic {
        message: message.to_string(),
        src: NamedSource::new(file, source_code.to_string()),
        span,
        help: Some("Check that your constraints are not contradictory".into()),
    }
    .into()
}

/// Read a file and return its contents, or a CLI error.
pub fn read_file(path: &Path) -> CliResult<String> {
    std::fs::read_to_string(path).map_err(|e| {
        CliError::io_with_source(format!("Failed to read '{}'", path.display()), e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error() {
        let err = CliError::io("test error");
        assert!(matches!(err, CliError::Io { .. }));
    }

    #[test]
    fn test_parse_error() {
        let err = CliError::parse(
            "test.ato",
            vec![ParseErrorInfo {
                message: "unexpected token".into(),
                span: Some((0, 5)),
                help: None,
            }],
            "hello".into(),
        );
        assert!(matches!(err, CliError::Parse { .. }));
    }
}
