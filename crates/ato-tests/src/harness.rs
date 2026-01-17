//! Test harness utilities for the Atopile compiler.

use std::path::Path;

/// Result of parsing a file.
#[derive(Debug)]
pub enum ParseResult {
    /// Parsing succeeded with an AST.
    Ok(ato_parser::File),
    /// Parsing failed with errors.
    Err(Vec<ato_parser::ChumskyParseError>),
}

/// Result of semantic analysis.
#[derive(Debug)]
pub enum SemaResult {
    /// Analysis succeeded with a design.
    Ok(ato_sema::Design),
    /// Analysis failed with errors.
    Err(Vec<ato_sema::SemaError>),
}

/// Parse an .ato file and return the result.
pub fn parse_file(source: &str) -> ParseResult {
    match ato_parser::parse(source) {
        Ok(ast) => ParseResult::Ok(ast),
        Err(errors) => ParseResult::Err(errors),
    }
}

/// Run semantic analysis on an .ato file.
pub fn analyze_file(source: &str, path: &Path) -> SemaResult {
    let mut analyzer = ato_sema::Analyzer::new();
    match analyzer.analyze_file(source, path) {
        Ok(design) => SemaResult::Ok(design),
        Err(errors) => SemaResult::Err(errors),
    }
}

/// Get a simplified string representation of parse errors.
pub fn format_parse_errors(errors: &[ato_parser::ChumskyParseError]) -> Vec<String> {
    errors.iter().map(|e| e.message.clone()).collect()
}

/// Get a simplified string representation of semantic errors.
pub fn format_sema_errors(errors: &[ato_sema::SemaError]) -> Vec<String> {
    errors.iter().map(|e| format!("{:?}", e)).collect()
}

/// Check if parsing succeeds.
pub fn parsing_succeeds(source: &str) -> bool {
    matches!(parse_file(source), ParseResult::Ok(_))
}

/// Check if semantic analysis succeeds.
pub fn sema_succeeds(source: &str) -> bool {
    let path = Path::new("test.ato");
    matches!(analyze_file(source, path), SemaResult::Ok(_))
}

/// Get the AST structure as a summary string.
pub fn ast_summary(ast: &ato_parser::File) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Statements: {}", ast.statements.len()));
    for (i, stmt) in ast.statements.iter().enumerate() {
        lines.push(format!("  {}: {:?}", i + 1, stmt_type(stmt)));
    }
    lines.join("\n")
}

fn stmt_type(stmt: &ato_parser::Statement) -> &'static str {
    use ato_parser::Statement;
    match stmt {
        Statement::BlockDef(_) => "BlockDef",
        Statement::Import(_) => "Import",
        Statement::DepImport(_) => "DepImport",
        Statement::Assignment(_) => "Assignment",
        Statement::Connection(_) => "Connection",
        Statement::DirectedConnection(_) => "DirectedConnection",
        Statement::PinDeclaration(_) => "PinDeclaration",
        Statement::SignalDef(_) => "SignalDef",
        Statement::Declaration(_) => "Declaration",
        Statement::Assert(_) => "Assert",
        Statement::For(_) => "For",
        Statement::Trait(_) => "Trait",
        Statement::Retype(_) => "Retype",
        Statement::CumAssignment(_) => "CumAssignment",
        Statement::SetAssignment(_) => "SetAssignment",
        Statement::StringStmt(_) => "StringStmt",
        Statement::Pass(_) => "Pass",
        Statement::Pragma(_) => "Pragma",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_module() {
        let source = "module Test:\n    pass";
        assert!(parsing_succeeds(source));
    }

    #[test]
    fn test_parse_empty_file() {
        let source = "";
        assert!(parsing_succeeds(source));
    }

    #[test]
    fn test_sema_simple_module() {
        let source = "module Test:\n    pin p1\n    pin p2\n    p1 ~ p2";
        assert!(sema_succeeds(source));
    }
}
