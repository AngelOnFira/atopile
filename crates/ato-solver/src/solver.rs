//! Main constraint solver implementation.
//!
//! The solver uses a fixed-point iteration approach, running simplification
//! passes until no more changes are made.

use crate::expression::{Expression, ExpressionId, ExpressionStore, Literal, Parameter, ParameterId};
use crate::predicate::{Predicate, PredicateId, PredicateKind};
use crate::simplify::{
    CanonicalPass, ConstantFoldPass, SimplificationContext, SimplificationPass, StructuralPass,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Configuration for the solver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverConfig {
    /// Maximum number of iterations before timeout.
    pub max_iterations: usize,
    /// Timeout duration.
    pub timeout: Duration,
    /// Whether to run in terminal mode (more aggressive simplification).
    pub terminal: bool,
    /// Whether to allow partial solutions when timing out.
    pub allow_partial: bool,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            timeout: Duration::from_secs(30),
            terminal: true,
            allow_partial: false,
        }
    }
}

/// Errors that can occur during solving.
#[derive(Debug, Clone, Error)]
pub enum SolverError {
    /// A contradiction was detected.
    #[error("Contradiction: {0}")]
    Contradiction(String),

    /// The solver timed out.
    #[error("Solver timed out after {iterations} iterations ({elapsed:?})")]
    Timeout {
        iterations: usize,
        elapsed: Duration,
    },

    /// Maximum iterations reached.
    #[error("Maximum iterations ({0}) reached, likely stuck in a loop")]
    MaxIterations(usize),

    /// A predicate could not be deduced.
    #[error("Predicate could not be deduced: {0}")]
    NotDeducible(String),
}

/// The state of the solver after running.
#[derive(Debug, Clone, Default)]
pub struct SolverState {
    /// Number of iterations performed.
    pub iterations: usize,
    /// Time elapsed during solving.
    pub elapsed: Duration,
    /// Whether all constraints were satisfied.
    pub all_satisfied: bool,
    /// Predicates that were not deduced.
    pub not_deduced: Vec<PredicateId>,
    /// The expression store after solving.
    pub expressions: ExpressionStore,
    /// The predicates after solving.
    pub predicates: HashMap<PredicateId, Predicate>,
}

impl SolverState {
    /// Get the solved value/domain of a parameter from the solver result.
    ///
    /// Returns the parameter's narrowed domain as a `Literal::Quantity` if
    /// the domain has been narrowed from unbounded. Returns `None` if the
    /// parameter is not found or still has an unbounded domain.
    pub fn get_parameter_value(&self, param_id: ParameterId) -> Option<Literal> {
        let param = self.expressions.get_parameter(param_id)?;
        if param.domain.is_unbounded() {
            None
        } else {
            Some(Literal::Quantity(param.domain.clone()))
        }
    }
}

/// Result of the solver.
pub type SolverResult = Result<SolverState, SolverError>;

/// The constraint solver.
///
/// Uses a fixed-point iteration approach with multiple simplification passes.
pub struct Solver {
    /// Configuration.
    config: SolverConfig,
    /// Expression store.
    expressions: ExpressionStore,
    /// Predicates.
    predicates: HashMap<PredicateId, Predicate>,
    /// Pre-processing passes (run once at start).
    pre_passes: Vec<Box<dyn SimplificationPass>>,
    /// Iterative passes (run until fixed point).
    iterative_passes: Vec<Box<dyn SimplificationPass>>,
}

impl Solver {
    /// Create a new solver with the given configuration.
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            expressions: ExpressionStore::new(),
            predicates: HashMap::new(),
            pre_passes: vec![Box::new(CanonicalPass)],
            iterative_passes: vec![
                Box::new(StructuralPass),
                Box::new(ConstantFoldPass),
            ],
        }
    }

    /// Create a new solver with default configuration.
    pub fn default_solver() -> Self {
        Self::new(SolverConfig::default())
    }

    /// Add a parameter to the solver.
    pub fn add_parameter(&mut self, param: Parameter) -> ParameterId {
        self.expressions.add_parameter(param)
    }

    /// Add an expression to the solver.
    pub fn add_expression(&mut self, expr: Expression) -> ExpressionId {
        self.expressions.add_expression(expr)
    }

    /// Add a predicate to the solver.
    pub fn add_predicate(&mut self, pred: Predicate) -> PredicateId {
        let id = pred.id;
        self.predicates.insert(id, pred);
        id
    }

    /// Add a constrained predicate (must be satisfied).
    pub fn constrain(&mut self, pred: Predicate) -> PredicateId {
        self.add_predicate(pred.constrain())
    }

    /// Get an expression by ID.
    pub fn get_expression(&self, id: ExpressionId) -> Option<&Expression> {
        self.expressions.get_expression(id)
    }

    /// Get a parameter by ID.
    pub fn get_parameter(&self, id: ParameterId) -> Option<&Parameter> {
        self.expressions.get_parameter(id)
    }

    /// Get a predicate by ID.
    pub fn get_predicate(&self, id: PredicateId) -> Option<&Predicate> {
        self.predicates.get(&id)
    }

    /// Get the number of predicates.
    pub fn predicate_count(&self) -> usize {
        self.predicates.len()
    }

    /// Get the number of constrained predicates.
    pub fn constrained_count(&self) -> usize {
        self.predicates.values().filter(|p| p.constrained).count()
    }

    /// Run the solver.
    pub fn solve(&mut self) -> SolverResult {
        let start = Instant::now();
        let mut iterations = 0;

        // Run pre-processing passes once
        {
            let mut ctx = SimplificationContext::new(&mut self.expressions, &mut self.predicates);
            for pass in &self.pre_passes {
                let result = pass.run(&mut ctx);
                if let Some(contradiction) = result.contradiction {
                    return Err(SolverError::Contradiction(contradiction.message));
                }
            }
        }

        // Run iterative passes until fixed point
        loop {
            iterations += 1;

            // Check timeout
            let elapsed = start.elapsed();
            if elapsed > self.config.timeout {
                if self.config.allow_partial {
                    break;
                }
                return Err(SolverError::Timeout {
                    iterations,
                    elapsed,
                });
            }

            // Check max iterations
            if iterations > self.config.max_iterations {
                return Err(SolverError::MaxIterations(self.config.max_iterations));
            }

            // Run all iterative passes
            let mut dirty = false;
            {
                let mut ctx =
                    SimplificationContext::new(&mut self.expressions, &mut self.predicates);
                for pass in &self.iterative_passes {
                    if !self.config.terminal && pass.terminal_only() {
                        continue;
                    }

                    let result = pass.run(&mut ctx);

                    if let Some(contradiction) = result.contradiction {
                        return Err(SolverError::Contradiction(contradiction.message));
                    }

                    dirty |= result.dirty;
                }
            }

            // If no changes, we've reached a fixed point
            if !dirty {
                break;
            }

            // If no predicates left, we're done
            if self.predicates.is_empty() {
                break;
            }
        }

        // Check which predicates were not deduced
        let not_deduced: Vec<PredicateId> = self
            .predicates
            .iter()
            .filter(|(_, p)| p.constrained && !p.solver_terminated)
            .map(|(id, _)| *id)
            .collect();

        let all_satisfied = not_deduced.is_empty();

        Ok(SolverState {
            iterations,
            elapsed: start.elapsed(),
            all_satisfied,
            not_deduced,
            expressions: self.expressions.clone(),
            predicates: self.predicates.clone(),
        })
    }

    /// Try to fulfill a predicate.
    ///
    /// Returns `true` if the predicate is satisfied, `false` if it's contradicted,
    /// and `None` if it couldn't be determined.
    pub fn try_fulfill(&mut self, pred: Predicate) -> Result<Option<bool>, SolverError> {
        // Clone current state
        let original_expressions = self.expressions.clone();
        let original_predicates = self.predicates.clone();

        // Add the predicate
        let pred_id = self.constrain(pred);

        // Try to solve
        let result = self.solve();

        // Check result
        match result {
            Ok(state) => {
                // Restore original state
                self.expressions = original_expressions;
                self.predicates = original_predicates;

                if state.not_deduced.contains(&pred_id) {
                    Ok(None) // Unknown
                } else {
                    Ok(Some(true)) // Satisfied
                }
            }
            Err(SolverError::Contradiction(_)) => {
                // Restore original state
                self.expressions = original_expressions;
                self.predicates = original_predicates;
                Ok(Some(false)) // Contradicted
            }
            Err(e) => Err(e),
        }
    }

    /// Get the solved value/domain of a parameter after solving.
    ///
    /// Returns the parameter's narrowed domain as a `Literal::Quantity` if
    /// the domain has been narrowed from unbounded. Returns `None` if the
    /// parameter is not found or still has an unbounded domain.
    pub fn get_parameter_value(&self, param_id: ParameterId) -> Option<Literal> {
        let param = self.expressions.get_parameter(param_id)?;
        if param.domain.is_unbounded() {
            None
        } else {
            Some(Literal::Quantity(param.domain.clone()))
        }
    }

    /// Get the known superset for a parameter.
    ///
    /// Returns the intersection of all constraints on the parameter.
    pub fn get_known_superset(&self, param_id: ParameterId) -> Option<Literal> {
        let param = self.expressions.get_parameter(param_id)?;

        // Start with the parameter's domain
        let mut superset = param.known_superset.clone();

        // Find all subset constraints where this parameter is the left operand
        for pred in self.predicates.values() {
            if !pred.constrained {
                continue;
            }

            if let PredicateKind::IsSubset { left, right } = &pred.kind {
                // Check if left references this parameter
                if let Some(expr) = self.expressions.get_expression(*left) {
                    if let Some(pid) = expr.as_parameter() {
                        if pid == param_id {
                            // Get the right side literal
                            if let Some(right_expr) = self.expressions.get_expression(*right) {
                                if let Some(Literal::Quantity(q)) = right_expr.as_literal() {
                                    superset = match superset {
                                        Some(s) => s.intersect(q).ok(),
                                        None => Some(q.clone()),
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }

        superset.map(Literal::Quantity)
    }
}

impl Default for Solver {
    fn default() -> Self {
        Self::default_solver()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ato_domain::Unit;

    #[test]
    fn test_solver_creation() {
        let solver = Solver::default();
        assert_eq!(solver.predicate_count(), 0);
    }

    #[test]
    fn test_add_parameter() {
        let mut solver = Solver::default();
        let param = Parameter::new(Unit::Volt).with_name("V1");
        let id = solver.add_parameter(param);
        assert!(solver.get_parameter(id).is_some());
    }

    #[test]
    fn test_add_expression() {
        let mut solver = Solver::default();
        let expr = Expression::literal(Literal::Integer(42));
        let id = solver.add_expression(expr);
        assert!(solver.get_expression(id).is_some());
    }

    #[test]
    fn test_add_predicate() {
        let mut solver = Solver::default();
        let left = solver.add_expression(Expression::literal(Literal::Integer(2)));
        let right = solver.add_expression(Expression::literal(Literal::Integer(3)));
        let pred = Predicate::less_or_equal(left, right);
        let id = solver.add_predicate(pred);
        assert!(solver.get_predicate(id).is_some());
    }

    #[test]
    fn test_solve_empty() {
        let mut solver = Solver::default();
        let result = solver.solve().unwrap();
        assert!(result.all_satisfied);
        assert_eq!(result.iterations, 1);
    }

    #[test]
    fn test_solve_simple_true() {
        let mut solver = Solver::default();

        // 2 <= 3 (true)
        let left = solver.add_expression(Expression::literal(Literal::Integer(2)));
        let right = solver.add_expression(Expression::literal(Literal::Integer(3)));
        solver.constrain(Predicate::less_or_equal(left, right));

        let result = solver.solve().unwrap();
        assert!(result.all_satisfied);
    }

    #[test]
    fn test_solve_contradiction() {
        let mut solver = Solver::default();

        // 5 is 3 (contradiction - different values)
        let left = solver.add_expression(Expression::literal(Literal::Integer(5)));
        let right = solver.add_expression(Expression::literal(Literal::Integer(3)));
        solver.constrain(Predicate::is(left, right));

        let result = solver.solve();
        assert!(matches!(result, Err(SolverError::Contradiction(_))));
    }

    #[test]
    fn test_solve_with_quantities() {
        let mut solver = Solver::default();

        // 5V <= 10V (true)
        let left = solver.add_expression(Expression::literal(Literal::from_quantity(
            5.0,
            Unit::Volt,
        )));
        let right = solver.add_expression(Expression::literal(Literal::from_quantity(
            10.0,
            Unit::Volt,
        )));
        solver.constrain(Predicate::less_or_equal(left, right));

        let result = solver.solve().unwrap();
        assert!(result.all_satisfied);
    }

    #[test]
    fn test_config_timeout() {
        let config = SolverConfig {
            timeout: Duration::from_millis(1),
            max_iterations: 1000000,
            ..Default::default()
        };
        let solver = Solver::new(config);
        assert_eq!(solver.config.timeout, Duration::from_millis(1));
    }
}
