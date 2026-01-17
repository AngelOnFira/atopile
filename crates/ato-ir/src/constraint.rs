//! Constraint types for the IR.
//!
//! Constraints represent assertions about parameter values (e.g., `assert x within 1V to 2V`).

use crate::{ConstraintId, FieldPath, ModuleId};
use ato_lexer::Span;
use serde::{Deserialize, Serialize};

/// A constraint (assertion) in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Unique identifier for this constraint.
    pub id: ConstraintId,

    /// The module that contains this constraint.
    pub parent: ModuleId,

    /// The expression being constrained.
    pub expression: ConstraintExpr,

    /// Source location for error reporting.
    pub span: Option<Span>,
}

impl Constraint {
    /// Create a new constraint.
    pub fn new(id: ConstraintId, parent: ModuleId, expression: ConstraintExpr) -> Self {
        Self {
            id,
            parent,
            expression,
            span: None,
        }
    }

    /// Set the source span for this constraint.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

/// A constraint expression representing an assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintExpr {
    /// The left-hand side of the comparison.
    pub left: ValueExpr,

    /// The comparison operations (can be chained, e.g., `5V < x < 10V`).
    pub operations: Vec<CompareOp>,
}

impl ConstraintExpr {
    /// Create a new constraint expression.
    pub fn new(left: ValueExpr, operations: Vec<CompareOp>) -> Self {
        Self { left, operations }
    }

    /// Create a simple comparison.
    pub fn compare(left: ValueExpr, op: CompareOpKind, right: ValueExpr) -> Self {
        Self {
            left,
            operations: vec![CompareOp { kind: op, right }],
        }
    }
}

/// A comparison operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareOp {
    /// The comparison operator.
    pub kind: CompareOpKind,

    /// The right-hand side of the comparison.
    pub right: ValueExpr,
}

/// The kind of comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOpKind {
    /// Less than: `<`
    LessThan,
    /// Greater than: `>`
    GreaterThan,
    /// Less than or equal: `<=`
    LessEq,
    /// Greater than or equal: `>=`
    GreaterEq,
    /// Within a range: `within`
    Within,
    /// Exactly equals: `is`
    Is,
}

impl CompareOpKind {
    /// Get the symbol for this operator.
    pub fn symbol(&self) -> &'static str {
        match self {
            CompareOpKind::LessThan => "<",
            CompareOpKind::GreaterThan => ">",
            CompareOpKind::LessEq => "<=",
            CompareOpKind::GreaterEq => ">=",
            CompareOpKind::Within => "within",
            CompareOpKind::Is => "is",
        }
    }
}

/// A value expression used in constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueExpr {
    /// A reference to a field (parameter).
    FieldRef(FieldPath),

    /// A literal value.
    Literal(ValueLiteral),

    /// A binary operation.
    Binary {
        left: Box<ValueExpr>,
        op: BinaryOp,
        right: Box<ValueExpr>,
    },

    /// A unary operation.
    Unary {
        op: UnaryOp,
        operand: Box<ValueExpr>,
    },

    /// A grouped expression: `(expr)`.
    Group(Box<ValueExpr>),
}

impl ValueExpr {
    /// Create a field reference expression.
    pub fn field(path: FieldPath) -> Self {
        Self::FieldRef(path)
    }

    /// Create a literal expression.
    pub fn literal(lit: ValueLiteral) -> Self {
        Self::Literal(lit)
    }

    /// Create a binary expression.
    pub fn binary(left: ValueExpr, op: BinaryOp, right: ValueExpr) -> Self {
        Self::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    /// Create a unary expression.
    pub fn unary(op: UnaryOp, operand: ValueExpr) -> Self {
        Self::Unary {
            op,
            operand: Box::new(operand),
        }
    }
}

/// A literal value in a constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValueLiteral {
    /// A simple quantity (number with optional unit).
    Quantity(QuantityValue),

    /// A range of quantities.
    Range {
        from: QuantityValue,
        to: QuantityValue,
    },

    /// A bilateral tolerance.
    Bilateral {
        base: QuantityValue,
        tolerance: ToleranceValue,
    },

    /// A boolean value.
    Bool(bool),

    /// A string value.
    String(String),
}

impl ValueLiteral {
    /// Create a quantity literal.
    pub fn quantity(value: f64, unit: Option<String>) -> Self {
        Self::Quantity(QuantityValue { value, unit })
    }

    /// Create a range literal.
    pub fn range(from: QuantityValue, to: QuantityValue) -> Self {
        Self::Range { from, to }
    }

    /// Create a bilateral tolerance literal.
    pub fn bilateral(base: QuantityValue, tolerance: ToleranceValue) -> Self {
        Self::Bilateral { base, tolerance }
    }
}

/// A quantity value (number with optional unit).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantityValue {
    /// The numeric value.
    pub value: f64,

    /// The unit (e.g., "V", "ohm", "F").
    pub unit: Option<String>,
}

impl QuantityValue {
    /// Create a new quantity value.
    pub fn new(value: f64, unit: Option<String>) -> Self {
        Self { value, unit }
    }

    /// Create a dimensionless quantity.
    pub fn dimensionless(value: f64) -> Self {
        Self { value, unit: None }
    }
}

/// A tolerance value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToleranceValue {
    /// The tolerance value.
    pub value: f64,

    /// Whether this is a percentage tolerance.
    pub is_percent: bool,

    /// The unit (for absolute tolerances).
    pub unit: Option<String>,
}

impl ToleranceValue {
    /// Create a percentage tolerance.
    pub fn percent(value: f64) -> Self {
        Self {
            value,
            is_percent: true,
            unit: None,
        }
    }

    /// Create an absolute tolerance.
    pub fn absolute(value: f64, unit: Option<String>) -> Self {
        Self {
            value,
            is_percent: false,
            unit,
        }
    }
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    /// Addition: `+`
    Add,
    /// Subtraction: `-`
    Sub,
    /// Multiplication: `*`
    Mul,
    /// Division: `/`
    Div,
    /// Power: `**`
    Power,
    /// Bitwise OR: `|`
    BitOr,
    /// Bitwise AND: `&`
    BitAnd,
}

impl BinaryOp {
    /// Get the symbol for this operator.
    pub fn symbol(&self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Power => "**",
            BinaryOp::BitOr => "|",
            BinaryOp::BitAnd => "&",
        }
    }
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    /// Negation: `-`
    Neg,
    /// Positive: `+`
    Pos,
}

impl UnaryOp {
    /// Get the symbol for this operator.
    pub fn symbol(&self) -> &'static str {
        match self {
            UnaryOp::Neg => "-",
            UnaryOp::Pos => "+",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_creation() {
        let constraint = Constraint::new(
            ConstraintId::new(0),
            ModuleId::new(0),
            ConstraintExpr::compare(
                ValueExpr::field(FieldPath::simple("voltage")),
                CompareOpKind::Within,
                ValueExpr::literal(ValueLiteral::range(
                    QuantityValue::new(3.0, Some("V".into())),
                    QuantityValue::new(3.6, Some("V".into())),
                )),
            ),
        );

        assert_eq!(constraint.expression.operations.len(), 1);
        assert_eq!(constraint.expression.operations[0].kind, CompareOpKind::Within);
    }

    #[test]
    fn test_quantity_value() {
        let q = QuantityValue::new(10.0, Some("kohm".into()));
        assert_eq!(q.value, 10.0);
        assert_eq!(q.unit, Some("kohm".into()));
    }

    #[test]
    fn test_tolerance_value() {
        let percent = ToleranceValue::percent(5.0);
        assert!(percent.is_percent);
        assert_eq!(percent.value, 5.0);

        let absolute = ToleranceValue::absolute(100.0, Some("mV".into()));
        assert!(!absolute.is_percent);
        assert_eq!(absolute.unit, Some("mV".into()));
    }

    #[test]
    fn test_value_literal() {
        let bilateral = ValueLiteral::bilateral(
            QuantityValue::new(10.0, Some("kohm".into())),
            ToleranceValue::percent(5.0),
        );

        if let ValueLiteral::Bilateral { base, tolerance } = bilateral {
            assert_eq!(base.value, 10.0);
            assert!(tolerance.is_percent);
        } else {
            panic!("Expected bilateral literal");
        }
    }

    #[test]
    fn test_compare_op_symbols() {
        assert_eq!(CompareOpKind::LessThan.symbol(), "<");
        assert_eq!(CompareOpKind::Within.symbol(), "within");
        assert_eq!(CompareOpKind::Is.symbol(), "is");
    }

    #[test]
    fn test_binary_expression() {
        let expr = ValueExpr::binary(
            ValueExpr::field(FieldPath::simple("a")),
            BinaryOp::Add,
            ValueExpr::field(FieldPath::simple("b")),
        );

        if let ValueExpr::Binary { op, .. } = expr {
            assert_eq!(op, BinaryOp::Add);
        } else {
            panic!("Expected binary expression");
        }
    }
}
