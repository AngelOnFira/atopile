//! Check command - semantic analysis without full build.
//!
//! This command parses a .ato file and runs semantic analysis to
//! catch errors without generating build artifacts.

use std::path::Path;

use crate::error::{CliError, CliResult, SemanticErrorInfo, read_file};
use ato_sema::{Analyzer, SemaError};

/// Run the check command.
pub fn run(path: &Path, verbose: bool) -> CliResult<()> {
    // Check file extension
    if path.extension().map_or(true, |ext| ext != "ato") {
        return Err(CliError::io(format!(
            "Expected .ato file, got '{}'",
            path.display()
        )));
    }

    // Read the file
    let source = read_file(path)?;
    let file_name = path.display().to_string();

    if verbose {
        println!("Checking {}...", file_name);
    }

    // Run semantic analysis
    let mut analyzer = Analyzer::new();
    match analyzer.analyze_file(&source, path) {
        Ok(design) => {
            if verbose {
                println!("  {} module(s)", design.module_count());
                println!("  {} field(s)", design.field_count());
                println!("  {} connection(s)", design.connection_count());
                println!("  {} constraint(s)", design.constraint_count());
            }
            println!("✓ {} checked successfully", file_name);
            Ok(())
        }
        Err(errors) => {
            let sema_errors = convert_sema_errors(&errors);
            Err(CliError::semantic(&file_name, sema_errors, source))
        }
    }
}

/// Convert semantic errors to CLI error info.
fn convert_sema_errors(errors: &[SemaError]) -> Vec<SemanticErrorInfo> {
    errors.iter().map(|e| {
        let (message, span, help) = match e {
            SemaError::UndefinedName { name, span } => {
                (format!("undefined name '{}'", name), span_to_tuple(span), Some(format!("Did you forget to define '{}'?", name)))
            }
            SemaError::DuplicateDefinition { name, span, .. } => {
                (format!("duplicate definition of '{}'", name), span_to_tuple(span), Some("Names must be unique within a scope".into()))
            }
            SemaError::TypeMismatch { left, right, span } => {
                (format!("cannot connect '{}' to '{}': incompatible types", left, right), span_to_tuple(span), None)
            }
            SemaError::NotConnectable { name, span } => {
                (format!("'{}' is not connectable", name), span_to_tuple(span), Some("Only pins, signals, and instances can be connected".into()))
            }
            SemaError::NotIterable { name, span } => {
                (format!("'{}' is not iterable", name), span_to_tuple(span), Some("For loops require an array instance".into()))
            }
            SemaError::IndexOutOfBounds { index, size, span } => {
                (format!("index {} out of bounds for array of size {}", index, size), span_to_tuple(span), None)
            }
            SemaError::BaseTypeNotFound { name, span } => {
                (format!("base type '{}' not found", name), span_to_tuple(span), None)
            }
            SemaError::CyclicInheritance { chain, span } => {
                (format!("cyclic inheritance detected: {}", chain), span_to_tuple(span), None)
            }
            SemaError::InvalidFieldAccess { field, parent, span } => {
                (format!("'{}' is not a field of '{}'", field, parent), span_to_tuple(span), None)
            }
            SemaError::UnresolvedImport { name, span } => {
                (format!("cannot resolve import '{}'", name), span_to_tuple(span), None)
            }
            SemaError::FileNotFound { path, span } => {
                (format!("file not found: '{}'", path), span_to_tuple(span), None)
            }
            SemaError::ParseError { file, message } => {
                (format!("parse error in '{}': {}", file, message), None, None)
            }
            SemaError::IoError { message } => {
                (message.clone(), None, None)
            }
        };
        SemanticErrorInfo { message, span, help }
    }).collect()
}

/// Convert an optional Span to a tuple.
fn span_to_tuple(span: &Option<ato_lexer::Span>) -> Option<(usize, usize)> {
    span.map(|s| (s.start, s.end - s.start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_check_valid_file() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pin p1\n    pin p2\n    p1 ~ p2").unwrap();

        let result = run(file.path(), false);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
    }

    #[test]
    fn test_check_empty_file() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "").unwrap();

        let result = run(file.path(), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_undefined_name() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pin p1\n    p1 ~ undefined").unwrap();

        let result = run(file.path(), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_verbose() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pass").unwrap();

        let result = run(file.path(), true);
        assert!(result.is_ok());
    }
}
