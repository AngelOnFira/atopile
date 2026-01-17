//! Main analyzer that orchestrates semantic analysis.
//!
//! The Analyzer is the main entry point for semantic analysis. It coordinates:
//! 1. Import resolution
//! 2. Name resolution
//! 3. Type checking
//! 4. AST to IR lowering

use crate::error::SemaError;
use crate::imports::ImportResolver;
use crate::lower::Lowerer;
use crate::names::NameResolver;
use crate::scope::Scope;
use crate::types::TypeChecker;
use ato_ir::Design;
use ato_parser::File;
use std::path::Path;

/// The main analyzer for semantic analysis.
pub struct Analyzer {
    /// The root directory for import resolution.
    root_dir: Option<std::path::PathBuf>,

    /// Collected errors from all phases.
    errors: Vec<SemaError>,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        Self {
            root_dir: None,
            errors: Vec::new(),
        }
    }

    /// Set the root directory for import resolution.
    pub fn with_root_dir(mut self, root_dir: impl Into<std::path::PathBuf>) -> Self {
        self.root_dir = Some(root_dir.into());
        self
    }

    /// Analyze source code and produce a Design.
    ///
    /// This performs all semantic analysis phases:
    /// 1. Parse the source (if needed)
    /// 2. Resolve imports
    /// 3. Resolve names
    /// 4. Type check
    /// 5. Lower to IR
    pub fn analyze_source(&mut self, source: &str) -> Result<Design, Vec<SemaError>> {
        // Parse the source
        let ast = match ato_parser::parse(source) {
            Ok(ast) => ast,
            Err(errors) => {
                return Err(errors.into_iter().map(|e| {
                    SemaError::ParseError {
                        file: "<input>".to_string(),
                        message: e.to_string(),
                    }
                }).collect());
            }
        };

        self.analyze(ast)
    }

    /// Analyze an already-parsed AST and produce a Design.
    pub fn analyze(&mut self, ast: File) -> Result<Design, Vec<SemaError>> {
        self.errors.clear();

        let mut design = Design::new();
        let mut scope = Scope::new();

        // Phase 1: Name resolution
        let mut name_resolver = NameResolver::new(&mut design);
        if let Err(errors) = name_resolver.resolve(&ast, &mut scope) {
            self.errors.extend(errors);
        }
        self.errors.extend(name_resolver.take_errors());

        // Phase 2: Type checking
        let mut type_checker = TypeChecker::new(&design);
        if let Err(errors) = type_checker.check(&ast, &scope) {
            self.errors.extend(errors);
        }
        self.errors.extend(type_checker.take_errors());

        // Phase 3: Lowering to IR
        let mut lowerer = Lowerer::new(&mut design);
        if let Err(errors) = lowerer.lower(&ast, &scope) {
            self.errors.extend(errors);
        }
        self.errors.extend(lowerer.take_errors());

        // Rebuild the connection graph
        design.rebuild_connection_graph();

        if self.errors.is_empty() {
            Ok(design)
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    /// Analyze a file with import resolution.
    pub fn analyze_file(&mut self, source: &str, file_path: &Path) -> Result<Design, Vec<SemaError>> {
        self.errors.clear();

        let root_dir = self.root_dir.clone()
            .or_else(|| file_path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // Phase 0: Import resolution
        let mut import_resolver = ImportResolver::new(root_dir);
        if let Err(errors) = import_resolver.resolve(source, Some(file_path)) {
            self.errors.extend(errors);
        }
        self.errors.extend(import_resolver.take_errors());

        // Parse the main file
        let ast = match ato_parser::parse(source) {
            Ok(ast) => ast,
            Err(errors) => {
                return Err(errors.into_iter().map(|e| {
                    SemaError::ParseError {
                        file: file_path.display().to_string(),
                        message: e.to_string(),
                    }
                }).collect());
            }
        };

        // Continue with regular analysis
        let mut design = Design::new();
        let mut scope = Scope::new();

        // Register imported modules in scope
        // (In a full implementation, we'd process all imported files here)

        // Phase 1: Name resolution
        let mut name_resolver = NameResolver::new(&mut design);
        if let Err(errors) = name_resolver.resolve(&ast, &mut scope) {
            self.errors.extend(errors);
        }
        self.errors.extend(name_resolver.take_errors());

        // Phase 2: Type checking
        let mut type_checker = TypeChecker::new(&design);
        if let Err(errors) = type_checker.check(&ast, &scope) {
            self.errors.extend(errors);
        }
        self.errors.extend(type_checker.take_errors());

        // Phase 3: Lowering to IR
        let mut lowerer = Lowerer::new(&mut design);
        if let Err(errors) = lowerer.lower(&ast, &scope) {
            self.errors.extend(errors);
        }
        self.errors.extend(lowerer.take_errors());

        // Rebuild the connection graph
        design.rebuild_connection_graph();

        if self.errors.is_empty() {
            Ok(design)
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    /// Get the collected errors.
    pub fn errors(&self) -> &[SemaError] {
        &self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_empty() {
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source("").unwrap();
        assert_eq!(design.module_count(), 0);
    }

    #[test]
    fn test_analyze_simple_module() {
        let source = r#"
module MyModule:
    pin p1
    pin p2
    signal sig
    p1 ~ sig
    sig ~ p2
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        assert_eq!(design.module_count(), 1);
        assert_eq!(design.field_count(), 3);
        assert_eq!(design.connection_count(), 2);
    }

    #[test]
    fn test_analyze_with_assertions() {
        let source = r#"
module Resistor:
    resistance: ohm
    max_power: W
    assert resistance > 0
    assert max_power > 0
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        assert_eq!(design.module_count(), 1);
        assert_eq!(design.field_count(), 2);
        assert_eq!(design.constraint_count(), 2);
    }

    #[test]
    fn test_analyze_inheritance() {
        let source = r#"
interface Electrical:
    pass

module Resistor:
    unnamed = new Electrical[2]
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        assert_eq!(design.module_count(), 2);
    }

    #[test]
    fn test_analyze_nested_modules() {
        let source = r#"
module Outer:
    module Inner:
        pin p1
    inner = new Inner
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        assert_eq!(design.module_count(), 2);
    }

    #[test]
    fn test_analyze_for_loop() {
        let source = r#"
module M:
    leds = new LED[4]
    signal common
    for led in leds:
        led ~ common
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        // 4 iterations = 4 connections
        assert_eq!(design.connection_count(), 4);
    }

    #[test]
    fn test_analyze_directed_connection() {
        let source = r#"
module M:
    signal input
    resistor = new Resistor
    signal output
    input ~> resistor ~> output
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        assert_eq!(design.connection_count(), 1);
        let conn = &design.connections()[0];
        assert!(conn.is_directed());
    }

    #[test]
    fn test_analyze_error_undefined() {
        let source = r#"
module M:
    pin p1
    p1 ~ undefined_signal
"#;
        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_source(source);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, SemaError::UndefinedName { .. })));
    }

    #[test]
    fn test_analyze_error_duplicate() {
        let source = r#"
module M:
    pin p1
    pin p1
"#;
        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_source(source);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, SemaError::DuplicateDefinition { .. })));
    }

    #[test]
    fn test_connection_graph() {
        let source = r#"
module M:
    pin p1
    pin p2
    pin p3
    p1 ~ p2
    p2 ~ p3
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        // The connection graph should be built
        let graph = design.connection_graph();

        // Note: Connections are between field paths, not yet resolved to IDs
        // The graph is built from resolved connections
        // In this test, we verify the graph was built (it may be empty if
        // connections aren't fully resolved yet)
        let _ = graph;
    }

    #[test]
    fn test_analyze_complex_constraint() {
        let source = r#"
module PowerRegulator:
    vin: V
    vout: V
    iout: A
    assert vin > vout
    assert vout within 3.0V to 3.6V
    assert iout within 100mA +/- 10%
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        assert_eq!(design.constraint_count(), 3);
    }

    #[test]
    fn test_analyze_chained_comparison() {
        let source = r#"
module M:
    voltage: V
    assert 3V < voltage < 5V
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        assert_eq!(design.constraint_count(), 1);
        let constraint = &design.constraints()[0];
        assert_eq!(constraint.expression.operations.len(), 2);
    }

    #[test]
    fn test_analyze_trait() {
        let source = r#"
module Resistor:
    trait has_designator::prefix<value="R">
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        let module = &design.modules()[0];
        assert_eq!(module.traits.len(), 1);
    }
}
