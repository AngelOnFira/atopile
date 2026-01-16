//! Numeric interval type for continuous ranges.
//!
//! An `Interval` represents a closed interval [min, max] of real numbers.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// Relative epsilon for floating point comparisons.
const EPSILON_REL: f64 = 1e-6;
/// Absolute epsilon for floating point comparisons.
const EPSILON_ABS: f64 = 1e-15;

/// Check if two floating point numbers are approximately equal.
fn approx_eq(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }
    let diff = (a - b).abs();
    if diff < EPSILON_ABS {
        return true;
    }
    let max_val = a.abs().max(b.abs());
    diff / max_val < EPSILON_REL
}

/// Check if a >= b with tolerance.
fn approx_ge(a: f64, b: f64) -> bool {
    a >= b || approx_eq(a, b)
}

/// A closed numeric interval [min, max].
///
/// Represents a continuous range of values. Supports arithmetic operations
/// following interval arithmetic rules.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Interval {
    /// Minimum value (inclusive).
    min: f64,
    /// Maximum value (inclusive).
    max: f64,
}

impl Interval {
    /// Create a new interval [min, max].
    ///
    /// Returns an error if min > max.
    pub fn new(min: f64, max: f64) -> Result<Self, IntervalError> {
        if min > max && !approx_eq(min, max) {
            return Err(IntervalError::InvalidBounds { min, max });
        }
        if min == f64::INFINITY || max == f64::NEG_INFINITY {
            return Err(IntervalError::InvalidInfinity);
        }
        Ok(Self {
            min: min.min(max),
            max: max.max(min),
        })
    }

    /// Create an unbounded interval (-∞, +∞).
    pub fn unbounded() -> Self {
        Self {
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
        }
    }

    /// Create a singleton interval [value, value].
    pub fn singleton(value: f64) -> Result<Self, IntervalError> {
        if value.is_infinite() {
            return Err(IntervalError::InvalidInfinity);
        }
        Ok(Self {
            min: value,
            max: value,
        })
    }

    /// Create an interval representing all non-negative numbers [0, +∞).
    pub fn non_negative() -> Self {
        Self {
            min: 0.0,
            max: f64::INFINITY,
        }
    }

    /// Create an interval representing all positive numbers (0, +∞).
    /// Note: This is still represented as [0, +∞) since we use closed intervals.
    pub fn positive() -> Self {
        Self {
            min: 0.0,
            max: f64::INFINITY,
        }
    }

    /// Get the lower bound (minimum value).
    pub fn lower(&self) -> f64 {
        self.min
    }

    /// Get the upper bound (maximum value).
    pub fn upper(&self) -> f64 {
        self.max
    }

    /// Check if this interval represents a single value.
    pub fn is_singleton(&self) -> bool {
        approx_eq(self.min, self.max)
    }

    /// Check if this interval is unbounded (-∞, +∞).
    pub fn is_unbounded(&self) -> bool {
        self.min == f64::NEG_INFINITY && self.max == f64::INFINITY
    }

    /// Check if this interval is finite (both bounds are finite).
    pub fn is_finite(&self) -> bool {
        self.min.is_finite() && self.max.is_finite()
    }

    /// Check if a value is contained in this interval.
    pub fn contains(&self, value: f64) -> bool {
        approx_ge(value, self.min) && approx_ge(self.max, value)
    }

    /// Check if this interval is a subset of another.
    pub fn is_subset_of(&self, other: &Interval) -> bool {
        approx_ge(self.min, other.min) && approx_ge(other.max, self.max)
    }

    /// Check if this interval overlaps with another.
    pub fn overlaps(&self, other: &Interval) -> bool {
        approx_ge(self.max, other.min) && approx_ge(other.max, self.min)
    }

    /// Check if this interval is adjacent to another (touching but not overlapping).
    pub fn is_adjacent(&self, other: &Interval) -> bool {
        approx_eq(self.max, other.min) || approx_eq(other.max, self.min)
    }

    /// Get the center of this interval.
    ///
    /// Returns None if the interval is unbounded on both sides.
    pub fn center(&self) -> Option<f64> {
        if self.min == f64::NEG_INFINITY && self.max == f64::INFINITY {
            return None;
        }
        if self.min == f64::NEG_INFINITY {
            return Some(self.max);
        }
        if self.max == f64::INFINITY {
            return Some(self.min);
        }
        Some((self.min + self.max) / 2.0)
    }

    /// Get the width (span) of this interval.
    ///
    /// Returns infinity if the interval is unbounded.
    pub fn width(&self) -> f64 {
        if !self.is_finite() {
            return f64::INFINITY;
        }
        self.max - self.min
    }

    /// Get the center and relative tolerance as a tuple.
    ///
    /// Returns (center, relative_tolerance) where relative_tolerance = half_width / center.
    pub fn as_center_rel(&self) -> Option<(f64, f64)> {
        let center = self.center()?;
        if center == 0.0 {
            return Some((center, f64::INFINITY));
        }
        let half_width = self.width() / 2.0;
        Some((center, (half_width / center).abs()))
    }

    // --- Arithmetic Operations ---

    /// Add two intervals: [a,b] + [c,d] = [a+c, b+d]
    pub fn add(&self, other: &Interval) -> Interval {
        Interval {
            min: self.min + other.min,
            max: self.max + other.max,
        }
    }

    /// Negate an interval: -[a,b] = [-b, -a]
    pub fn negate(&self) -> Interval {
        Interval {
            min: -self.max,
            max: -self.min,
        }
    }

    /// Subtract an interval: [a,b] - [c,d] = [a-d, b-c]
    pub fn subtract(&self, other: &Interval) -> Interval {
        self.add(&other.negate())
    }

    /// Multiply two intervals using interval arithmetic.
    pub fn multiply(&self, other: &Interval) -> Interval {
        // Handle multiplication with zero specially
        let products = [
            guarded_mul(self.min, other.min),
            guarded_mul(self.min, other.max),
            guarded_mul(self.max, other.min),
            guarded_mul(self.max, other.max),
        ];

        let min = products.iter().copied().fold(f64::INFINITY, f64::min);
        let max = products.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        Interval { min, max }
    }

    /// Compute the reciprocal of an interval: 1/[a,b]
    ///
    /// Returns a DisjointIntervals because 1/[a,b] where a < 0 < b
    /// results in (-∞, 1/a] ∪ [1/b, +∞).
    pub fn reciprocal(&self) -> super::DisjointIntervals {
        use super::DisjointIntervals;

        if self.min == 0.0 && self.max == 0.0 {
            // 1/0 is undefined - return empty
            return DisjointIntervals::empty();
        }

        if self.min < 0.0 && self.max > 0.0 {
            // Interval spans zero - results in two disjoint intervals
            DisjointIntervals::from_intervals(vec![
                Interval {
                    min: f64::NEG_INFINITY,
                    max: 1.0 / self.min,
                },
                Interval {
                    min: 1.0 / self.max,
                    max: f64::INFINITY,
                },
            ])
        } else if self.min < 0.0 && self.max == 0.0 {
            // (-∞, 1/min]
            DisjointIntervals::single(Interval {
                min: f64::NEG_INFINITY,
                max: 1.0 / self.min,
            })
        } else if self.min == 0.0 && self.max > 0.0 {
            // [1/max, +∞)
            DisjointIntervals::single(Interval {
                min: 1.0 / self.max,
                max: f64::INFINITY,
            })
        } else {
            // Normal case: interval doesn't contain zero
            DisjointIntervals::single(Interval {
                min: 1.0 / self.max,
                max: 1.0 / self.min,
            })
        }
    }

    /// Divide two intervals: [a,b] / [c,d]
    ///
    /// Returns DisjointIntervals because division can produce disjoint results.
    pub fn divide(&self, other: &Interval) -> super::DisjointIntervals {
        let reciprocal = other.reciprocal();
        reciprocal.multiply_interval(self)
    }

    /// Compute the absolute value of an interval.
    pub fn abs(&self) -> Interval {
        if self.min >= 0.0 {
            *self
        } else if self.max <= 0.0 {
            Interval {
                min: -self.max,
                max: -self.min,
            }
        } else {
            // Interval spans zero
            Interval {
                min: 0.0,
                max: self.max.max(-self.min),
            }
        }
    }

    // --- Set Operations ---

    /// Compute the intersection of two intervals.
    ///
    /// Returns None if the intervals don't overlap.
    pub fn intersect(&self, other: &Interval) -> Option<Interval> {
        let min = self.min.max(other.min);
        let max = self.max.min(other.max);

        if min <= max || approx_eq(min, max) {
            Some(Interval {
                min: min.min(max),
                max: max.max(min),
            })
        } else {
            None
        }
    }

    /// Try to merge two intervals if they overlap or are adjacent.
    ///
    /// Returns Some(merged) if the intervals can be merged, None otherwise.
    pub fn try_merge(&self, other: &Interval) -> Option<Interval> {
        if self.overlaps(other) || self.is_adjacent(other) {
            Some(Interval {
                min: self.min.min(other.min),
                max: self.max.max(other.max),
            })
        } else {
            None
        }
    }

    /// Compute the difference of two intervals: self - other.
    ///
    /// Returns a DisjointIntervals representing the set difference.
    pub fn difference(&self, other: &Interval) -> super::DisjointIntervals {
        use super::DisjointIntervals;

        // No overlap - return self unchanged
        if !self.overlaps(other) {
            return DisjointIntervals::single(*self);
        }

        // Other completely covers self
        if other.min <= self.min && other.max >= self.max {
            return DisjointIntervals::empty();
        }

        // Other is contained within self - creates two intervals
        if self.min < other.min && self.max > other.max {
            return DisjointIntervals::from_intervals(vec![
                Interval {
                    min: self.min,
                    max: other.min,
                },
                Interval {
                    min: other.max,
                    max: self.max,
                },
            ]);
        }

        // Left overlap
        if other.min <= self.min {
            return DisjointIntervals::single(Interval {
                min: other.max,
                max: self.max,
            });
        }

        // Right overlap
        DisjointIntervals::single(Interval {
            min: self.min,
            max: other.min,
        })
    }
}

/// Guarded multiplication that handles 0 * infinity = 0.
fn guarded_mul(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        return 0.0;
    }
    a * b
}

impl fmt::Debug for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Interval[{}, {}]", self.min, self.max)
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_singleton() {
            write!(f, "[{}]", self.min)
        } else if let Some((center, rel)) = self.as_center_rel() {
            if rel < 0.5 && rel.is_finite() {
                write!(f, "[{} ± {:.2}%]", center, rel * 100.0)
            } else {
                write!(f, "[{}, {}]", self.min, self.max)
            }
        } else {
            write!(f, "[{}, {}]", self.min, self.max)
        }
    }
}

impl PartialEq for Interval {
    fn eq(&self, other: &Self) -> bool {
        approx_eq(self.min, other.min) && approx_eq(self.max, other.max)
    }
}

impl Eq for Interval {}

impl std::hash::Hash for Interval {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Use bit representation for hashing to maintain consistency
        self.min.to_bits().hash(state);
        self.max.to_bits().hash(state);
    }
}

impl PartialOrd for Interval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Order by min, then by max
        match self.min.partial_cmp(&other.min) {
            Some(Ordering::Equal) => self.max.partial_cmp(&other.max),
            other => other,
        }
    }
}

impl Ord for Interval {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Errors that can occur when working with intervals.
#[derive(Debug, Clone, thiserror::Error)]
pub enum IntervalError {
    #[error("invalid bounds: min ({min}) > max ({max})")]
    InvalidBounds { min: f64, max: f64 },

    #[error("invalid infinite value in interval bounds")]
    InvalidInfinity,
}

// --- Operator implementations ---

impl std::ops::Add for Interval {
    type Output = Interval;

    fn add(self, rhs: Self) -> Self::Output {
        Interval::add(&self, &rhs)
    }
}

impl std::ops::Sub for Interval {
    type Output = Interval;

    fn sub(self, rhs: Self) -> Self::Output {
        Interval::subtract(&self, &rhs)
    }
}

impl std::ops::Neg for Interval {
    type Output = Interval;

    fn neg(self) -> Self::Output {
        Interval::negate(&self)
    }
}

impl std::ops::Mul for Interval {
    type Output = Interval;

    fn mul(self, rhs: Self) -> Self::Output {
        Interval::multiply(&self, &rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_creation() {
        let i = Interval::new(1.0, 5.0).unwrap();
        assert_eq!(i.lower(), 1.0);
        assert_eq!(i.upper(), 5.0);
    }

    #[test]
    fn test_interval_invalid_bounds() {
        let result = Interval::new(5.0, 1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_singleton() {
        let i = Interval::singleton(3.0).unwrap();
        assert!(i.is_singleton());
        assert!(i.contains(3.0));
    }

    #[test]
    fn test_contains() {
        let i = Interval::new(1.0, 10.0).unwrap();
        assert!(i.contains(1.0));
        assert!(i.contains(5.0));
        assert!(i.contains(10.0));
        assert!(!i.contains(0.0));
        assert!(!i.contains(11.0));
    }

    #[test]
    fn test_is_subset() {
        let outer = Interval::new(0.0, 10.0).unwrap();
        let inner = Interval::new(2.0, 8.0).unwrap();
        assert!(inner.is_subset_of(&outer));
        assert!(!outer.is_subset_of(&inner));
    }

    #[test]
    fn test_add() {
        let a = Interval::new(1.0, 3.0).unwrap();
        let b = Interval::new(2.0, 4.0).unwrap();
        let result = a + b;
        assert_eq!(result.lower(), 3.0);
        assert_eq!(result.upper(), 7.0);
    }

    #[test]
    fn test_multiply() {
        let a = Interval::new(1.0, 2.0).unwrap();
        let b = Interval::new(3.0, 4.0).unwrap();
        let result = a * b;
        assert_eq!(result.lower(), 3.0);
        assert_eq!(result.upper(), 8.0);
    }

    #[test]
    fn test_multiply_with_negative() {
        let a = Interval::new(-2.0, 3.0).unwrap();
        let b = Interval::new(1.0, 2.0).unwrap();
        let result = a * b;
        assert_eq!(result.lower(), -4.0);
        assert_eq!(result.upper(), 6.0);
    }

    #[test]
    fn test_intersect() {
        let a = Interval::new(1.0, 5.0).unwrap();
        let b = Interval::new(3.0, 7.0).unwrap();
        let result = a.intersect(&b).unwrap();
        assert_eq!(result.lower(), 3.0);
        assert_eq!(result.upper(), 5.0);
    }

    #[test]
    fn test_intersect_no_overlap() {
        let a = Interval::new(1.0, 3.0).unwrap();
        let b = Interval::new(5.0, 7.0).unwrap();
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn test_center_rel() {
        let i = Interval::new(9.0, 11.0).unwrap();
        let (center, rel) = i.as_center_rel().unwrap();
        assert!((center - 10.0).abs() < 1e-10);
        assert!((rel - 0.1).abs() < 1e-10);
    }
}
