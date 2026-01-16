//! Physical quantities with unit tracking.
//!
//! This module provides types for representing physical quantities
//! (numbers with units) and intervals of quantities.

use crate::{DisjointIntervals, Interval, Unit};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A physical quantity with a numeric value and unit.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Quantity {
    /// The numeric value in base units.
    value: f64,
    /// The unit of this quantity.
    unit: Unit,
}

impl Quantity {
    /// Create a new quantity with the given value and unit.
    pub fn new(value: f64, unit: Unit) -> Self {
        Self { value, unit }
    }

    /// Create a dimensionless quantity.
    pub fn dimensionless(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Dimensionless,
        }
    }

    /// Get the numeric value.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Get the unit.
    pub fn unit(&self) -> Unit {
        self.unit
    }

    /// Convert this quantity to base units.
    pub fn to_base_units(&self) -> Quantity {
        let factor = self.unit.to_base_factor();
        Quantity {
            value: self.value * factor,
            unit: self.unit.base_unit(),
        }
    }

    /// Check if this quantity is compatible with another (same unit type).
    pub fn is_compatible_with(&self, other: &Quantity) -> bool {
        self.unit.is_compatible_with(&other.unit)
    }

    /// Check if this quantity is dimensionless.
    pub fn is_dimensionless(&self) -> bool {
        self.unit.is_dimensionless()
    }
}

impl PartialEq for Quantity {
    fn eq(&self, other: &Self) -> bool {
        if !self.unit.is_compatible_with(&other.unit) {
            return false;
        }
        let a = self.to_base_units();
        let b = other.to_base_units();
        (a.value - b.value).abs() < 1e-15
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        crate::unit::format_si(self.value, &self.unit, 3).fmt(f)
    }
}

// --- Arithmetic operators for Quantity ---

impl std::ops::Add for Quantity {
    type Output = Result<Quantity, QuantityError>;

    fn add(self, rhs: Self) -> Self::Output {
        if !self.unit.is_compatible_with(&rhs.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: rhs.unit,
            });
        }
        let a = self.to_base_units();
        let b = rhs.to_base_units();
        Ok(Quantity::new(a.value + b.value, a.unit))
    }
}

impl std::ops::Sub for Quantity {
    type Output = Result<Quantity, QuantityError>;

    fn sub(self, rhs: Self) -> Self::Output {
        if !self.unit.is_compatible_with(&rhs.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: rhs.unit,
            });
        }
        let a = self.to_base_units();
        let b = rhs.to_base_units();
        Ok(Quantity::new(a.value - b.value, a.unit))
    }
}

impl std::ops::Neg for Quantity {
    type Output = Quantity;

    fn neg(self) -> Self::Output {
        Quantity::new(-self.value, self.unit)
    }
}

impl std::ops::Mul<f64> for Quantity {
    type Output = Quantity;

    fn mul(self, rhs: f64) -> Self::Output {
        Quantity::new(self.value * rhs, self.unit)
    }
}

impl std::ops::Div<f64> for Quantity {
    type Output = Quantity;

    fn div(self, rhs: f64) -> Self::Output {
        Quantity::new(self.value / rhs, self.unit)
    }
}

/// A closed interval of physical quantities with the same unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantityInterval {
    /// The underlying numeric interval.
    interval: Interval,
    /// The unit for this interval.
    unit: Unit,
}

impl QuantityInterval {
    /// Create a new quantity interval.
    pub fn new(min: Quantity, max: Quantity) -> Result<Self, QuantityError> {
        if !min.unit.is_compatible_with(&max.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: min.unit,
                right: max.unit,
            });
        }

        let min_base = min.to_base_units();
        let max_base = max.to_base_units();

        let interval = Interval::new(min_base.value, max_base.value)
            .map_err(|e| QuantityError::IntervalError(e.to_string()))?;

        Ok(Self {
            interval,
            unit: min_base.unit,
        })
    }

    /// Create an unbounded quantity interval with the given unit.
    pub fn unbounded(unit: Unit) -> Self {
        Self {
            interval: Interval::unbounded(),
            unit,
        }
    }

    /// Create a singleton quantity interval.
    pub fn singleton(value: Quantity) -> Result<Self, QuantityError> {
        let base = value.to_base_units();
        let interval = Interval::singleton(base.value)
            .map_err(|e| QuantityError::IntervalError(e.to_string()))?;
        Ok(Self {
            interval,
            unit: base.unit,
        })
    }

    /// Create a quantity interval from a center value and absolute tolerance.
    pub fn from_center_abs(center: Quantity, tolerance: Quantity) -> Result<Self, QuantityError> {
        if !center.unit.is_compatible_with(&tolerance.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: center.unit,
                right: tolerance.unit,
            });
        }
        let min = (center - tolerance)?;
        let max = (center + tolerance)?;
        Self::new(min, max)
    }

    /// Create a quantity interval from a center value and relative tolerance.
    pub fn from_center_rel(center: Quantity, rel_tolerance: f64) -> Result<Self, QuantityError> {
        let abs_tolerance = Quantity::new(center.value.abs() * rel_tolerance, center.unit);
        Self::from_center_abs(center, abs_tolerance)
    }

    /// Get the unit for this interval.
    pub fn unit(&self) -> Unit {
        self.unit
    }

    /// Get the minimum quantity.
    pub fn min(&self) -> Quantity {
        Quantity::new(self.interval.lower(), self.unit)
    }

    /// Get the maximum quantity.
    pub fn max(&self) -> Quantity {
        Quantity::new(self.interval.upper(), self.unit)
    }

    /// Get the underlying numeric interval.
    pub fn interval(&self) -> &Interval {
        &self.interval
    }

    /// Check if a quantity is contained in this interval.
    pub fn contains(&self, value: &Quantity) -> bool {
        if !self.unit.is_compatible_with(&value.unit) {
            return false;
        }
        let base = value.to_base_units();
        self.interval.contains(base.value)
    }

    /// Check if this interval is a singleton (single value).
    pub fn is_singleton(&self) -> bool {
        self.interval.is_singleton()
    }

    /// Check if this interval is unbounded.
    pub fn is_unbounded(&self) -> bool {
        self.interval.is_unbounded()
    }

    /// Check if this interval is finite.
    pub fn is_finite(&self) -> bool {
        self.interval.is_finite()
    }

    /// Check if this interval is a subset of another.
    pub fn is_subset_of(&self, other: &QuantityInterval) -> bool {
        if !self.unit.is_compatible_with(&other.unit) {
            return false;
        }
        self.interval.is_subset_of(&other.interval)
    }

    /// Get the center and relative tolerance.
    pub fn as_center_rel(&self) -> Option<(Quantity, f64)> {
        let (center, rel) = self.interval.as_center_rel()?;
        Some((Quantity::new(center, self.unit), rel))
    }

    // --- Arithmetic Operations ---

    /// Add two quantity intervals.
    pub fn add(&self, other: &QuantityInterval) -> Result<QuantityInterval, QuantityError> {
        if !self.unit.is_compatible_with(&other.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: other.unit,
            });
        }
        Ok(QuantityInterval {
            interval: self.interval.add(&other.interval),
            unit: self.unit,
        })
    }

    /// Negate this quantity interval.
    pub fn negate(&self) -> QuantityInterval {
        QuantityInterval {
            interval: self.interval.negate(),
            unit: self.unit,
        }
    }

    /// Subtract another quantity interval.
    pub fn subtract(&self, other: &QuantityInterval) -> Result<QuantityInterval, QuantityError> {
        if !self.unit.is_compatible_with(&other.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: other.unit,
            });
        }
        Ok(QuantityInterval {
            interval: self.interval.subtract(&other.interval),
            unit: self.unit,
        })
    }

    /// Intersect with another quantity interval.
    pub fn intersect(&self, other: &QuantityInterval) -> Result<QuantityIntervalDisjoint, QuantityError> {
        if !self.unit.is_compatible_with(&other.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: other.unit,
            });
        }
        match self.interval.intersect(&other.interval) {
            Some(interval) => Ok(QuantityIntervalDisjoint {
                intervals: DisjointIntervals::single(interval),
                unit: self.unit,
            }),
            None => Ok(QuantityIntervalDisjoint::empty(self.unit)),
        }
    }
}

impl PartialEq for QuantityInterval {
    fn eq(&self, other: &Self) -> bool {
        self.unit.is_compatible_with(&other.unit) && self.interval == other.interval
    }
}

impl fmt::Display for QuantityInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.interval.is_singleton() {
            write!(f, "[{}]", self.min())
        } else if let Some((center, rel)) = self.as_center_rel() {
            if rel < 0.5 && rel.is_finite() {
                write!(f, "[{} ± {:.2}%]", center, rel * 100.0)
            } else {
                write!(f, "[{}, {}]", self.min(), self.max())
            }
        } else {
            write!(f, "[{}, {}]", self.min(), self.max())
        }
    }
}

/// A disjoint set of quantity intervals with the same unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantityIntervalDisjoint {
    /// The underlying disjoint intervals.
    intervals: DisjointIntervals,
    /// The unit for all intervals.
    unit: Unit,
}

impl QuantityIntervalDisjoint {
    /// Create an empty set with the given unit.
    pub fn empty(unit: Unit) -> Self {
        Self {
            intervals: DisjointIntervals::empty(),
            unit,
        }
    }

    /// Create a set containing a single quantity interval.
    pub fn single(interval: QuantityInterval) -> Self {
        Self {
            intervals: DisjointIntervals::single(*interval.interval()),
            unit: interval.unit(),
        }
    }

    /// Create an unbounded set with the given unit.
    pub fn unbounded(unit: Unit) -> Self {
        Self {
            intervals: DisjointIntervals::unbounded(),
            unit,
        }
    }

    /// Create from a quantity value (singleton).
    pub fn from_quantity(value: Quantity) -> Self {
        let base = value.to_base_units();
        Self {
            intervals: DisjointIntervals::single(Interval::singleton(base.value).unwrap()),
            unit: base.unit,
        }
    }

    /// Get the unit for this set.
    pub fn unit(&self) -> Unit {
        self.unit
    }

    /// Check if this set is empty.
    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    /// Check if this set is a singleton.
    pub fn is_singleton(&self) -> bool {
        self.intervals.is_singleton()
    }

    /// Get the minimum quantity.
    pub fn min(&self) -> Option<Quantity> {
        self.intervals.min().map(|v| Quantity::new(v, self.unit))
    }

    /// Get the maximum quantity.
    pub fn max(&self) -> Option<Quantity> {
        self.intervals.max().map(|v| Quantity::new(v, self.unit))
    }

    /// Check if a quantity is contained in this set.
    pub fn contains(&self, value: &Quantity) -> bool {
        if !self.unit.is_compatible_with(&value.unit) {
            return false;
        }
        let base = value.to_base_units();
        self.intervals.contains(base.value)
    }

    /// Check if this set is a subset of another.
    pub fn is_subset_of(&self, other: &QuantityIntervalDisjoint) -> bool {
        if !self.unit.is_compatible_with(&other.unit) {
            return false;
        }
        self.intervals.is_subset_of(&other.intervals)
    }

    /// Check if this set is a superset of another.
    pub fn is_superset_of(&self, other: &QuantityIntervalDisjoint) -> bool {
        other.is_subset_of(self)
    }

    // --- Set Operations ---

    /// Compute the union with another set.
    pub fn union(&self, other: &QuantityIntervalDisjoint) -> Result<QuantityIntervalDisjoint, QuantityError> {
        if !self.unit.is_compatible_with(&other.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: other.unit,
            });
        }
        Ok(QuantityIntervalDisjoint {
            intervals: self.intervals.union(&other.intervals),
            unit: self.unit,
        })
    }

    /// Compute the intersection with another set.
    pub fn intersect(&self, other: &QuantityIntervalDisjoint) -> Result<QuantityIntervalDisjoint, QuantityError> {
        if !self.unit.is_compatible_with(&other.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: other.unit,
            });
        }
        Ok(QuantityIntervalDisjoint {
            intervals: self.intervals.intersect(&other.intervals),
            unit: self.unit,
        })
    }

    /// Compute the difference (self - other).
    pub fn difference(&self, other: &QuantityIntervalDisjoint) -> Result<QuantityIntervalDisjoint, QuantityError> {
        if !self.unit.is_compatible_with(&other.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: other.unit,
            });
        }
        Ok(QuantityIntervalDisjoint {
            intervals: self.intervals.difference(&other.intervals),
            unit: self.unit,
        })
    }

    // --- Arithmetic Operations ---

    /// Add another set.
    pub fn add(&self, other: &QuantityIntervalDisjoint) -> Result<QuantityIntervalDisjoint, QuantityError> {
        if !self.unit.is_compatible_with(&other.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: other.unit,
            });
        }
        Ok(QuantityIntervalDisjoint {
            intervals: self.intervals.add(&other.intervals),
            unit: self.unit,
        })
    }

    /// Negate this set.
    pub fn negate(&self) -> QuantityIntervalDisjoint {
        QuantityIntervalDisjoint {
            intervals: self.intervals.negate(),
            unit: self.unit,
        }
    }

    /// Subtract another set.
    pub fn subtract(&self, other: &QuantityIntervalDisjoint) -> Result<QuantityIntervalDisjoint, QuantityError> {
        if !self.unit.is_compatible_with(&other.unit) {
            return Err(QuantityError::IncompatibleUnits {
                left: self.unit,
                right: other.unit,
            });
        }
        Ok(QuantityIntervalDisjoint {
            intervals: self.intervals.subtract(&other.intervals),
            unit: self.unit,
        })
    }
}

impl PartialEq for QuantityIntervalDisjoint {
    fn eq(&self, other: &Self) -> bool {
        self.unit.is_compatible_with(&other.unit) && self.intervals == other.intervals
    }
}

impl fmt::Display for QuantityIntervalDisjoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "∅")
        } else {
            write!(f, "{}", self.intervals)
        }
    }
}

/// Errors that can occur when working with quantities.
#[derive(Debug, Clone, thiserror::Error)]
pub enum QuantityError {
    #[error("incompatible units: {left} and {right}")]
    IncompatibleUnits { left: Unit, right: Unit },

    #[error("interval error: {0}")]
    IntervalError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantity_creation() {
        let q = Quantity::new(10.0, Unit::Ohm);
        assert_eq!(q.value(), 10.0);
        assert_eq!(q.unit(), Unit::Ohm);
    }

    #[test]
    fn test_quantity_add() {
        let a = Quantity::new(5.0, Unit::Volt);
        let b = Quantity::new(3.0, Unit::Volt);
        let result = (a + b).unwrap();
        assert_eq!(result.value(), 8.0);
    }

    #[test]
    fn test_quantity_add_incompatible() {
        let a = Quantity::new(5.0, Unit::Volt);
        let b = Quantity::new(3.0, Unit::Ampere);
        assert!((a + b).is_err());
    }

    #[test]
    fn test_quantity_interval() {
        let min = Quantity::new(9.0, Unit::Volt);
        let max = Quantity::new(11.0, Unit::Volt);
        let interval = QuantityInterval::new(min, max).unwrap();
        assert!(interval.contains(&Quantity::new(10.0, Unit::Volt)));
        assert!(!interval.contains(&Quantity::new(12.0, Unit::Volt)));
    }

    #[test]
    fn test_quantity_interval_from_center_rel() {
        let center = Quantity::new(10.0, Unit::Ohm);
        let interval = QuantityInterval::from_center_rel(center, 0.1).unwrap();
        assert!(interval.contains(&Quantity::new(9.5, Unit::Ohm)));
        assert!(interval.contains(&Quantity::new(10.5, Unit::Ohm)));
        assert!(!interval.contains(&Quantity::new(8.0, Unit::Ohm)));
    }

    #[test]
    fn test_quantity_interval_disjoint() {
        let interval1 = QuantityInterval::new(
            Quantity::new(1.0, Unit::Volt),
            Quantity::new(3.0, Unit::Volt),
        ).unwrap();
        let interval2 = QuantityInterval::new(
            Quantity::new(5.0, Unit::Volt),
            Quantity::new(7.0, Unit::Volt),
        ).unwrap();

        let set1 = QuantityIntervalDisjoint::single(interval1);
        let set2 = QuantityIntervalDisjoint::single(interval2);

        let union = set1.union(&set2).unwrap();
        assert!(union.contains(&Quantity::new(2.0, Unit::Volt)));
        assert!(union.contains(&Quantity::new(6.0, Unit::Volt)));
        assert!(!union.contains(&Quantity::new(4.0, Unit::Volt)));
    }
}
