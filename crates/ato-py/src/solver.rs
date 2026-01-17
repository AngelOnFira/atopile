//! Python bindings for the Ato constraint solver.

use ato_domain::Unit;
use ato_solver::{
    Expression, ExpressionId, Literal, Parameter,
    Predicate, PredicateId,
    Solver, SolverConfig, SolverError,
};
use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use std::time::Duration;

use crate::domain::PyUnit;

/// Solver configuration.
#[pyclass(name = "SolverConfig")]
#[derive(Clone)]
pub struct PySolverConfig {
    inner: SolverConfig,
}

#[pymethods]
impl PySolverConfig {
    #[new]
    #[pyo3(signature = (max_iterations=1000, timeout_secs=30.0, terminal=true, allow_partial=false))]
    fn new(
        max_iterations: usize,
        timeout_secs: f64,
        terminal: bool,
        allow_partial: bool,
    ) -> Self {
        Self {
            inner: SolverConfig {
                max_iterations,
                timeout: Duration::from_secs_f64(timeout_secs),
                terminal,
                allow_partial,
            },
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "SolverConfig(max_iterations={}, timeout={:?}, terminal={}, allow_partial={})",
            self.inner.max_iterations,
            self.inner.timeout,
            self.inner.terminal,
            self.inner.allow_partial
        )
    }

    #[getter]
    fn max_iterations(&self) -> usize {
        self.inner.max_iterations
    }

    #[getter]
    fn timeout_secs(&self) -> f64 {
        self.inner.timeout.as_secs_f64()
    }

    #[getter]
    fn terminal(&self) -> bool {
        self.inner.terminal
    }

    #[getter]
    fn allow_partial(&self) -> bool {
        self.inner.allow_partial
    }
}

impl Default for PySolverConfig {
    fn default() -> Self {
        Self {
            inner: SolverConfig::default(),
        }
    }
}

/// An expression in the constraint solver.
#[pyclass(name = "Expression")]
#[derive(Clone)]
pub struct PyExpression {
    id: ExpressionId,
}

#[pymethods]
impl PyExpression {
    fn __repr__(&self) -> String {
        format!("Expression(id={})", self.id)
    }

    /// Get the expression ID.
    #[getter]
    fn id(&self) -> u64 {
        self.id.raw()
    }
}

impl PyExpression {
    pub fn new(id: ExpressionId) -> Self {
        Self { id }
    }

    pub fn inner(&self) -> ExpressionId {
        self.id
    }
}

/// A predicate (constraint) in the solver.
#[pyclass(name = "Predicate")]
#[derive(Clone)]
pub struct PyPredicate {
    id: PredicateId,
}

#[pymethods]
impl PyPredicate {
    fn __repr__(&self) -> String {
        format!("Predicate(id={})", self.id)
    }

    /// Get the predicate ID.
    #[getter]
    fn id(&self) -> u64 {
        self.id.raw()
    }
}

impl PyPredicate {
    pub fn new(id: PredicateId) -> Self {
        Self { id }
    }

    #[allow(dead_code)]
    pub fn inner(&self) -> PredicateId {
        self.id
    }
}

/// The constraint solver.
///
/// Note: This class is not thread-safe and cannot be shared between threads.
#[pyclass(name = "Solver", unsendable)]
pub struct PySolver {
    inner: Solver,
}

#[pymethods]
impl PySolver {
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<PySolverConfig>) -> Self {
        let config = config.unwrap_or_default();
        Self {
            inner: Solver::new(config.inner),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Solver(predicates={}, constrained={})",
            self.inner.predicate_count(),
            self.inner.constrained_count()
        )
    }

    /// Add a parameter to the solver.
    ///
    /// Args:
    ///     name: Optional name for the parameter.
    ///     unit: Optional unit for the parameter.
    ///
    /// Returns:
    ///     An Expression representing the parameter.
    #[pyo3(signature = (name=None, unit=None))]
    fn add_parameter(&mut self, name: Option<&str>, unit: Option<&PyUnit>) -> PyExpression {
        let unit = unit.map(|u| u.inner()).unwrap_or(Unit::Dimensionless);
        let mut param = Parameter::new(unit);
        if let Some(n) = name {
            param = param.with_name(n);
        }
        let param_id = self.inner.add_parameter(param);

        // Create an expression for this parameter
        let expr = Expression::parameter(param_id, unit);
        let expr_id = self.inner.add_expression(expr);
        PyExpression::new(expr_id)
    }

    /// Add a literal integer expression.
    fn add_integer(&mut self, value: i64) -> PyExpression {
        let expr = Expression::literal(Literal::Integer(value));
        let id = self.inner.add_expression(expr);
        PyExpression::new(id)
    }

    /// Add a literal float expression.
    fn add_float(&mut self, value: f64) -> PyExpression {
        let expr = Expression::literal(Literal::Float(value));
        let id = self.inner.add_expression(expr);
        PyExpression::new(id)
    }

    /// Add a literal boolean expression.
    fn add_bool(&mut self, value: bool) -> PyExpression {
        let expr = Expression::literal(Literal::Bool(value));
        let id = self.inner.add_expression(expr);
        PyExpression::new(id)
    }

    /// Add a literal quantity expression.
    #[pyo3(signature = (value, unit))]
    fn add_quantity(&mut self, value: f64, unit: &PyUnit) -> PyExpression {
        let expr = Expression::literal(Literal::from_quantity(value, unit.inner()));
        let id = self.inner.add_expression(expr);
        PyExpression::new(id)
    }

    /// Add a quantity range expression.
    #[pyo3(signature = (min, max, unit))]
    fn add_quantity_range(&mut self, min: f64, max: f64, unit: &PyUnit) -> PyExpression {
        let expr = Expression::literal(Literal::from_interval(min, max, unit.inner()));
        let id = self.inner.add_expression(expr);
        PyExpression::new(id)
    }

    /// Add an "is" constraint (equality).
    fn constrain_is(&mut self, left: &PyExpression, right: &PyExpression) -> PyPredicate {
        let pred = Predicate::is(left.inner(), right.inner());
        let id = self.inner.constrain(pred);
        PyPredicate::new(id)
    }

    /// Add a "less or equal" constraint.
    fn constrain_less_or_equal(&mut self, left: &PyExpression, right: &PyExpression) -> PyPredicate {
        let pred = Predicate::less_or_equal(left.inner(), right.inner());
        let id = self.inner.constrain(pred);
        PyPredicate::new(id)
    }

    /// Add a "greater or equal" constraint.
    fn constrain_greater_or_equal(&mut self, left: &PyExpression, right: &PyExpression) -> PyPredicate {
        let pred = Predicate::greater_or_equal(left.inner(), right.inner());
        let id = self.inner.constrain(pred);
        PyPredicate::new(id)
    }

    /// Add a "less than" constraint.
    fn constrain_less_than(&mut self, left: &PyExpression, right: &PyExpression) -> PyPredicate {
        let pred = Predicate::less_than(left.inner(), right.inner());
        let id = self.inner.constrain(pred);
        PyPredicate::new(id)
    }

    /// Add a "greater than" constraint.
    fn constrain_greater_than(&mut self, left: &PyExpression, right: &PyExpression) -> PyPredicate {
        let pred = Predicate::greater_than(left.inner(), right.inner());
        let id = self.inner.constrain(pred);
        PyPredicate::new(id)
    }

    /// Add an "is subset" constraint.
    fn constrain_is_subset(&mut self, left: &PyExpression, right: &PyExpression) -> PyPredicate {
        let pred = Predicate::is_subset(left.inner(), right.inner());
        let id = self.inner.constrain(pred);
        PyPredicate::new(id)
    }

    /// Run the solver.
    ///
    /// Returns:
    ///     A dictionary containing:
    ///     - iterations: Number of iterations performed
    ///     - elapsed_secs: Time elapsed
    ///     - all_satisfied: Whether all constraints were satisfied
    ///     - not_deduced: List of predicate IDs that weren't deduced
    ///
    /// Raises:
    ///     RuntimeError: If a contradiction is detected or timeout occurs.
    fn solve(&mut self, py: Python<'_>) -> PyResult<PyObject> {
        match self.inner.solve() {
            Ok(state) => {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("iterations", state.iterations)?;
                dict.set_item("elapsed_secs", state.elapsed.as_secs_f64())?;
                dict.set_item("all_satisfied", state.all_satisfied)?;
                dict.set_item(
                    "not_deduced",
                    state.not_deduced.iter().map(|id| id.raw()).collect::<Vec<_>>(),
                )?;
                Ok(dict.into())
            }
            Err(e) => match e {
                SolverError::Contradiction(msg) => {
                    Err(PyRuntimeError::new_err(format!("Contradiction: {}", msg)))
                }
                SolverError::Timeout { iterations, elapsed } => {
                    Err(PyRuntimeError::new_err(format!(
                        "Solver timed out after {} iterations ({:?})",
                        iterations, elapsed
                    )))
                }
                SolverError::MaxIterations(n) => {
                    Err(PyRuntimeError::new_err(format!(
                        "Maximum iterations ({}) reached",
                        n
                    )))
                }
                SolverError::NotDeducible(msg) => {
                    Err(PyRuntimeError::new_err(format!("Not deducible: {}", msg)))
                }
            },
        }
    }

    /// Get the number of predicates.
    #[getter]
    fn predicate_count(&self) -> usize {
        self.inner.predicate_count()
    }

    /// Get the number of constrained predicates.
    #[getter]
    fn constrained_count(&self) -> usize {
        self.inner.constrained_count()
    }
}
