//! AST to IR lowering.
//!
//! This module converts the parsed AST to the IR representation,
//! expanding for loops, instantiating templates, and resolving connections.

use crate::error::{ErrorCollector, SemaError};
use crate::scope::{Binding, Scope};
use ato_ir::{
    CompareOpKind as IrCompareOpKind, ConnectionEndpoint, ConnectionKind, ConstraintExpr,
    Design, FieldId, FieldKind, FieldPath, FieldPathPart, ModuleId,
    QuantityValue, ToleranceValue, ValueExpr, ValueLiteral,
};
use ato_parser::{
    Assignable, AssertStmt, Assignment, AssignTarget, BinaryOp, BlockDef, CompareOpKind, Comparison,
    Connectable, Connection, ConnectionDirection, DirectedConnection, Expression, FieldRef,
    File, ForStmt, Iterable, Literal, PhysicalLiteral, PinDeclaration, PinName,
    Quantity, Slice, Statement, Tolerance,
};

/// Lowers an AST to IR.
pub struct Lowerer<'a> {
    /// The design being built.
    design: &'a mut Design,

    /// Collected errors.
    errors: ErrorCollector,
}

impl<'a> Lowerer<'a> {
    /// Create a new lowerer.
    pub fn new(design: &'a mut Design) -> Self {
        Self {
            design,
            errors: ErrorCollector::new(),
        }
    }

    /// Lower a file, adding connections and constraints to the design.
    pub fn lower(&mut self, file: &File, scope: &Scope) -> Result<(), Vec<SemaError>> {
        for stmt in &file.statements {
            self.lower_statement(stmt, scope);
        }

        if self.errors.has_errors() {
            Err(self.errors.errors().to_vec())
        } else {
            Ok(())
        }
    }

    /// Take the errors out of the lowerer.
    pub fn take_errors(&mut self) -> Vec<SemaError> {
        std::mem::take(&mut self.errors).into_errors()
    }

    /// Lower a single statement.
    fn lower_statement(&mut self, stmt: &Statement, scope: &Scope) {
        if let Statement::BlockDef(block) = stmt {
            self.lower_block_def(block, scope);
        }
    }

    /// Lower a block definition.
    fn lower_block_def(&mut self, block: &BlockDef, scope: &Scope) {
        // Find the module ID
        let module_id = if let Some(Binding::Module(id)) = scope.lookup(&block.name.name) {
            *id
        } else {
            return;
        };

        // Create child scope
        let mut block_scope = scope.child_with_module(module_id);

        // Register fields in scope
        if let Some(module) = self.design.get_module(module_id) {
            for field_id in &module.fields.clone() {
                if let Some(field) = self.design.get_field(*field_id) {
                    block_scope.define_field(&field.name, *field_id, field.span);
                }
            }
        }

        // Lower body statements
        for stmt in &block.body {
            self.lower_block_statement(stmt, module_id, &mut block_scope);
        }
    }

    /// Lower a statement within a block.
    fn lower_block_statement(&mut self, stmt: &Statement, module_id: ModuleId, scope: &mut Scope) {
        match stmt {
            Statement::Connection(conn) => {
                self.lower_connection(conn, module_id, scope);
            }
            Statement::DirectedConnection(conn) => {
                self.lower_directed_connection(conn, module_id, scope);
            }
            Statement::Assert(assert_stmt) => {
                self.lower_assert(assert_stmt, module_id, scope);
            }
            Statement::For(for_stmt) => {
                self.lower_for_statement(for_stmt, module_id, scope);
            }
            Statement::BlockDef(nested) => {
                // Nested module - lower recursively
                self.lower_block_def(nested, scope);
            }
            Statement::Assignment(assign) => {
                // Convert physical value assignments to constraints
                self.lower_assignment(assign, module_id, scope);
            }
            _ => {
                // Other statements handled during name resolution
            }
        }
    }

    /// Lower a simple connection (a ~ b).
    fn lower_connection(&mut self, conn: &Connection, module_id: ModuleId, scope: &Scope) {
        let left = self.lower_connectable(&conn.left, module_id, scope);
        let right = self.lower_connectable(&conn.right, module_id, scope);

        let _conn_id = self.design.add_connection(module_id, left, right);
        // Note: Span is not set as the connections field is private.
        // The span could be added to the add_connection API if needed.
    }

    /// Lower a directed connection (a ~> b ~> c).
    fn lower_directed_connection(&mut self, conn: &DirectedConnection, module_id: ModuleId, scope: &Scope) {
        let endpoints: Vec<ConnectionEndpoint> = conn.elements
            .iter()
            .map(|e| self.lower_connectable(e, module_id, scope))
            .collect();

        let kind = match conn.direction {
            ConnectionDirection::Forward => {
                ConnectionKind::Directed(ato_ir::ConnectionDirection::Forward)
            }
            ConnectionDirection::Backward => {
                ConnectionKind::Directed(ato_ir::ConnectionDirection::Backward)
            }
        };

        self.design.add_directed_connection(module_id, endpoints, kind);
    }

    /// Lower a connectable element to a connection endpoint.
    fn lower_connectable(&mut self, connectable: &Connectable, module_id: ModuleId, scope: &Scope) -> ConnectionEndpoint {
        match connectable {
            Connectable::FieldRef(field_ref) => {
                let path = self.lower_field_ref(field_ref);

                // Try to resolve to a field ID
                let resolved = self.resolve_field_path(&path, scope);

                ConnectionEndpoint {
                    kind: ato_ir::EndpointKind::FieldRef(path),
                    resolved,
                }
            }
            Connectable::SignalDef(signal) => {
                // Inline signal definition - create a new signal field
                let field_id = self.design.add_field(
                    module_id,
                    &signal.name.name,
                    FieldKind::signal(),
                );

                if let Some(field) = self.design.get_field_mut(field_id) {
                    field.span = Some(signal.span);
                }

                ConnectionEndpoint::resolved(field_id)
            }
            Connectable::PinDef(pin) => {
                // Inline pin definition - create a new pin field
                let (name, kind) = self.pin_to_field_kind(pin);
                let field_id = self.design.add_field(module_id, &name, kind);

                if let Some(field) = self.design.get_field_mut(field_id) {
                    field.span = Some(pin.span);
                }

                ConnectionEndpoint::resolved(field_id)
            }
        }
    }

    /// Convert a pin declaration to a name and field kind.
    fn pin_to_field_kind(&self, pin: &PinDeclaration) -> (String, FieldKind) {
        match &pin.name {
            PinName::Identifier(id) => {
                (id.name.clone(), FieldKind::pin(&id.name))
            }
            PinName::Number(num) => {
                let n = num.value.parse::<u32>().unwrap_or(0);
                (num.value.clone(), FieldKind::pin_number(n))
            }
            PinName::String(s) => {
                (s.value.clone(), FieldKind::pin_string(&s.value))
            }
        }
    }

    /// Lower a field reference to a field path.
    fn lower_field_ref(&self, field_ref: &FieldRef) -> FieldPath {
        let mut parts = Vec::new();

        for part in &field_ref.parts {
            parts.push(FieldPathPart::Name(part.name.name.clone()));

            if let Some(index) = &part.index {
                let n = index.value.parse::<u32>().unwrap_or(0);
                parts.push(FieldPathPart::Index(n));
            }
        }

        // Handle trailing pin reference (e.g., .1)
        if let Some(pin_ref) = &field_ref.pin_ref {
            let n = pin_ref.value.parse::<u32>().unwrap_or(0);
            parts.push(FieldPathPart::PinRef(n));
        }

        FieldPath::new(parts)
    }

    /// Resolve a field path to a field ID.
    fn resolve_field_path(&self, path: &FieldPath, scope: &Scope) -> Option<FieldId> {
        let first_name = path.first_name()?;

        match scope.lookup(first_name)? {
            Binding::Field(id) => Some(*id),
            Binding::LoopVariable { source, .. } => Some(*source),
            _ => None,
        }
    }

    /// Lower an assert statement to a constraint.
    fn lower_assert(&mut self, assert_stmt: &AssertStmt, module_id: ModuleId, scope: &Scope) {
        let constraint_expr = self.lower_comparison(&assert_stmt.comparison, scope);
        let _constraint_id = self.design.create_constraint(module_id, constraint_expr);
        // Note: Span is not set as the constraints field is private.
        // The span could be added to the create_constraint API if needed.
    }

    /// Lower an assignment statement to a constraint (for physical values).
    fn lower_assignment(&mut self, assign: &Assignment, module_id: ModuleId, _scope: &Scope) {
        // Only convert physical value assignments to constraints
        let value_literal = match &assign.value {
            Assignable::Physical(phys) => self.lower_physical_literal(phys),
            Assignable::Arithmetic(expr) => {
                // Arithmetic expressions can also be converted to constraints
                // if they contain physical literals
                if let Expression::Literal(Literal::Physical(phys)) = expr {
                    self.lower_physical_literal(phys)
                } else {
                    // Non-physical arithmetic - not a constraint
                    return;
                }
            }
            // String, New, Boolean - not constraints
            _ => return,
        };

        // Get the target field path
        let target_path = match &assign.target {
            AssignTarget::FieldRef(field_ref) => self.lower_field_ref(field_ref),
            AssignTarget::Declaration(decl) => {
                // Declaration target - use the field reference
                self.lower_field_ref(&decl.field)
            }
        };

        // Determine the comparison operator based on the value type
        let op_kind = match &assign.value {
            Assignable::Physical(PhysicalLiteral::Quantity(_)) => IrCompareOpKind::Is,
            Assignable::Physical(PhysicalLiteral::Range(_)) => IrCompareOpKind::Within,
            Assignable::Physical(PhysicalLiteral::Bilateral(_)) => IrCompareOpKind::Within,
            _ => IrCompareOpKind::Is,
        };

        // Create the constraint expression: target IS/WITHIN value
        let left = ValueExpr::field(target_path);
        let right = ValueExpr::literal(value_literal);
        let constraint_expr = ConstraintExpr::compare(left, op_kind, right);

        self.design.create_constraint(module_id, constraint_expr);
    }

    /// Lower a comparison to a constraint expression.
    fn lower_comparison(&self, comparison: &Comparison, scope: &Scope) -> ConstraintExpr {
        let left = self.lower_expression(&comparison.left, scope);

        let operations = comparison.operations
            .iter()
            .map(|op| ato_ir::CompareOp {
                kind: self.lower_compare_op_kind(op.kind),
                right: self.lower_expression(&op.right, scope),
            })
            .collect();

        ConstraintExpr::new(left, operations)
    }

    /// Lower a comparison operator kind.
    fn lower_compare_op_kind(&self, kind: CompareOpKind) -> IrCompareOpKind {
        match kind {
            CompareOpKind::LessThan => IrCompareOpKind::LessThan,
            CompareOpKind::GreaterThan => IrCompareOpKind::GreaterThan,
            CompareOpKind::LessEq => IrCompareOpKind::LessEq,
            CompareOpKind::GreaterEq => IrCompareOpKind::GreaterEq,
            CompareOpKind::Within => IrCompareOpKind::Within,
            CompareOpKind::Is => IrCompareOpKind::Is,
        }
    }

    /// Lower an expression to a value expression.
    fn lower_expression(&self, expr: &Expression, scope: &Scope) -> ValueExpr {
        match expr {
            Expression::FieldRef(field_ref) => {
                let path = self.lower_field_ref(field_ref);
                ValueExpr::field(path)
            }
            Expression::Literal(lit) => {
                ValueExpr::literal(self.lower_literal(lit))
            }
            Expression::Binary(binary) => {
                let left = self.lower_expression(&binary.left, scope);
                let right = self.lower_expression(&binary.right, scope);
                let op = self.lower_binary_op(binary.operator);
                ValueExpr::binary(left, op, right)
            }
            Expression::Unary(unary) => {
                let operand = self.lower_expression(&unary.operand, scope);
                let op = match unary.operator {
                    ato_parser::UnaryOp::Neg => ato_ir::UnaryOp::Neg,
                    ato_parser::UnaryOp::Pos => ato_ir::UnaryOp::Pos,
                };
                ValueExpr::unary(op, operand)
            }
            Expression::Group(inner) => {
                ValueExpr::Group(Box::new(self.lower_expression(inner, scope)))
            }
            Expression::FunctionCall(_) => {
                // Function calls in constraints are not yet supported
                ValueExpr::literal(ValueLiteral::Bool(false))
            }
        }
    }

    /// Lower a literal to a value literal.
    fn lower_literal(&self, lit: &Literal) -> ValueLiteral {
        match lit {
            Literal::Number(num) => {
                let value = self.parse_number(&num.value);
                ValueLiteral::Quantity(QuantityValue::dimensionless(value))
            }
            Literal::String(s) => {
                ValueLiteral::String(s.value.clone())
            }
            Literal::Bool(b) => {
                ValueLiteral::Bool(b.value)
            }
            Literal::Physical(phys) => {
                self.lower_physical_literal(phys)
            }
        }
    }

    /// Lower a physical literal.
    fn lower_physical_literal(&self, phys: &PhysicalLiteral) -> ValueLiteral {
        match phys {
            PhysicalLiteral::Quantity(q) => {
                ValueLiteral::Quantity(self.lower_quantity(q))
            }
            PhysicalLiteral::Range(r) => {
                ValueLiteral::Range {
                    from: self.lower_quantity(&r.from),
                    to: self.lower_quantity(&r.to),
                }
            }
            PhysicalLiteral::Bilateral(b) => {
                ValueLiteral::Bilateral {
                    base: self.lower_quantity(&b.base),
                    tolerance: self.lower_tolerance(&b.tolerance),
                }
            }
        }
    }

    /// Lower a quantity.
    fn lower_quantity(&self, q: &Quantity) -> QuantityValue {
        let value = self.parse_number(&q.number.value);
        let unit = q.unit.as_ref().map(|u| u.name.clone());
        QuantityValue::new(value, unit)
    }

    /// Lower a tolerance.
    fn lower_tolerance(&self, t: &Tolerance) -> ToleranceValue {
        let value = self.parse_tolerance_number(&t.value);
        if t.is_percent {
            ToleranceValue::percent(value)
        } else {
            let unit = t.unit.as_ref().map(|u| u.name.clone());
            ToleranceValue::absolute(value, unit)
        }
    }

    /// Lower a binary operator.
    fn lower_binary_op(&self, op: BinaryOp) -> ato_ir::BinaryOp {
        match op {
            BinaryOp::Add => ato_ir::BinaryOp::Add,
            BinaryOp::Sub => ato_ir::BinaryOp::Sub,
            BinaryOp::Mul => ato_ir::BinaryOp::Mul,
            BinaryOp::Div => ato_ir::BinaryOp::Div,
            BinaryOp::Power => ato_ir::BinaryOp::Power,
            BinaryOp::BitOr => ato_ir::BinaryOp::BitOr,
            BinaryOp::BitAnd => ato_ir::BinaryOp::BitAnd,
        }
    }

    /// Parse a number string to f64.
    fn parse_number(&self, s: &str) -> f64 {
        // Handle different number formats
        if s.starts_with("0x") || s.starts_with("0X") {
            i64::from_str_radix(&s[2..], 16).unwrap_or(0) as f64
        } else if s.starts_with("0b") || s.starts_with("0B") {
            i64::from_str_radix(&s[2..], 2).unwrap_or(0) as f64
        } else if s.starts_with("0o") || s.starts_with("0O") {
            i64::from_str_radix(&s[2..], 8).unwrap_or(0) as f64
        } else {
            s.parse::<f64>().unwrap_or(0.0)
        }
    }

    /// Parse a tolerance number (may be unsigned).
    fn parse_tolerance_number(&self, s: &str) -> f64 {
        s.parse::<f64>().unwrap_or(0.0)
    }

    /// Lower a for statement (expand the loop).
    fn lower_for_statement(&mut self, for_stmt: &ForStmt, module_id: ModuleId, scope: &mut Scope) {
        // Get the iterable range
        let iterations = match &for_stmt.iterable {
            Iterable::FieldRef { field, slice } => {
                self.get_iteration_range(field, slice.as_ref(), scope)
            }
            Iterable::List(refs) => {
                // For list iteration, we iterate over each element
                (0..refs.len() as u32).collect()
            }
        };

        // Expand the loop body for each iteration
        for _index in iterations {
            // Create a child scope with the loop variable bound
            let mut iteration_scope = scope.child();

            // The loop variable is bound based on the iterable
            // For now, we just track the index
            // In a full implementation, we'd bind to the actual array element

            // Lower the body statements
            for stmt in &for_stmt.body {
                self.lower_block_statement(stmt, module_id, &mut iteration_scope);
            }
        }
    }

    /// Get the iteration range for a for loop.
    fn get_iteration_range(&self, field: &FieldRef, slice: Option<&Slice>, scope: &Scope) -> Vec<u32> {
        // Resolve the field
        let path = self.lower_field_ref(field);
        let field_id = match self.resolve_field_path(&path, scope) {
            Some(id) => id,
            None => return vec![],
        };

        // Get the field's array size
        let count = if let Some(f) = self.design.get_field(field_id) {
            if let FieldKind::Instance { count: Some(c), .. } = &f.kind {
                *c
            } else {
                1
            }
        } else {
            1
        };

        // Apply slice if present
        if let Some(s) = slice {
            let start = s.start.as_ref()
                .and_then(|n| n.value.parse::<u32>().ok())
                .unwrap_or(0);
            let stop = s.stop.as_ref()
                .and_then(|n| n.value.parse::<u32>().ok())
                .unwrap_or(count);
            let step = s.step.as_ref()
                .and_then(|n| n.value.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1);

            (start..stop).step_by(step as usize).collect()
        } else {
            (0..count).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::NameResolver;

    fn lower(source: &str) -> (Design, Vec<SemaError>) {
        let ast = ato_parser::parse(source).unwrap();
        let mut design = Design::new();
        let mut scope = Scope::new();

        // Name resolution
        let mut name_resolver = NameResolver::new(&mut design);
        let _ = name_resolver.resolve(&ast, &mut scope);
        let mut errors = name_resolver.take_errors();

        // Lowering
        let mut lowerer = Lowerer::new(&mut design);
        let _ = lowerer.lower(&ast, &scope);
        errors.extend(lowerer.take_errors());

        (design, errors)
    }

    #[test]
    fn test_lower_connection() {
        let source = r#"
module M:
    pin p1
    pin p2
    p1 ~ p2
"#;
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 1);
    }

    #[test]
    fn test_lower_directed_connection() {
        let source = r#"
module M:
    pin p1
    pin p2
    pin p3
    p1 ~> p2 ~> p3
"#;
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 1);

        let conn = &design.connections()[0];
        assert!(conn.is_directed());
        assert_eq!(conn.endpoints.len(), 3);
    }

    #[test]
    fn test_lower_inline_signal() {
        let source = r#"
module M:
    pin p1
    p1 ~ signal gnd
"#;
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);

        // Should have created an inline signal field
        assert!(design.field_count() >= 2);
    }

    #[test]
    fn test_lower_assert() {
        let source = r#"
module M:
    resistance: ohm
    assert resistance > 0
"#;
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 1);
    }

    #[test]
    fn test_lower_assert_within() {
        let source = r#"
module M:
    voltage: V
    assert voltage within 3V to 3.6V
"#;
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 1);
    }

    #[test]
    fn test_lower_for_loop() {
        let source = r#"
module M:
    items = new Item[3]
    signal common
    for item in items:
        item ~ common
"#;
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);

        // Should have created 3 connections (one per iteration)
        assert_eq!(design.connection_count(), 3);
    }

    #[test]
    fn test_lower_quantity() {
        let source = r#"
module M:
    voltage: V = 3.3V
"#;
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_lower_bilateral() {
        let source = r#"
module M:
    resistance: ohm
    assert resistance within 10kohm +/- 5%
"#;
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 1);
    }
}
