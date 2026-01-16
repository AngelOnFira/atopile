//! Domain types for the Ato constraint solver.
//!
//! This crate provides the core types for representing physical quantities,
//! intervals, and sets used in constraint solving for electronic design.
//!
//! # Main Types
//!
//! - [`Interval`] - A continuous numeric interval [min, max]
//! - [`DisjointIntervals`] - A set of non-overlapping intervals (union type)
//! - [`Quantity`] - A numeric value with an associated unit
//! - [`QuantityInterval`] - An interval of quantities with units
//! - [`BilateralTolerance`] - A tolerance specification (e.g., 10kohm +/- 5%)
//!
//! # Example
//!
//! ```
//! use ato_domain::{Interval, DisjointIntervals, Unit, Quantity, QuantityInterval};
//!
//! // Create a simple numeric interval
//! let interval = Interval::new(1.0, 10.0).unwrap();
//! assert!(interval.contains(5.0));
//!
//! // Create a quantity with units
//! let resistance = Quantity::new(10_000.0, Unit::Ohm);
//!
//! // Create a quantity interval (e.g., 9.5kohm to 10.5kohm)
//! let range = QuantityInterval::new(
//!     Quantity::new(9_500.0, Unit::Ohm),
//!     Quantity::new(10_500.0, Unit::Ohm),
//! ).unwrap();
//! ```

mod interval;
mod disjoint;
mod unit;
mod quantity;
mod tolerance;

pub use interval::Interval;
pub use disjoint::DisjointIntervals;
pub use unit::Unit;
pub use quantity::{Quantity, QuantityInterval, QuantityIntervalDisjoint};
pub use tolerance::BilateralTolerance;
