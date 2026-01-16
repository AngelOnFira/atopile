//! Structural simplification pass.
//!
//! This pass performs structural simplifications like:
//! - Removing unconstrained predicates
//! - Detecting literal contradictions
//! - Propagating known values

use super::{
    ContradictionInfo, SimplificationContext, SimplificationPass, SimplificationResult,
};
use crate::expression::{ExpressionId, Literal};
use crate::predicate::{PredicateId, PredicateKind};
use std::collections::HashMap;

/// Structural simplification pass.
///
/// Performs high-level structural simplifications:
/// - Removes unconstrained, non-terminated predicates
/// - Detects empty set contradictions
/// - Resolves alias classes (Is predicates)
/// - Propagates transitive subset relationships
pub struct StructuralPass;

impl SimplificationPass for StructuralPass {
    fn name(&self) -> &'static str {
        "structural"
    }

    fn run(&self, ctx: &mut SimplificationContext) -> SimplificationResult {
        let mut result = SimplificationResult::new();

        // Check for literal contradictions (empty sets)
        result.merge(check_literal_contradictions(ctx));
        if result.contradiction.is_some() {
            return result;
        }

        // Remove unconstrained, non-terminated predicates
        result.merge(remove_unconstrained(ctx));

        // Resolve alias classes from Is predicates
        result.merge(resolve_alias_classes(ctx));

        // Merge intersecting subset constraints
        result.merge(merge_intersect_subsets(ctx));

        // Propagate transitive subset relationships
        result.merge(transitive_subset(ctx));

        result
    }
}

/// Check for literal contradictions (empty sets in constrained predicates).
fn check_literal_contradictions(ctx: &SimplificationContext) -> SimplificationResult {
    let mut result = SimplificationResult::new();

    for (pred_id, pred) in ctx.predicates.iter() {
        if !pred.constrained {
            continue;
        }

        // Check for empty set in subset predicates
        match &pred.kind {
            PredicateKind::IsSubset { left, right } => {
                // If right is empty, that's a contradiction
                if let Some(expr) = ctx.get_expression(*right) {
                    if let Some(lit) = expr.as_literal() {
                        if lit.is_empty() {
                            // Check if left is also empty (which would be fine)
                            if let Some(left_expr) = ctx.get_expression(*left) {
                                if let Some(left_lit) = left_expr.as_literal() {
                                    if !left_lit.is_empty() {
                                        result.set_contradiction(
                                            ContradictionInfo::new(
                                                "Non-empty value constrained to be subset of empty set",
                                            )
                                            .with_predicate(*pred_id)
                                            .with_expression(*left)
                                            .with_expression(*right),
                                        );
                                        return result;
                                    }
                                }
                            }
                        }
                    }
                }

                // If left is non-empty and has no overlap with right, contradiction
                if let (Some(left_expr), Some(right_expr)) =
                    (ctx.get_expression(*left), ctx.get_expression(*right))
                {
                    if let (Some(left_lit), Some(right_lit)) =
                        (left_expr.as_literal(), right_expr.as_literal())
                    {
                        if let (
                            Literal::Quantity(left_q),
                            Literal::Quantity(right_q),
                        ) = (left_lit, right_lit)
                        {
                            // Check if left is a subset of right
                            if !left_q.is_subset_of(right_q) && left_q.is_singleton() {
                                result.set_contradiction(
                                    ContradictionInfo::new(
                                        "Value is not a subset of constraint",
                                    )
                                    .with_predicate(*pred_id)
                                    .with_expression(*left)
                                    .with_expression(*right),
                                );
                                return result;
                            }
                        }
                    }
                }
            }

            PredicateKind::Is { left, right } => {
                // Check if both are singletons with different values
                if let (Some(left_expr), Some(right_expr)) =
                    (ctx.get_expression(*left), ctx.get_expression(*right))
                {
                    if let (Some(left_lit), Some(right_lit)) =
                        (left_expr.as_literal(), right_expr.as_literal())
                    {
                        if left_lit.is_singleton()
                            && right_lit.is_singleton()
                            && left_lit != right_lit
                        {
                            result.set_contradiction(
                                ContradictionInfo::new("Is constraint between different values")
                                    .with_predicate(*pred_id)
                                    .with_expression(*left)
                                    .with_expression(*right),
                            );
                            return result;
                        }
                    }
                }
            }

            _ => {}
        }
    }

    result
}

/// Remove unconstrained, non-terminated predicates.
fn remove_unconstrained(ctx: &mut SimplificationContext) -> SimplificationResult {
    let mut result = SimplificationResult::new();

    let to_remove: Vec<PredicateId> = ctx
        .predicates
        .iter()
        .filter(|(_, pred)| !pred.constrained && !pred.solver_terminated)
        .map(|(id, _)| *id)
        .collect();

    for pred_id in to_remove {
        ctx.predicates.remove(&pred_id);
        result.remove_predicate(pred_id);
    }

    result
}

/// Resolve alias classes from Is predicates.
///
/// When we have `A is B` and `B is C`, we can unify A, B, C into an alias class.
/// This pass just marks Is predicates as terminated when both sides are the same.
fn resolve_alias_classes(ctx: &mut SimplificationContext) -> SimplificationResult {
    let mut result = SimplificationResult::new();

    // Process Is predicates - mark as terminated if both sides are equal
    let pred_ids: Vec<_> = ctx.predicates.keys().copied().collect();
    for pred_id in pred_ids {
        if let Some(pred) = ctx.predicates.get(&pred_id) {
            if !pred.constrained || pred.solver_terminated {
                continue;
            }

            if let PredicateKind::Is { left, right } = &pred.kind {
                // If both sides refer to the same expression, it's trivially true
                if left == right {
                    if let Some(p) = ctx.predicates.get_mut(&pred_id) {
                        p.solver_terminated = true;
                    }
                    result.terminate_predicate(pred_id);
                }
            }
        }
    }

    result
}

/// Merge intersecting subset constraints.
///
/// If we have `X ⊆ A` and `X ⊆ B`, we can merge to `X ⊆ (A ∩ B)`.
fn merge_intersect_subsets(ctx: &mut SimplificationContext) -> SimplificationResult {
    let mut result = SimplificationResult::new();

    // Group subset predicates by their left operand
    let mut subset_by_left: HashMap<ExpressionId, Vec<(PredicateId, ExpressionId)>> =
        HashMap::new();

    for (pred_id, pred) in ctx.predicates.iter() {
        if !pred.constrained {
            continue;
        }

        if let PredicateKind::IsSubset { left, right } = &pred.kind {
            subset_by_left
                .entry(*left)
                .or_default()
                .push((*pred_id, *right));
        }
    }

    // For each left operand with multiple constraints, intersect the right operands
    for (left, constraints) in subset_by_left.iter() {
        if constraints.len() < 2 {
            continue;
        }

        // Check if all right operands are literals we can intersect
        let mut right_lits: Vec<&ato_domain::QuantityIntervalDisjoint> = Vec::new();
        for (_, right) in constraints {
            if let Some(expr) = ctx.get_expression(*right) {
                if let Some(Literal::Quantity(q)) = expr.as_literal() {
                    right_lits.push(q);
                } else {
                    continue;
                }
            }
        }

        if right_lits.len() == constraints.len() && right_lits.len() >= 2 {
            // Intersect all the right operands
            let mut intersection = right_lits[0].clone();
            for q in &right_lits[1..] {
                if let Ok(intersected) = intersection.intersect(q) {
                    intersection = intersected;
                } else {
                    break;
                }
            }

            // If intersection is empty, that's a contradiction
            if intersection.is_empty() {
                result.set_contradiction(
                    ContradictionInfo::new("Subset constraints have empty intersection")
                        .with_expression(*left),
                );
                return result;
            }

            // TODO: Create new expression and predicate with intersection
            // Don't mark dirty unless we actually change something
        }
    }

    result
}

/// Propagate transitive subset relationships.
///
/// If we have `A ⊆ B` and `B ⊆ C`, then `A ⊆ C`.
/// Note: This is a placeholder - we don't yet add new predicates for transitive relationships.
fn transitive_subset(_ctx: &SimplificationContext) -> SimplificationResult {
    // For now, just return without making changes.
    // A full implementation would need to add new predicates for discovered transitive relationships.
    SimplificationResult::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::ExpressionStore;
    use crate::predicate::Predicate;

    fn setup_ctx<'a>(
        store: &'a mut ExpressionStore,
        predicates: &'a mut HashMap<PredicateId, Predicate>,
    ) -> SimplificationContext<'a> {
        SimplificationContext::new(store, predicates)
    }

    #[test]
    fn test_remove_unconstrained() {
        let mut store = ExpressionStore::new();
        let mut predicates = HashMap::new();

        let left = ExpressionId::new();
        let right = ExpressionId::new();

        // Add unconstrained predicate
        let pred = Predicate::is(left, right);
        let pred_id = pred.id;
        predicates.insert(pred_id, pred);

        // Add constrained predicate
        let pred2 = Predicate::is(left, right).constrain();
        let pred_id2 = pred2.id;
        predicates.insert(pred_id2, pred2);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = remove_unconstrained(&mut ctx);

        assert!(result.dirty);
        assert!(!predicates.contains_key(&pred_id));
        assert!(predicates.contains_key(&pred_id2));
    }

    #[test]
    fn test_resolve_alias_classes() {
        let mut store = ExpressionStore::new();
        let mut predicates = HashMap::new();

        let a = ExpressionId::new();

        // A is A (trivially true)
        let pred1 = Predicate::is(a, a).constrain();
        predicates.insert(pred1.id, pred1);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = resolve_alias_classes(&mut ctx);

        // Should mark the trivially true predicate as terminated
        assert!(result.dirty);
        assert_eq!(result.terminated_predicates.len(), 1);
    }
}
