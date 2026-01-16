//! Expression types for the constraint solver.
//!
//! Expressions represent values and operations in the constraint system.

use ato_domain::{QuantityIntervalDisjoint, Unit};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for generating unique expression IDs.
static NEXT_EXPRESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Global counter for generating unique parameter IDs.
static NEXT_PARAMETER_ID: AtomicU64 = AtomicU64::new(1);

/// A unique identifier for an expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExpressionId(u64);

impl ExpressionId {
    /// Create a new unique expression ID.
    pub fn new() -> Self {
        Self(NEXT_EXPRESSION_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for ExpressionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExpressionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E{}", self.0)
    }
}

/// A unique identifier for a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParameterId(u64);

impl ParameterId {
    /// Create a new unique parameter ID.
    pub fn new() -> Self {
        Self(NEXT_PARAMETER_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for ParameterId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ParameterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P{}", self.0)
    }
}

/// A literal value in the constraint system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    /// A boolean value.
    Bool(bool),
    /// A numeric interval (can represent a single value or a range).
    Quantity(QuantityIntervalDisjoint),
    /// An integer value.
    Integer(i64),
    /// A floating point value.
    Float(f64),
}

impl Literal {
    /// Create a literal from a single f64 value with a unit.
    pub fn from_quantity(value: f64, unit: Unit) -> Self {
        use ato_domain::{Quantity, QuantityInterval};
        let q = Quantity::new(value, unit);
        let interval = QuantityInterval::singleton(q).unwrap();
        Literal::Quantity(QuantityIntervalDisjoint::single(interval))
    }

    /// Create a literal from a quantity interval.
    pub fn from_interval(min: f64, max: f64, unit: Unit) -> Self {
        use ato_domain::{Quantity, QuantityInterval};
        let min_q = Quantity::new(min, unit);
        let max_q = Quantity::new(max, unit);
        let interval = QuantityInterval::new(min_q, max_q).unwrap();
        Literal::Quantity(QuantityIntervalDisjoint::single(interval))
    }

    /// Check if this literal is a boolean.
    pub fn is_bool(&self) -> bool {
        matches!(self, Literal::Bool(_))
    }

    /// Check if this literal is a numeric quantity.
    pub fn is_quantity(&self) -> bool {
        matches!(self, Literal::Quantity(_))
    }

    /// Try to get this literal as a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Literal::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get this literal as a quantity interval.
    pub fn as_quantity(&self) -> Option<&QuantityIntervalDisjoint> {
        match self {
            Literal::Quantity(q) => Some(q),
            _ => None,
        }
    }

    /// Check if this literal represents a single value (not a range).
    pub fn is_singleton(&self) -> bool {
        match self {
            Literal::Bool(_) => true,
            Literal::Integer(_) => true,
            Literal::Float(_) => true,
            Literal::Quantity(q) => q.is_singleton(),
        }
    }

    /// Check if this literal is empty (no valid values).
    pub fn is_empty(&self) -> bool {
        match self {
            Literal::Bool(_) => false,
            Literal::Integer(_) => false,
            Literal::Float(_) => false,
            Literal::Quantity(q) => q.is_empty(),
        }
    }
}

impl PartialEq for Literal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Literal::Bool(a), Literal::Bool(b)) => a == b,
            (Literal::Integer(a), Literal::Integer(b)) => a == b,
            (Literal::Float(a), Literal::Float(b)) => (a - b).abs() < 1e-15,
            (Literal::Quantity(a), Literal::Quantity(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Bool(b) => write!(f, "{}", b),
            Literal::Integer(i) => write!(f, "{}", i),
            Literal::Float(v) => write!(f, "{}", v),
            Literal::Quantity(q) => write!(f, "{}", q),
        }
    }
}

/// A parameter in the constraint system.
///
/// Parameters represent unknown values that the solver tries to resolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Unique identifier.
    pub id: ParameterId,
    /// Optional name for debugging.
    pub name: Option<String>,
    /// The domain of valid values for this parameter.
    pub domain: QuantityIntervalDisjoint,
    /// The unit of this parameter.
    pub unit: Unit,
    /// Whether this parameter is likely to be constrained (hint for solver).
    pub likely_constrained: bool,
    /// A soft set suggestion (preferred values).
    pub soft_set: Option<QuantityIntervalDisjoint>,
    /// Known superset of possible values (gets narrowed by constraints).
    pub known_superset: Option<QuantityIntervalDisjoint>,
}

impl Parameter {
    /// Create a new parameter with the given unit.
    pub fn new(unit: Unit) -> Self {
        Self {
            id: ParameterId::new(),
            name: None,
            domain: QuantityIntervalDisjoint::unbounded(unit),
            unit,
            likely_constrained: true,
            soft_set: None,
            known_superset: None,
        }
    }

    /// Create a new parameter with a name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Create a new parameter with a domain constraint.
    pub fn with_domain(mut self, domain: QuantityIntervalDisjoint) -> Self {
        self.domain = domain;
        self
    }

    /// Get the display name for this parameter.
    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("P{}", self.id.0))
    }
}

impl PartialEq for Parameter {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Parameter {}

impl std::hash::Hash for Parameter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{}", name)
        } else {
            write!(f, "P{}", self.id.0)
        }
    }
}

/// Arithmetic operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArithmeticOp {
    /// Addition: a + b
    Add,
    /// Subtraction: a - b
    Subtract,
    /// Multiplication: a * b
    Multiply,
    /// Division: a / b
    Divide,
    /// Power: a ^ b
    Power,
    /// Negation: -a
    Negate,
    /// Absolute value: |a|
    Abs,
    /// Minimum of operands
    Min,
    /// Maximum of operands
    Max,
}

impl fmt::Display for ArithmeticOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArithmeticOp::Add => write!(f, "+"),
            ArithmeticOp::Subtract => write!(f, "-"),
            ArithmeticOp::Multiply => write!(f, "*"),
            ArithmeticOp::Divide => write!(f, "/"),
            ArithmeticOp::Power => write!(f, "^"),
            ArithmeticOp::Negate => write!(f, "-"),
            ArithmeticOp::Abs => write!(f, "abs"),
            ArithmeticOp::Min => write!(f, "min"),
            ArithmeticOp::Max => write!(f, "max"),
        }
    }
}

/// The kind of an expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpressionKind {
    /// A literal value.
    Literal(Literal),
    /// A reference to a parameter.
    Parameter(ParameterId),
    /// An arithmetic operation on sub-expressions.
    Arithmetic {
        op: ArithmeticOp,
        operands: Vec<ExpressionId>,
    },
    /// Set union of intervals.
    Union(Vec<ExpressionId>),
    /// Set intersection of intervals.
    Intersection(Vec<ExpressionId>),
    /// Set difference: first - rest.
    Difference(Vec<ExpressionId>),
}

/// An expression in the constraint system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expression {
    /// Unique identifier.
    pub id: ExpressionId,
    /// The kind of expression.
    pub kind: ExpressionKind,
    /// The unit of this expression (if numeric).
    pub unit: Option<Unit>,
}

impl Expression {
    /// Create a new literal expression.
    pub fn literal(lit: Literal) -> Self {
        let unit = match &lit {
            Literal::Quantity(q) => Some(q.unit()),
            _ => None,
        };
        Self {
            id: ExpressionId::new(),
            kind: ExpressionKind::Literal(lit),
            unit,
        }
    }

    /// Create a new parameter reference expression.
    pub fn parameter(param_id: ParameterId, unit: Unit) -> Self {
        Self {
            id: ExpressionId::new(),
            kind: ExpressionKind::Parameter(param_id),
            unit: Some(unit),
        }
    }

    /// Create a new arithmetic expression.
    pub fn arithmetic(op: ArithmeticOp, operands: Vec<ExpressionId>, unit: Option<Unit>) -> Self {
        Self {
            id: ExpressionId::new(),
            kind: ExpressionKind::Arithmetic { op, operands },
            unit,
        }
    }

    /// Create an addition expression.
    pub fn add(left: ExpressionId, right: ExpressionId, unit: Option<Unit>) -> Self {
        Self::arithmetic(ArithmeticOp::Add, vec![left, right], unit)
    }

    /// Create a subtraction expression.
    pub fn subtract(left: ExpressionId, right: ExpressionId, unit: Option<Unit>) -> Self {
        Self::arithmetic(ArithmeticOp::Subtract, vec![left, right], unit)
    }

    /// Create a multiplication expression.
    pub fn multiply(left: ExpressionId, right: ExpressionId, unit: Option<Unit>) -> Self {
        Self::arithmetic(ArithmeticOp::Multiply, vec![left, right], unit)
    }

    /// Create a division expression.
    pub fn divide(left: ExpressionId, right: ExpressionId, unit: Option<Unit>) -> Self {
        Self::arithmetic(ArithmeticOp::Divide, vec![left, right], unit)
    }

    /// Check if this is a literal expression.
    pub fn is_literal(&self) -> bool {
        matches!(self.kind, ExpressionKind::Literal(_))
    }

    /// Check if this is a parameter reference.
    pub fn is_parameter(&self) -> bool {
        matches!(self.kind, ExpressionKind::Parameter(_))
    }

    /// Try to get the literal value.
    pub fn as_literal(&self) -> Option<&Literal> {
        match &self.kind {
            ExpressionKind::Literal(lit) => Some(lit),
            _ => None,
        }
    }

    /// Try to get the parameter ID.
    pub fn as_parameter(&self) -> Option<ParameterId> {
        match &self.kind {
            ExpressionKind::Parameter(id) => Some(*id),
            _ => None,
        }
    }

    /// Get the operand expression IDs for this expression.
    pub fn operands(&self) -> &[ExpressionId] {
        match &self.kind {
            ExpressionKind::Literal(_) => &[],
            ExpressionKind::Parameter(_) => &[],
            ExpressionKind::Arithmetic { operands, .. } => operands,
            ExpressionKind::Union(operands) => operands,
            ExpressionKind::Intersection(operands) => operands,
            ExpressionKind::Difference(operands) => operands,
        }
    }
}

impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Expression {}

impl std::hash::Hash for Expression {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ExpressionKind::Literal(lit) => write!(f, "{}", lit),
            ExpressionKind::Parameter(id) => write!(f, "{}", id),
            ExpressionKind::Arithmetic { op, operands } => {
                if operands.len() == 1 {
                    write!(f, "{}({})", op, operands[0])
                } else {
                    write!(
                        f,
                        "({})",
                        operands
                            .iter()
                            .map(|o| o.to_string())
                            .collect::<Vec<_>>()
                            .join(&format!(" {} ", op))
                    )
                }
            }
            ExpressionKind::Union(operands) => {
                write!(
                    f,
                    "({})",
                    operands
                        .iter()
                        .map(|o| o.to_string())
                        .collect::<Vec<_>>()
                        .join(" ∪ ")
                )
            }
            ExpressionKind::Intersection(operands) => {
                write!(
                    f,
                    "({})",
                    operands
                        .iter()
                        .map(|o| o.to_string())
                        .collect::<Vec<_>>()
                        .join(" ∩ ")
                )
            }
            ExpressionKind::Difference(operands) => {
                write!(
                    f,
                    "({})",
                    operands
                        .iter()
                        .map(|o| o.to_string())
                        .collect::<Vec<_>>()
                        .join(" \\ ")
                )
            }
        }
    }
}

/// A store for expressions, allowing lookup by ID.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExpressionStore {
    expressions: HashMap<ExpressionId, Expression>,
    parameters: HashMap<ParameterId, Parameter>,
}

impl ExpressionStore {
    /// Create a new empty expression store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an expression to the store.
    pub fn add_expression(&mut self, expr: Expression) -> ExpressionId {
        let id = expr.id;
        self.expressions.insert(id, expr);
        id
    }

    /// Add a parameter to the store.
    pub fn add_parameter(&mut self, param: Parameter) -> ParameterId {
        let id = param.id;
        self.parameters.insert(id, param);
        id
    }

    /// Get an expression by ID.
    pub fn get_expression(&self, id: ExpressionId) -> Option<&Expression> {
        self.expressions.get(&id)
    }

    /// Get a mutable expression by ID.
    pub fn get_expression_mut(&mut self, id: ExpressionId) -> Option<&mut Expression> {
        self.expressions.get_mut(&id)
    }

    /// Get a parameter by ID.
    pub fn get_parameter(&self, id: ParameterId) -> Option<&Parameter> {
        self.parameters.get(&id)
    }

    /// Get a mutable parameter by ID.
    pub fn get_parameter_mut(&mut self, id: ParameterId) -> Option<&mut Parameter> {
        self.parameters.get_mut(&id)
    }

    /// Remove an expression by ID.
    pub fn remove_expression(&mut self, id: ExpressionId) -> Option<Expression> {
        self.expressions.remove(&id)
    }

    /// Get all expression IDs.
    pub fn expression_ids(&self) -> impl Iterator<Item = ExpressionId> + '_ {
        self.expressions.keys().copied()
    }

    /// Get all parameter IDs.
    pub fn parameter_ids(&self) -> impl Iterator<Item = ParameterId> + '_ {
        self.parameters.keys().copied()
    }

    /// Get the number of expressions.
    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    /// Get the number of parameters.
    pub fn parameter_count(&self) -> usize {
        self.parameters.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expression_id_unique() {
        let id1 = ExpressionId::new();
        let id2 = ExpressionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_parameter_id_unique() {
        let id1 = ParameterId::new();
        let id2 = ParameterId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_literal_bool() {
        let lit = Literal::Bool(true);
        assert!(lit.is_bool());
        assert_eq!(lit.as_bool(), Some(true));
    }

    #[test]
    fn test_literal_quantity() {
        let lit = Literal::from_quantity(5.0, Unit::Volt);
        assert!(lit.is_quantity());
        assert!(lit.is_singleton());
    }

    #[test]
    fn test_parameter_creation() {
        let param = Parameter::new(Unit::Ohm).with_name("R1");
        assert_eq!(param.name, Some("R1".to_string()));
        assert_eq!(param.unit, Unit::Ohm);
    }

    #[test]
    fn test_expression_literal() {
        let expr = Expression::literal(Literal::Bool(true));
        assert!(expr.is_literal());
        assert!(!expr.is_parameter());
    }

    #[test]
    fn test_expression_store() {
        let mut store = ExpressionStore::new();

        let param = Parameter::new(Unit::Volt).with_name("V1");
        let param_id = store.add_parameter(param);

        let expr = Expression::parameter(param_id, Unit::Volt);
        let expr_id = store.add_expression(expr);

        assert!(store.get_expression(expr_id).is_some());
        assert!(store.get_parameter(param_id).is_some());
    }
}
