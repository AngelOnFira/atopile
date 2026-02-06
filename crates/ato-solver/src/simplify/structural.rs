//! Structural simplification pass.
//!
//! This pass performs structural simplifications like:
//! - Removing unconstrained predicates
//! - Detecting literal contradictions
//! - Propagating known values
//! - Narrowing parameter domains from constraints

use super::{
    ContradictionInfo, SimplificationContext, SimplificationPass, SimplificationResult,
};
use crate::expression::{Expression, ExpressionId, ExpressionKind, Literal, ParameterId};
use crate::predicate::{Predicate, PredicateId, PredicateKind};
use ato_domain::{QuantityIntervalDisjoint, Unit};
use std::collections::HashMap;

/// Structural simplification pass.
///
/// Performs high-level structural simplifications:
/// - Removes unconstrained, non-terminated predicates
/// - Detects empty set contradictions
/// - Resolves alias classes (Is predicates)
/// - Narrows parameter domains from constraints
/// - Merges intersecting subset constraints
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

        // Narrow parameter domains from constraints
        result.merge(narrow_parameter_domains(ctx));
        if result.contradiction.is_some() {
            return result;
        }

        // Merge intersecting subset constraints
        result.merge(merge_intersect_subsets(ctx));
        if result.contradiction.is_some() {
            return result;
        }

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

        match &pred.kind {
            PredicateKind::IsSubset { left, right } => {
                // If right is empty, that's a contradiction
                if let Some(expr) = ctx.get_expression(*right) {
                    if let Some(lit) = expr.as_literal() {
                        if lit.is_empty() {
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

                if let (Some(left_expr), Some(right_expr)) =
                    (ctx.get_expression(*left), ctx.get_expression(*right))
                {
                    if let (Some(left_lit), Some(right_lit)) =
                        (left_expr.as_literal(), right_expr.as_literal())
                    {
                        if let (Literal::Quantity(left_q), Literal::Quantity(right_q)) =
                            (left_lit, right_lit)
                        {
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
fn resolve_alias_classes(ctx: &mut SimplificationContext) -> SimplificationResult {
    let mut result = SimplificationResult::new();

    let pred_ids: Vec<_> = ctx.predicates.keys().copied().collect();
    for pred_id in pred_ids {
        if let Some(pred) = ctx.predicates.get(&pred_id) {
            if !pred.constrained || pred.solver_terminated {
                continue;
            }

            if let PredicateKind::Is { left, right } = &pred.kind {
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

/// Helper: resolve an expression ID to its parameter ID.
fn expr_as_param(ctx: &SimplificationContext, expr_id: ExpressionId) -> Option<ParameterId> {
    ctx.get_expression(expr_id)?.as_parameter()
}

/// Helper: resolve an expression ID to a literal quantity interval.
fn expr_as_quantity(
    ctx: &SimplificationContext,
    expr_id: ExpressionId,
) -> Option<QuantityIntervalDisjoint> {
    let expr = ctx.get_expression(expr_id)?;
    if let Some(Literal::Quantity(q)) = expr.as_literal() {
        Some(q.clone())
    } else {
        None
    }
}

/// Create a half-bounded interval as a Literal.
fn make_upper_bound(max_val: f64, unit: Unit) -> Literal {
    Literal::from_interval(f64::NEG_INFINITY, max_val, unit)
}

/// Create a half-bounded interval as a Literal.
fn make_lower_bound(min_val: f64, unit: Unit) -> Literal {
    Literal::from_interval(min_val, f64::INFINITY, unit)
}

/// Narrow parameter domains from constraints.
///
/// For each constrained predicate involving a parameter on one side and a literal on the other,
/// narrow the parameter's domain by intersecting with the constraint.
fn narrow_parameter_domains(ctx: &mut SimplificationContext) -> SimplificationResult {
    let mut result = SimplificationResult::new();

    // Collect narrowing operations: param_id -> new domain constraints
    let mut narrowings: HashMap<ParameterId, Vec<QuantityIntervalDisjoint>> = HashMap::new();

    let pred_ids: Vec<_> = ctx.predicates.keys().copied().collect();
    for pred_id in &pred_ids {
        let pred = match ctx.predicates.get(pred_id) {
            Some(p) => p.clone(),
            None => continue,
        };

        if !pred.constrained || pred.solver_terminated {
            continue;
        }

        match &pred.kind {
            PredicateKind::IsSubset { left, right } => {
                if let Some(param_id) = expr_as_param(ctx, *left) {
                    if let Some(interval) = expr_as_quantity(ctx, *right) {
                        narrowings.entry(param_id).or_default().push(interval);
                    }
                }
            }

            PredicateKind::Is { left, right } => {
                if let Some(param_id) = expr_as_param(ctx, *left) {
                    if let Some(interval) = expr_as_quantity(ctx, *right) {
                        narrowings.entry(param_id).or_default().push(interval);
                    }
                }
                if let Some(param_id) = expr_as_param(ctx, *right) {
                    if let Some(interval) = expr_as_quantity(ctx, *left) {
                        narrowings.entry(param_id).or_default().push(interval);
                    }
                }
            }

            PredicateKind::LessOrEqual { left, right } => {
                // param <= literal -> param in (-inf, literal_max]
                if let Some(param_id) = expr_as_param(ctx, *left) {
                    if let Some(interval) = expr_as_quantity(ctx, *right) {
                        if let Some(max_q) = interval.max() {
                            let bound = make_upper_bound(max_q.value(), interval.unit());
                            if let Literal::Quantity(q) = bound {
                                narrowings.entry(param_id).or_default().push(q);
                            }
                        }
                    }
                }
                // literal <= param -> param in [literal_min, +inf)
                if let Some(param_id) = expr_as_param(ctx, *right) {
                    if let Some(interval) = expr_as_quantity(ctx, *left) {
                        if let Some(min_q) = interval.min() {
                            let bound = make_lower_bound(min_q.value(), interval.unit());
                            if let Literal::Quantity(q) = bound {
                                narrowings.entry(param_id).or_default().push(q);
                            }
                        }
                    }
                }
            }

            PredicateKind::LessThan { left, right } => {
                // param < literal -> param in (-inf, literal_min]
                if let Some(param_id) = expr_as_param(ctx, *left) {
                    if let Some(interval) = expr_as_quantity(ctx, *right) {
                        if let Some(min_q) = interval.min() {
                            let bound = make_upper_bound(min_q.value(), interval.unit());
                            if let Literal::Quantity(q) = bound {
                                narrowings.entry(param_id).or_default().push(q);
                            }
                        }
                    }
                }
                // literal < param -> param in [literal_max, +inf)
                if let Some(param_id) = expr_as_param(ctx, *right) {
                    if let Some(interval) = expr_as_quantity(ctx, *left) {
                        if let Some(max_q) = interval.max() {
                            let bound = make_lower_bound(max_q.value(), interval.unit());
                            if let Literal::Quantity(q) = bound {
                                narrowings.entry(param_id).or_default().push(q);
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }

    // Apply narrowings
    for (param_id, constraints) in narrowings {
        let param = match ctx.expressions.get_parameter(param_id) {
            Some(p) => p.clone(),
            None => continue,
        };

        let mut domain = param.domain.clone();
        let mut changed = false;

        for constraint in &constraints {
            if let Ok(narrowed) = domain.intersect(constraint) {
                if narrowed != domain {
                    domain = narrowed;
                    changed = true;
                }
            }
        }

        if !changed {
            continue;
        }

        // Empty domain -> contradiction
        if domain.is_empty() {
            result.set_contradiction(
                ContradictionInfo::new(format!(
                    "Parameter '{}' domain narrowed to empty set",
                    param.display_name()
                )),
            );
            return result;
        }

        // Update the parameter's domain
        if let Some(p) = ctx.expressions.get_parameter_mut(param_id) {
            p.domain = domain.clone();
            p.known_superset = Some(domain.clone());
        }

        // Replace all expressions referencing this parameter with the narrowed domain
        let expr_ids: Vec<_> = ctx.expressions.expression_ids().collect();
        for expr_id in expr_ids {
            let is_param_ref = ctx
                .expressions
                .get_expression(expr_id)
                .map(|e| matches!(&e.kind, ExpressionKind::Parameter(pid) if *pid == param_id))
                .unwrap_or(false);

            if is_param_ref {
                let new_expr = Expression::literal(Literal::Quantity(domain.clone()));
                let new_id = new_expr.id;
                ctx.expressions.add_expression(new_expr);

                // Update all predicates that reference this expression
                let pred_ids: Vec<_> = ctx.predicates.keys().copied().collect();
                for pid in pred_ids {
                    if let Some(pred) = ctx.predicates.get_mut(&pid) {
                        replace_expr_in_predicate(pred, expr_id, new_id);
                    }
                }

                result.replace_expression(expr_id, new_id);
            }
        }
    }

    result
}

/// Replace references to old_id with new_id in a predicate.
fn replace_expr_in_predicate(pred: &mut Predicate, old_id: ExpressionId, new_id: ExpressionId) {
    match &mut pred.kind {
        PredicateKind::Is { left, right }
        | PredicateKind::LessOrEqual { left, right }
        | PredicateKind::LessThan { left, right }
        | PredicateKind::NotEqual { left, right }
        | PredicateKind::IsSubset { left, right }
        | PredicateKind::IsSuperset { left, right }
        | PredicateKind::GreaterOrEqual { left, right }
        | PredicateKind::GreaterThan { left, right } => {
            if *left == old_id {
                *left = new_id;
            }
            if *right == old_id {
                *right = new_id;
            }
        }
        PredicateKind::Within { value, tolerance } => {
            if *value == old_id {
                *value = new_id;
            }
            if *tolerance == old_id {
                *tolerance = new_id;
            }
        }
        _ => {}
    }
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
        if !pred.constrained || pred.solver_terminated {
            continue;
        }

        if let PredicateKind::IsSubset { left, right } = &pred.kind {
            subset_by_left
                .entry(*left)
                .or_default()
                .push((*pred_id, *right));
        }
    }

    for (left, constraints) in subset_by_left.iter() {
        if constraints.len() < 2 {
            continue;
        }

        let mut right_lits: Vec<&ato_domain::QuantityIntervalDisjoint> = Vec::new();
        let mut all_literal = true;
        for (_, right) in constraints {
            if let Some(expr) = ctx.get_expression(*right) {
                if let Some(Literal::Quantity(q)) = expr.as_literal() {
                    right_lits.push(q);
                } else {
                    all_literal = false;
                    break;
                }
            } else {
                all_literal = false;
                break;
            }
        }

        if !all_literal || right_lits.len() < 2 {
            continue;
        }

        let mut intersection = right_lits[0].clone();
        let mut ok = true;
        for q in &right_lits[1..] {
            if let Ok(intersected) = intersection.intersect(q) {
                intersection = intersected;
            } else {
                ok = false;
                break;
            }
        }

        if !ok {
            continue;
        }

        if intersection.is_empty() {
            result.set_contradiction(
                ContradictionInfo::new("Subset constraints have empty intersection")
                    .with_expression(*left),
            );
            return result;
        }

        // Create merged predicate with intersection
        let new_expr = Expression::literal(Literal::Quantity(intersection));
        let new_expr_id = new_expr.id;
        ctx.expressions.add_expression(new_expr);

        let new_pred = Predicate::is_subset(*left, new_expr_id).constrain();
        ctx.predicates.insert(new_pred.id, new_pred);

        // Remove old predicates
        for (pred_id, _) in constraints {
            ctx.predicates.remove(pred_id);
            result.remove_predicate(*pred_id);
        }

        result.mark_dirty();
    }

    result
}

/// Propagate transitive subset relationships.
///
/// If we have `A ⊆ B` and `B ⊆ C`, then `A ⊆ C`.
fn transitive_subset(ctx: &mut SimplificationContext) -> SimplificationResult {
    let mut result = SimplificationResult::new();

    let mut subset_map: HashMap<ExpressionId, Vec<(PredicateId, ExpressionId)>> = HashMap::new();

    for (pred_id, pred) in ctx.predicates.iter() {
        if !pred.constrained || pred.solver_terminated {
            continue;
        }

        if let PredicateKind::IsSubset { left, right } = &pred.kind {
            subset_map
                .entry(*left)
                .or_default()
                .push((*pred_id, *right));
        }
    }

    let mut new_predicates = Vec::new();
    let existing_pairs: std::collections::HashSet<(ExpressionId, ExpressionId)> = ctx
        .predicates
        .values()
        .filter_map(|p| {
            if let PredicateKind::IsSubset { left, right } = &p.kind {
                Some((*left, *right))
            } else {
                None
            }
        })
        .collect();

    for (a, a_rights) in &subset_map {
        for (_, b) in a_rights {
            if let Some(b_rights) = subset_map.get(b) {
                for (_, c) in b_rights {
                    if a != c && !existing_pairs.contains(&(*a, *c)) {
                        new_predicates.push((*a, *c));
                    }
                }
            }
        }
    }

    for (a, c) in new_predicates {
        let pred = Predicate::is_subset(a, c).constrain();
        ctx.predicates.insert(pred.id, pred);
        result.mark_dirty();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::{Expression, ExpressionStore, Parameter};
    use crate::predicate::Predicate;
    use ato_domain::Unit;

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

        let pred = Predicate::is(left, right);
        let pred_id = pred.id;
        predicates.insert(pred_id, pred);

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

        let pred1 = Predicate::is(a, a).constrain();
        predicates.insert(pred1.id, pred1);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = resolve_alias_classes(&mut ctx);

        assert!(result.dirty);
        assert_eq!(result.terminated_predicates.len(), 1);
    }

    #[test]
    fn test_narrow_parameter_from_subset() {
        let mut store = ExpressionStore::new();
        let mut predicates = HashMap::new();

        let param = Parameter::new(Unit::Ohm).with_name("resistance");
        let param_id = store.add_parameter(param);

        let param_expr = Expression::parameter(param_id, Unit::Ohm);
        let param_expr_id = store.add_expression(param_expr);

        // 10kohm +/- 10% = [9000, 11000]
        let interval_expr =
            Expression::literal(Literal::from_interval(9000.0, 11000.0, Unit::Ohm));
        let interval_expr_id = store.add_expression(interval_expr);

        let pred = Predicate::is_subset(param_expr_id, interval_expr_id).constrain();
        predicates.insert(pred.id, pred);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = narrow_parameter_domains(&mut ctx);

        assert!(result.dirty);

        let param = store.get_parameter(param_id).unwrap();
        let min = param.domain.min().unwrap().value();
        let max = param.domain.max().unwrap().value();
        assert!((min - 9000.0).abs() < 1.0);
        assert!((max - 11000.0).abs() < 1.0);
    }

    #[test]
    fn test_narrow_parameter_multiple_constraints() {
        let mut store = ExpressionStore::new();
        let mut predicates = HashMap::new();

        let param = Parameter::new(Unit::Volt).with_name("voltage");
        let param_id = store.add_parameter(param);

        let param_expr = Expression::parameter(param_id, Unit::Volt);
        let param_expr_id = store.add_expression(param_expr);

        let interval1 = Expression::literal(Literal::from_interval(3.0, 5.0, Unit::Volt));
        let interval1_id = store.add_expression(interval1);
        let pred1 = Predicate::is_subset(param_expr_id, interval1_id).constrain();
        predicates.insert(pred1.id, pred1);

        let interval2 = Expression::literal(Literal::from_interval(4.0, 6.0, Unit::Volt));
        let interval2_id = store.add_expression(interval2);
        let pred2 = Predicate::is_subset(param_expr_id, interval2_id).constrain();
        predicates.insert(pred2.id, pred2);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = narrow_parameter_domains(&mut ctx);

        assert!(result.dirty);

        let param = store.get_parameter(param_id).unwrap();
        let min = param.domain.min().unwrap().value();
        let max = param.domain.max().unwrap().value();
        assert!((min - 4.0).abs() < 0.01);
        assert!((max - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_narrow_parameter_contradiction() {
        let mut store = ExpressionStore::new();
        let mut predicates = HashMap::new();

        let param = Parameter::new(Unit::Volt).with_name("voltage");
        let param_id = store.add_parameter(param);

        let param_expr = Expression::parameter(param_id, Unit::Volt);
        let param_expr_id = store.add_expression(param_expr);

        let interval1 = Expression::literal(Literal::from_interval(1.0, 2.0, Unit::Volt));
        let interval1_id = store.add_expression(interval1);
        let pred1 = Predicate::is_subset(param_expr_id, interval1_id).constrain();
        predicates.insert(pred1.id, pred1);

        let interval2 = Expression::literal(Literal::from_interval(5.0, 6.0, Unit::Volt));
        let interval2_id = store.add_expression(interval2);
        let pred2 = Predicate::is_subset(param_expr_id, interval2_id).constrain();
        predicates.insert(pred2.id, pred2);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = narrow_parameter_domains(&mut ctx);

        assert!(result.contradiction.is_some());
    }

    #[test]
    fn test_narrow_parameter_less_or_equal() {
        let mut store = ExpressionStore::new();
        let mut predicates = HashMap::new();

        let param = Parameter::new(Unit::Volt).with_name("voltage");
        let param_id = store.add_parameter(param);

        let param_expr = Expression::parameter(param_id, Unit::Volt);
        let param_expr_id = store.add_expression(param_expr);

        let five_v = Expression::literal(Literal::from_quantity(5.0, Unit::Volt));
        let five_v_id = store.add_expression(five_v);
        let pred = Predicate::less_or_equal(param_expr_id, five_v_id).constrain();
        predicates.insert(pred.id, pred);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = narrow_parameter_domains(&mut ctx);

        assert!(result.dirty);

        let param = store.get_parameter(param_id).unwrap();
        let max = param.domain.max().unwrap().value();
        assert!((max - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_merge_intersect_subsets() {
        let mut store = ExpressionStore::new();
        let mut predicates = HashMap::new();

        let x_expr = Expression::literal(Literal::from_quantity(10000.0, Unit::Ohm));
        let x_id = store.add_expression(x_expr);

        let a_expr = Expression::literal(Literal::from_interval(8000.0, 12000.0, Unit::Ohm));
        let a_id = store.add_expression(a_expr);
        let pred1 = Predicate::is_subset(x_id, a_id).constrain();
        let pred1_id = pred1.id;
        predicates.insert(pred1_id, pred1);

        let b_expr = Expression::literal(Literal::from_interval(9000.0, 15000.0, Unit::Ohm));
        let b_id = store.add_expression(b_expr);
        let pred2 = Predicate::is_subset(x_id, b_id).constrain();
        let pred2_id = pred2.id;
        predicates.insert(pred2_id, pred2);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = merge_intersect_subsets(&mut ctx);

        assert!(result.dirty);
        assert!(!predicates.contains_key(&pred1_id));
        assert!(!predicates.contains_key(&pred2_id));
        assert_eq!(predicates.len(), 1);
    }

    #[test]
    fn test_transitive_subset() {
        let mut store = ExpressionStore::new();
        let mut predicates = HashMap::new();

        let a = ExpressionId::new();
        let b = ExpressionId::new();
        let c = ExpressionId::new();

        let pred1 = Predicate::is_subset(a, b).constrain();
        predicates.insert(pred1.id, pred1);

        let pred2 = Predicate::is_subset(b, c).constrain();
        predicates.insert(pred2.id, pred2);

        let mut ctx = setup_ctx(&mut store, &mut predicates);
        let result = transitive_subset(&mut ctx);

        assert!(result.dirty);
        assert_eq!(predicates.len(), 3);

        let has_transitive = predicates.values().any(|p| {
            if let PredicateKind::IsSubset { left, right } = &p.kind {
                *left == a && *right == c
            } else {
                false
            }
        });
        assert!(has_transitive);
    }
}
