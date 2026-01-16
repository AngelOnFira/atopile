//! Predicate types for constraints in the solver.
//!
//! Predicates represent constraints that must be satisfied.

use crate::expression::ExpressionId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for generating unique predicate IDs.
static NEXT_PREDICATE_ID: AtomicU64 = AtomicU64::new(1);

/// A unique identifier for a predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PredicateId(u64);

impl PredicateId {
    /// Create a new unique predicate ID.
    pub fn new() -> Self {
        Self(NEXT_PREDICATE_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for PredicateId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PredicateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pred{}", self.0)
    }
}

/// The kind of predicate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PredicateKind {
    /// Equality/alias: left IS right (same value).
    Is {
        left: ExpressionId,
        right: ExpressionId,
    },

    /// Less than or equal: left <= right.
    LessOrEqual {
        left: ExpressionId,
        right: ExpressionId,
    },

    /// Greater than or equal: left >= right.
    GreaterOrEqual {
        left: ExpressionId,
        right: ExpressionId,
    },

    /// Less than: left < right.
    LessThan {
        left: ExpressionId,
        right: ExpressionId,
    },

    /// Greater than: left > right.
    GreaterThan {
        left: ExpressionId,
        right: ExpressionId,
    },

    /// Not equal: left != right.
    NotEqual {
        left: ExpressionId,
        right: ExpressionId,
    },

    /// Subset: left ⊆ right.
    IsSubset {
        left: ExpressionId,
        right: ExpressionId,
    },

    /// Superset: left ⊇ right.
    IsSuperset {
        left: ExpressionId,
        right: ExpressionId,
    },

    /// Within tolerance: value is within tolerance_spec.
    /// This is essentially `value ⊆ tolerance_spec`.
    Within {
        value: ExpressionId,
        tolerance: ExpressionId,
    },

    /// Logical AND of predicates.
    And(Vec<PredicateId>),

    /// Logical OR of predicates.
    Or(Vec<PredicateId>),

    /// Logical NOT of a predicate.
    Not(PredicateId),

    /// Implication: if condition then implication.
    Implies {
        condition: PredicateId,
        implication: PredicateId,
    },

    /// A literal true value (tautology).
    True,

    /// A literal false value (contradiction).
    False,
}

impl PredicateKind {
    /// Get the symbol for this predicate kind.
    pub fn symbol(&self) -> &'static str {
        match self {
            PredicateKind::Is { .. } => "is",
            PredicateKind::LessOrEqual { .. } => "≤",
            PredicateKind::GreaterOrEqual { .. } => "≥",
            PredicateKind::LessThan { .. } => "<",
            PredicateKind::GreaterThan { .. } => ">",
            PredicateKind::NotEqual { .. } => "≠",
            PredicateKind::IsSubset { .. } => "⊆",
            PredicateKind::IsSuperset { .. } => "⊇",
            PredicateKind::Within { .. } => "within",
            PredicateKind::And(_) => "∧",
            PredicateKind::Or(_) => "∨",
            PredicateKind::Not(_) => "¬",
            PredicateKind::Implies { .. } => "→",
            PredicateKind::True => "⊤",
            PredicateKind::False => "⊥",
        }
    }
}

/// A predicate (constraint) in the solver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Predicate {
    /// Unique identifier.
    pub id: PredicateId,
    /// The kind of predicate.
    pub kind: PredicateKind,
    /// Whether this predicate is constrained (must be satisfied).
    pub constrained: bool,
    /// Whether the solver has determined this predicate to be true.
    pub solver_terminated: bool,
}

impl Predicate {
    /// Create a new predicate.
    fn new(kind: PredicateKind) -> Self {
        Self {
            id: PredicateId::new(),
            kind,
            constrained: false,
            solver_terminated: false,
        }
    }

    /// Create an IS predicate (equality/alias).
    pub fn is(left: ExpressionId, right: ExpressionId) -> Self {
        Self::new(PredicateKind::Is { left, right })
    }

    /// Create a less-than-or-equal predicate.
    pub fn less_or_equal(left: ExpressionId, right: ExpressionId) -> Self {
        Self::new(PredicateKind::LessOrEqual { left, right })
    }

    /// Create a greater-than-or-equal predicate.
    pub fn greater_or_equal(left: ExpressionId, right: ExpressionId) -> Self {
        Self::new(PredicateKind::GreaterOrEqual { left, right })
    }

    /// Create a less-than predicate.
    pub fn less_than(left: ExpressionId, right: ExpressionId) -> Self {
        Self::new(PredicateKind::LessThan { left, right })
    }

    /// Create a greater-than predicate.
    pub fn greater_than(left: ExpressionId, right: ExpressionId) -> Self {
        Self::new(PredicateKind::GreaterThan { left, right })
    }

    /// Create a not-equal predicate.
    pub fn not_equal(left: ExpressionId, right: ExpressionId) -> Self {
        Self::new(PredicateKind::NotEqual { left, right })
    }

    /// Create a subset predicate.
    pub fn is_subset(left: ExpressionId, right: ExpressionId) -> Self {
        Self::new(PredicateKind::IsSubset { left, right })
    }

    /// Create a superset predicate.
    pub fn is_superset(left: ExpressionId, right: ExpressionId) -> Self {
        Self::new(PredicateKind::IsSuperset { left, right })
    }

    /// Create a within-tolerance predicate.
    pub fn within(value: ExpressionId, tolerance: ExpressionId) -> Self {
        Self::new(PredicateKind::Within { value, tolerance })
    }

    /// Create a logical AND predicate.
    pub fn and(predicates: Vec<PredicateId>) -> Self {
        Self::new(PredicateKind::And(predicates))
    }

    /// Create a logical OR predicate.
    pub fn or(predicates: Vec<PredicateId>) -> Self {
        Self::new(PredicateKind::Or(predicates))
    }

    /// Create a logical NOT predicate.
    pub fn not(predicate: PredicateId) -> Self {
        Self::new(PredicateKind::Not(predicate))
    }

    /// Create an implication predicate.
    pub fn implies(condition: PredicateId, implication: PredicateId) -> Self {
        Self::new(PredicateKind::Implies {
            condition,
            implication,
        })
    }

    /// Create a true (tautology) predicate.
    pub fn tautology() -> Self {
        Self::new(PredicateKind::True)
    }

    /// Create a false (contradiction) predicate.
    pub fn contradiction() -> Self {
        Self::new(PredicateKind::False)
    }

    /// Mark this predicate as constrained (must be satisfied).
    pub fn constrain(mut self) -> Self {
        self.constrained = true;
        self
    }

    /// Check if this predicate is a binary comparison.
    pub fn is_binary_comparison(&self) -> bool {
        matches!(
            self.kind,
            PredicateKind::Is { .. }
                | PredicateKind::LessOrEqual { .. }
                | PredicateKind::GreaterOrEqual { .. }
                | PredicateKind::LessThan { .. }
                | PredicateKind::GreaterThan { .. }
                | PredicateKind::NotEqual { .. }
                | PredicateKind::IsSubset { .. }
                | PredicateKind::IsSuperset { .. }
                | PredicateKind::Within { .. }
        )
    }

    /// Check if this predicate is a logical operation.
    pub fn is_logical(&self) -> bool {
        matches!(
            self.kind,
            PredicateKind::And(_)
                | PredicateKind::Or(_)
                | PredicateKind::Not(_)
                | PredicateKind::Implies { .. }
        )
    }

    /// Check if this is a literal true/false.
    pub fn is_literal(&self) -> bool {
        matches!(self.kind, PredicateKind::True | PredicateKind::False)
    }

    /// Get the expression operands for this predicate.
    pub fn expression_operands(&self) -> Vec<ExpressionId> {
        match &self.kind {
            PredicateKind::Is { left, right } => vec![*left, *right],
            PredicateKind::LessOrEqual { left, right } => vec![*left, *right],
            PredicateKind::GreaterOrEqual { left, right } => vec![*left, *right],
            PredicateKind::LessThan { left, right } => vec![*left, *right],
            PredicateKind::GreaterThan { left, right } => vec![*left, *right],
            PredicateKind::NotEqual { left, right } => vec![*left, *right],
            PredicateKind::IsSubset { left, right } => vec![*left, *right],
            PredicateKind::IsSuperset { left, right } => vec![*left, *right],
            PredicateKind::Within { value, tolerance } => vec![*value, *tolerance],
            PredicateKind::And(_) => vec![],
            PredicateKind::Or(_) => vec![],
            PredicateKind::Not(_) => vec![],
            PredicateKind::Implies { .. } => vec![],
            PredicateKind::True => vec![],
            PredicateKind::False => vec![],
        }
    }

    /// Get the predicate operands for this predicate.
    pub fn predicate_operands(&self) -> Vec<PredicateId> {
        match &self.kind {
            PredicateKind::And(preds) => preds.clone(),
            PredicateKind::Or(preds) => preds.clone(),
            PredicateKind::Not(pred) => vec![*pred],
            PredicateKind::Implies {
                condition,
                implication,
            } => vec![*condition, *implication],
            _ => vec![],
        }
    }
}

impl PartialEq for Predicate {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Predicate {}

impl std::hash::Hash for Predicate {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let constrained_marker = if self.constrained { "!" } else { "" };
        let terminated_marker = if self.solver_terminated { "!!" } else { "" };

        match &self.kind {
            PredicateKind::Is { left, right } => {
                write!(
                    f,
                    "{} is{}{} {}",
                    left, constrained_marker, terminated_marker, right
                )
            }
            PredicateKind::LessOrEqual { left, right } => {
                write!(
                    f,
                    "{} ≤{}{} {}",
                    left, constrained_marker, terminated_marker, right
                )
            }
            PredicateKind::GreaterOrEqual { left, right } => {
                write!(
                    f,
                    "{} ≥{}{} {}",
                    left, constrained_marker, terminated_marker, right
                )
            }
            PredicateKind::LessThan { left, right } => {
                write!(
                    f,
                    "{} <{}{} {}",
                    left, constrained_marker, terminated_marker, right
                )
            }
            PredicateKind::GreaterThan { left, right } => {
                write!(
                    f,
                    "{} >{}{} {}",
                    left, constrained_marker, terminated_marker, right
                )
            }
            PredicateKind::NotEqual { left, right } => {
                write!(
                    f,
                    "{} ≠{}{} {}",
                    left, constrained_marker, terminated_marker, right
                )
            }
            PredicateKind::IsSubset { left, right } => {
                write!(
                    f,
                    "{} ⊆{}{} {}",
                    left, constrained_marker, terminated_marker, right
                )
            }
            PredicateKind::IsSuperset { left, right } => {
                write!(
                    f,
                    "{} ⊇{}{} {}",
                    left, constrained_marker, terminated_marker, right
                )
            }
            PredicateKind::Within { value, tolerance } => {
                write!(
                    f,
                    "{} within{}{} {}",
                    value, constrained_marker, terminated_marker, tolerance
                )
            }
            PredicateKind::And(preds) => {
                let pred_strs: Vec<_> = preds.iter().map(|p| p.to_string()).collect();
                write!(f, "({})", pred_strs.join(" ∧ "))
            }
            PredicateKind::Or(preds) => {
                let pred_strs: Vec<_> = preds.iter().map(|p| p.to_string()).collect();
                write!(f, "({})", pred_strs.join(" ∨ "))
            }
            PredicateKind::Not(pred) => {
                write!(f, "¬{}", pred)
            }
            PredicateKind::Implies {
                condition,
                implication,
            } => {
                write!(f, "{} → {}", condition, implication)
            }
            PredicateKind::True => write!(f, "⊤"),
            PredicateKind::False => write!(f, "⊥"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predicate_id_unique() {
        let id1 = PredicateId::new();
        let id2 = PredicateId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_is_predicate() {
        let left = ExpressionId::new();
        let right = ExpressionId::new();
        let pred = Predicate::is(left, right);
        assert!(!pred.constrained);
        assert!(pred.is_binary_comparison());
    }

    #[test]
    fn test_constrain() {
        let left = ExpressionId::new();
        let right = ExpressionId::new();
        let pred = Predicate::less_or_equal(left, right).constrain();
        assert!(pred.constrained);
    }

    #[test]
    fn test_logical_predicates() {
        let p1 = PredicateId::new();
        let p2 = PredicateId::new();

        let and_pred = Predicate::and(vec![p1, p2]);
        assert!(and_pred.is_logical());
        assert_eq!(and_pred.predicate_operands(), vec![p1, p2]);

        let not_pred = Predicate::not(p1);
        assert!(not_pred.is_logical());
    }

    #[test]
    fn test_expression_operands() {
        let left = ExpressionId::new();
        let right = ExpressionId::new();
        let pred = Predicate::is_subset(left, right);
        let operands = pred.expression_operands();
        assert_eq!(operands.len(), 2);
        assert_eq!(operands[0], left);
        assert_eq!(operands[1], right);
    }
}
