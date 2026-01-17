//! Parse command - parse and dump AST.
//!
//! This command parses a .ato file and outputs the AST in either
//! JSON or pretty-printed format. Useful for debugging.

use std::path::Path;

use crate::error::{CliError, CliResult, ParseErrorInfo, read_file};

/// Run the parse command.
pub fn run(path: &Path, format: &str) -> CliResult<()> {
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

    // Parse the file
    let ast = match ato_parser::parse(&source) {
        Ok(ast) => ast,
        Err(errors) => {
            let parse_errors: Vec<ParseErrorInfo> = errors
                .iter()
                .map(|e| ParseErrorInfo {
                    message: e.message.clone(),
                    span: Some((e.span.start, e.span.end - e.span.start)),
                    help: e.help.clone(),
                })
                .collect();
            return Err(CliError::parse(&file_name, parse_errors, source));
        }
    };

    // Output the AST
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&ast)
                .map_err(|e| CliError::io(format!("Failed to serialize AST: {}", e)))?;
            println!("{}", json);
        }
        "pretty" | _ => {
            println!("Parsed {} successfully!", file_name);
            println!();
            println!("File contains {} top-level statements:", ast.statements.len());
            for (i, stmt) in ast.statements.iter().enumerate() {
                println!("  {}. {:?}", i + 1, stmt_summary(stmt));
            }
        }
    }

    Ok(())
}

/// Get a summary string for a statement.
fn stmt_summary(stmt: &ato_parser::Statement) -> String {
    use ato_parser::Statement;

    match stmt {
        Statement::BlockDef(block) => {
            format!("{:?} '{}'", block.kind, block.name.name)
        }
        Statement::Import(import) => {
            let names: Vec<_> = import.imports.iter()
                .map(|t| t.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join("."))
                .collect();
            format!("import {}", names.join(", "))
        }
        Statement::DepImport(import) => {
            let name = import.type_ref.parts.iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            format!("import {} from \"...\"", name)
        }
        Statement::Assignment(assign) => {
            format!("assignment")
        }
        Statement::Connection(conn) => {
            "connection (~)".to_string()
        }
        Statement::DirectedConnection(conn) => {
            "directed connection (~>)".to_string()
        }
        Statement::PinDeclaration(_) => "pin declaration".to_string(),
        Statement::SignalDef(_) => "signal definition".to_string(),
        Statement::Declaration(_) => "declaration".to_string(),
        Statement::Assert(_) => "assertion".to_string(),
        Statement::For(_) => "for loop".to_string(),
        Statement::Trait(_) => "trait".to_string(),
        Statement::Retype(_) => "retype".to_string(),
        Statement::CumAssignment(_) => "cumulative assignment".to_string(),
        Statement::SetAssignment(_) => "set assignment".to_string(),
        Statement::StringStmt(_) => "string statement".to_string(),
        Statement::Pass(_) => "pass".to_string(),
        Statement::Pragma(_) => "pragma".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_valid_file() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pass").unwrap();

        let result = run(file.path(), "pretty");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_extension() {
        let file = NamedTempFile::with_suffix(".txt").unwrap();
        let result = run(file.path(), "pretty");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_json_output() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pass").unwrap();

        let result = run(file.path(), "json");
        assert!(result.is_ok());
    }
}
