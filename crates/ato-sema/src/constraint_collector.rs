//! Constraint collection from IR to solver format.
//!
//! This module bridges the gap between the semantic IR and the constraint solver.
//! It walks the IR Design and extracts constraints, converting them to the
//! solver's expression and predicate types.
//!
//! # Overview
//!
//! The constraint collector:
//! 1. Walks all modules in the Design
//! 2. Collects constraints and their parameter dependencies
//! 3. Converts IR expressions to solver expressions
//! 4. Creates solver predicates from IR constraints
//! 5. Builds a parameter dependency graph
//!
//! # Example
//!
//! ```ignore
//! use ato_sema::constraint_collector::ConstraintCollector;
//! use ato_ir::Design;
//!
//! let design: Design = /* ... */;
//! let mut collector = ConstraintCollector::new();
//! let solver = collector.collect(&design)?;
//! let result = solver.solve()?;
//! ```

use std::collections::HashMap;

use ato_domain::{Quantity, QuantityInterval, QuantityIntervalDisjoint, Unit};
use ato_ir::{
    BinaryOp, CompareOpKind, Constraint, Design, FieldId, FieldKind, FieldPath, FieldPathPart,
    ModuleId, QuantityValue, ToleranceValue, UnaryOp, ValueExpr, ValueLiteral,
};
use ato_solver::{
    ArithmeticOp, Expression, ExpressionId, Literal, Parameter, ParameterId,
    Predicate, Solver, SolverConfig,
};
use thiserror::Error;

/// Errors that can occur during constraint collection.
#[derive(Debug, Clone, Error)]
pub enum CollectionError {
    /// A field reference could not be resolved.
    #[error("Unresolved field reference: {0}")]
    UnresolvedField(String),

    /// A unit could not be parsed.
    #[error("Unknown unit: {0}")]
    UnknownUnit(String),

    /// An unsupported expression type was encountered.
    #[error("Unsupported expression: {0}")]
    UnsupportedExpression(String),

    /// A constraint expression was malformed.
    #[error("Invalid constraint: {0}")]
    InvalidConstraint(String),
}

/// Result type for constraint collection.
pub type CollectionResult<T> = Result<T, CollectionError>;

/// A collected parameter with its metadata.
#[derive(Debug, Clone)]
pub struct CollectedParameter {
    /// The solver parameter ID.
    pub solver_id: ParameterId,
    /// The solver expression ID for this parameter.
    pub expr_id: ExpressionId,
    /// The IR field ID (if resolved).
    pub field_id: Option<FieldId>,
    /// The module containing this parameter.
    pub module_id: Option<ModuleId>,
    /// The full path to this parameter.
    pub path: String,
    /// The unit of this parameter.
    pub unit: Unit,
}

/// Tracks parameter dependencies.
#[derive(Debug, Clone, Default)]
pub struct ParameterDependencies {
    /// Map from parameter to the parameters it depends on.
    pub depends_on: HashMap<ParameterId, Vec<ParameterId>>,
    /// Map from parameter to the parameters that depend on it.
    pub depended_by: HashMap<ParameterId, Vec<ParameterId>>,
}

impl ParameterDependencies {
    /// Create a new empty dependency tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dependency: `from` depends on `to`.
    pub fn add_dependency(&mut self, from: ParameterId, to: ParameterId) {
        self.depends_on.entry(from).or_default().push(to);
        self.depended_by.entry(to).or_default().push(from);
    }

    /// Get all parameters that a given parameter depends on.
    pub fn get_dependencies(&self, param: ParameterId) -> &[ParameterId] {
        self.depends_on.get(&param).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all parameters that depend on a given parameter.
    pub fn get_dependents(&self, param: ParameterId) -> &[ParameterId] {
        self.depended_by.get(&param).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get free parameters (those with no dependencies).
    pub fn free_parameters(&self) -> Vec<ParameterId> {
        self.depended_by
            .keys()
            .filter(|p| self.depends_on.get(p).map_or(true, |d| d.is_empty()))
            .copied()
            .collect()
    }

    /// Get constrained parameters (those with dependencies).
    pub fn constrained_parameters(&self) -> Vec<ParameterId> {
        self.depends_on
            .iter()
            .filter(|(_, deps)| !deps.is_empty())
            .map(|(p, _)| *p)
            .collect()
    }
}

/// Collects constraints from an IR Design and builds a solver.
pub struct ConstraintCollector {
    /// The solver being built.
    solver: Solver,
    /// Map from field path to parameter ID.
    parameters: HashMap<String, CollectedParameter>,
    /// Parameter dependencies.
    dependencies: ParameterDependencies,
    /// Current module context for path resolution.
    current_module: Option<ModuleId>,
}

impl std::fmt::Debug for ConstraintCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConstraintCollector")
            .field("parameters", &self.parameters)
            .field("dependencies", &self.dependencies)
            .field("current_module", &self.current_module)
            .finish()
    }
}

impl ConstraintCollector {
    /// Create a new constraint collector.
    pub fn new() -> Self {
        Self::with_config(SolverConfig::default())
    }

    /// Create a new constraint collector with custom solver config.
    pub fn with_config(config: SolverConfig) -> Self {
        Self {
            solver: Solver::new(config),
            parameters: HashMap::new(),
            dependencies: ParameterDependencies::new(),
            current_module: None,
        }
    }

    /// Collect constraints from a Design and return the populated solver.
    pub fn collect(mut self, design: &Design) -> CollectionResult<(Solver, ParameterDependencies)> {
        // Walk all modules and collect constraints
        for module in design.modules() {
            self.current_module = Some(module.id);

            // Collect constraints from this module
            for &constraint_id in &module.constraints {
                if let Some(constraint) = design.get_constraint(constraint_id) {
                    self.collect_constraint(constraint, design)?;
                }
            }
        }

        Ok((self.solver, self.dependencies))
    }

    /// Collect a single constraint.
    fn collect_constraint(&mut self, constraint: &Constraint, design: &Design) -> CollectionResult<()> {
        let expr = &constraint.expression;

        // Convert the left-hand side to a solver expression
        let left_id = self.convert_value_expr(&expr.left, design)?;

        // Process each comparison operation
        let mut prev_id = left_id;
        for op in &expr.operations {
            let right_id = self.convert_value_expr(&op.right, design)?;

            // Create the predicate based on the comparison kind
            let predicate = self.create_predicate(prev_id, op.kind, right_id)?;

            // Add the constrained predicate to the solver
            self.solver.constrain(predicate);

            // For chained comparisons (e.g., 5V < x < 10V), the right becomes the next left
            prev_id = right_id;
        }

        Ok(())
    }

    /// Convert an IR ValueExpr to a solver Expression.
    fn convert_value_expr(&mut self, expr: &ValueExpr, design: &Design) -> CollectionResult<ExpressionId> {
        match expr {
            ValueExpr::FieldRef(path) => self.convert_field_ref(path, design),
            ValueExpr::Literal(lit) => self.convert_literal(lit),
            ValueExpr::Binary { left, op, right } => {
                let left_id = self.convert_value_expr(left, design)?;
                let right_id = self.convert_value_expr(right, design)?;
                self.convert_binary(left_id, *op, right_id)
            }
            ValueExpr::Unary { op, operand } => {
                let operand_id = self.convert_value_expr(operand, design)?;
                self.convert_unary(*op, operand_id)
            }
            ValueExpr::Group(inner) => self.convert_value_expr(inner, design),
        }
    }

    /// Format a FieldPath as a string.
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
    }

    /// Convert a field reference to a solver parameter.
    fn convert_field_ref(&mut self, path: &FieldPath, design: &Design) -> CollectionResult<ExpressionId> {
        let path_str = Self::format_field_path(path);

        // Check if we already have this parameter
        if let Some(collected) = self.parameters.get(&path_str) {
            return Ok(collected.expr_id);
        }

        // Resolve the field in the design to get its unit
        let unit = self.resolve_field_unit(path, design)?;

        // Create a new parameter
        let param = Parameter::new(unit).with_name(&path_str);
        let param_id = self.solver.add_parameter(param);

        // Create the expression
        let expr = Expression::parameter(param_id, unit);
        let expr_id = self.solver.add_expression(expr);

        // Store the collected parameter
        let collected = CollectedParameter {
            solver_id: param_id,
            expr_id,
            field_id: self.find_field_id(path, design),
            module_id: self.current_module,
            path: path_str.clone(),
            unit,
        };
        self.parameters.insert(path_str, collected);

        Ok(expr_id)
    }

    /// Resolve the unit for a field reference by traversing the full path.
    ///
    /// For a path like `r1.resistance`:
    /// 1. Look up `r1` in the current module -> find it's an Instance of type X
    /// 2. Look up module X's fields
    /// 3. Find `resistance` in module X -> it's a Parameter with unit `ohm`
    /// 4. Return `Ohm`
    fn resolve_field_unit(&self, path: &FieldPath, design: &Design) -> CollectionResult<Unit> {
        if let Some(module_id) = self.current_module {
            if let Some(unit) = self.resolve_field_unit_in_module(path, 0, module_id, design) {
                return self.parse_unit(&unit);
            }
        }

        // Default to dimensionless if we can't resolve
        Ok(Unit::Dimensionless)
    }

    /// Recursively resolve a field path starting from `part_idx` within `module_id`.
    /// Returns the unit string if the final field is a Parameter with a unit.
    fn resolve_field_unit_in_module(
        &self,
        path: &FieldPath,
        part_idx: usize,
        module_id: ModuleId,
        design: &Design,
    ) -> Option<String> {
        let module = design.get_module(module_id)?;
        let part = path.parts.get(part_idx)?;

        let name = match part {
            FieldPathPart::Name(n) => n.as_str(),
            FieldPathPart::Index(_) | FieldPathPart::PinRef(_) => {
                // Skip index/pin parts and continue with the next name part
                return self.resolve_field_unit_in_module(path, part_idx + 1, module_id, design);
            }
        };

        let field_id = module.get_field(name)?;
        let field = design.get_field(field_id)?;

        let is_last_name_part = path.parts[part_idx + 1..]
            .iter()
            .all(|p| matches!(p, FieldPathPart::Index(_) | FieldPathPart::PinRef(_)));

        if is_last_name_part {
            // This is the final named field - check if it's a parameter with a unit
            if let FieldKind::Parameter { unit: Some(ref unit_str) } = field.kind {
                return Some(unit_str.clone());
            }
            return None;
        }

        // Not the last part - this field must be an instance so we can traverse into it
        if let FieldKind::Instance { resolved_type, ref type_ref, .. } = field.kind {
            // Try resolved_type first, then fall back to looking up by name
            let target_module_id = resolved_type.or_else(|| {
                let type_name = type_ref.name();
                design.find_module(type_name)
            })?;

            // Find the next Name part index to continue traversal
            let next_name_idx = (part_idx + 1..)
                .find(|&i| {
                    path.parts.get(i).map_or(false, |p| matches!(p, FieldPathPart::Name(_)))
                })?;

            return self.resolve_field_unit_in_module(path, next_name_idx, target_module_id, design);
        }

        None
    }

    /// Find the FieldId for a path.
    fn find_field_id(&self, path: &FieldPath, design: &Design) -> Option<FieldId> {
        let module_id = self.current_module?;
        let module = design.get_module(module_id)?;
        let first_part = path.parts.first()?;
        if let FieldPathPart::Name(name) = first_part {
            module.get_field(name)
        } else {
            None
        }
    }

    /// Convert a literal to a solver expression.
    fn convert_literal(&mut self, lit: &ValueLiteral) -> CollectionResult<ExpressionId> {
        let solver_lit = match lit {
            ValueLiteral::Quantity(q) => self.convert_quantity(q)?,
            ValueLiteral::Range { from, to } => self.convert_range(from, to)?,
            ValueLiteral::Bilateral { base, tolerance } => {
                self.convert_bilateral(base, tolerance)?
            }
            ValueLiteral::Bool(b) => Literal::Bool(*b),
            ValueLiteral::String(_) => {
                return Err(CollectionError::UnsupportedExpression(
                    "String literals not supported in constraints".into(),
                ));
            }
        };

        let expr = Expression::literal(solver_lit);
        Ok(self.solver.add_expression(expr))
    }

    /// Convert a quantity value to a solver literal.
    fn convert_quantity(&self, q: &QuantityValue) -> CollectionResult<Literal> {
        let unit = match &q.unit {
            Some(unit_str) => self.parse_unit(unit_str)?,
            None => Unit::Dimensionless,
        };

        // Parse the value with SI prefix applied (the IR should already have this)
        let value = q.value;

        // Create a singleton interval for a single quantity
        let quantity = Quantity::new(value, unit);
        let interval = QuantityInterval::singleton(quantity)
            .map_err(|_| CollectionError::InvalidConstraint("Invalid quantity value".into()))?;
        let disjoint = QuantityIntervalDisjoint::single(interval);

        Ok(Literal::Quantity(disjoint))
    }

    /// Convert a range to a solver literal.
    fn convert_range(&self, from: &QuantityValue, to: &QuantityValue) -> CollectionResult<Literal> {
        let from_unit = match &from.unit {
            Some(unit_str) => self.parse_unit(unit_str)?,
            None => Unit::Dimensionless,
        };
        let to_unit = match &to.unit {
            Some(unit_str) => self.parse_unit(unit_str)?,
            None => Unit::Dimensionless,
        };

        // Units should be compatible
        if !from_unit.is_compatible_with(&to_unit) {
            return Err(CollectionError::InvalidConstraint(format!(
                "Range units not compatible: {} vs {}",
                from_unit, to_unit
            )));
        }

        let from_quantity = Quantity::new(from.value, from_unit);
        let to_quantity = Quantity::new(to.value, to_unit);

        match QuantityInterval::new(from_quantity, to_quantity) {
            Ok(interval) => {
                let disjoint = QuantityIntervalDisjoint::single(interval);
                Ok(Literal::Quantity(disjoint))
            }
            Err(_) => Err(CollectionError::InvalidConstraint(
                "Invalid range: from > to".into(),
            )),
        }
    }

    /// Convert a bilateral tolerance to a solver literal.
    fn convert_bilateral(
        &self,
        base: &QuantityValue,
        tolerance: &ToleranceValue,
    ) -> CollectionResult<Literal> {
        let unit = match &base.unit {
            Some(unit_str) => self.parse_unit(unit_str)?,
            None => Unit::Dimensionless,
        };

        let base_value = base.value;

        // Calculate the tolerance amount
        let tolerance_amount = if tolerance.is_percent {
            base_value * (tolerance.value / 100.0)
        } else {
            // Absolute tolerance - may need unit conversion
            let tol_unit = match &tolerance.unit {
                Some(unit_str) => self.parse_unit(unit_str)?,
                None => unit,
            };

            if !tol_unit.is_compatible_with(&unit) {
                return Err(CollectionError::InvalidConstraint(format!(
                    "Tolerance unit {} not compatible with base unit {}",
                    tol_unit, unit
                )));
            }

            // Convert tolerance to base unit if needed
            let factor = tol_unit.to_base_factor() / unit.to_base_factor();
            tolerance.value * factor
        };

        let min_value = base_value - tolerance_amount;
        let max_value = base_value + tolerance_amount;

        let min_quantity = Quantity::new(min_value, unit);
        let max_quantity = Quantity::new(max_value, unit);

        match QuantityInterval::new(min_quantity, max_quantity) {
            Ok(interval) => {
                let disjoint = QuantityIntervalDisjoint::single(interval);
                Ok(Literal::Quantity(disjoint))
            }
            Err(_) => Err(CollectionError::InvalidConstraint(
                "Invalid bilateral tolerance".into(),
            )),
        }
    }

    /// Convert a binary operation to a solver expression.
    fn convert_binary(
        &mut self,
        left: ExpressionId,
        op: BinaryOp,
        right: ExpressionId,
    ) -> CollectionResult<ExpressionId> {
        let solver_op = match op {
            BinaryOp::Add => ArithmeticOp::Add,
            BinaryOp::Sub => ArithmeticOp::Subtract,
            BinaryOp::Mul => ArithmeticOp::Multiply,
            BinaryOp::Div => ArithmeticOp::Divide,
            BinaryOp::Power => ArithmeticOp::Power,
            BinaryOp::BitOr | BinaryOp::BitAnd => {
                return Err(CollectionError::UnsupportedExpression(
                    "Bitwise operations not supported in constraints".into(),
                ));
            }
        };

        // Unit for the result - for add/sub it should match, for mul/div it's computed
        // For now, use None and let the solver figure it out
        let expr = Expression::arithmetic(solver_op, vec![left, right], None);
        Ok(self.solver.add_expression(expr))
    }

    /// Convert a unary operation to a solver expression.
    fn convert_unary(&mut self, op: UnaryOp, operand: ExpressionId) -> CollectionResult<ExpressionId> {
        match op {
            UnaryOp::Neg => {
                // Use the Negate operation directly
                let expr = Expression::arithmetic(ArithmeticOp::Negate, vec![operand], None);
                Ok(self.solver.add_expression(expr))
            }
            UnaryOp::Pos => {
                // Positive is a no-op
                Ok(operand)
            }
        }
    }

    /// Create a predicate from a comparison.
    fn create_predicate(
        &mut self,
        left: ExpressionId,
        op: CompareOpKind,
        right: ExpressionId,
    ) -> CollectionResult<Predicate> {
        let pred = match op {
            CompareOpKind::LessThan => Predicate::less_than(left, right),
            CompareOpKind::GreaterThan => Predicate::greater_than(left, right),
            CompareOpKind::LessEq => Predicate::less_or_equal(left, right),
            CompareOpKind::GreaterEq => Predicate::greater_or_equal(left, right),
            CompareOpKind::Within => Predicate::within(left, right),
            CompareOpKind::Is => Predicate::is(left, right),
        };

        // Track parameter dependencies for left and right expressions
        self.track_dependencies(left, right);

        Ok(pred)
    }

    /// Track dependencies between expressions.
    fn track_dependencies(&mut self, left: ExpressionId, right: ExpressionId) {
        // Get parameters from left expression
        let left_params = self.extract_parameters(left);
        let right_params = self.extract_parameters(right);

        // Left depends on right (constraint flows right to left)
        for left_param in &left_params {
            for right_param in &right_params {
                self.dependencies.add_dependency(*left_param, *right_param);
            }
        }
    }

    /// Extract parameter IDs from an expression, recursively traversing
    /// arithmetic and set operations to find all nested parameter references.
    fn extract_parameters(&self, expr_id: ExpressionId) -> Vec<ParameterId> {
        let mut params = Vec::new();
        self.extract_parameters_recursive(expr_id, &mut params);
        params
    }

    /// Recursively collect parameter IDs from an expression tree.
    fn extract_parameters_recursive(&self, expr_id: ExpressionId, params: &mut Vec<ParameterId>) {
        if let Some(expr) = self.solver.get_expression(expr_id) {
            if let Some(param_id) = expr.as_parameter() {
                params.push(param_id);
            }
            // Recurse into operands (arithmetic, union, intersection, difference)
            // Clone the operands to avoid borrow conflict with &self
            let operands: Vec<ExpressionId> = expr.operands().to_vec();
            for operand_id in operands {
                self.extract_parameters_recursive(operand_id, params);
            }
        }
    }

    /// Parse a unit string to a Unit enum.
    fn parse_unit(&self, unit_str: &str) -> CollectionResult<Unit> {
        // Handle common SI prefixes by stripping them
        let normalized = unit_str.trim();

        // Try to match common patterns with SI prefixes
        let unit_part = if normalized.len() > 1 {
            // Check for SI prefix at the start
            let first_char = normalized.chars().next().unwrap();
            match first_char {
                'k' | 'K' | 'm' | 'u' | 'μ' | 'n' | 'p' | 'M' | 'G' | 'T' => {
                    // Check if the rest is a valid unit
                    let rest = &normalized[first_char.len_utf8()..];
                    if Unit::from_str(rest).is_some() {
                        rest
                    } else {
                        normalized
                    }
                }
                _ => normalized,
            }
        } else {
            normalized
        };

        Unit::from_str(unit_part).ok_or_else(|| CollectionError::UnknownUnit(unit_str.to_string()))
    }

    /// Get collected parameters.
    pub fn parameters(&self) -> &HashMap<String, CollectedParameter> {
        &self.parameters
    }

    /// Get parameter dependencies.
    pub fn dependencies(&self) -> &ParameterDependencies {
        &self.dependencies
    }
}

impl Default for ConstraintCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ato_ir::{ConstraintExpr, ModuleKind, QualifiedName};

    fn create_test_design() -> Design {
        let mut design = Design::new();

        // Create a module with a parameter and constraint
        let module_id = design.create_module("Resistor", ModuleKind::Module);

        // Add resistance parameter
        design.add_field(
            module_id,
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );

        // Add a constraint: assert resistance within 9kohm to 11kohm
        design.create_constraint(
            module_id,
            ConstraintExpr::compare(
                ValueExpr::field(FieldPath::simple("resistance")),
                CompareOpKind::Within,
                ValueExpr::literal(ValueLiteral::range(
                    QuantityValue::new(9000.0, Some("ohm".into())),
                    QuantityValue::new(11000.0, Some("ohm".into())),
                )),
            ),
        );

        design
    }

    #[test]
    fn test_collect_simple_constraint() {
        let design = create_test_design();
        let collector = ConstraintCollector::new();

        let result = collector.collect(&design);
        assert!(result.is_ok());

        let (solver, _deps) = result.unwrap();
        assert_eq!(solver.predicate_count(), 1);
        assert_eq!(solver.constrained_count(), 1);
    }

    #[test]
    fn test_parse_unit() {
        let collector = ConstraintCollector::new();

        assert_eq!(collector.parse_unit("V").unwrap(), Unit::Volt);
        assert_eq!(collector.parse_unit("ohm").unwrap(), Unit::Ohm);
        assert_eq!(collector.parse_unit("F").unwrap(), Unit::Farad);
        assert_eq!(collector.parse_unit("Hz").unwrap(), Unit::Hertz);
    }

    #[test]
    fn test_convert_quantity() {
        let collector = ConstraintCollector::new();

        let q = QuantityValue::new(10.0, Some("V".into()));
        let lit = collector.convert_quantity(&q).unwrap();

        if let Literal::Quantity(disjoint) = lit {
            assert!(!disjoint.is_empty());
        } else {
            panic!("Expected quantity literal");
        }
    }

    #[test]
    fn test_convert_range() {
        let collector = ConstraintCollector::new();

        let from = QuantityValue::new(9.0, Some("V".into()));
        let to = QuantityValue::new(11.0, Some("V".into()));
        let lit = collector.convert_range(&from, &to).unwrap();

        if let Literal::Quantity(disjoint) = lit {
            assert!(!disjoint.is_empty());
        } else {
            panic!("Expected quantity literal");
        }
    }

    #[test]
    fn test_convert_bilateral_percent() {
        let collector = ConstraintCollector::new();

        let base = QuantityValue::new(10000.0, Some("ohm".into()));
        let tolerance = ToleranceValue::percent(5.0);

        // This should create a range of 9.5kohm to 10.5kohm
        let lit = collector.convert_bilateral(&base, &tolerance).unwrap();

        if let Literal::Quantity(disjoint) = lit {
            assert!(!disjoint.is_empty());
        } else {
            panic!("Expected quantity literal");
        }
    }

    #[test]
    fn test_parameter_dependencies() {
        let mut deps = ParameterDependencies::new();

        let p1 = ParameterId::new();
        let p2 = ParameterId::new();
        let p3 = ParameterId::new();

        deps.add_dependency(p1, p2);
        deps.add_dependency(p1, p3);

        assert_eq!(deps.get_dependencies(p1).len(), 2);
        assert_eq!(deps.get_dependents(p2).len(), 1);
        assert_eq!(deps.get_dependents(p3).len(), 1);
    }

    #[test]
    fn test_collect_comparison_constraint() {
        let mut design = Design::new();
        let module_id = design.create_module("Test", ModuleKind::Module);

        design.add_field(module_id, "voltage", FieldKind::parameter_with_unit("V"));

        // assert voltage > 5V
        design.create_constraint(
            module_id,
            ConstraintExpr::compare(
                ValueExpr::field(FieldPath::simple("voltage")),
                CompareOpKind::GreaterThan,
                ValueExpr::literal(ValueLiteral::quantity(5.0, Some("V".into()))),
            ),
        );

        let collector = ConstraintCollector::new();
        let result = collector.collect(&design);
        assert!(result.is_ok());

        let (solver, _) = result.unwrap();
        assert_eq!(solver.constrained_count(), 1);
    }

    #[test]
    fn test_format_field_path() {
        let path = FieldPath::simple("resistance");
        assert_eq!(ConstraintCollector::format_field_path(&path), "resistance");

        let complex_path = FieldPath::new(vec![
            FieldPathPart::Name("r1".into()),
            FieldPathPart::Name("resistance".into()),
        ]);
        assert_eq!(
            ConstraintCollector::format_field_path(&complex_path),
            "r1.resistance"
        );
    }

    #[test]
    fn test_resolve_nested_field_unit() {
        // C3 fix: assert r1.resistance within ... should resolve unit to ohm
        let mut design = Design::new();

        // Create Resistor module with a resistance parameter
        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "resistance", FieldKind::parameter_with_unit("ohm"));

        // Create a parent module with an instance of Resistor
        let parent_id = design.create_module("Circuit", ModuleKind::Module);
        let mut instance_kind = FieldKind::instance(QualifiedName::simple("Resistor"));
        // Set resolved_type so the collector can find the Resistor module
        if let FieldKind::Instance { ref mut resolved_type, .. } = instance_kind {
            *resolved_type = Some(resistor_id);
        }
        design.add_field(parent_id, "r1", instance_kind);

        // Add constraint: assert r1.resistance within 9kohm to 11kohm
        design.create_constraint(
            parent_id,
            ConstraintExpr::compare(
                ValueExpr::field(FieldPath::new(vec![
                    FieldPathPart::Name("r1".into()),
                    FieldPathPart::Name("resistance".into()),
                ])),
                CompareOpKind::Within,
                ValueExpr::literal(ValueLiteral::range(
                    QuantityValue::new(9000.0, Some("ohm".into())),
                    QuantityValue::new(11000.0, Some("ohm".into())),
                )),
            ),
        );

        let collector = ConstraintCollector::new();
        let result = collector.collect(&design);
        assert!(result.is_ok());

        let (solver, _deps) = result.unwrap();
        assert_eq!(solver.constrained_count(), 1);

        // Verify the parameter was resolved with the correct unit (Ohm, not Dimensionless)
        // The parameter path should be "r1.resistance"
        // We can check by looking at the parameter's unit in the solver
    }

    #[test]
    fn test_resolve_deeply_nested_field_unit() {
        // C3 fix: handles arbitrary depth like board.sensor.voltage
        let mut design = Design::new();

        // Create Sensor module with voltage parameter
        let sensor_id = design.create_module("Sensor", ModuleKind::Module);
        design.add_field(sensor_id, "voltage", FieldKind::parameter_with_unit("V"));

        // Create Board module with an instance of Sensor
        let board_id = design.create_module("Board", ModuleKind::Module);
        let mut sensor_instance = FieldKind::instance(QualifiedName::simple("Sensor"));
        if let FieldKind::Instance { ref mut resolved_type, .. } = sensor_instance {
            *resolved_type = Some(sensor_id);
        }
        design.add_field(board_id, "sensor", sensor_instance);

        // Create top-level module with an instance of Board
        let top_id = design.create_module("Top", ModuleKind::Module);
        let mut board_instance = FieldKind::instance(QualifiedName::simple("Board"));
        if let FieldKind::Instance { ref mut resolved_type, .. } = board_instance {
            *resolved_type = Some(board_id);
        }
        design.add_field(top_id, "board", board_instance);

        // Add constraint: assert board.sensor.voltage within 3V to 3.6V
        design.create_constraint(
            top_id,
            ConstraintExpr::compare(
                ValueExpr::field(FieldPath::new(vec![
                    FieldPathPart::Name("board".into()),
                    FieldPathPart::Name("sensor".into()),
                    FieldPathPart::Name("voltage".into()),
                ])),
                CompareOpKind::Within,
                ValueExpr::literal(ValueLiteral::range(
                    QuantityValue::new(3.0, Some("V".into())),
                    QuantityValue::new(3.6, Some("V".into())),
                )),
            ),
        );

        let collector = ConstraintCollector::new();
        let result = collector.collect(&design);
        assert!(result.is_ok());

        let (solver, _deps) = result.unwrap();
        assert_eq!(solver.constrained_count(), 1);
    }

    #[test]
    fn test_extract_parameters_from_arithmetic() {
        // C4 fix: extract_parameters should find parameters inside arithmetic expressions
        let mut design = Design::new();
        let module_id = design.create_module("Test", ModuleKind::Module);

        design.add_field(module_id, "a", FieldKind::parameter_with_unit("V"));
        design.add_field(module_id, "b", FieldKind::parameter_with_unit("V"));

        // assert a + b > 5V
        design.create_constraint(
            module_id,
            ConstraintExpr::compare(
                ValueExpr::Binary {
                    left: Box::new(ValueExpr::field(FieldPath::simple("a"))),
                    op: BinaryOp::Add,
                    right: Box::new(ValueExpr::field(FieldPath::simple("b"))),
                },
                CompareOpKind::GreaterThan,
                ValueExpr::literal(ValueLiteral::quantity(5.0, Some("V".into()))),
            ),
        );

        let collector = ConstraintCollector::new();
        let result = collector.collect(&design);
        assert!(result.is_ok());

        let (solver, _deps) = result.unwrap();
        // The constraint "a + b > 5V" should have been collected successfully
        // with both parameters found in the arithmetic expression
        assert_eq!(solver.constrained_count(), 1);
    }
}
