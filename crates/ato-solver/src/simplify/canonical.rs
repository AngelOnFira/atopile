//! Canonical form conversion pass.
//!
//! This pass converts expressions and predicates to canonical forms
//! to make subsequent simplification easier.

use super::{SimplificationContext, SimplificationPass, SimplificationResult};
use crate::predicate::{Predicate, PredicateKind};

/// Canonical form conversion pass.
///
/// Converts expressions and predicates to standard canonical forms:
/// - Normalizes literal values
/// - Converts >= to <= by flipping operands
/// - Converts IsSuperset to IsSubset by flipping operands
pub struct CanonicalPass;

impl SimplificationPass for CanonicalPass {
    fn name(&self) -> &'static str {
        "canonical"
    }

    fn run(&self, ctx: &mut SimplificationContext) -> SimplificationResult {
        let mut result = SimplificationResult::new();

        // Process predicates to convert to canonical forms
        let pred_ids: Vec<_> = ctx.predicates.keys().copied().collect();

        for pred_id in pred_ids {
            if let Some(pred) = ctx.predicates.get(&pred_id) {
                let pred = pred.clone();
                if let Some(canonical) = canonicalize_predicate(&pred) {
                    ctx.predicates.insert(pred_id, canonical);
                    result.mark_dirty();
                }
            }
        }

        result
    }
}

/// Convert a predicate to canonical form if possible.
fn canonicalize_predicate(pred: &Predicate) -> Option<Predicate> {
    match &pred.kind {
        // Convert >= to <= by swapping operands
        PredicateKind::GreaterOrEqual { left, right } => Some(Predicate {
            id: pred.id,
            kind: PredicateKind::LessOrEqual {
                left: *right,
                right: *left,
            },
            constrained: pred.constrained,
            solver_terminated: pred.solver_terminated,
        }),

        // Convert > to < by swapping operands
        PredicateKind::GreaterThan { left, right } => Some(Predicate {
            id: pred.id,
            kind: PredicateKind::LessThan {
                left: *right,
                right: *left,
            },
            constrained: pred.constrained,
            solver_terminated: pred.solver_terminated,
        }),

        // Convert IsSuperset to IsSubset by swapping operands
        PredicateKind::IsSuperset { left, right } => Some(Predicate {
            id: pred.id,
            kind: PredicateKind::IsSubset {
                left: *right,
                right: *left,
            },
            constrained: pred.constrained,
            solver_terminated: pred.solver_terminated,
        }),

        // Convert Within to IsSubset
        PredicateKind::Within { value, tolerance } => Some(Predicate {
            id: pred.id,
            kind: PredicateKind::IsSubset {
                left: *value,
                right: *tolerance,
            },
            constrained: pred.constrained,
            solver_terminated: pred.solver_terminated,
        }),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::ExpressionId;

    #[test]
    fn test_canonicalize_greater_or_equal() {
        let left = ExpressionId::new();
        let right = ExpressionId::new();
        let pred = Predicate::greater_or_equal(left, right);

        let canonical = canonicalize_predicate(&pred).unwrap();
        match canonical.kind {
            PredicateKind::LessOrEqual {
                left: l,
                right: r,
            } => {
                assert_eq!(l, right);
                assert_eq!(r, left);
            }
            _ => panic!("Expected LessOrEqual"),
        }
    }

    #[test]
    fn test_canonicalize_superset() {
        let left = ExpressionId::new();
        let right = ExpressionId::new();
        let pred = Predicate::is_superset(left, right);

        let canonical = canonicalize_predicate(&pred).unwrap();
        match canonical.kind {
            PredicateKind::IsSubset {
                left: l,
                right: r,
            } => {
                assert_eq!(l, right);
                assert_eq!(r, left);
            }
            _ => panic!("Expected IsSubset"),
        }
    }

    #[test]
    fn test_no_change_for_canonical() {
        let left = ExpressionId::new();
        let right = ExpressionId::new();
        let pred = Predicate::less_or_equal(left, right);

        assert!(canonicalize_predicate(&pred).is_none());
    }
}
