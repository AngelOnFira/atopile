//! Simplification passes for the constraint solver.
//!
//! This module contains various simplification algorithms that transform
//! expressions and predicates into simpler equivalent forms.

mod canonical;
mod constant_fold;
mod structural;

pub use canonical::CanonicalPass;
pub use constant_fold::ConstantFoldPass;
pub use structural::StructuralPass;

use crate::expression::{Expression, ExpressionId, ExpressionStore};
use crate::predicate::{Predicate, PredicateId, PredicateKind};
use std::collections::HashMap;

/// The result of a simplification pass.
#[derive(Debug, Clone, Default)]
pub struct SimplificationResult {
    /// Whether any changes were made.
    pub dirty: bool,
    /// Expressions that were replaced (old -> new).
    pub replaced_expressions: HashMap<ExpressionId, ExpressionId>,
    /// Predicates that were replaced (old -> new).
    pub replaced_predicates: HashMap<PredicateId, PredicateId>,
    /// Expressions that were removed.
    pub removed_expressions: Vec<ExpressionId>,
    /// Predicates that were removed.
    pub removed_predicates: Vec<PredicateId>,
    /// New expressions that were added.
    pub added_expressions: Vec<Expression>,
    /// New predicates that were added.
    pub added_predicates: Vec<Predicate>,
    /// Predicates that were terminated (proven true).
    pub terminated_predicates: Vec<PredicateId>,
    /// Whether a contradiction was detected.
    pub contradiction: Option<ContradictionInfo>,
}

impl SimplificationResult {
    /// Create a new empty result.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark that changes were made.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Record an expression replacement.
    pub fn replace_expression(&mut self, old: ExpressionId, new: ExpressionId) {
        self.replaced_expressions.insert(old, new);
        self.dirty = true;
    }

    /// Record a predicate replacement.
    pub fn replace_predicate(&mut self, old: PredicateId, new: PredicateId) {
        self.replaced_predicates.insert(old, new);
        self.dirty = true;
    }

    /// Record an expression removal.
    pub fn remove_expression(&mut self, id: ExpressionId) {
        self.removed_expressions.push(id);
        self.dirty = true;
    }

    /// Record a predicate removal.
    pub fn remove_predicate(&mut self, id: PredicateId) {
        self.removed_predicates.push(id);
        self.dirty = true;
    }

    /// Add a new expression.
    pub fn add_expression(&mut self, expr: Expression) {
        self.added_expressions.push(expr);
        self.dirty = true;
    }

    /// Add a new predicate.
    pub fn add_predicate(&mut self, pred: Predicate) {
        self.added_predicates.push(pred);
        self.dirty = true;
    }

    /// Mark a predicate as terminated (proven true).
    pub fn terminate_predicate(&mut self, id: PredicateId) {
        self.terminated_predicates.push(id);
        self.dirty = true;
    }

    /// Record a contradiction.
    pub fn set_contradiction(&mut self, info: ContradictionInfo) {
        self.contradiction = Some(info);
        self.dirty = true;
    }

    /// Merge another result into this one.
    pub fn merge(&mut self, other: SimplificationResult) {
        self.dirty |= other.dirty;
        self.replaced_expressions.extend(other.replaced_expressions);
        self.replaced_predicates.extend(other.replaced_predicates);
        self.removed_expressions.extend(other.removed_expressions);
        self.removed_predicates.extend(other.removed_predicates);
        self.added_expressions.extend(other.added_expressions);
        self.added_predicates.extend(other.added_predicates);
        self.terminated_predicates.extend(other.terminated_predicates);
        if other.contradiction.is_some() {
            self.contradiction = other.contradiction;
        }
    }
}

/// Information about a detected contradiction.
#[derive(Debug, Clone)]
pub struct ContradictionInfo {
    /// A description of the contradiction.
    pub message: String,
    /// The predicate(s) that caused the contradiction.
    pub involved_predicates: Vec<PredicateId>,
    /// The expression(s) that caused the contradiction.
    pub involved_expressions: Vec<ExpressionId>,
}

impl ContradictionInfo {
    /// Create a new contradiction info.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            involved_predicates: Vec::new(),
            involved_expressions: Vec::new(),
        }
    }

    /// Add an involved predicate.
    pub fn with_predicate(mut self, id: PredicateId) -> Self {
        self.involved_predicates.push(id);
        self
    }

    /// Add an involved expression.
    pub fn with_expression(mut self, id: ExpressionId) -> Self {
        self.involved_expressions.push(id);
        self
    }
}

/// Context for simplification passes.
pub struct SimplificationContext<'a> {
    /// The expression store.
    pub expressions: &'a mut ExpressionStore,
    /// All predicates in the system.
    pub predicates: &'a mut HashMap<PredicateId, Predicate>,
}

impl<'a> SimplificationContext<'a> {
    /// Create a new simplification context.
    pub fn new(
        expressions: &'a mut ExpressionStore,
        predicates: &'a mut HashMap<PredicateId, Predicate>,
    ) -> Self {
        Self {
            expressions,
            predicates,
        }
    }

    /// Get an expression by ID.
    pub fn get_expression(&self, id: ExpressionId) -> Option<&Expression> {
        self.expressions.get_expression(id)
    }

    /// Get a predicate by ID.
    pub fn get_predicate(&self, id: PredicateId) -> Option<&Predicate> {
        self.predicates.get(&id)
    }

    /// Get all constrained predicates.
    pub fn constrained_predicates(&self) -> Vec<PredicateId> {
        self.predicates
            .iter()
            .filter(|(_, p)| p.constrained)
            .map(|(id, _)| *id)
            .collect()
    }
}

/// Trait for simplification passes.
pub trait SimplificationPass {
    /// The name of this pass.
    fn name(&self) -> &'static str;

    /// Whether this pass should only run in terminal mode.
    fn terminal_only(&self) -> bool {
        false
    }

    /// Run the simplification pass.
    fn run(&self, ctx: &mut SimplificationContext) -> SimplificationResult;
}

/// Replace references to old_id with new_id in a predicate.
pub fn replace_expr_in_predicate(
    pred: &mut Predicate,
    old_id: ExpressionId,
    new_id: ExpressionId,
) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplification_result_dirty() {
        let mut result = SimplificationResult::new();
        assert!(!result.dirty);
        result.mark_dirty();
        assert!(result.dirty);
    }

    #[test]
    fn test_simplification_result_merge() {
        let mut r1 = SimplificationResult::new();
        let mut r2 = SimplificationResult::new();

        let expr_id = ExpressionId::new();
        r2.remove_expression(expr_id);

        r1.merge(r2);
        assert!(r1.dirty);
        assert_eq!(r1.removed_expressions.len(), 1);
    }

    #[test]
    fn test_contradiction_info() {
        let info = ContradictionInfo::new("Empty set intersection")
            .with_predicate(PredicateId::new())
            .with_expression(ExpressionId::new());

        assert_eq!(info.involved_predicates.len(), 1);
        assert_eq!(info.involved_expressions.len(), 1);
    }
}
