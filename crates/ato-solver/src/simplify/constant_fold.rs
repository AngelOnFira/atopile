//! Constant folding pass.
//!
//! This pass evaluates expressions with literal operands.

use super::{
    replace_expr_in_predicate, ContradictionInfo, SimplificationContext, SimplificationPass,
    SimplificationResult,
};
use crate::expression::{ArithmeticOp, Expression, ExpressionId, ExpressionKind, Literal};
use crate::predicate::{Predicate, PredicateKind};
use ato_domain::QuantityIntervalDisjoint;

/// Constant folding pass.
///
/// Evaluates expressions where all operands are literals:
/// - Arithmetic: 2 + 3 -> 5
/// - Comparisons: 2 < 3 -> true
/// - Set operations: [1,3] intersect [2,4] -> [2,3]
pub struct ConstantFoldPass;

impl SimplificationPass for ConstantFoldPass {
    fn name(&self) -> &'static str {
        "constant_fold"
    }

    fn run(&self, ctx: &mut SimplificationContext) -> SimplificationResult {
        let mut result = SimplificationResult::new();

        // Fold arithmetic expressions
        let expr_ids: Vec<_> = ctx.expressions.expression_ids().collect();
        for expr_id in expr_ids {
            if let Some(expr) = ctx.expressions.get_expression(expr_id) {
                let expr = expr.clone();
                if let Some(folded) = try_fold_expression(&expr, ctx) {
                    let new_expr = Expression::literal(folded);
                    let new_id = new_expr.id;
                    ctx.expressions.add_expression(new_expr);

                    // Update all predicates that reference the old expression
                    let pred_ids: Vec<_> = ctx.predicates.keys().copied().collect();
                    for pid in pred_ids {
                        if let Some(pred) = ctx.predicates.get_mut(&pid) {
                            replace_expr_in_predicate(pred, expr_id, new_id);
                        }
                    }

                    // Remove the old arithmetic expression so it isn't re-folded
                    ctx.expressions.remove_expression(expr_id);
                    result.replace_expression(expr_id, new_id);
                }
            }
        }

        // Evaluate predicate with literal operands
        let pred_ids: Vec<_> = ctx.predicates.keys().copied().collect();
        for pred_id in pred_ids {
            if let Some(pred) = ctx.predicates.get(&pred_id) {
                // Skip already-terminated predicates
                if pred.solver_terminated {
                    continue;
                }
                let pred = pred.clone();
                if let Some(value) = try_evaluate_predicate(&pred, ctx) {
                    if value {
                        // Predicate is true - mark as terminated
                        if let Some(p) = ctx.predicates.get_mut(&pred_id) {
                            p.solver_terminated = true;
                        }
                        result.terminate_predicate(pred_id);
                    } else {
                        // Predicate is false - contradiction if constrained
                        if pred.constrained {
                            result.set_contradiction(
                                ContradictionInfo::new("Predicate evaluated to false")
                                    .with_predicate(pred_id),
                            );
                        } else {
                            result.remove_predicate(pred_id);
                        }
                    }
                }
            }
        }

        result
    }
}

/// Try to fold an expression with literal operands.
fn try_fold_expression(expr: &Expression, ctx: &SimplificationContext) -> Option<Literal> {
    match &expr.kind {
        ExpressionKind::Literal(_) => None, // Already a literal
        ExpressionKind::Parameter(_) => None, // Can't fold parameters
        ExpressionKind::Arithmetic { op, operands } => {
            fold_arithmetic(*op, operands, ctx)
        }
        ExpressionKind::Union(operands) => fold_set_union(operands, ctx),
        ExpressionKind::Intersection(operands) => fold_set_intersection(operands, ctx),
        ExpressionKind::Difference(operands) => fold_set_difference(operands, ctx),
    }
}

/// Try to fold an arithmetic expression.
fn fold_arithmetic(
    op: ArithmeticOp,
    operands: &[ExpressionId],
    ctx: &SimplificationContext,
) -> Option<Literal> {
    // Get all operand literals
    let mut lits: Vec<Literal> = Vec::new();
    for &operand_id in operands {
        let expr = ctx.get_expression(operand_id)?;
        let lit = expr.as_literal()?.clone();
        lits.push(lit);
    }

    match op {
        ArithmeticOp::Add => fold_add(&lits),
        ArithmeticOp::Subtract => fold_subtract(&lits),
        ArithmeticOp::Multiply => fold_multiply(&lits),
        ArithmeticOp::Divide => fold_divide(&lits),
        ArithmeticOp::Negate => fold_negate(&lits),
        ArithmeticOp::Abs => fold_abs(&lits),
        _ => None,
    }
}

/// Fold addition of literals.
fn fold_add(lits: &[Literal]) -> Option<Literal> {
    if lits.len() != 2 {
        return None;
    }

    match (&lits[0], &lits[1]) {
        (Literal::Integer(a), Literal::Integer(b)) => Some(Literal::Integer(a + b)),
        (Literal::Float(a), Literal::Float(b)) => Some(Literal::Float(a + b)),
        (Literal::Integer(a), Literal::Float(b)) => Some(Literal::Float(*a as f64 + b)),
        (Literal::Float(a), Literal::Integer(b)) => Some(Literal::Float(a + *b as f64)),
        (Literal::Quantity(a), Literal::Quantity(b)) => {
            a.add(b).ok().map(Literal::Quantity)
        }
        _ => None,
    }
}

/// Fold subtraction of literals.
fn fold_subtract(lits: &[Literal]) -> Option<Literal> {
    if lits.len() != 2 {
        return None;
    }

    match (&lits[0], &lits[1]) {
        (Literal::Integer(a), Literal::Integer(b)) => Some(Literal::Integer(a - b)),
        (Literal::Float(a), Literal::Float(b)) => Some(Literal::Float(a - b)),
        (Literal::Integer(a), Literal::Float(b)) => Some(Literal::Float(*a as f64 - b)),
        (Literal::Float(a), Literal::Integer(b)) => Some(Literal::Float(a - *b as f64)),
        (Literal::Quantity(a), Literal::Quantity(b)) => {
            a.subtract(b).ok().map(Literal::Quantity)
        }
        _ => None,
    }
}

/// Fold multiplication of literals.
fn fold_multiply(lits: &[Literal]) -> Option<Literal> {
    if lits.len() != 2 {
        return None;
    }

    match (&lits[0], &lits[1]) {
        (Literal::Integer(a), Literal::Integer(b)) => Some(Literal::Integer(a * b)),
        (Literal::Float(a), Literal::Float(b)) => Some(Literal::Float(a * b)),
        (Literal::Integer(a), Literal::Float(b)) => Some(Literal::Float(*a as f64 * b)),
        (Literal::Float(a), Literal::Integer(b)) => Some(Literal::Float(a * *b as f64)),
        (Literal::Quantity(a), Literal::Quantity(b)) => a.multiply(b).ok().map(Literal::Quantity),
        (Literal::Integer(a), Literal::Quantity(b)) => Some(Literal::Quantity(b.scale(*a as f64))),
        (Literal::Quantity(a), Literal::Integer(b)) => Some(Literal::Quantity(a.scale(*b as f64))),
        (Literal::Float(a), Literal::Quantity(b)) => Some(Literal::Quantity(b.scale(*a))),
        (Literal::Quantity(a), Literal::Float(b)) => Some(Literal::Quantity(a.scale(*b))),
        _ => None,
    }
}

/// Fold division of literals.
fn fold_divide(lits: &[Literal]) -> Option<Literal> {
    if lits.len() != 2 {
        return None;
    }

    match (&lits[0], &lits[1]) {
        (Literal::Integer(a), Literal::Integer(b)) if *b != 0 => {
            Some(Literal::Float(*a as f64 / *b as f64))
        }
        (Literal::Float(a), Literal::Float(b)) if *b != 0.0 => Some(Literal::Float(a / b)),
        (Literal::Integer(a), Literal::Float(b)) if *b != 0.0 => {
            Some(Literal::Float(*a as f64 / b))
        }
        (Literal::Float(a), Literal::Integer(b)) if *b != 0 => {
            Some(Literal::Float(a / *b as f64))
        }
        (Literal::Quantity(a), Literal::Quantity(b)) => a.divide(b).ok().map(Literal::Quantity),
        (Literal::Quantity(a), Literal::Integer(b)) if *b != 0 => {
            Some(Literal::Quantity(a.scale(1.0 / *b as f64)))
        }
        (Literal::Quantity(a), Literal::Float(b)) if *b != 0.0 => {
            Some(Literal::Quantity(a.scale(1.0 / *b)))
        }
        _ => None,
    }
}

/// Fold negation of a literal.
fn fold_negate(lits: &[Literal]) -> Option<Literal> {
    if lits.len() != 1 {
        return None;
    }

    match &lits[0] {
        Literal::Integer(a) => Some(Literal::Integer(-a)),
        Literal::Float(a) => Some(Literal::Float(-a)),
        Literal::Quantity(a) => Some(Literal::Quantity(a.negate())),
        _ => None,
    }
}

/// Fold absolute value of a literal.
fn fold_abs(lits: &[Literal]) -> Option<Literal> {
    if lits.len() != 1 {
        return None;
    }

    match &lits[0] {
        Literal::Integer(a) => Some(Literal::Integer(a.abs())),
        Literal::Float(a) => Some(Literal::Float(a.abs())),
        _ => None,
    }
}

/// Fold set union.
fn fold_set_union(operands: &[ExpressionId], ctx: &SimplificationContext) -> Option<Literal> {
    let mut lits: Vec<&QuantityIntervalDisjoint> = Vec::new();
    for &operand_id in operands {
        let expr = ctx.get_expression(operand_id)?;
        let lit = expr.as_literal()?;
        match lit {
            Literal::Quantity(q) => lits.push(q),
            _ => return None,
        }
    }

    if lits.is_empty() {
        return None;
    }

    let mut result = lits[0].clone();
    for q in &lits[1..] {
        result = result.union(q).ok()?;
    }

    Some(Literal::Quantity(result))
}

/// Fold set intersection.
fn fold_set_intersection(
    operands: &[ExpressionId],
    ctx: &SimplificationContext,
) -> Option<Literal> {
    let mut lits: Vec<&QuantityIntervalDisjoint> = Vec::new();
    for &operand_id in operands {
        let expr = ctx.get_expression(operand_id)?;
        let lit = expr.as_literal()?;
        match lit {
            Literal::Quantity(q) => lits.push(q),
            _ => return None,
        }
    }

    if lits.is_empty() {
        return None;
    }

    let mut result = lits[0].clone();
    for q in &lits[1..] {
        result = result.intersect(q).ok()?;
    }

    Some(Literal::Quantity(result))
}

/// Fold set difference.
fn fold_set_difference(operands: &[ExpressionId], ctx: &SimplificationContext) -> Option<Literal> {
    if operands.len() < 2 {
        return None;
    }

    let mut lits: Vec<&QuantityIntervalDisjoint> = Vec::new();
    for &operand_id in operands {
        let expr = ctx.get_expression(operand_id)?;
        let lit = expr.as_literal()?;
        match lit {
            Literal::Quantity(q) => lits.push(q),
            _ => return None,
        }
    }

    let mut result = lits[0].clone();
    for q in &lits[1..] {
        result = result.difference(q).ok()?;
    }

    Some(Literal::Quantity(result))
}

/// Try to evaluate a predicate with literal operands.
fn try_evaluate_predicate(pred: &Predicate, ctx: &SimplificationContext) -> Option<bool> {
    match &pred.kind {
        PredicateKind::Is { left, right } => {
            let left_expr = ctx.get_expression(*left)?;
            let right_expr = ctx.get_expression(*right)?;
            let left_lit = left_expr.as_literal()?;
            let right_lit = right_expr.as_literal()?;
            Some(left_lit == right_lit)
        }

        PredicateKind::LessOrEqual { left, right } => {
            let left_expr = ctx.get_expression(*left)?;
            let right_expr = ctx.get_expression(*right)?;
            evaluate_less_or_equal(left_expr.as_literal()?, right_expr.as_literal()?)
        }

        PredicateKind::LessThan { left, right } => {
            let left_expr = ctx.get_expression(*left)?;
            let right_expr = ctx.get_expression(*right)?;
            evaluate_less_than(left_expr.as_literal()?, right_expr.as_literal()?)
        }

        PredicateKind::NotEqual { left, right } => {
            let left_expr = ctx.get_expression(*left)?;
            let right_expr = ctx.get_expression(*right)?;
            let left_lit = left_expr.as_literal()?;
            let right_lit = right_expr.as_literal()?;
            Some(left_lit != right_lit)
        }

        PredicateKind::IsSubset { left, right } => {
            let left_expr = ctx.get_expression(*left)?;
            let right_expr = ctx.get_expression(*right)?;
            evaluate_is_subset(left_expr.as_literal()?, right_expr.as_literal()?)
        }

        PredicateKind::True => Some(true),
        PredicateKind::False => Some(false),

        PredicateKind::And(preds) => {
            let mut all_true = true;
            for &pred_id in preds {
                let p = ctx.get_predicate(pred_id)?;
                match try_evaluate_predicate(p, ctx) {
                    Some(true) => continue,
                    Some(false) => return Some(false),
                    None => all_true = false,
                }
            }
            if all_true {
                Some(true)
            } else {
                None
            }
        }

        PredicateKind::Or(preds) => {
            let mut all_false = true;
            for &pred_id in preds {
                let p = ctx.get_predicate(pred_id)?;
                match try_evaluate_predicate(p, ctx) {
                    Some(true) => return Some(true),
                    Some(false) => continue,
                    None => all_false = false,
                }
            }
            if all_false {
                Some(false)
            } else {
                None
            }
        }

        PredicateKind::Not(pred_id) => {
            let p = ctx.get_predicate(*pred_id)?;
            try_evaluate_predicate(p, ctx).map(|v| !v)
        }

        _ => None,
    }
}

/// Evaluate less-than-or-equal for literals.
fn evaluate_less_or_equal(left: &Literal, right: &Literal) -> Option<bool> {
    match (left, right) {
        (Literal::Integer(a), Literal::Integer(b)) => Some(a <= b),
        (Literal::Float(a), Literal::Float(b)) => Some(a <= b),
        (Literal::Integer(a), Literal::Float(b)) => Some((*a as f64) <= *b),
        (Literal::Float(a), Literal::Integer(b)) => Some(*a <= (*b as f64)),
        (Literal::Quantity(a), Literal::Quantity(b)) => {
            // For intervals, check if max(a) <= min(b)
            let a_max = a.max()?;
            let b_min = b.min()?;
            if a_max.unit() != b_min.unit() {
                return None;
            }
            Some(a_max.value() <= b_min.value())
        }
        _ => None,
    }
}

/// Evaluate less-than for literals.
fn evaluate_less_than(left: &Literal, right: &Literal) -> Option<bool> {
    match (left, right) {
        (Literal::Integer(a), Literal::Integer(b)) => Some(a < b),
        (Literal::Float(a), Literal::Float(b)) => Some(a < b),
        (Literal::Integer(a), Literal::Float(b)) => Some((*a as f64) < *b),
        (Literal::Float(a), Literal::Integer(b)) => Some(*a < (*b as f64)),
        (Literal::Quantity(a), Literal::Quantity(b)) => {
            let a_max = a.max()?;
            let b_min = b.min()?;
            if a_max.unit() != b_min.unit() {
                return None;
            }
            Some(a_max.value() < b_min.value())
        }
        _ => None,
    }
}

/// Evaluate is-subset for literals.
fn evaluate_is_subset(left: &Literal, right: &Literal) -> Option<bool> {
    match (left, right) {
        (Literal::Quantity(a), Literal::Quantity(b)) => Some(a.is_subset_of(b)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ato_domain::Unit;

    #[test]
    fn test_fold_add_integers() {
        let lits = vec![Literal::Integer(2), Literal::Integer(3)];
        let result = fold_add(&lits).unwrap();
        assert_eq!(result, Literal::Integer(5));
    }

    #[test]
    fn test_fold_add_floats() {
        let lits = vec![Literal::Float(2.5), Literal::Float(3.5)];
        let result = fold_add(&lits).unwrap();
        match result {
            Literal::Float(v) => assert!((v - 6.0).abs() < 1e-10),
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_fold_multiply() {
        let lits = vec![Literal::Integer(4), Literal::Integer(5)];
        let result = fold_multiply(&lits).unwrap();
        assert_eq!(result, Literal::Integer(20));
    }

    #[test]
    fn test_fold_negate() {
        let lits = vec![Literal::Integer(5)];
        let result = fold_negate(&lits).unwrap();
        assert_eq!(result, Literal::Integer(-5));
    }

    #[test]
    fn test_evaluate_less_or_equal() {
        assert_eq!(
            evaluate_less_or_equal(&Literal::Integer(2), &Literal::Integer(3)),
            Some(true)
        );
        assert_eq!(
            evaluate_less_or_equal(&Literal::Integer(3), &Literal::Integer(2)),
            Some(false)
        );
        assert_eq!(
            evaluate_less_or_equal(&Literal::Integer(2), &Literal::Integer(2)),
            Some(true)
        );
    }

    #[test]
    fn test_fold_multiply_quantity_by_scalar() {
        let q = Literal::from_quantity(5.0, Unit::Volt);
        let scalar = Literal::Integer(2);
        let lits = vec![scalar, q];
        let result = fold_multiply(&lits).unwrap();
        match result {
            Literal::Quantity(q) => {
                assert!((q.min().unwrap().value() - 10.0).abs() < 1e-10);
            }
            _ => panic!("Expected Quantity"),
        }
    }

    #[test]
    fn test_fold_divide_volt_by_ampere() {
        let v = Literal::from_quantity(10.0, Unit::Volt);
        let a = Literal::from_quantity(2.0, Unit::Ampere);
        let lits = vec![v, a];
        let result = fold_divide(&lits).unwrap();
        match result {
            Literal::Quantity(q) => {
                assert_eq!(q.unit(), Unit::Ohm);
                assert!((q.min().unwrap().value() - 5.0).abs() < 1e-10);
            }
            _ => panic!("Expected Quantity"),
        }
    }
}
