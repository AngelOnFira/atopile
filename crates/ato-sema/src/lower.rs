//! AST to IR lowering.
//!
//! This module converts the parsed AST to the IR representation,
//! expanding for loops, instantiating templates, and resolving connections.

use crate::error::{ErrorCollector, SemaError};
use crate::scope::{Binding, Scope};
use ato_ir::{
    CompareOpKind as IrCompareOpKind, ConnectionEndpoint, ConstraintExpr,
    Design, FieldId, FieldKind, FieldPath, FieldPathPart, ModuleId,
    QuantityValue, ToleranceValue, ValueExpr, ValueLiteral,
};
use ato_parser::{
    Assignable, AssertStmt, Assignment, AssignTarget, BinaryOp, BlockDef, CompareOpKind, Comparison,
    Connectable, Connection, ConnectionDirection, CumAssignment, CumOperator, DirectedConnection,
    Expression, FieldRef, File, ForStmt, Iterable, Literal, PhysicalLiteral, PinDeclaration,
    PinName, Quantity, Retype, SetAssignment, SetOperator, Slice, Statement, Tolerance,
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
            Statement::CumAssignment(cum) => {
                self.lower_cum_assignment(cum, module_id, scope);
            }
            Statement::SetAssignment(set) => {
                self.lower_set_assignment(set, module_id, scope);
            }
            Statement::Retype(retype) => {
                self.lower_retype(retype, module_id, scope);
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
    }

    /// Lower a directed connection (a ~> b ~> c).
    fn lower_directed_connection(&mut self, conn: &DirectedConnection, module_id: ModuleId, scope: &Scope) {
        let elements: Vec<(ConnectionEndpoint, &Connectable)> = conn.elements
            .iter()
            .map(|e| (self.lower_connectable(e, module_id, scope), e))
            .collect();

        if elements.len() < 2 {
            return;
        }

        let is_forward = matches!(conn.direction, ConnectionDirection::Forward);

        let has_bridges = elements.iter().enumerate().any(|(i, (ep, _))| {
            i > 0 && i < elements.len() - 1 && self.is_bridge_element(ep)
        });

        if !has_bridges {
            let endpoints: Vec<ConnectionEndpoint> = elements.into_iter().map(|(ep, _)| ep).collect();
            let kind = match conn.direction {
                ConnectionDirection::Forward => {
                    ato_ir::ConnectionKind::Directed(ato_ir::ConnectionDirection::Forward)
                }
                ConnectionDirection::Backward => {
                    ato_ir::ConnectionKind::Directed(ato_ir::ConnectionDirection::Backward)
                }
            };
            self.design.add_directed_connection(module_id, endpoints, kind);
            return;
        }

        for i in 0..elements.len() - 1 {
            let (left_endpoint, _left_connectable) = &elements[i];
            let (right_endpoint, right_connectable) = &elements[i + 1];

            let left_is_bridge = i > 0 && self.is_bridge_element(left_endpoint);
            let right_is_bridge = (i + 1) < elements.len() - 1 && self.is_bridge_element(right_endpoint);

            let actual_left = if left_is_bridge {
                self.get_bridge_output_endpoint(left_endpoint, is_forward)
            } else {
                left_endpoint.clone()
            };

            let actual_right = if right_is_bridge {
                self.get_bridge_input_endpoint(right_endpoint, right_connectable, is_forward)
            } else {
                right_endpoint.clone()
            };

            self.design.add_connection(module_id, actual_left, actual_right);
        }
    }

    fn is_bridge_element(&self, endpoint: &ConnectionEndpoint) -> bool {
        if let Some(field_id) = endpoint.resolved {
            if let Some(field) = self.design.get_field(field_id) {
                if let FieldKind::Instance { resolved_type: Some(type_id), .. } = &field.kind {
                    if let Some(module) = self.design.get_module(*type_id) {
                        return module.traits.iter().any(|t| {
                            let name = t.name.name();
                            name == "can_bridge" || name == "can_bridge_by_name"
                        });
                    }
                }
            }
        }
        false
    }

    fn get_bridge_field_names(&self, endpoint: &ConnectionEndpoint) -> (String, String) {
        if let Some(field_id) = endpoint.resolved {
            if let Some(field) = self.design.get_field(field_id) {
                if let FieldKind::Instance { resolved_type: Some(type_id), .. } = &field.kind {
                    if let Some(module) = self.design.get_module(*type_id) {
                        for trait_ref in &module.traits {
                            if trait_ref.name.name() == "can_bridge_by_name" {
                                let input = trait_ref.get_string_arg("input_name")
                                    .unwrap_or("input");
                                let output = trait_ref.get_string_arg("output_name")
                                    .unwrap_or("output");
                                return (input.to_string(), output.to_string());
                            }
                        }
                        for trait_ref in &module.traits {
                            if trait_ref.name.name() == "can_bridge" {
                                return ("unnamed[0]".to_string(), "unnamed[1]".to_string());
                            }
                        }
                    }
                }
            }
        }
        ("unnamed[0]".to_string(), "unnamed[1]".to_string())
    }

    fn get_bridge_input_endpoint(&self, endpoint: &ConnectionEndpoint, _connectable: &Connectable, is_forward: bool) -> ConnectionEndpoint {
        let (input_name, output_name) = self.get_bridge_field_names(endpoint);
        let field_name = if is_forward { &input_name } else { &output_name };

        if let ato_ir::EndpointKind::FieldRef(base_path) = &endpoint.kind {
            let mut new_path = base_path.clone();
            if field_name.contains('[') {
                let parts: Vec<&str> = field_name.split('[').collect();
                let name = parts[0];
                let index: u32 = parts.get(1)
                    .and_then(|s| s.trim_end_matches(']').parse().ok())
                    .unwrap_or(0);
                new_path = new_path.append_indexed(name, index);
            } else {
                new_path = new_path.append(field_name);
            }
            ConnectionEndpoint {
                kind: ato_ir::EndpointKind::FieldRef(new_path),
                resolved: None,
            }
        } else {
            endpoint.clone()
        }
    }

    fn get_bridge_output_endpoint(&self, endpoint: &ConnectionEndpoint, is_forward: bool) -> ConnectionEndpoint {
        let (input_name, output_name) = self.get_bridge_field_names(endpoint);
        let field_name = if is_forward { &output_name } else { &input_name };

        if let ato_ir::EndpointKind::FieldRef(base_path) = &endpoint.kind {
            let mut new_path = base_path.clone();
            if field_name.contains('[') {
                let parts: Vec<&str> = field_name.split('[').collect();
                let name = parts[0];
                let index: u32 = parts.get(1)
                    .and_then(|s| s.trim_end_matches(']').parse().ok())
                    .unwrap_or(1);
                new_path = new_path.append_indexed(name, index);
            } else {
                new_path = new_path.append(field_name);
            }
            ConnectionEndpoint {
                kind: ato_ir::EndpointKind::FieldRef(new_path),
                resolved: None,
            }
        } else {
            endpoint.clone()
        }
    }

    fn lower_connectable(&mut self, connectable: &Connectable, module_id: ModuleId, scope: &Scope) -> ConnectionEndpoint {
        match connectable {
            Connectable::FieldRef(field_ref) => {
                let path = self.lower_field_ref(field_ref);
                let resolved = self.resolve_field_path(&path, scope);
                ConnectionEndpoint {
                    kind: ato_ir::EndpointKind::FieldRef(path),
                    resolved,
                }
            }
            Connectable::SignalDef(signal) => {
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
                let (name, kind) = self.pin_to_field_kind(pin);
                let field_id = self.design.add_field(module_id, &name, kind);
                if let Some(field) = self.design.get_field_mut(field_id) {
                    field.span = Some(pin.span);
                }
                ConnectionEndpoint::resolved(field_id)
            }
        }
    }

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

    fn lower_field_ref(&self, field_ref: &FieldRef) -> FieldPath {
        let mut parts = Vec::new();
        for part in &field_ref.parts {
            parts.push(FieldPathPart::Name(part.name.name.clone()));
            if let Some(index) = &part.index {
                let n = index.value.parse::<u32>().unwrap_or(0);
                parts.push(FieldPathPart::Index(n));
            }
        }
        if let Some(pin_ref) = &field_ref.pin_ref {
            let n = pin_ref.value.parse::<u32>().unwrap_or(0);
            parts.push(FieldPathPart::PinRef(n));
        }
        FieldPath::new(parts)
    }

    fn resolve_field_path(&self, path: &FieldPath, scope: &Scope) -> Option<FieldId> {
        let first_name = path.first_name()?;
        match scope.lookup(first_name)? {
            Binding::Field(id) => Some(*id),
            Binding::LoopVariable { source, .. } => Some(*source),
            _ => None,
        }
    }

    fn lower_assert(&mut self, assert_stmt: &AssertStmt, module_id: ModuleId, scope: &Scope) {
        let constraint_expr = self.lower_comparison(&assert_stmt.comparison, scope);
        let _constraint_id = self.design.create_constraint(module_id, constraint_expr);
    }

    /// Lower an assignment statement to a constraint (for physical values)
    /// or apply template args from `new` expressions.
    fn lower_assignment(&mut self, assign: &Assignment, module_id: ModuleId, _scope: &Scope) {
        // Handle template arguments from `new` expressions
        if let Assignable::New(new_expr) = &assign.value {
            if let Some(template) = &new_expr.template {
                let instance_path = match &assign.target {
                    AssignTarget::FieldRef(field_ref) => self.lower_field_ref(field_ref),
                    AssignTarget::Declaration(decl) => self.lower_field_ref(&decl.field),
                };
                for arg in &template.args {
                    let param_path = instance_path.append(&arg.name.name);
                    let value_literal = self.lower_literal(&arg.value);
                    let op_kind = match &arg.value {
                        Literal::Physical(PhysicalLiteral::Range(_)) => IrCompareOpKind::Within,
                        Literal::Physical(PhysicalLiteral::Bilateral(_)) => IrCompareOpKind::Within,
                        _ => IrCompareOpKind::Is,
                    };
                    let left = ValueExpr::field(param_path);
                    let right = ValueExpr::literal(value_literal);
                    let constraint_expr = ConstraintExpr::compare(left, op_kind, right);
                    self.design.create_constraint(module_id, constraint_expr);
                }
            }
            return;
        }

        let value_literal = match &assign.value {
            Assignable::Physical(phys) => self.lower_physical_literal(phys),
            Assignable::Arithmetic(expr) => {
                if let Expression::Literal(Literal::Physical(phys)) = expr {
                    self.lower_physical_literal(phys)
                } else {
                    return;
                }
            }
            _ => return,
        };

        let target_path = match &assign.target {
            AssignTarget::FieldRef(field_ref) => self.lower_field_ref(field_ref),
            AssignTarget::Declaration(decl) => self.lower_field_ref(&decl.field),
        };

        let op_kind = match &assign.value {
            Assignable::Physical(PhysicalLiteral::Quantity(_)) => IrCompareOpKind::Is,
            Assignable::Physical(PhysicalLiteral::Range(_)) => IrCompareOpKind::Within,
            Assignable::Physical(PhysicalLiteral::Bilateral(_)) => IrCompareOpKind::Within,
            _ => IrCompareOpKind::Is,
        };

        let left = ValueExpr::field(target_path);
        let right = ValueExpr::literal(value_literal);
        let constraint_expr = ConstraintExpr::compare(left, op_kind, right);
        self.design.create_constraint(module_id, constraint_expr);
    }

    /// Lower a cumulative assignment (`x += 5` or `x -= 3`).
    fn lower_cum_assignment(&mut self, cum: &CumAssignment, module_id: ModuleId, scope: &Scope) {
        let target_path = match &cum.target {
            AssignTarget::FieldRef(field_ref) => self.lower_field_ref(field_ref),
            AssignTarget::Declaration(decl) => self.lower_field_ref(&decl.field),
        };
        let target_expr = ValueExpr::field(target_path.clone());
        let value_expr = self.lower_expression(&cum.value, scope);
        let bin_op = match cum.operator {
            CumOperator::Add => ato_ir::BinaryOp::Add,
            CumOperator::Sub => ato_ir::BinaryOp::Sub,
        };
        let combined = ValueExpr::binary(target_expr, bin_op, value_expr);
        let left = ValueExpr::field(target_path);
        let constraint_expr = ConstraintExpr::compare(left, IrCompareOpKind::Is, combined);
        self.design.create_constraint(module_id, constraint_expr);
    }

    /// Lower a set assignment (`x |= mask` or `x &= mask`).
    fn lower_set_assignment(&mut self, set: &SetAssignment, module_id: ModuleId, scope: &Scope) {
        let target_path = match &set.target {
            AssignTarget::FieldRef(field_ref) => self.lower_field_ref(field_ref),
            AssignTarget::Declaration(decl) => self.lower_field_ref(&decl.field),
        };
        let target_expr = ValueExpr::field(target_path.clone());
        let value_expr = self.lower_expression(&set.value, scope);
        let bin_op = match set.operator {
            SetOperator::Or => ato_ir::BinaryOp::BitOr,
            SetOperator::And => ato_ir::BinaryOp::BitAnd,
        };
        let combined = ValueExpr::binary(target_expr, bin_op, value_expr);
        let left = ValueExpr::field(target_path);
        let constraint_expr = ConstraintExpr::compare(left, IrCompareOpKind::Is, combined);
        self.design.create_constraint(module_id, constraint_expr);
    }

    /// Lower a retype statement (`instance.field -> NewType`).
    fn lower_retype(&mut self, retype: &Retype, _module_id: ModuleId, scope: &Scope) {
        let field_path = self.lower_field_ref(&retype.field);
        if let Some(field_id) = self.resolve_field_path(&field_path, scope) {
            let type_name = retype.new_type.parts.last()
                .map(|p| p.name.as_str())
                .unwrap_or("");
            if let Some(binding) = scope.lookup(type_name) {
                if let Some(type_module_id) = binding.as_module() {
                    if let Some(field) = self.design.get_field_mut(field_id) {
                        if let FieldKind::Instance { resolved_type, .. } = &mut field.kind {
                            *resolved_type = Some(type_module_id);
                        }
                    }
                }
            }
        }
    }

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
                ValueExpr::literal(ValueLiteral::Bool(false))
            }
        }
    }

    fn lower_literal(&self, lit: &Literal) -> ValueLiteral {
        match lit {
            Literal::Number(num) => {
                let value = self.parse_number(&num.value);
                ValueLiteral::Quantity(QuantityValue::dimensionless(value))
            }
            Literal::String(s) => ValueLiteral::String(s.value.clone()),
            Literal::Bool(b) => ValueLiteral::Bool(b.value),
            Literal::Physical(phys) => self.lower_physical_literal(phys),
        }
    }

    fn lower_physical_literal(&self, phys: &PhysicalLiteral) -> ValueLiteral {
        match phys {
            PhysicalLiteral::Quantity(q) => ValueLiteral::Quantity(self.lower_quantity(q)),
            PhysicalLiteral::Range(r) => ValueLiteral::Range {
                from: self.lower_quantity(&r.from),
                to: self.lower_quantity(&r.to),
            },
            PhysicalLiteral::Bilateral(b) => ValueLiteral::Bilateral {
                base: self.lower_quantity(&b.base),
                tolerance: self.lower_tolerance(&b.tolerance),
            },
        }
    }

    fn lower_quantity(&self, q: &Quantity) -> QuantityValue {
        let value = self.parse_number(&q.number.value);
        let unit = q.unit.as_ref().map(|u| u.name.clone());
        QuantityValue::new(value, unit)
    }

    fn lower_tolerance(&self, t: &Tolerance) -> ToleranceValue {
        let value = self.parse_tolerance_number(&t.value);
        if t.is_percent {
            ToleranceValue::percent(value)
        } else {
            let unit = t.unit.as_ref().map(|u| u.name.clone());
            ToleranceValue::absolute(value, unit)
        }
    }

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

    fn parse_number(&self, s: &str) -> f64 {
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

    fn parse_tolerance_number(&self, s: &str) -> f64 {
        s.parse::<f64>().unwrap_or(0.0)
    }

    /// Lower a for statement (expand the loop).
    fn lower_for_statement(&mut self, for_stmt: &ForStmt, module_id: ModuleId, scope: &mut Scope) {
        let (iterations, iterable_path) = match &for_stmt.iterable {
            Iterable::FieldRef { field, slice } => {
                let iterations = self.get_iteration_range(field, slice.as_ref(), scope);
                let path = self.lower_field_ref(field);
                (iterations, Some(path))
            }
            Iterable::List(refs) => {
                ((0..refs.len() as u32).collect(), None)
            }
        };

        let loop_var_name = &for_stmt.variable.name;

        for index in iterations {
            let mut iteration_scope = scope.child();

            match &for_stmt.iterable {
                Iterable::FieldRef { field, .. } => {
                    let path = self.lower_field_ref(field);
                    if let Some(field_id) = self.resolve_field_path(&path, scope) {
                        iteration_scope.define(
                            loop_var_name.clone(),
                            Binding::LoopVariable { source: field_id, index: Some(index) },
                            None,
                        );
                    }
                }
                Iterable::List(refs) => {
                    if let Some(field_ref) = refs.get(index as usize) {
                        let path = self.lower_field_ref(field_ref);
                        if let Some(field_id) = self.resolve_field_path(&path, scope) {
                            iteration_scope.define(
                                loop_var_name.clone(),
                                Binding::Field(field_id),
                                None,
                            );
                        }
                    }
                }
            }

            for stmt in &for_stmt.body {
                self.lower_for_body_statement(
                    stmt, module_id, &mut iteration_scope,
                    loop_var_name, iterable_path.as_ref(), index,
                );
            }
        }
    }

    fn lower_for_body_statement(
        &mut self, stmt: &Statement, module_id: ModuleId, scope: &mut Scope,
        loop_var: &str, iterable_path: Option<&FieldPath>, index: u32,
    ) {
        match stmt {
            Statement::Connection(conn) => {
                let left = self.lower_connectable_with_loop_var(&conn.left, module_id, scope, loop_var, iterable_path, index);
                let right = self.lower_connectable_with_loop_var(&conn.right, module_id, scope, loop_var, iterable_path, index);
                self.design.add_connection(module_id, left, right);
            }
            Statement::Assignment(assign) => {
                self.lower_assignment_with_loop_var(assign, module_id, scope, loop_var, iterable_path, index);
            }
            Statement::Assert(assert_stmt) => {
                self.lower_assert(assert_stmt, module_id, scope);
            }
            _ => {
                self.lower_block_statement(stmt, module_id, scope);
            }
        }
    }

    fn rewrite_loop_var_path(
        &self, path: &FieldPath, loop_var: &str,
        iterable_path: Option<&FieldPath>, index: u32,
    ) -> FieldPath {
        if let Some(first_name) = path.first_name() {
            if first_name == loop_var {
                if let Some(base) = iterable_path {
                    let mut new_parts = base.parts.clone();
                    new_parts.push(FieldPathPart::Index(index));
                    if path.parts.len() > 1 {
                        new_parts.extend_from_slice(&path.parts[1..]);
                    }
                    return FieldPath::new(new_parts);
                }
            }
        }
        path.clone()
    }

    fn lower_connectable_with_loop_var(
        &mut self, connectable: &Connectable, module_id: ModuleId, scope: &Scope,
        loop_var: &str, iterable_path: Option<&FieldPath>, index: u32,
    ) -> ConnectionEndpoint {
        match connectable {
            Connectable::FieldRef(field_ref) => {
                let path = self.lower_field_ref(field_ref);
                let path = self.rewrite_loop_var_path(&path, loop_var, iterable_path, index);
                let resolved = self.resolve_field_path(&path, scope);
                ConnectionEndpoint {
                    kind: ato_ir::EndpointKind::FieldRef(path),
                    resolved,
                }
            }
            other => self.lower_connectable(other, module_id, scope),
        }
    }

    fn lower_assignment_with_loop_var(
        &mut self, assign: &Assignment, module_id: ModuleId, scope: &Scope,
        loop_var: &str, iterable_path: Option<&FieldPath>, index: u32,
    ) {
        if let Assignable::New(new_expr) = &assign.value {
            if let Some(template) = &new_expr.template {
                let instance_path = match &assign.target {
                    AssignTarget::FieldRef(field_ref) => self.lower_field_ref(field_ref),
                    AssignTarget::Declaration(decl) => self.lower_field_ref(&decl.field),
                };
                let instance_path = self.rewrite_loop_var_path(&instance_path, loop_var, iterable_path, index);
                for arg in &template.args {
                    let param_path = instance_path.append(&arg.name.name);
                    let value_literal = self.lower_literal(&arg.value);
                    let op_kind = match &arg.value {
                        Literal::Physical(PhysicalLiteral::Range(_)) => IrCompareOpKind::Within,
                        Literal::Physical(PhysicalLiteral::Bilateral(_)) => IrCompareOpKind::Within,
                        _ => IrCompareOpKind::Is,
                    };
                    let left = ValueExpr::field(param_path);
                    let right = ValueExpr::literal(value_literal);
                    self.design.create_constraint(module_id, ConstraintExpr::compare(left, op_kind, right));
                }
            }
            return;
        }

        let value_literal = match &assign.value {
            Assignable::Physical(phys) => self.lower_physical_literal(phys),
            Assignable::Arithmetic(expr) => {
                if let Expression::Literal(Literal::Physical(phys)) = expr {
                    self.lower_physical_literal(phys)
                } else { return; }
            }
            _ => return,
        };

        let target_path = match &assign.target {
            AssignTarget::FieldRef(field_ref) => self.lower_field_ref(field_ref),
            AssignTarget::Declaration(decl) => self.lower_field_ref(&decl.field),
        };
        let target_path = self.rewrite_loop_var_path(&target_path, loop_var, iterable_path, index);

        let op_kind = match &assign.value {
            Assignable::Physical(PhysicalLiteral::Quantity(_)) => IrCompareOpKind::Is,
            Assignable::Physical(PhysicalLiteral::Range(_)) => IrCompareOpKind::Within,
            Assignable::Physical(PhysicalLiteral::Bilateral(_)) => IrCompareOpKind::Within,
            _ => IrCompareOpKind::Is,
        };

        let left = ValueExpr::field(target_path);
        let right = ValueExpr::literal(value_literal);
        self.design.create_constraint(module_id, ConstraintExpr::compare(left, op_kind, right));
    }

    fn get_iteration_range(&self, field: &FieldRef, slice: Option<&Slice>, scope: &Scope) -> Vec<u32> {
        let path = self.lower_field_ref(field);
        let field_id = match self.resolve_field_path(&path, scope) {
            Some(id) => id,
            None => return vec![],
        };
        let count = if let Some(f) = self.design.get_field(field_id) {
            if let FieldKind::Instance { count: Some(c), .. } = &f.kind { *c } else { 1 }
        } else { 1 };

        if let Some(s) = slice {
            let start = s.start.as_ref().and_then(|n| n.value.parse::<u32>().ok()).unwrap_or(0);
            let stop = s.stop.as_ref().and_then(|n| n.value.parse::<u32>().ok()).unwrap_or(count);
            let step = s.step.as_ref().and_then(|n| n.value.parse::<u32>().ok()).unwrap_or(1).max(1);
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

    fn format_field_path(path: &FieldPath) -> String {
        path.parts
            .iter()
            .map(|part| match part {
                FieldPathPart::Name(n) => n.clone(),
                FieldPathPart::Index(i) => format!("[{}]", i),
                FieldPathPart::PinRef(p) => format!(".{}", p),
            })
            .collect::<Vec<_>>()
            .join(".")
            .replace(".[", "[")
    }

    fn lower(source: &str) -> (Design, Vec<SemaError>) {
        let ast = ato_parser::parse(source).unwrap();
        let mut design = Design::new();
        let mut scope = Scope::new();
        let mut name_resolver = NameResolver::new(&mut design);
        let _ = name_resolver.resolve(&ast, &mut scope);
        let mut errors = name_resolver.take_errors();
        let mut lowerer = Lowerer::new(&mut design);
        let _ = lowerer.lower(&ast, &scope);
        errors.extend(lowerer.take_errors());
        (design, errors)
    }

    #[test]
    fn test_lower_connection() {
        let source = "module M:\n    pin p1\n    pin p2\n    p1 ~ p2\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 1);
    }

    #[test]
    fn test_lower_directed_connection() {
        let source = "module M:\n    pin p1\n    pin p2\n    pin p3\n    p1 ~> p2 ~> p3\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 1);
        let conn = &design.connections()[0];
        assert!(conn.is_directed());
        assert_eq!(conn.endpoints.len(), 3);
    }

    #[test]
    fn test_lower_inline_signal() {
        let source = "module M:\n    pin p1\n    p1 ~ signal gnd\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert!(design.field_count() >= 2);
    }

    #[test]
    fn test_lower_assert() {
        let source = "module M:\n    resistance: ohm\n    assert resistance > 0\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 1);
    }

    #[test]
    fn test_lower_assert_within() {
        let source = "module M:\n    voltage: V\n    assert voltage within 3V to 3.6V\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 1);
    }

    #[test]
    fn test_lower_for_loop() {
        let source = "module M:\n    items = new Item[3]\n    signal common\n    for item in items:\n        item ~ common\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 3);
    }

    #[test]
    fn test_lower_quantity() {
        let source = "module M:\n    voltage: V = 3.3V\n";
        let (_design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_lower_bilateral() {
        let source = "module M:\n    resistance: ohm\n    assert resistance within 10kohm +/- 5%\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 1);
    }

    #[test]
    fn test_can_bridge_trait_expansion() {
        let source = "#pragma experiment(\"BRIDGE_CONNECT\")\n#pragma experiment(\"TRAITS\")\ninterface Electrical:\n    pass\n\nmodule Resistor:\n    unnamed = new Electrical[2]\n    trait can_bridge\n\nmodule App:\n    signal input\n    signal output\n    r = new Resistor\n    input ~> r ~> output\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 2, "Expected 2 connections for bridge expansion");
    }

    #[test]
    fn test_can_bridge_by_name_trait_expansion() {
        let source = "#pragma experiment(\"BRIDGE_CONNECT\")\n#pragma experiment(\"TRAITS\")\ninterface Electrical:\n    pass\n\nmodule Button:\n    in_field = new Electrical\n    out_field = new Electrical\n    trait can_bridge_by_name<input_name=\"in_field\", output_name=\"out_field\">\n\nmodule App:\n    signal a\n    signal b\n    btn = new Button\n    a ~> btn ~> b\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 2, "Expected 2 connections for bridge expansion");
    }

    #[test]
    fn test_directed_connection_without_bridge() {
        let source = "#pragma experiment(\"BRIDGE_CONNECT\")\nmodule M:\n    pin p1\n    pin p2\n    pin p3\n    p1 ~> p2 ~> p3\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 1);
        let conn = &design.connections()[0];
        assert!(conn.is_directed());
        assert_eq!(conn.endpoints.len(), 3);
    }

    #[test]
    fn test_chained_bridges() {
        let source = "#pragma experiment(\"BRIDGE_CONNECT\")\n#pragma experiment(\"TRAITS\")\ninterface Electrical:\n    pass\n\nmodule Resistor:\n    unnamed = new Electrical[2]\n    trait can_bridge\n\nmodule App:\n    signal input\n    signal output\n    r1 = new Resistor\n    r2 = new Resistor\n    input ~> r1 ~> r2 ~> output\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 3, "Expected 3 connections for chained bridges");
    }

    #[test]
    fn test_template_instantiation_creates_constraint() {
        let source = "#pragma experiment(\"MODULE_TEMPLATING\")\nmodule Resistor:\n    resistance: ohm\n\nmodule App:\n    r1 = new Resistor<resistance=10000>\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert!(design.constraint_count() > 0, "Template instantiation should create constraint");
    }

    #[test]
    fn test_template_instantiation_multiple_args() {
        let source = "#pragma experiment(\"MODULE_TEMPLATING\")\nmodule Resistor:\n    resistance: ohm\n    max_power: W\n\nmodule App:\n    r1 = new Resistor<resistance=10000, max_power=250>\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 2, "Two template args should create 2 constraints");
    }

    #[test]
    fn test_for_loop_connection_paths_are_indexed() {
        let source = "module M:\n    items = new Item[3]\n    signal common\n    for item in items:\n        item ~ common\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.connection_count(), 3);
        for (i, conn) in design.connections().iter().enumerate() {
            let left_path = match &conn.endpoints[0].kind {
                ato_ir::EndpointKind::FieldRef(path) => format_field_path(path),
                _ => String::new(),
            };
            let expected = format!("items[{}]", i);
            assert!(left_path.contains(&expected),
                "Connection {} left path should contain '{}', got '{}'", i, expected, left_path);
        }
    }

    #[test]
    fn test_for_loop_assignment_creates_indexed_constraints() {
        let source = "module M:\n    caps = new Capacitor[3]\n    for cap in caps:\n        cap.capacitance = 100nF\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 3, "For loop with 3 iterations should create 3 constraints");
    }

    #[test]
    fn test_cum_assignment_add() {
        let source = "module M:\n    value: ohm\n    value = 10ohm\n    value += 5\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 2, "Should have constraint from assignment and cum assignment");
    }

    #[test]
    fn test_set_assignment_or() {
        let source = "module M:\n    flags: dimensionless\n    flags = 0\n    flags |= 1\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert_eq!(design.constraint_count(), 2, "Should have constraint from assignment and set assignment");
    }

    #[test]
    fn test_retype_statement() {
        let source = "module Base:\n    pass\n\nmodule Derived from Base:\n    pass\n\nmodule App:\n    inst = new Base\n    inst -> Derived\n";
        let (design, errors) = lower(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        let app = design.modules().iter().find(|m| m.name == "App").unwrap();
        let inst_id = app.get_field("inst").expect("inst field should exist");
        let inst_field = design.get_field(inst_id).unwrap();
        if let FieldKind::Instance { resolved_type, .. } = &inst_field.kind {
            let derived_id = design.find_module("Derived").unwrap();
            assert_eq!(*resolved_type, Some(derived_id), "Retype should update resolved type to Derived");
        } else {
            panic!("Expected instance field");
        }
    }
}
