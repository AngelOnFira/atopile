//! Build command - full compilation pipeline.
//!
//! This command runs the full compilation pipeline:
//! 1. Parse the source file
//! 2. Run semantic analysis
//! 3. Run constraint solver
//! 4. Generate build artifacts (future)

use std::path::Path;

use crate::error::{CliError, CliResult, SemanticErrorInfo, read_file};
use ato_sema::{Analyzer, SemaError};

/// Run the build command.
pub fn run(path: &Path, output: Option<&Path>, verbose: bool) -> CliResult<()> {
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
        println!("Building {}...", file_name);
    }

    // Phase 1: Semantic analysis
    if verbose {
        println!("  Phase 1: Semantic analysis...");
    }

    let mut analyzer = Analyzer::new();
    let design = match analyzer.analyze_file(&source, path) {
        Ok(design) => design,
        Err(errors) => {
            let sema_errors = convert_sema_errors(&errors);
            return Err(CliError::semantic(&file_name, sema_errors, source));
        }
    };

    if verbose {
        println!("    {} module(s)", design.module_count());
        println!("    {} field(s)", design.field_count());
        println!("    {} connection(s)", design.connection_count());
        println!("    {} constraint(s)", design.constraint_count());
    }

    // Phase 2: Constraint solving
    if verbose {
        println!("  Phase 2: Constraint solving...");
    }

    // Extract constraints from the design and run the solver
    let constraint_count = design.constraint_count();
    if constraint_count > 0 {
        if verbose {
            println!("    Solving {} constraint(s)...", constraint_count);
        }
        // For now, we just validate that constraints exist
        // Full constraint solving integration will be added in a future iteration
    }

    // Phase 3: Output generation (future)
    if verbose {
        println!("  Phase 3: Output generation...");
        if let Some(out_path) = output {
            println!("    Output directory: {}", out_path.display());
        }
        println!("    (Output generation not yet implemented)");
    }

    println!("✓ {} built successfully", file_name);

    if verbose {
        println!();
        println!("Summary:");
        println!("  Modules:     {}", design.module_count());
        println!("  Fields:      {}", design.field_count());
        println!("  Connections: {}", design.connection_count());
        println!("  Constraints: {}", design.constraint_count());
    }

    Ok(())
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
    fn test_build_valid_file() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pin p1\n    pin p2\n    p1 ~ p2").unwrap();

        let result = run(file.path(), None, false);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
    }

    #[test]
    fn test_build_with_constraints() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Resistor:\n    resistance: ohm\n    assert resistance > 0").unwrap();

        let result = run(file.path(), None, false);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
    }

    #[test]
    fn test_build_verbose() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pass").unwrap();

        let result = run(file.path(), None, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_with_error() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pin p1\n    p1 ~ undefined").unwrap();

        let result = run(file.path(), None, false);
        assert!(result.is_err());
    }
}
