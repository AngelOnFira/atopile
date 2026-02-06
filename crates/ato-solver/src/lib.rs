//! Constraint solver for Ato parameter resolution.
//!
//! This crate provides a constraint solver that can simplify expressions
//! and resolve parameter values in Ato designs.
//!
//! # Example
//!
//! ```
//! use ato_solver::{Solver, SolverConfig, Expression, Predicate};
//! use ato_domain::{Quantity, Unit, QuantityIntervalDisjoint};
//!
//! // Create a solver
//! let mut solver = Solver::new(SolverConfig::default());
//!
//! // Add constraints
//! // solver.add_constraint(...);
//!
//! // Solve
//! // let result = solver.solve();
//! ```

mod expression;
mod predicate;
mod simplify;
mod solver;

pub use expression::{
    ArithmeticOp, Expression, ExpressionId, ExpressionKind, ExpressionStore, Literal, Parameter,
    ParameterId,
};
pub use predicate::{Predicate, PredicateId, PredicateKind};
pub use simplify::{SimplificationPass, SimplificationResult};
pub use solver::{Solver, SolverConfig, SolverError, SolverResult, SolverState};
