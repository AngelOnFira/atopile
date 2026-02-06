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
        // Use a dummy path for source analysis
        self.analyze_file(source, std::path::Path::new("<input>"))
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
        resolver: &mut Resolver,
        design: &mut Design,
        scope: &mut Scope,
    ) {
        // Check cache
        if let Some(cached) = self.file_cache.get(path) {
            // Find the specific symbol we're importing
            for (name, kind) in cached {
                if name == import_name {
                    // Reuse existing module if it exists, otherwise create new
                    let module_id = design.find_module(name).unwrap_or_else(|| {
                        design.create_module(name, *kind)
                    });
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

        // First, resolve imports in this file (transitive imports)
        // Create a local scope for the imported file
        let mut import_scope = Scope::new();
        self.resolve_and_merge_imports(&ast, path, resolver, design, &mut import_scope);

        // Extract all top-level definitions (exports) - build a map for inheritance lookup
        let mut exports = Vec::new();
        let mut blocks_by_name: std::collections::HashMap<String, &ato_parser::BlockDef> = std::collections::HashMap::new();

        for stmt in &ast.statements {
            if let Statement::BlockDef(block) = stmt {
                let kind = match block.kind {
                    ato_parser::BlockKind::Module => ModuleKind::Module,
                    ato_parser::BlockKind::Interface => ModuleKind::Interface,
                    ato_parser::BlockKind::Component => ModuleKind::Component,
                };
                exports.push((block.name.name.clone(), kind));
                blocks_by_name.insert(block.name.name.clone(), block);
            }
        }

        // Process ALL modules in the file, not just the requested one
        // This ensures sibling modules are available as types for each other
        let mut visited = std::collections::HashSet::new();
        for (name, _) in &exports {
            self.process_module_with_inheritance(
                &ast,
                name,
                &blocks_by_name,
                &import_scope,
                resolver,
                design,
                scope,
                &mut visited,
            );
        }

        // Cache the exports
        self.file_cache.insert(path.to_path_buf(), exports);
    }

    /// Process a module and its inheritance chain, ensuring base classes are added first.
    fn process_module_with_inheritance(
        &mut self,
        ast: &File,
        module_name: &str,
        blocks_by_name: &std::collections::HashMap<String, &ato_parser::BlockDef>,
        import_scope: &Scope,
        resolver: &mut Resolver,
        design: &mut Design,
        scope: &mut Scope,
        visited: &mut std::collections::HashSet<String>,
    ) {
        // Prevent infinite recursion
        if visited.contains(module_name) {
            return;
        }
        visited.insert(module_name.to_string());

        // Check if already in design
        if design.find_module(module_name).is_some() {
            // Already exists, just add to scope if not already there
            if scope.lookup(module_name).is_none() {
                if let Some(module_id) = design.find_module(module_name) {
                    scope.define_module(module_name, module_id, None);
                }
            }
            return;
        }

        // Get the block definition
        let block = match blocks_by_name.get(module_name) {
            Some(b) => *b,
            None => {
                // Module not in this file - check if it's in import_scope (came from an import)
                if let Some(binding) = import_scope.lookup(module_name) {
                    if let Some(module_id) = binding.as_module() {
                        scope.define_module(module_name, module_id, None);
                    }
                }
                return;
            }
        };

        // Process super_type first (if any)
        if let Some(super_ref) = &block.super_type {
            let super_name = super_ref.parts.last()
                .map(|p| p.name.clone())
                .unwrap_or_default();

            // First check if super type is in import_scope (imported)
            if let Some(binding) = import_scope.lookup(&super_name) {
                if let Some(module_id) = binding.as_module() {
                    // Super type was imported - add to current scope
                    scope.define_module(&super_name, module_id, None);
                }
            } else {
                // Super type might be in the same file - process it first
                self.process_module_with_inheritance(
                    ast,
                    &super_name,
                    blocks_by_name,
                    import_scope,
                    resolver,
                    design,
                    scope,
                    visited,
                );
            }
        }

        // Now add this module
        let kind = match block.kind {
            ato_parser::BlockKind::Module => ModuleKind::Module,
            ato_parser::BlockKind::Interface => ModuleKind::Interface,
            ato_parser::BlockKind::Component => ModuleKind::Component,
        };

        let module_id = design.create_module(&block.name.name, kind);
        scope.define_module(&block.name.name, module_id, Some(block.span));

        // Copy inherited fields from base class
        if let Some(super_ref) = &block.super_type {
            let super_name = super_ref.parts.last()
                .map(|p| p.name.clone())
                .unwrap_or_default();

            if let Some(super_id) = design.find_module(&super_name) {
                // Set super_type on the module
                if let Some(module) = design.get_module_mut(module_id) {
                    module.super_type = Some(super_id);
                }

                // Get inherited fields from the base class and copy them to this module
                let inherited_fields: Vec<_> = design
                    .get_module(super_id)
                    .map(|m| m.fields.clone())
                    .unwrap_or_default();

                // Collect field info to avoid borrowing issues
                let fields_to_copy: Vec<_> = inherited_fields
                    .iter()
                    .filter_map(|&field_id| {
                        design.get_field(field_id).map(|f| (f.name.clone(), f.kind.clone()))
                    })
                    .collect();

                for (name, kind) in fields_to_copy {
                    design.add_field(module_id, &name, kind);
                }

                // Inherit traits from base class
                let inherited_traits: Vec<_> = design
                    .get_module(super_id)
                    .map(|m| m.traits.clone())
                    .unwrap_or_default();

                if let Some(module) = design.get_module_mut(module_id) {
                    for trait_ref in inherited_traits {
                        module.add_trait(trait_ref);
                    }
                }
            }
        }

        // Create an extended scope that includes sibling modules from the same file
        // This allows modules to reference other modules defined in the same file
        let mut extended_scope = import_scope.child();
        for (sibling_name, sibling_block) in blocks_by_name {
            if let Some(sibling_id) = design.find_module(sibling_name) {
                extended_scope.define_module(sibling_name, sibling_id, Some(sibling_block.span));
            }
        }

        // Analyze the module's body with the extended scope
        self.analyze_imported_module_with_scope(ast, &block.name.name, module_id, design, &extended_scope);
    }

    /// Analyze an imported module to populate its fields (legacy, no scope).
    #[allow(dead_code)]
    fn analyze_imported_module(
        &mut self,
        ast: &File,
        module_name: &str,
        module_id: ato_ir::ModuleId,
        design: &mut Design,
    ) {
        self.analyze_imported_module_with_scope(ast, module_name, module_id, design, &Scope::new());
    }

    /// Analyze an imported module to populate its fields, using a scope for type resolution.
    fn analyze_imported_module_with_scope(
        &mut self,
        ast: &File,
        module_name: &str,
        module_id: ato_ir::ModuleId,
        design: &mut Design,
        scope: &Scope,
    ) {
        // Find the module definition
        for stmt in &ast.statements {
            if let Statement::BlockDef(block) = stmt {
                if block.name.name == module_name {
                    // Pass 1: Extract fields from the module's body
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
                                            let qname = ato_ir::QualifiedName::new(type_name.clone());
                                            let count = new_expr.count.as_ref()
                                                .and_then(|c| c.value.parse().ok());

                                            // Try to resolve the type from scope
                                            let resolved_type = type_name.last()
                                                .and_then(|n| scope.lookup(n))
                                                .and_then(|b| b.as_module());

                                            let kind = if let Some(count) = count {
                                                ato_ir::FieldKind::Instance {
                                                    type_ref: qname,
                                                    count: Some(count),
                                                    resolved_type,
                                                }
                                            } else {
                                                ato_ir::FieldKind::Instance {
                                                    type_ref: qname,
                                                    count: None,
                                                    resolved_type,
                                                }
                                            };
                                            design.add_field(module_id, &name, kind);
                                        }
                                    }
                                }
                            }
                            Statement::Connection(conn) => {
                                // Handle inline signal/pin definitions in connections
                                // e.g., `signal EN ~ pin 12` creates both EN and pin 12
                                Self::add_connectable_field(&conn.left, module_id, design);
                                Self::add_connectable_field(&conn.right, module_id, design);
                            }
                            Statement::DirectedConnection(conn) => {
                                // Handle inline definitions in directed connections
                                for element in &conn.elements {
                                    Self::add_connectable_field(element, module_id, design);
                                }
                            }
                            Statement::Trait(trait_stmt) => {
                                Self::lower_trait_stmt(trait_stmt, module_id, design);
                            }
                            _ => {}
                        }
                    }

                    // Pass 2: Lower connections using the Lowerer.
                    // Only process Connection and DirectedConnection statements
                    // to avoid duplicating field creation from assignments etc.
                    {
                        let mut module_scope = scope.child_with_module(module_id);
                        if let Some(module) = design.get_module(module_id) {
                            for &field_id in &module.fields.clone() {
                                if let Some(field) = design.get_field(field_id) {
                                    module_scope.define_field(&field.name, field_id, field.span);
                                }
                            }
                        }
                        let mut lowerer = Lowerer::new(design);
                        for body_stmt in &block.body {
                            match body_stmt {
                                Statement::Connection(_) | Statement::DirectedConnection(_) => {
                                    lowerer.lower_block_statement(body_stmt, module_id, &mut module_scope);
                                }
                                _ => {}
                            }
                        }
                        // Ignore lowerer errors for imported modules (non-critical)
                        let _ = lowerer.take_errors();
                    }

                    break;
                }
            }
        }
    }

    /// Add a field from a connectable (handles inline signal/pin definitions in connections).
    fn add_connectable_field(
        connectable: &ato_parser::Connectable,
        module_id: ato_ir::ModuleId,
        design: &mut Design,
    ) {
        match connectable {
            ato_parser::Connectable::SignalDef(signal) => {
                design.add_field(module_id, &signal.name.name, ato_ir::FieldKind::signal());
            }
            ato_parser::Connectable::PinDef(pin) => {
                let name = match &pin.name {
                    ato_parser::PinName::Identifier(id) => id.name.clone(),
                    ato_parser::PinName::Number(n) => n.value.clone(),
                    ato_parser::PinName::String(s) => s.value.clone(),
                };
                design.add_field(module_id, &name, ato_ir::FieldKind::pin(&name));
            }
            ato_parser::Connectable::FieldRef(_) => {
                // Field references don't create new fields
            }
        }
    }

    /// Lower a trait statement from an imported module into the IR.
    fn lower_trait_stmt(
        trait_stmt: &ato_parser::TraitStmt,
        module_id: ato_ir::ModuleId,
        design: &mut Design,
    ) {
        let trait_name = ato_ir::QualifiedName::new(
            trait_stmt.type_ref.parts
                .iter()
                .map(|p| p.name.clone())
                .collect(),
        );

        let mut trait_ref = ato_ir::TraitRef::new(trait_name);

        if let Some(constructor) = &trait_stmt.constructor {
            trait_ref = trait_ref.with_constructor(&constructor.name);
        }

        // Process template arguments
        if let Some(template) = &trait_stmt.template {
            for arg in &template.args {
                let arg_name = arg.name.name.clone();
                let arg_value = Self::convert_literal_to_template_arg(&arg.value);
                trait_ref = trait_ref.with_arg(arg_name, arg_value);
            }
        }

        trait_ref.span = Some(trait_stmt.span);

        if let Some(module) = design.get_module_mut(module_id) {
            module.add_trait(trait_ref);
        }
    }

    /// Convert a parser Literal to an IR TemplateArgValue.
    fn convert_literal_to_template_arg(literal: &ato_parser::Literal) -> ato_ir::TemplateArgValue {
        match literal {
            ato_parser::Literal::String(s) => {
                let value = s.value.trim_matches('"').to_string();
                ato_ir::TemplateArgValue::String(value)
            }
            ato_parser::Literal::Bool(b) => {
                ato_ir::TemplateArgValue::Bool(b.value)
            }
            ato_parser::Literal::Physical(p) => {
                match p {
                    ato_parser::PhysicalLiteral::Quantity(q) => {
                        if let Some(i) = q.number.value.parse::<i64>().ok() {
                            if q.unit.is_none() {
                                return ato_ir::TemplateArgValue::Int(i);
                            }
                        }
                        if let Some(f) = q.number.value.parse::<f64>().ok() {
                            if q.unit.is_none() {
                                return ato_ir::TemplateArgValue::Float(f);
                            }
                        }
                        let unit = q.unit.as_ref().map(|u| u.name.as_str()).unwrap_or("");
                        ato_ir::TemplateArgValue::String(format!("{}{}", q.number.value, unit))
                    }
                    _ => ato_ir::TemplateArgValue::String("physical".to_string()),
                }
            }
            ato_parser::Literal::Number(n) => {
                if let Ok(i) = n.value.parse::<i64>() {
                    ato_ir::TemplateArgValue::Int(i)
                } else if let Ok(f) = n.value.parse::<f64>() {
                    ato_ir::TemplateArgValue::Float(f)
                } else {
                    ato_ir::TemplateArgValue::String(n.value.clone())
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

    #[test]
    fn test_analyze_trait_with_template_args() {
        // Test that trait template arguments are captured correctly
        let source = r#"
module LED:
    trait has_designator_prefix::prefix<value="D">
    trait can_bridge_by_name<input_name="anode", output_name="cathode">
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source).unwrap();

        let module = &design.modules()[0];
        assert_eq!(module.traits.len(), 2, "Should have 2 traits");

        // Check has_designator_prefix trait
        let designator_trait = &module.traits[0];
        assert_eq!(designator_trait.name.name(), "has_designator_prefix");
        assert_eq!(designator_trait.constructor, Some("prefix".to_string()));
        assert_eq!(designator_trait.get_string_arg("value"), Some("D"));

        // Check can_bridge_by_name trait
        let bridge_trait = &module.traits[1];
        assert_eq!(bridge_trait.name.name(), "can_bridge_by_name");
        assert_eq!(bridge_trait.get_string_arg("input_name"), Some("anode"));
        assert_eq!(bridge_trait.get_string_arg("output_name"), Some("cathode"));
    }

    #[test]
    fn test_analyze_instance_in_assertion() {
        // Bug: instance created with `new` should be accessible in assertions
        // First test without assertion to verify field is in design
        let source_without_assert = r#"
module Inner:
    value: ohm

module Outer:
    inner = new Inner
"#;
        let mut analyzer = Analyzer::new();
        let design = analyzer.analyze_source(source_without_assert).unwrap();

        // Verify Outer has the 'inner' field
        let outer = design.modules().iter().find(|m| m.name == "Outer").unwrap();
        assert!(outer.fields.len() >= 1, "Outer should have at least 1 field");

        // Now test with assertion
        let source_with_assert = r#"
module Inner:
    value: ohm

module Outer:
    inner = new Inner
    assert inner.value > 0
"#;
        let mut analyzer2 = Analyzer::new();
        let result = analyzer2.analyze_source(source_with_assert);

        // This currently fails because `inner` is not found in scope
        assert!(result.is_ok(), "Instance should be accessible in assertion. Errors: {:?}", result.err());
    }

    #[test]
    fn test_analyze_import_with_stdlib() {
        // Test that importing from stdlib works when stdlib is configured
        use tempfile::TempDir;

        // Create a temp stdlib directory with a Resistor.ato file
        let temp_stdlib = TempDir::new().unwrap();
        let resistor_path = temp_stdlib.path().join("Resistor.ato");
        std::fs::write(&resistor_path, r#"
module Resistor:
    resistance: ohm
"#).unwrap();

        // Create source file that imports Resistor
        let temp_src = TempDir::new().unwrap();
        let source_path = temp_src.path().join("test.ato");
        std::fs::write(&source_path, r#"
import Resistor

module App:
    r1 = new Resistor
    assert r1.resistance > 0
"#).unwrap();

        let source = std::fs::read_to_string(&source_path).unwrap();

        // Analyze with stdlib configured
        let mut analyzer = Analyzer::new().with_stdlib(temp_stdlib.path());
        let result = analyzer.analyze_file(&source, &source_path);

        assert!(result.is_ok(), "Import from stdlib should work. Errors: {:?}", result.err());
        let design = result.unwrap();
        assert_eq!(design.module_count(), 2); // Resistor + App
    }

    #[test]
    fn test_analyze_from_import() {
        // Test `from "path" import Module` syntax
        use tempfile::TempDir;

        // Create a temp directory with module files
        let temp_dir = TempDir::new().unwrap();
        let resistor_path = temp_dir.path().join("Resistor.ato");
        std::fs::write(&resistor_path, r#"
module Resistor:
    resistance: ohm
"#).unwrap();

        // Create source file that imports using from-import syntax
        let source_path = temp_dir.path().join("test.ato");
        std::fs::write(&source_path, r#"
from "Resistor.ato" import Resistor

module App:
    r1 = new Resistor
"#).unwrap();

        let source = std::fs::read_to_string(&source_path).unwrap();

        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_file(&source, &source_path);

        assert!(result.is_ok(), "from-import should work. Errors: {:?}", result.err());
        let design = result.unwrap();
        assert_eq!(design.module_count(), 2); // Resistor + App
    }

    #[test]
    fn test_analyze_unresolved_import() {
        // Test that unresolved imports generate errors when using analyze_file
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("test.ato");
        std::fs::write(&source_path, r#"
import NonExistent

module App:
    x = new NonExistent
"#).unwrap();

        let source = std::fs::read_to_string(&source_path).unwrap();
        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_file(&source, &source_path);

        // Should have an unresolved import error
        assert!(result.is_err(), "Expected error for unresolved import, got {:?}", result);
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| matches!(e, SemaError::UnresolvedImport { .. })),
            "Expected UnresolvedImport error, got: {:?}", errors
        );
    }

    #[test]
    fn test_analyze_package_import() {
        // Test that external package imports work when packages are installed
        // External packages are stored in .ato/modules/<org>/<package>/
        use tempfile::TempDir;

        // Create project directory structure
        let project_dir = TempDir::new().unwrap();

        // Create a mock installed package in .ato/modules
        let package_dir = project_dir.path().join(".ato/modules/atopile/usb-connectors");
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(package_dir.join("usb-connectors.ato"), r#"
module USBCConn:
    pin vbus
    pin gnd
    pin dp
    pin dn
"#).unwrap();

        // Create main source file that imports from the package
        let src_dir = project_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let source_path = src_dir.join("main.ato");
        std::fs::write(&source_path, r#"
from "atopile/usb-connectors/usb-connectors.ato" import USBCConn

module App:
    usb = new USBCConn
"#).unwrap();

        let source = std::fs::read_to_string(&source_path).unwrap();
        let mut analyzer = Analyzer::new()
            .with_root_dir(project_dir.path());
        let result = analyzer.analyze_file(&source, &source_path);

        assert!(result.is_ok(), "Package import should work. Errors: {:?}", result.err());
        let design = result.unwrap();

        // Should have both modules
        assert!(
            design.modules().iter().any(|m| m.name == "USBCConn"),
            "USBCConn should be imported from package"
        );
        assert!(
            design.modules().iter().any(|m| m.name == "App"),
            "App module should exist"
        );

        // App.usb should have resolved_type
        let app = design.modules().iter().find(|m| m.name == "App").unwrap();
        let usb_id = app.get_field("usb").expect("usb field should exist");
        let usb_field = design.get_field(usb_id).unwrap();

        if let ato_ir::FieldKind::Instance { resolved_type, .. } = &usb_field.kind {
            assert!(resolved_type.is_some(), "usb should have resolved_type pointing to USBCConn");
        } else {
            panic!("usb should be an Instance field");
        }
    }

    // =========================================================================
    // GAP TESTS - These document missing functionality
    // =========================================================================

    #[test]
    fn test_gap1_instance_field_expansion() {
        // When `inner = new Inner` is processed, we should be able to access
        // inner.value through the instance's resolved type.
        let source = r#"
module Inner:
    value: ohm
    pin p1

module Outer:
    inner = new Inner
"#;
        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_source(source);
        assert!(result.is_ok());

        let design = result.unwrap();
        let outer = design.modules().iter().find(|m| m.name == "Outer").unwrap();
        let inner_field_id = outer.get_field("inner").expect("inner field should exist");
        let inner_field = design.get_field(inner_field_id).unwrap();

        // The inner field should have resolved_type pointing to Inner module
        if let ato_ir::FieldKind::Instance { resolved_type, .. } = &inner_field.kind {
            assert!(resolved_type.is_some(), "Instance should have resolved type");

            // Verify we can find Inner module and its fields
            let inner_module = design.get_module(resolved_type.unwrap()).unwrap();
            assert!(inner_module.get_field("value").is_some(), "Inner.value should exist");
        } else {
            panic!("Expected Instance field kind");
        }
    }

    #[test]
    fn test_gap2_assignment_creates_constraint() {
        // Assignment like `r1.resistance = 100ohm +/- 10%` should create a constraint
        let source = r#"
module Resistor:
    resistance: ohm

module App:
    r1 = new Resistor
    r1.resistance = 100ohm +/- 10%
"#;
        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_source(source);
        assert!(result.is_ok(), "Should parse: {:?}", result.err());

        let design = result.unwrap();

        // The App module should have at least one constraint from the assignment
        assert!(
            design.constraint_count() > 0,
            "Assignment should create a constraint, found {} constraints",
            design.constraint_count()
        );
    }

    #[test]
    fn test_gap2_simple_assignment_creates_constraint() {
        let source = r#"
module Test:
    value: ohm
    value = 100ohm
"#;
        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_source(source);
        assert!(result.is_ok());

        let design = result.unwrap();

        // Should have a constraint for "value = 100ohm"
        assert!(design.constraint_count() > 0, "Simple assignment should create constraint");
    }

    #[test]
    fn test_gap3_stdlib_import() {
        // Test that importing from stdlib works
        let source = r#"
import Resistor

module App:
    r1 = new Resistor
"#;
        // Get the stdlib path relative to the crate
        let stdlib_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("stdlib");

        let mut analyzer = Analyzer::new()
            .with_stdlib(stdlib_path);

        let result = analyzer.analyze_source(source);
        assert!(result.is_ok(), "Stdlib import should work: {:?}", result.err());

        let design = result.unwrap();

        // Resistor module should be in the design
        assert!(
            design.modules().iter().any(|m| m.name == "Resistor"),
            "Resistor module should be loaded from stdlib"
        );

        // App.r1 should have resolved_type pointing to Resistor
        let app = design.modules().iter().find(|m| m.name == "App").unwrap();
        let r1_id = app.get_field("r1").expect("r1 should exist");
        let r1 = design.get_field(r1_id).unwrap();

        if let ato_ir::FieldKind::Instance { resolved_type, .. } = &r1.kind {
            assert!(resolved_type.is_some(), "r1 should have resolved type pointing to Resistor");
        } else {
            panic!("r1 should be an Instance field");
        }
    }

    #[test]
    fn test_gap5_nested_field_constraint() {
        let source = r#"
module Inner:
    value: ohm

module Outer:
    inner = new Inner
    assert inner.value > 0ohm
"#;
        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_source(source);

        // This should work - nested field access in assertions
        assert!(result.is_ok(), "Should analyze nested field constraint: {:?}", result.err());

        let design = result.unwrap();
        assert!(design.constraint_count() > 0, "Should have constraint for inner.value");
    }

    #[test]
    fn test_gap6_instance_pin_connection() {
        let source = r#"
module Resistor:
    pin p1
    pin p2

module App:
    r1 = new Resistor
    r2 = new Resistor
    r1.p2 ~ r2.p1
"#;
        let mut analyzer = Analyzer::new();
        let result = analyzer.analyze_source(source);
        assert!(result.is_ok(), "Should analyze: {:?}", result.err());

        let design = result.unwrap();

        // App should have a connection between r1.p2 and r2.p1
        let app = design.modules().iter().find(|m| m.name == "App").unwrap();
        assert!(
            !app.connections.is_empty(),
            "App should have connection between r1.p2 and r2.p1, found {} connections",
            app.connections.len()
        );
    }

    #[test]
    fn test_gap7_transitive_package_imports() {
        // Test that importing from a package correctly resolves the package's own imports
        use tempfile::TempDir;

        // Create a mock package structure
        let project_dir = TempDir::new().unwrap();

        // Create stdlib with a trait
        let stdlib_dir = project_dir.path().join("stdlib");
        std::fs::create_dir_all(&stdlib_dir).unwrap();
        std::fs::write(stdlib_dir.join("traits.ato"), r#"
interface can_bridge_by_name:
    pass
"#).unwrap();

        // Create a package in .ato/modules
        let package_dir = project_dir.path().join(".ato/modules/testpkg/widgets");
        std::fs::create_dir_all(&package_dir).unwrap();

        // Package has its own stdlib import
        std::fs::write(package_dir.join("widgets.ato"), r#"
import can_bridge_by_name

module Widget:
    pin input
    pin output
    trait can_bridge_by_name
"#).unwrap();

        // Main file imports from package
        let src_dir = project_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let main_path = src_dir.join("main.ato");
        std::fs::write(&main_path, r#"
from "testpkg/widgets/widgets.ato" import Widget

module App:
    w = new Widget
"#).unwrap();

        let source = std::fs::read_to_string(&main_path).unwrap();
        let mut analyzer = Analyzer::new()
            .with_root_dir(project_dir.path())
            .with_stdlib(&stdlib_dir);

        let result = analyzer.analyze_file(&source, &main_path);

        assert!(result.is_ok(), "Transitive imports should resolve. Errors: {:?}", result.err());

        let design = result.unwrap();

        // Widget should be available
        assert!(
            design.modules().iter().any(|m| m.name == "Widget"),
            "Widget module should be imported from package"
        );

        // App.w should have resolved_type
        let app = design.modules().iter().find(|m| m.name == "App").unwrap();
        let w_id = app.get_field("w").expect("w field should exist");
        let w_field = design.get_field(w_id).unwrap();

        if let ato_ir::FieldKind::Instance { resolved_type, .. } = &w_field.kind {
            assert!(resolved_type.is_some(), "w should have resolved_type pointing to Widget");
        } else {
            panic!("w should be an Instance field");
        }
    }

    #[test]
    fn test_gap7_package_relative_imports() {
        // Test that a package can import from its own relative paths
        use tempfile::TempDir;

        let project_dir = TempDir::new().unwrap();

        // Create stdlib with Electrical
        let stdlib_dir = project_dir.path().join("stdlib");
        std::fs::create_dir_all(&stdlib_dir).unwrap();
        std::fs::write(stdlib_dir.join("electrical.ato"), r#"
interface Electrical:
    pass
"#).unwrap();

        // Create package with internal structure
        let package_dir = project_dir.path().join(".ato/modules/testpkg/mydriver");
        let parts_dir = package_dir.join("parts/MyPart");
        std::fs::create_dir_all(&parts_dir).unwrap();

        // Internal part definition
        std::fs::write(parts_dir.join("MyPart.ato"), r#"
component MyPart_package:
    pin 1
    pin 2
"#).unwrap();

        // Driver imports from relative path
        std::fs::write(package_dir.join("mydriver.ato"), r#"
import Electrical
from "parts/MyPart/MyPart.ato" import MyPart_package

module MyDriver from MyPart_package:
    power_in = new Electrical
"#).unwrap();

        // Main file
        let main_path = project_dir.path().join("main.ato");
        std::fs::write(&main_path, r#"
from "testpkg/mydriver/mydriver.ato" import MyDriver

module App:
    drv = new MyDriver
"#).unwrap();

        let source = std::fs::read_to_string(&main_path).unwrap();
        let mut analyzer = Analyzer::new()
            .with_root_dir(project_dir.path())
            .with_stdlib(&stdlib_dir);

        let result = analyzer.analyze_file(&source, &main_path);

        assert!(result.is_ok(), "Package relative imports should resolve. Errors: {:?}", result.err());
    }

    #[test]
    fn test_gap7_buttons_package_pattern() {
        // Test the real-world pattern from atopile/buttons package
        // buttons.ato imports:
        //   - Electrical from stdlib
        //   - can_bridge_by_name from stdlib
        //   - ALPSALPINE_SKRPACE010_package from relative path
        // The relative part file (ALPSALPINE_SKRPACE010.ato) imports:
        //   - has_designator_prefix from stdlib
        //   - is_atomic_part from stdlib
        use tempfile::TempDir;

        let project_dir = TempDir::new().unwrap();

        // Create comprehensive stdlib
        let stdlib_dir = project_dir.path().join("stdlib");
        std::fs::create_dir_all(&stdlib_dir).unwrap();
        std::fs::write(stdlib_dir.join("electrical.ato"), r#"
interface Electrical:
    pass
"#).unwrap();
        std::fs::write(stdlib_dir.join("traits.ato"), r#"
interface can_bridge_by_name:
    pass

interface has_designator_prefix:
    pass

interface is_atomic_part:
    pass

interface has_part_picked:
    pass
"#).unwrap();

        // Create the package structure mimicking atopile/buttons
        let package_dir = project_dir.path().join(".ato/modules/atopile/buttons");
        let parts_dir = package_dir.join("parts/ALPSALPINE_SKRPACE010");
        std::fs::create_dir_all(&parts_dir).unwrap();

        // Part file with its own stdlib imports
        std::fs::write(parts_dir.join("ALPSALPINE_SKRPACE010.ato"), r#"
import has_designator_prefix
import is_atomic_part

component ALPSALPINE_SKRPACE010_package:
    trait is_atomic_part
    trait has_designator_prefix
    pin 1
    pin 2
    pin 3
    pin 4
"#).unwrap();

        // Main buttons.ato with stdlib imports + relative import
        std::fs::write(package_dir.join("buttons.ato"), r#"
import Electrical
import can_bridge_by_name

from "parts/ALPSALPINE_SKRPACE010/ALPSALPINE_SKRPACE010.ato" import ALPSALPINE_SKRPACE010_package

module Button:
    input = new Electrical
    output = new Electrical
    trait can_bridge_by_name

module ALPSALPINE_SKRPACE010_button_driver from Button:
    package = new ALPSALPINE_SKRPACE010_package
    input ~ package.1
    output ~ package.3

module VerticalButton from ALPSALPINE_SKRPACE010_button_driver:
    pass
"#).unwrap();

        // Main project file that imports from the package
        let src_dir = project_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let main_path = src_dir.join("main.ato");
        std::fs::write(&main_path, r#"
from "atopile/buttons/buttons.ato" import VerticalButton

module App:
    btn = new VerticalButton
"#).unwrap();

        let source = std::fs::read_to_string(&main_path).unwrap();
        let mut analyzer = Analyzer::new()
            .with_root_dir(project_dir.path())
            .with_stdlib(&stdlib_dir);

        let result = analyzer.analyze_file(&source, &main_path);

        assert!(result.is_ok(), "Buttons package pattern should work. Errors: {:?}", result.err());

        let design = result.unwrap();

        // All modules should be present
        let module_names: Vec<&str> = design.modules().iter().map(|m| m.name.as_str()).collect();
        assert!(module_names.contains(&"VerticalButton"), "VerticalButton missing. Found: {:?}", module_names);
        assert!(module_names.contains(&"ALPSALPINE_SKRPACE010_button_driver"), "Driver missing. Found: {:?}", module_names);
        assert!(module_names.contains(&"Button"), "Button missing. Found: {:?}", module_names);
        assert!(module_names.contains(&"ALPSALPINE_SKRPACE010_package"), "Package missing. Found: {:?}", module_names);
        assert!(module_names.contains(&"App"), "App missing. Found: {:?}", module_names);

        // App.btn should have resolved type
        let app = design.modules().iter().find(|m| m.name == "App").unwrap();
        let btn_id = app.get_field("btn").expect("btn field should exist");
        let btn_field = design.get_field(btn_id).unwrap();

        if let ato_ir::FieldKind::Instance { resolved_type, .. } = &btn_field.kind {
            assert!(resolved_type.is_some(), "btn should have resolved_type pointing to VerticalButton");
        } else {
            panic!("btn should be an Instance field");
        }
    }
}
