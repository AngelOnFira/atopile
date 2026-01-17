//! Name resolution for Ato source files.
//!
//! This module resolves all identifiers in the AST to their definitions,
//! building up the scope tree and detecting undefined names.

use crate::error::{ErrorCollector, SemaError};
use crate::scope::{Binding, Scope};
use ato_ir::{Design, ModuleId, ModuleKind, QualifiedName};
use ato_parser::{
    AssignTarget, Assignable, BlockDef, BlockKind, Connectable, Declaration, FieldRef,
    File, ForStmt, NewExpr, PinDeclaration, PinName, SignalDef, Statement, TraitStmt, TypeRef,
};

/// Resolves names in an AST, building up module and field definitions.
pub struct NameResolver<'a> {
    /// The design being built.
    design: &'a mut Design,

    /// Collected errors.
    errors: ErrorCollector,
}

impl<'a> NameResolver<'a> {
    /// Create a new name resolver.
    pub fn new(design: &'a mut Design) -> Self {
        Self {
            design,
            errors: ErrorCollector::new(),
        }
    }

    /// Resolve names in a file, populating the design.
    pub fn resolve(&mut self, file: &File, scope: &mut Scope) -> Result<(), Vec<SemaError>> {
        for stmt in &file.statements {
            self.resolve_statement(stmt, scope);
        }

        if self.errors.has_errors() {
            Err(self.errors.errors().to_vec())
        } else {
            Ok(())
        }
    }

    /// Take the errors out of the resolver.
    pub fn take_errors(&mut self) -> Vec<SemaError> {
        std::mem::take(&mut self.errors).into_errors()
    }

    /// Resolve a single statement.
    fn resolve_statement(&mut self, stmt: &Statement, scope: &mut Scope) {
        match stmt {
            Statement::BlockDef(block) => {
                self.resolve_block_def(block, scope);
            }
            Statement::Import(import) => {
                // Register imports in scope
                let from_path = import.from_path.as_ref().map(|s| s.value.clone());
                for type_ref in &import.imports {
                    let name = type_ref.parts.last()
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    let qualified = type_ref.parts
                        .iter()
                        .map(|p| p.name.clone())
                        .collect::<Vec<_>>()
                        .join(".");
                    scope.define_import(&name, &qualified, from_path.clone(), Some(import.span));
                }
            }
            Statement::DepImport(import) => {
                let name = import.type_ref.parts.last()
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let qualified = import.type_ref.parts
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(".");
                scope.define_import(&name, &qualified, Some(import.from_path.value.clone()), Some(import.span));
            }
            // Other statements are handled within block definitions
            _ => {}
        }
    }

    /// Resolve a block definition (module/component/interface).
    fn resolve_block_def(&mut self, block: &BlockDef, scope: &mut Scope) {
        // Convert BlockKind to ModuleKind
        let kind = match block.kind {
            BlockKind::Module => ModuleKind::Module,
            BlockKind::Component => ModuleKind::Component,
            BlockKind::Interface => ModuleKind::Interface,
        };

        // Create the module
        let module_id = self.design.create_module(&block.name.name, kind);

        // Set source info
        if let Some(module) = self.design.get_module_mut(module_id) {
            module.span = Some(block.span);
        }

        // Check for duplicate definition
        if scope.is_defined_locally(&block.name.name) {
            let (_, first_span) = scope.lookup_with_span(&block.name.name).unwrap();
            self.errors.push(SemaError::duplicate_definition(
                &block.name.name,
                Some(block.span),
                first_span,
            ));
        }

        // Register in parent scope
        scope.define_module(&block.name.name, module_id, Some(block.span));

        // Create a child scope for the block body
        let mut block_scope = scope.child_with_module(module_id);

        // Handle inheritance (from clause)
        if let Some(super_ref) = &block.super_type {
            self.resolve_inheritance(module_id, super_ref, scope);
        }

        // Resolve body statements
        for stmt in &block.body {
            self.resolve_block_statement(stmt, module_id, &mut block_scope);
        }
    }

    /// Resolve inheritance (from clause).
    fn resolve_inheritance(&mut self, module_id: ModuleId, super_ref: &TypeRef, scope: &Scope) {
        let super_name = super_ref.parts.last()
            .map(|p| p.name.clone())
            .unwrap_or_default();

        // Look up the base type
        if let Some(binding) = scope.lookup(&super_name) {
            if let Some(super_id) = binding.as_module() {
                // Set the super type
                if let Some(module) = self.design.get_module_mut(module_id) {
                    module.super_type = Some(super_id);
                }
            } else {
                self.errors.push(SemaError::base_not_found(&super_name, Some(super_ref.span)));
            }
        } else {
            self.errors.push(SemaError::base_not_found(&super_name, Some(super_ref.span)));
        }
    }

    /// Resolve a statement within a block.
    fn resolve_block_statement(&mut self, stmt: &Statement, module_id: ModuleId, scope: &mut Scope) {
        match stmt {
            Statement::PinDeclaration(pin) => {
                self.resolve_pin_declaration(pin, module_id, scope);
            }
            Statement::SignalDef(signal) => {
                self.resolve_signal_def(signal, module_id, scope);
            }
            Statement::Declaration(decl) => {
                self.resolve_declaration(decl, module_id, scope);
            }
            Statement::Assignment(assign) => {
                self.resolve_assignment_target(&assign.target, &assign.value, module_id, scope);
            }
            Statement::BlockDef(nested) => {
                // Nested module - resolve recursively
                self.resolve_block_def(nested, scope);
                // Also register as nested module
                if let Some(binding) = scope.lookup(&nested.name.name) {
                    if let Some(nested_id) = binding.as_module() {
                        if let Some(module) = self.design.get_module_mut(module_id) {
                            module.add_nested_module(nested_id);
                        }
                    }
                }
            }
            Statement::Trait(trait_stmt) => {
                self.resolve_trait(trait_stmt, module_id, scope);
            }
            Statement::For(for_stmt) => {
                self.resolve_for_stmt(for_stmt, module_id, scope);
            }
            Statement::Connection(_) | Statement::DirectedConnection(_) => {
                // Connections are validated during type checking, not name resolution
            }
            Statement::Assert(_) => {
                // Assertions are validated during type checking
            }
            Statement::Retype(_) => {
                // Retyping is handled during lowering
            }
            Statement::CumAssignment(_) | Statement::SetAssignment(_) => {
                // Cumulative assignments are handled during lowering
            }
            Statement::StringStmt(_) | Statement::Pass(_) | Statement::Pragma(_) => {
                // These don't introduce names
            }
            Statement::Import(_) | Statement::DepImport(_) => {
                // Imports at block level are unusual but we handle them
                self.resolve_statement(stmt, scope);
            }
        }
    }

    /// Resolve a pin declaration.
    fn resolve_pin_declaration(&mut self, pin: &PinDeclaration, module_id: ModuleId, scope: &mut Scope) {
        let (name, kind) = match &pin.name {
            PinName::Identifier(id) => {
                (id.name.clone(), ato_ir::FieldKind::pin(&id.name))
            }
            PinName::Number(num) => {
                let n = num.value.parse::<u32>().unwrap_or(0);
                (num.value.clone(), ato_ir::FieldKind::pin_number(n))
            }
            PinName::String(s) => {
                (s.value.clone(), ato_ir::FieldKind::pin_string(&s.value))
            }
        };

        // Check for duplicate
        if scope.is_defined_locally(&name) {
            let (_, first_span) = scope.lookup_with_span(&name).unwrap();
            self.errors.push(SemaError::duplicate_definition(&name, Some(pin.span), first_span));
            return;
        }

        let field_id = self.design.add_field(module_id, &name, kind);

        // Set span
        if let Some(field) = self.design.get_field_mut(field_id) {
            field.span = Some(pin.span);
        }

        scope.define_field(&name, field_id, Some(pin.span));
    }

    /// Resolve a signal definition.
    fn resolve_signal_def(&mut self, signal: &SignalDef, module_id: ModuleId, scope: &mut Scope) {
        let name = &signal.name.name;

        // Check for duplicate
        if scope.is_defined_locally(name) {
            let (_, first_span) = scope.lookup_with_span(name).unwrap();
            self.errors.push(SemaError::duplicate_definition(name, Some(signal.span), first_span));
            return;
        }

        let field_id = self.design.add_field(module_id, name, ato_ir::FieldKind::signal());

        // Set span
        if let Some(field) = self.design.get_field_mut(field_id) {
            field.span = Some(signal.span);
        }

        scope.define_field(name, field_id, Some(signal.span));
    }

    /// Resolve a declaration (field: type).
    fn resolve_declaration(&mut self, decl: &Declaration, module_id: ModuleId, scope: &mut Scope) {
        let name = decl.field.parts.first()
            .map(|p| p.name.name.clone())
            .unwrap_or_default();

        // Check for duplicate
        if scope.is_defined_locally(&name) {
            let (_, first_span) = scope.lookup_with_span(&name).unwrap();
            self.errors.push(SemaError::duplicate_definition(&name, Some(decl.span), first_span));
            return;
        }

        // Create a parameter field with the type as unit
        let kind = ato_ir::FieldKind::parameter_with_unit(&decl.type_info.name);
        let field_id = self.design.add_field(module_id, &name, kind);

        // Set span
        if let Some(field) = self.design.get_field_mut(field_id) {
            field.span = Some(decl.span);
        }

        scope.define_field(&name, field_id, Some(decl.span));
    }

    /// Resolve an assignment target.
    fn resolve_assignment_target(
        &mut self,
        target: &AssignTarget,
        value: &Assignable,
        module_id: ModuleId,
        scope: &mut Scope,
    ) {
        match target {
            AssignTarget::FieldRef(field_ref) => {
                // This is an assignment to an existing field or a new field
                let name = field_ref.parts.first()
                    .map(|p| p.name.name.clone())
                    .unwrap_or_default();

                // If it's a simple name and not already defined, create a new field
                if field_ref.parts.len() == 1 && !scope.is_defined_locally(&name) {
                    self.create_field_from_assignment(&name, value, module_id, scope, field_ref);
                }
                // Otherwise, the field should already exist (verified during type checking)
            }
            AssignTarget::Declaration(decl) => {
                // Declaration with assignment: `field: type = value`
                self.resolve_declaration(decl, module_id, scope);
            }
        }
    }

    /// Create a field from an assignment (e.g., `x = new Resistor`).
    fn create_field_from_assignment(
        &mut self,
        name: &str,
        value: &Assignable,
        module_id: ModuleId,
        scope: &mut Scope,
        field_ref: &FieldRef,
    ) {
        let kind = match value {
            Assignable::New(new_expr) => {
                self.field_kind_from_new(new_expr, scope)
            }
            Assignable::Physical(_) | Assignable::Arithmetic(_) => {
                // Creating a parameter
                ato_ir::FieldKind::parameter()
            }
            Assignable::String(_) => {
                // String-valued parameter
                ato_ir::FieldKind::parameter()
            }
            Assignable::Boolean(_) => {
                // Boolean-valued parameter
                ato_ir::FieldKind::parameter()
            }
        };

        let field_id = self.design.add_field(module_id, name, kind);

        // Set span
        if let Some(field) = self.design.get_field_mut(field_id) {
            field.span = Some(field_ref.span);
        }

        scope.define_field(name, field_id, Some(field_ref.span));
    }

    /// Create a FieldKind from a new expression.
    fn field_kind_from_new(&self, new_expr: &NewExpr, _scope: &Scope) -> ato_ir::FieldKind {
        let type_name = QualifiedName::new(
            new_expr.type_ref.parts
                .iter()
                .map(|p| p.name.clone())
                .collect()
        );

        if let Some(count) = &new_expr.count {
            let n = count.value.parse::<u32>().unwrap_or(1);
            ato_ir::FieldKind::instance_array(type_name, n)
        } else {
            ato_ir::FieldKind::instance(type_name)
        }
    }

    /// Resolve a trait statement.
    fn resolve_trait(&mut self, trait_stmt: &TraitStmt, module_id: ModuleId, _scope: &mut Scope) {
        let trait_name = QualifiedName::new(
            trait_stmt.type_ref.parts
                .iter()
                .map(|p| p.name.clone())
                .collect()
        );

        let mut trait_ref = ato_ir::TraitRef::new(trait_name);

        if let Some(constructor) = &trait_stmt.constructor {
            trait_ref = trait_ref.with_constructor(&constructor.name);
        }

        trait_ref.span = Some(trait_stmt.span);

        if let Some(module) = self.design.get_module_mut(module_id) {
            module.add_trait(trait_ref);
        }
    }

    /// Resolve a for statement.
    fn resolve_for_stmt(&mut self, for_stmt: &ForStmt, module_id: ModuleId, scope: &mut Scope) {
        // For loops are expanded during lowering, but we need to check
        // that the iterable exists
        match &for_stmt.iterable {
            ato_parser::Iterable::FieldRef { field, .. } => {
                let name = field.parts.first()
                    .map(|p| p.name.name.clone())
                    .unwrap_or_default();

                if !scope.is_defined(&name) {
                    self.errors.push(SemaError::undefined_name(&name, Some(field.span)));
                }
            }
            ato_parser::Iterable::List(refs) => {
                for field_ref in refs {
                    let name = field_ref.parts.first()
                        .map(|p| p.name.name.clone())
                        .unwrap_or_default();

                    if !scope.is_defined(&name) {
                        self.errors.push(SemaError::undefined_name(&name, Some(field_ref.span)));
                    }
                }
            }
        }

        // Create a child scope for the loop body
        let mut loop_scope = scope.child();

        // The loop variable will be bound during lowering
        // For now, just resolve the body
        for stmt in &for_stmt.body {
            self.resolve_block_statement(stmt, module_id, &mut loop_scope);
        }
    }
}

/// Resolve a field reference, returning the binding if found.
pub fn resolve_field_ref(field_ref: &FieldRef, scope: &Scope) -> Option<Binding> {
    let first_name = field_ref.parts.first()
        .map(|p| p.name.name.as_str())?;

    scope.lookup(first_name).cloned()
}

/// Resolve a connectable element.
pub fn resolve_connectable(connectable: &Connectable, scope: &Scope) -> Option<Binding> {
    match connectable {
        Connectable::FieldRef(field_ref) => resolve_field_ref(field_ref, scope),
        Connectable::SignalDef(_) | Connectable::PinDef(_) => {
            // Inline definitions don't resolve to existing bindings
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_resolve(source: &str) -> (Design, Vec<SemaError>) {
        let ast = ato_parser::parse(source).unwrap();
        let mut design = Design::new();
        let mut scope = Scope::new();
        let mut resolver = NameResolver::new(&mut design);
        let _ = resolver.resolve(&ast, &mut scope);
        let errors = resolver.take_errors();
        (design, errors)
    }

    #[test]
    fn test_resolve_empty() {
        let (design, errors) = parse_and_resolve("");
        assert!(errors.is_empty());
        assert_eq!(design.module_count(), 0);
    }

    #[test]
    fn test_resolve_simple_module() {
        let (design, errors) = parse_and_resolve("module M:\n    pass\n");
        assert!(errors.is_empty());
        assert_eq!(design.module_count(), 1);

        let module = &design.modules()[0];
        assert_eq!(module.name, "M");
        assert!(module.is_module());
    }

    #[test]
    fn test_resolve_component() {
        let (design, errors) = parse_and_resolve("component C:\n    pass\n");
        assert!(errors.is_empty());

        let module = &design.modules()[0];
        assert!(module.is_component());
    }

    #[test]
    fn test_resolve_interface() {
        let (design, errors) = parse_and_resolve("interface I:\n    pass\n");
        assert!(errors.is_empty());

        let module = &design.modules()[0];
        assert!(module.is_interface());
    }

    #[test]
    fn test_resolve_pin() {
        let (design, errors) = parse_and_resolve("module M:\n    pin p1\n");
        assert!(errors.is_empty());
        assert_eq!(design.field_count(), 1);

        let field = &design.fields()[0];
        assert_eq!(field.name, "p1");
        assert!(field.is_pin());
    }

    #[test]
    fn test_resolve_signal() {
        let (design, errors) = parse_and_resolve("module M:\n    signal sig\n");
        assert!(errors.is_empty());

        let field = &design.fields()[0];
        assert_eq!(field.name, "sig");
        assert!(field.is_signal());
    }

    #[test]
    fn test_resolve_parameter() {
        let (design, errors) = parse_and_resolve("module M:\n    resistance: ohm\n");
        assert!(errors.is_empty());

        let field = &design.fields()[0];
        assert_eq!(field.name, "resistance");
        assert!(field.is_parameter());
    }

    #[test]
    fn test_resolve_instance() {
        let (design, errors) = parse_and_resolve("module M:\n    r1 = new Resistor\n");
        assert!(errors.is_empty());

        let field = &design.fields()[0];
        assert_eq!(field.name, "r1");
        assert!(field.is_instance());
    }

    #[test]
    fn test_resolve_array_instance() {
        let (design, errors) = parse_and_resolve("module M:\n    items = new Item[10]\n");
        assert!(errors.is_empty());

        let field = &design.fields()[0];
        if let ato_ir::FieldKind::Instance { count, .. } = &field.kind {
            assert_eq!(*count, Some(10));
        } else {
            panic!("Expected instance field");
        }
    }

    #[test]
    fn test_resolve_inheritance() {
        let source = r#"
module Base:
    pass
module Child from Base:
    pass
"#;
        let (design, errors) = parse_and_resolve(source);
        assert!(errors.is_empty());
        assert_eq!(design.module_count(), 2);

        let base_id = design.find_module("Base").unwrap();
        let child = design.find_module("Child")
            .and_then(|id| design.get_module(id))
            .unwrap();
        assert_eq!(child.super_type, Some(base_id));
    }

    #[test]
    fn test_duplicate_definition_error() {
        let source = r#"
module M:
    pin p1
    pin p1
"#;
        let (_, errors) = parse_and_resolve(source);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], SemaError::DuplicateDefinition { .. }));
    }

    #[test]
    fn test_nested_module() {
        let source = r#"
module Outer:
    module Inner:
        pass
"#;
        let (design, errors) = parse_and_resolve(source);
        assert!(errors.is_empty());
        assert_eq!(design.module_count(), 2);

        let outer_id = design.find_module("Outer").unwrap();
        let inner_id = design.find_module("Inner").unwrap();
        let outer = design.get_module(outer_id).unwrap();
        assert!(outer.nested_modules.contains(&inner_id));
    }
}
