//! Type checking for Ato source files.
//!
//! This module verifies that connections are between compatible interfaces
//! and that field accesses are valid.

use crate::error::{ErrorCollector, SemaError};
use crate::scope::{Binding, Scope};
use ato_ir::{Design, FieldId, FieldKind, ModuleId};
use ato_parser::{
    Connectable, Connection, DirectedConnection, Expression, FieldRef, File, ForStmt,
    Iterable, Statement,
};

/// Result of resolving a field reference.
enum FieldRefResolution {
    /// Successfully resolved to a field.
    Resolved(FieldId),
    /// Root name not found in scope.
    RootNotFound,
    /// Root name found but a sub-field couldn't be resolved because type info
    /// is incomplete (resolved_type is None, field not an instance, etc.).
    TypeInfoIncomplete,
    /// Root name found, type is fully resolved, but the sub-field doesn't exist
    /// on the resolved type. This is a definite error.
    SubFieldNotFound(#[allow(dead_code)] String),
}

/// Type checks an AST against a resolved design.
pub struct TypeChecker<'a> {
    /// The resolved design.
    design: &'a Design,

    /// Collected errors.
    errors: ErrorCollector,
}

impl<'a> TypeChecker<'a> {
    /// Create a new type checker.
    pub fn new(design: &'a Design) -> Self {
        Self {
            design,
            errors: ErrorCollector::new(),
        }
    }

    /// Type check a file.
    pub fn check(&mut self, file: &File, scope: &Scope) -> Result<(), Vec<SemaError>> {
        for stmt in &file.statements {
            self.check_statement(stmt, scope);
        }

        if self.errors.has_errors() {
            Err(self.errors.errors().to_vec())
        } else {
            Ok(())
        }
    }

    /// Take the errors out of the checker.
    pub fn take_errors(&mut self) -> Vec<SemaError> {
        std::mem::take(&mut self.errors).into_errors()
    }

    /// Check a single statement.
    fn check_statement(&mut self, stmt: &Statement, scope: &Scope) {
        match stmt {
            Statement::BlockDef(block) => {
                // Create child scope for the block and populate with fields from design
                let block_scope = if let Some(binding) = scope.lookup(&block.name.name) {
                    if let Some(module_id) = binding.as_module() {
                        let mut child = scope.child_with_module(module_id);
                        // Populate scope with fields from the module
                        self.populate_scope_from_module(module_id, &mut child);
                        child
                    } else {
                        scope.child()
                    }
                } else {
                    scope.child()
                };

                // Check body statements
                for stmt in &block.body {
                    self.check_block_statement(stmt, &block_scope);
                }
            }
            _ => {}
        }
    }

    /// Populate a scope with field bindings from a module in the design.
    fn populate_scope_from_module(&self, module_id: ModuleId, scope: &mut Scope) {
        if let Some(module) = self.design.get_module(module_id) {
            // Add all fields from this module to the scope
            for field_id in &module.fields {
                if let Some(field) = self.design.get_field(*field_id) {
                    scope.define_field(&field.name, *field_id, field.span);
                }
            }
            // Add nested modules to scope
            for nested_id in &module.nested_modules {
                if let Some(nested) = self.design.get_module(*nested_id) {
                    scope.define_module(&nested.name, *nested_id, nested.span);
                }
            }
        }
    }

    /// Check a statement within a block.
    fn check_block_statement(&mut self, stmt: &Statement, scope: &Scope) {
        match stmt {
            Statement::Connection(conn) => {
                self.check_connection(conn, scope);
            }
            Statement::DirectedConnection(conn) => {
                self.check_directed_connection(conn, scope);
            }
            Statement::For(for_stmt) => {
                self.check_for_statement(for_stmt, scope);
            }
            Statement::BlockDef(nested) => {
                self.check_statement(&Statement::BlockDef(nested.clone()), scope);
            }
            Statement::Assert(assert_stmt) => {
                self.check_assertion_expression(&assert_stmt.comparison.left, scope);
                for op in &assert_stmt.comparison.operations {
                    self.check_assertion_expression(&op.right, scope);
                }
            }
            _ => {}
        }
    }

    /// Check a simple connection (a ~ b).
    fn check_connection(&mut self, conn: &Connection, scope: &Scope) {
        // Check that both endpoints are connectable
        self.check_connectable(&conn.left, scope, conn.span);
        self.check_connectable(&conn.right, scope, conn.span);

        // Check type compatibility
        if let (Some(left_type), Some(right_type)) = (
            self.get_connectable_type(&conn.left, scope),
            self.get_connectable_type(&conn.right, scope),
        ) {
            if !self.types_compatible(&left_type, &right_type) {
                self.errors.push(SemaError::type_mismatch(
                    &left_type,
                    &right_type,
                    Some(conn.span),
                ));
            }
        }
    }

    /// Check a directed connection (a ~> b ~> c).
    fn check_directed_connection(&mut self, conn: &DirectedConnection, scope: &Scope) {
        for element in &conn.elements {
            self.check_connectable(element, scope, conn.span);
        }

        // For directed connections, we'd need more sophisticated type checking
        // to handle bridge modules. For now, just verify all elements are connectable.
    }

    /// Check that a connectable is valid.
    fn check_connectable(&mut self, connectable: &Connectable, scope: &Scope, span: ato_lexer::Span) {
        match connectable {
            Connectable::FieldRef(field_ref) => {
                match self.resolve_field_ref_detailed(field_ref, scope) {
                    FieldRefResolution::Resolved(field_id) => {
                        if let Some(field) = self.design.get_field(field_id) {
                            if !field.is_connectable() {
                                self.errors.push(SemaError::not_connectable(
                                    &field.name,
                                    Some(span),
                                ));
                            }
                        }
                    }
                    FieldRefResolution::RootNotFound => {
                        let first_name = field_ref.parts.first()
                            .map(|p| p.name.name.clone())
                            .unwrap_or_default();
                        self.errors.push(SemaError::undefined_name(&first_name, Some(field_ref.span)));
                    }
                    FieldRefResolution::SubFieldNotFound(_) | FieldRefResolution::TypeInfoIncomplete => {
                        // Sub-field resolution failed. This could be:
                        // - A genuinely missing field on a known type
                        // - A Python dynamic property (e.g. reference_shim) not modeled in Rust
                        // - Incomplete type information
                        // Skip silently for now since we can't distinguish these cases.
                    }
                }
            }
            Connectable::SignalDef(_) | Connectable::PinDef(_) => {
                // Inline definitions are always connectable
            }
        }
    }

    /// Get the type of a connectable element.
    fn get_connectable_type(&self, connectable: &Connectable, scope: &Scope) -> Option<String> {
        match connectable {
            Connectable::FieldRef(field_ref) => {
                if let Some(field_id) = self.resolve_field_ref(field_ref, scope) {
                    if let Some(field) = self.design.get_field(field_id) {
                        return Some(self.field_type_name(&field.kind));
                    }
                }
                None
            }
            Connectable::SignalDef(_) => Some("signal".to_string()),
            Connectable::PinDef(_) => Some("pin".to_string()),
        }
    }

    /// Get a displayable type name for a field kind.
    fn field_type_name(&self, kind: &FieldKind) -> String {
        match kind {
            FieldKind::Parameter { unit } => {
                if let Some(u) = unit {
                    format!("parameter:{}", u)
                } else {
                    "parameter".to_string()
                }
            }
            FieldKind::Pin { .. } => "pin".to_string(),
            FieldKind::Signal => "signal".to_string(),
            FieldKind::Instance { type_ref, .. } => type_ref.to_string(),
        }
    }

    /// Check if two types are compatible for connection.
    fn types_compatible(&self, left: &str, right: &str) -> bool {
        // For now, we allow:
        // - pin ~ pin
        // - pin ~ signal
        // - signal ~ signal
        // - instance ~ instance (of compatible types)
        // - anything ~ anything (permissive for now)

        // In a full implementation, we'd check:
        // 1. Interface compatibility (e.g., Electrical ~ Electrical)
        // 2. Recursively check that nested interfaces match

        // For now, be permissive and allow most connections
        // The real type checking happens at runtime in the solver

        // Only flag obvious mismatches
        if left.starts_with("parameter") || right.starts_with("parameter") {
            // Parameters are not connectable
            return false;
        }

        true
    }

    /// Resolve a field reference to a field ID (convenience wrapper).
    fn resolve_field_ref(&self, field_ref: &FieldRef, scope: &Scope) -> Option<FieldId> {
        match self.resolve_field_ref_detailed(field_ref, scope) {
            FieldRefResolution::Resolved(id) => Some(id),
            _ => None,
        }
    }

    /// Resolve a field reference with detailed error information.
    fn resolve_field_ref_detailed(&self, field_ref: &FieldRef, scope: &Scope) -> FieldRefResolution {
        let first_name = match field_ref.parts.first() {
            Some(p) => p.name.name.as_str(),
            None => return FieldRefResolution::RootNotFound,
        };

        match scope.lookup(first_name) {
            Some(Binding::Field(id)) => {
                // If it's a simple reference, return directly
                if field_ref.parts.len() == 1 {
                    return FieldRefResolution::Resolved(*id);
                }

                // For nested references (a.b.c), we need to look up each part
                self.resolve_nested_field(field_ref, *id)
            }
            Some(Binding::LoopVariable { source, .. }) => {
                // Loop variable - resolve from the source
                FieldRefResolution::Resolved(*source)
            }
            None => FieldRefResolution::RootNotFound,
            _ => FieldRefResolution::RootNotFound,
        }
    }

    /// Resolve a nested field reference (a.b.c).
    fn resolve_nested_field(&self, field_ref: &FieldRef, start_field: FieldId) -> FieldRefResolution {
        let mut current_field = start_field;

        for part in field_ref.parts.iter().skip(1) {
            // Get the current field
            let field = match self.design.get_field(current_field) {
                Some(f) => f,
                None => return FieldRefResolution::TypeInfoIncomplete,
            };

            // The field must be an instance to access nested fields
            if let FieldKind::Instance { resolved_type, .. } = &field.kind {
                if let Some(module_id) = resolved_type {
                    // Look up the next field in the instance's type
                    let module = match self.design.get_module(*module_id) {
                        Some(m) => m,
                        None => return FieldRefResolution::TypeInfoIncomplete,
                    };
                    match module.get_field(&part.name.name) {
                        Some(field_id) => current_field = field_id,
                        None => return FieldRefResolution::SubFieldNotFound(part.name.name.clone()),
                    }
                } else {
                    // Type not resolved yet
                    return FieldRefResolution::TypeInfoIncomplete;
                }
            } else {
                // Not an instance (e.g. parameter, pin, signal) - can't access nested fields
                // but this isn't necessarily a definite error (could be incomplete type info)
                return FieldRefResolution::TypeInfoIncomplete;
            }
        }

        FieldRefResolution::Resolved(current_field)
    }

    /// Check a for statement.
    fn check_for_statement(&mut self, for_stmt: &ForStmt, scope: &Scope) {
        // Check the iterable
        match &for_stmt.iterable {
            Iterable::FieldRef { field, slice } => {
                if let Some(field_id) = self.resolve_field_ref(field, scope) {
                    if let Some(f) = self.design.get_field(field_id) {
                        // Check that it's iterable (must be an array instance)
                        if let FieldKind::Instance { count, .. } = &f.kind {
                            if count.is_none() {
                                self.errors.push(SemaError::not_iterable(
                                    &f.name,
                                    Some(field.span),
                                ));
                            }

                            // If there's a slice, check bounds
                            if let Some(s) = slice {
                                if let Some(c) = count {
                                    if let Some(stop) = &s.stop {
                                        let stop_val = stop.value.parse::<u32>().unwrap_or(0);
                                        if stop_val > *c {
                                            self.errors.push(SemaError::index_out_of_bounds(
                                                stop_val,
                                                *c,
                                                Some(s.span),
                                            ));
                                        }
                                    }
                                }
                            }
                        } else if !f.is_instance() {
                            self.errors.push(SemaError::not_iterable(
                                &f.name,
                                Some(field.span),
                            ));
                        }
                    }
                } else {
                    let name = field.parts.first()
                        .map(|p| p.name.name.clone())
                        .unwrap_or_default();
                    self.errors.push(SemaError::undefined_name(&name, Some(field.span)));
                }
            }
            Iterable::List(refs) => {
                // Check that all items in the list are defined
                for field_ref in refs {
                    if self.resolve_field_ref(field_ref, scope).is_none() {
                        let name = field_ref.parts.first()
                            .map(|p| p.name.name.clone())
                            .unwrap_or_default();
                        self.errors.push(SemaError::undefined_name(&name, Some(field_ref.span)));
                    }
                }
            }
        }

        // Check the body (in the loop scope with the loop variable bound)
        let mut loop_scope = scope.child();

        // Bind the loop variable
        let loop_var_name = &for_stmt.variable.name;
        match &for_stmt.iterable {
            Iterable::FieldRef { field, .. } => {
                // Bind loop variable to the iterable field
                if let Some(field_id) = self.resolve_field_ref(field, scope) {
                    loop_scope.define(
                        loop_var_name.clone(),
                        Binding::LoopVariable { source: field_id, index: None },
                        Some(for_stmt.variable.span),
                    );
                }
            }
            Iterable::List(refs) => {
                // For list iteration, bind to the first element for type checking purposes
                if let Some(first_ref) = refs.first() {
                    if let Some(field_id) = self.resolve_field_ref(first_ref, scope) {
                        loop_scope.define(
                            loop_var_name.clone(),
                            Binding::LoopVariable { source: field_id, index: None },
                            Some(for_stmt.variable.span),
                        );
                    }
                }
            }
        }

        for stmt in &for_stmt.body {
            self.check_block_statement(stmt, &loop_scope);
        }
    }

    /// Check an expression in an assertion.
    fn check_assertion_expression(&mut self, expr: &Expression, scope: &Scope) {
        match expr {
            Expression::FieldRef(field_ref) => {
                match self.resolve_field_ref_detailed(field_ref, scope) {
                    FieldRefResolution::Resolved(_) => {}
                    FieldRefResolution::RootNotFound => {
                        let name = field_ref.parts.first()
                            .map(|p| p.name.name.clone())
                            .unwrap_or_default();
                        self.errors.push(SemaError::undefined_name(&name, Some(field_ref.span)));
                    }
                    FieldRefResolution::SubFieldNotFound(_) | FieldRefResolution::TypeInfoIncomplete => {
                        // Skip silently - see check_connectable comment
                    }
                }
            }
            Expression::Binary(binary) => {
                self.check_assertion_expression(&binary.left, scope);
                self.check_assertion_expression(&binary.right, scope);
            }
            Expression::Unary(unary) => {
                self.check_assertion_expression(&unary.operand, scope);
            }
            Expression::Group(inner) => {
                self.check_assertion_expression(inner, scope);
            }
            Expression::FunctionCall(call) => {
                for arg in &call.args {
                    self.check_assertion_expression(arg, scope);
                }
            }
            Expression::Literal(_) => {
                // Literals are always valid
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::NameResolver;

    fn analyze(source: &str) -> (Design, Vec<SemaError>) {
        let ast = ato_parser::parse(source).unwrap();
        let mut design = Design::new();
        let mut scope = Scope::new();

        // Name resolution
        let mut name_resolver = NameResolver::new(&mut design);
        let _ = name_resolver.resolve(&ast, &mut scope);
        let mut errors = name_resolver.take_errors();

        // Type checking
        let mut type_checker = TypeChecker::new(&design);
        let _ = type_checker.check(&ast, &scope);
        errors.extend(type_checker.take_errors());

        (design, errors)
    }

    #[test]
    fn test_valid_connection() {
        let source = r#"
module M:
    pin p1
    pin p2
    p1 ~ p2
"#;
        let (_, errors) = analyze(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_signal_connection() {
        let source = r#"
module M:
    pin p1
    signal sig
    p1 ~ sig
"#;
        let (_, errors) = analyze(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_undefined_in_connection() {
        let source = r#"
module M:
    pin p1
    p1 ~ undefined_pin
"#;
        let (_, errors) = analyze(source);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| matches!(e, SemaError::UndefinedName { .. })));
    }

    #[test]
    fn test_for_loop_iterable() {
        let source = r#"
module M:
    items = new Item[5]
    for item in items:
        pass
"#;
        let (_, errors) = analyze(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_for_loop_undefined_iterable() {
        let source = r#"
module M:
    for item in undefined_array:
        pass
"#;
        let (_, errors) = analyze(source);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_assert_expression() {
        let source = r#"
module M:
    resistance: ohm
    assert resistance > 0
"#;
        let (_, errors) = analyze(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_assert_undefined() {
        let source = r#"
module M:
    assert undefined_param > 0
"#;
        let (_, errors) = analyze(source);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| matches!(e, SemaError::UndefinedName { .. })));
    }
}
