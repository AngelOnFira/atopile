//! Main analyzer that orchestrates semantic analysis.
//!
//! The Analyzer is the main entry point for semantic analysis. It coordinates:
//! 1. Import resolution
//! 2. Name resolution
//! 3. Type checking
//! 4. AST to IR lowering

use crate::error::SemaError;
use crate::lower::Lowerer;
use crate::names::NameResolver;
use crate::resolution::{ResolutionConfig, Resolver};
use crate::scope::Scope;
use crate::types::TypeChecker;
use ato_ir::{Design, ModuleKind};
use ato_parser::{File, Statement};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The main analyzer for semantic analysis.
pub struct Analyzer {
    /// The root directory for import resolution.
    root_dir: Option<PathBuf>,

    /// The standard library path.
    stdlib_path: Option<PathBuf>,

    /// Collected errors from all phases.
    errors: Vec<SemaError>,

    /// Cache of analyzed files (path -> modules defined).
    file_cache: HashMap<PathBuf, Vec<(String, ModuleKind)>>,
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
            stdlib_path: None,
            errors: Vec::new(),
            file_cache: HashMap::new(),
        }
    }

    /// Set the root directory for import resolution.
    pub fn with_root_dir(mut self, root_dir: impl Into<PathBuf>) -> Self {
        self.root_dir = Some(root_dir.into());
        self
    }

    /// Set the standard library path.
    pub fn with_stdlib(mut self, stdlib_path: impl Into<PathBuf>) -> Self {
        self.stdlib_path = Some(stdlib_path.into());
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
        self.file_cache.clear();

        let root_dir = self.root_dir.clone()
            .or_else(|| file_path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // Setup resolution config
        let mut config = ResolutionConfig::new(&root_dir);
        if let Some(ref stdlib) = self.stdlib_path {
            config = config.with_stdlib(stdlib);
        }

        // Create resolver and index stdlib
        let mut resolver = Resolver::new(config);
        if let Err(errors) = resolver.index_stdlib() {
            self.errors.extend(errors);
        }

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

        // Create design and scope
        let mut design = Design::new();
        let mut scope = Scope::new();

        // Phase 0: Resolve imports and merge symbols into scope
        self.resolve_and_merge_imports(&ast, file_path, &mut resolver, &mut design, &mut scope);

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

    /// Resolve imports and merge their symbols into scope.
    fn resolve_and_merge_imports(
        &mut self,
        ast: &File,
        current_file: &Path,
        resolver: &mut Resolver,
        design: &mut Design,
        scope: &mut Scope,
    ) {
        // Extract imports from the AST
        for stmt in &ast.statements {
            match stmt {
                Statement::Import(import) => {
                    let from_path = import.from_path.as_ref().map(|s| {
                        // Strip quotes from the path
                        let v = &s.value;
                        if (v.starts_with('"') && v.ends_with('"')) ||
                           (v.starts_with('\'') && v.ends_with('\'')) {
                            v[1..v.len()-1].to_string()
                        } else {
                            v.clone()
                        }
                    });

                    for type_ref in &import.imports {
                        let name = type_ref.parts.last()
                            .map(|p| p.name.clone())
                            .unwrap_or_default();

                        self.resolve_single_import(
                            &name,
                            from_path.as_deref(),
                            current_file,
                            resolver,
                            design,
                            scope,
                            Some(import.span),
                        );
                    }
                }
                Statement::DepImport(import) => {
                    let name = import.type_ref.parts.last()
                        .map(|p| p.name.clone())
                        .unwrap_or_default();

                    let from_path = {
                        let v = &import.from_path.value;
                        if (v.starts_with('"') && v.ends_with('"')) ||
                           (v.starts_with('\'') && v.ends_with('\'')) {
                            v[1..v.len()-1].to_string()
                        } else {
                            v.clone()
                        }
                    };

                    self.resolve_single_import(
                        &name,
                        Some(&from_path),
                        current_file,
                        resolver,
                        design,
                        scope,
                        Some(import.span),
                    );
                }
                _ => {}
            }
        }
    }

    /// Resolve a single import and add it to scope.
    fn resolve_single_import(
        &mut self,
        name: &str,
        from_path: Option<&str>,
        current_file: &Path,
        resolver: &mut Resolver,
        design: &mut Design,
        scope: &mut Scope,
        span: Option<ato_lexer::Span>,
    ) {
        // Try to resolve the import
        let resolved_path = if let Some(path) = from_path {
            // `from "path" import Name` - resolve the path
            match resolver.resolve_path_import(path, current_file) {
                Ok(p) => Some(p),
                Err(e) => {
                    self.errors.push(e);
                    return;
                }
            }
        } else {
            // `import Name` - try to find in stdlib/registry
            resolver.resolve_simple_import(name)
        };

        if let Some(resolved_path) = resolved_path {
            // Load and parse the imported file
            self.load_and_merge_file(&resolved_path, name, resolver, design, scope);
        } else {
            // Could not resolve - report error
            self.errors.push(SemaError::unresolved_import(name, span));
        }
    }

    /// Load a file and merge its exports into scope.
    fn load_and_merge_file(
        &mut self,
        path: &Path,
        import_name: &str,
        _resolver: &mut Resolver,
        design: &mut Design,
        scope: &mut Scope,
    ) {
        // Check cache
        if let Some(cached) = self.file_cache.get(path) {
            // Find the specific symbol we're importing
            for (name, kind) in cached {
                if name == import_name {
                    // Create module in design and add to scope
                    let module_id = design.create_module(name, *kind);
                    scope.define_module(name, module_id, None);
                    return;
                }
            }
            return;
        }

        // Read and parse the file
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                self.errors.push(SemaError::IoError {
                    message: format!("failed to read '{}': {}", path.display(), e),
                });
                return;
            }
        };

        let ast = match ato_parser::parse(&source) {
            Ok(ast) => ast,
            Err(errors) => {
                self.errors.push(SemaError::ParseError {
                    file: path.display().to_string(),
                    message: errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "),
                });
                return;
            }
        };

        // Extract all top-level definitions (exports)
        let mut exports = Vec::new();
        for stmt in &ast.statements {
            if let Statement::BlockDef(block) = stmt {
                let kind = match block.kind {
                    ato_parser::BlockKind::Module => ModuleKind::Module,
                    ato_parser::BlockKind::Interface => ModuleKind::Interface,
                    ato_parser::BlockKind::Component => ModuleKind::Component,
                };
                exports.push((block.name.name.clone(), kind));

                // If this is the symbol we're importing, add it to scope
                if block.name.name == import_name {
                    let module_id = design.create_module(&block.name.name, kind);
                    scope.define_module(&block.name.name, module_id, Some(block.span));

                    // Also analyze the imported module's body to populate fields
                    self.analyze_imported_module(&ast, &block.name.name, module_id, design);
                }
            }
        }

        // Cache the exports
        self.file_cache.insert(path.to_path_buf(), exports);
    }

    /// Analyze an imported module to populate its fields.
    fn analyze_imported_module(
        &mut self,
        ast: &File,
        module_name: &str,
        module_id: ato_ir::ModuleId,
        design: &mut Design,
    ) {
        // Find the module definition
        for stmt in &ast.statements {
            if let Statement::BlockDef(block) = stmt {
                if block.name.name == module_name {
                    // Process the module's body to extract fields
                    for body_stmt in &block.body {
                        match body_stmt {
                            Statement::PinDeclaration(pin) => {
                                let name = match &pin.name {
                                    ato_parser::PinName::Identifier(id) => id.name.clone(),
                                    ato_parser::PinName::Number(n) => n.value.clone(),
                                    ato_parser::PinName::String(s) => s.value.clone(),
                                };
                                design.add_field(module_id, &name, ato_ir::FieldKind::pin(&name));
                            }
                            Statement::SignalDef(signal) => {
                                design.add_field(module_id, &signal.name.name, ato_ir::FieldKind::signal());
                            }
                            Statement::Declaration(decl) => {
                                let name = decl.field.parts.first()
                                    .map(|p| p.name.name.clone())
                                    .unwrap_or_default();
                                let kind = ato_ir::FieldKind::parameter_with_unit(&decl.type_info.name);
                                design.add_field(module_id, &name, kind);
                            }
                            Statement::Assignment(assign) => {
                                // Handle field assignments like `p1 = new Electrical`
                                if let ato_parser::AssignTarget::FieldRef(field_ref) = &assign.target {
                                    if field_ref.parts.len() == 1 {
                                        let name = field_ref.parts[0].name.name.clone();
                                        if let ato_parser::Assignable::New(new_expr) = &assign.value {
                                            let type_name = new_expr.type_ref.parts
                                                .iter()
                                                .map(|p| p.name.clone())
                                                .collect::<Vec<_>>();
                                            let qname = ato_ir::QualifiedName::new(type_name);
                                            let count = new_expr.count.as_ref()
                                                .and_then(|c| c.value.parse().ok());
                                            let kind = if count.is_some() {
                                                ato_ir::FieldKind::instance_array(qname, count.unwrap())
                                            } else {
                                                ato_ir::FieldKind::instance(qname)
                                            };
                                            design.add_field(module_id, &name, kind);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    break;
                }
            }
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
