//! Disjoint intervals - a union of non-overlapping intervals.

use crate::Interval;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A set of non-overlapping intervals representing a union.
///
/// DisjointIntervals maintains its intervals sorted by minimum value
/// and merged when possible. This is useful for representing sets that
/// have gaps, such as the result of dividing intervals that span zero.
#[derive(Clone, Serialize, Deserialize)]
pub struct DisjointIntervals {
    /// The intervals, sorted by min and non-overlapping.
    intervals: Vec<Interval>,
}

impl DisjointIntervals {
    /// Create an empty set (no intervals).
    pub fn empty() -> Self {
        Self { intervals: vec![] }
    }

    /// Create a set containing a single interval.
    pub fn single(interval: Interval) -> Self {
        Self {
            intervals: vec![interval],
        }
    }

    /// Create a set from multiple intervals.
    ///
    /// The intervals will be sorted and merged if they overlap.
    pub fn from_intervals(intervals: Vec<Interval>) -> Self {
        let mut result = Self::empty();
        for interval in intervals {
            result = result.union_interval(&interval);
        }
        result
    }

    /// Create an unbounded set (-∞, +∞).
    pub fn unbounded() -> Self {
        Self::single(Interval::unbounded())
    }

    /// Check if this set is empty.
    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    /// Get the number of disjoint intervals in this set.
    pub fn len(&self) -> usize {
        self.intervals.len()
    }

    /// Check if this set is unbounded (-∞, +∞).
    pub fn is_unbounded(&self) -> bool {
        self.intervals.len() == 1 && self.intervals[0].is_unbounded()
    }

    /// Check if all intervals are finite.
    pub fn is_finite(&self) -> bool {
        self.intervals.iter().all(|i| i.is_finite())
    }

    /// Get the minimum element of this set.
    ///
    /// Returns None if the set is empty.
    pub fn min(&self) -> Option<f64> {
        self.intervals.first().map(|i| i.lower())
    }

    /// Get the maximum element of this set.
    ///
    /// Returns None if the set is empty.
    pub fn max(&self) -> Option<f64> {
        self.intervals.last().map(|i| i.upper())
    }

    /// Get an iterator over the intervals.
    pub fn iter(&self) -> impl Iterator<Item = &Interval> {
        self.intervals.iter()
    }

    /// Get the intervals as a slice.
    pub fn as_slice(&self) -> &[Interval] {
        &self.intervals
    }

    /// Check if a value is contained in any interval.
    pub fn contains(&self, value: f64) -> bool {
        // Binary search could be used here for large sets
        self.intervals.iter().any(|i| i.contains(value))
    }

    /// Check if this set is a subset of another.
    pub fn is_subset_of(&self, other: &DisjointIntervals) -> bool {
        // Every interval in self must be covered by intervals in other
        for interval in &self.intervals {
            let intersection = other.intersect_interval(interval);
            if intersection.intervals.len() != 1 || intersection.intervals[0] != *interval {
                return false;
            }
        }
        true
    }

    /// Check if this set is a superset of another.
    pub fn is_superset_of(&self, other: &DisjointIntervals) -> bool {
        other.is_subset_of(self)
    }

    /// Check if this set represents a single value.
    pub fn is_singleton(&self) -> bool {
        self.intervals.len() == 1 && self.intervals[0].is_singleton()
    }

    /// Get any element from this set (returns the minimum).
    pub fn any(&self) -> Option<f64> {
        self.min()
    }

    // --- Set Operations ---

    /// Compute the union with a single interval.
    pub fn union_interval(&self, interval: &Interval) -> DisjointIntervals {
        let mut result = Vec::with_capacity(self.intervals.len() + 1);
        let mut to_merge = *interval;
        let mut merged = false;

        for existing in &self.intervals {
            if let Some(merged_interval) = to_merge.try_merge(existing) {
                to_merge = merged_interval;
                merged = true;
            } else if existing.lower() > to_merge.upper() {
                if merged || result.is_empty() || result.last().map(|l: &Interval| l.upper() < to_merge.lower()).unwrap_or(true) {
                    result.push(to_merge);
                    to_merge = *existing;
                    merged = false;
                } else {
                    result.push(*existing);
                }
            } else {
                result.push(*existing);
            }
        }

        // Add the final interval
        if result.is_empty() || result.last().map(|l| l.upper() < to_merge.lower()).unwrap_or(true) {
            result.push(to_merge);
        }

        DisjointIntervals { intervals: result }
    }

    /// Compute the union with another DisjointIntervals.
    pub fn union(&self, other: &DisjointIntervals) -> DisjointIntervals {
        let mut result = self.clone();
        for interval in &other.intervals {
            result = result.union_interval(interval);
        }
        result
    }

    /// Compute the intersection with a single interval.
    pub fn intersect_interval(&self, interval: &Interval) -> DisjointIntervals {
        let mut result = Vec::new();
        for existing in &self.intervals {
            if let Some(intersection) = existing.intersect(interval) {
                result.push(intersection);
            }
        }
        DisjointIntervals { intervals: result }
    }

    /// Compute the intersection with another DisjointIntervals.
    pub fn intersect(&self, other: &DisjointIntervals) -> DisjointIntervals {
        let mut result = Vec::new();
        let mut i = 0;
        let mut j = 0;

        while i < self.intervals.len() && j < other.intervals.len() {
            let a = &self.intervals[i];
            let b = &other.intervals[j];

            if let Some(intersection) = a.intersect(b) {
                result.push(intersection);
            }

            // Advance the pointer for the interval that ends first
            if a.upper() < b.upper() {
                i += 1;
            } else if b.upper() < a.upper() {
                j += 1;
            } else {
                i += 1;
                j += 1;
            }
        }

        DisjointIntervals { intervals: result }
    }

    /// Compute the difference (self - other).
    pub fn difference(&self, other: &DisjointIntervals) -> DisjointIntervals {
        let mut result = self.clone();
        for interval in &other.intervals {
            result = result.difference_interval(interval);
        }
        result
    }

    /// Compute the difference with a single interval.
    pub fn difference_interval(&self, interval: &Interval) -> DisjointIntervals {
        let mut result = Vec::new();
        for existing in &self.intervals {
            let diff = existing.difference(interval);
            result.extend(diff.intervals);
        }
        DisjointIntervals { intervals: result }
    }

    /// Compute the symmetric difference (elements in either but not both).
    pub fn symmetric_difference(&self, other: &DisjointIntervals) -> DisjointIntervals {
        let union = self.union(other);
        let intersection = self.intersect(other);
        union.difference(&intersection)
    }

    // --- Arithmetic Operations ---

    /// Add another DisjointIntervals to this one.
    pub fn add(&self, other: &DisjointIntervals) -> DisjointIntervals {
        if self.is_empty() || other.is_empty() {
            return DisjointIntervals::empty();
        }

        let mut result = Vec::new();
        for a in &self.intervals {
            for b in &other.intervals {
                result.push(a.add(b));
            }
        }

        DisjointIntervals::from_intervals(result)
    }

    /// Negate all intervals.
    pub fn negate(&self) -> DisjointIntervals {
        let intervals: Vec<_> = self.intervals.iter().map(|i| i.negate()).collect();
        // Negation reverses order, so we need to re-sort
        DisjointIntervals::from_intervals(intervals)
    }

    /// Subtract another DisjointIntervals.
    pub fn subtract(&self, other: &DisjointIntervals) -> DisjointIntervals {
        self.add(&other.negate())
    }

    /// Multiply by a single interval.
    pub fn multiply_interval(&self, interval: &Interval) -> DisjointIntervals {
        if self.is_empty() {
            return DisjointIntervals::empty();
        }

        let mut result = Vec::new();
        for existing in &self.intervals {
            result.push(existing.multiply(interval));
        }

        DisjointIntervals::from_intervals(result)
    }

    /// Multiply two DisjointIntervals.
    pub fn multiply(&self, other: &DisjointIntervals) -> DisjointIntervals {
        if self.is_empty() || other.is_empty() {
            return DisjointIntervals::empty();
        }

        let mut result = Vec::new();
        for a in &self.intervals {
            for b in &other.intervals {
                result.push(a.multiply(b));
            }
        }

        DisjointIntervals::from_intervals(result)
    }

    /// Compute the reciprocal (1/x) of all intervals.
    pub fn reciprocal(&self) -> DisjointIntervals {
        if self.is_empty() {
            return DisjointIntervals::empty();
        }

        let mut result = DisjointIntervals::empty();
        for interval in &self.intervals {
            result = result.union(&interval.reciprocal());
        }
        result
    }

    /// Divide by another DisjointIntervals.
    pub fn divide(&self, other: &DisjointIntervals) -> DisjointIntervals {
        self.multiply(&other.reciprocal())
    }

    /// Compute the absolute value of all intervals.
    pub fn abs(&self) -> DisjointIntervals {
        let intervals: Vec<_> = self.intervals.iter().map(|i| i.abs()).collect();
        DisjointIntervals::from_intervals(intervals)
    }

    /// Get the total span (sum of widths) of all intervals.
    pub fn total_span(&self) -> f64 {
        self.intervals.iter().map(|i| i.width()).sum()
    }

    /// Find the closest element to a target value.
    pub fn closest_to(&self, target: f64) -> Option<f64> {
        if self.is_empty() {
            return None;
        }

        if self.contains(target) {
            return Some(target);
        }

        let mut closest = None;
        let mut min_dist = f64::INFINITY;

        for interval in &self.intervals {
            let dist_to_min = (target - interval.lower()).abs();
            let dist_to_max = (target - interval.upper()).abs();

            if dist_to_min < min_dist {
                min_dist = dist_to_min;
                closest = Some(interval.lower());
            }
            if dist_to_max < min_dist {
                min_dist = dist_to_max;
                closest = Some(interval.upper());
            }
        }

        closest
    }
}

impl Default for DisjointIntervals {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for DisjointIntervals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DisjointIntervals{:?}", self.intervals)
    }
}

impl fmt::Display for DisjointIntervals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "∅")
        } else {
            let intervals: Vec<_> = self.intervals.iter().map(|i| format!("{}", i)).collect();
            write!(f, "({})", intervals.join(" ∪ "))
        }
    }
}

impl PartialEq for DisjointIntervals {
    fn eq(&self, other: &Self) -> bool {
        self.intervals == other.intervals
    }
}

impl Eq for DisjointIntervals {}

impl std::hash::Hash for DisjointIntervals {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.intervals.hash(state);
    }
}

impl From<Interval> for DisjointIntervals {
    fn from(interval: Interval) -> Self {
        DisjointIntervals::single(interval)
    }
}

// --- Operator implementations ---

impl std::ops::Add for DisjointIntervals {
    type Output = DisjointIntervals;

    fn add(self, rhs: Self) -> Self::Output {
        DisjointIntervals::add(&self, &rhs)
    }
}

impl std::ops::Sub for DisjointIntervals {
    type Output = DisjointIntervals;

    fn sub(self, rhs: Self) -> Self::Output {
        DisjointIntervals::subtract(&self, &rhs)
    }
}

impl std::ops::Neg for DisjointIntervals {
    type Output = DisjointIntervals;

    fn neg(self) -> Self::Output {
        DisjointIntervals::negate(&self)
    }
}

impl std::ops::Mul for DisjointIntervals {
    type Output = DisjointIntervals;

    fn mul(self, rhs: Self) -> Self::Output {
        DisjointIntervals::multiply(&self, &rhs)
    }
}

impl std::ops::Div for DisjointIntervals {
    type Output = DisjointIntervals;

    fn div(self, rhs: Self) -> Self::Output {
        DisjointIntervals::divide(&self, &rhs)
    }
}

impl std::ops::BitAnd for DisjointIntervals {
    type Output = DisjointIntervals;

    fn bitand(self, rhs: Self) -> Self::Output {
        DisjointIntervals::intersect(&self, &rhs)
    }
}

impl std::ops::BitOr for DisjointIntervals {
    type Output = DisjointIntervals;

    fn bitor(self, rhs: Self) -> Self::Output {
        DisjointIntervals::union(&self, &rhs)
    }
}

impl std::ops::BitXor for DisjointIntervals {
    type Output = DisjointIntervals;

    fn bitxor(self, rhs: Self) -> Self::Output {
        DisjointIntervals::symmetric_difference(&self, &rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let empty = DisjointIntervals::empty();
        assert!(empty.is_empty());
        assert!(!empty.contains(0.0));
    }

    #[test]
    fn test_single() {
        let interval = Interval::new(1.0, 5.0).unwrap();
        let set = DisjointIntervals::single(interval);
        assert!(!set.is_empty());
        assert_eq!(set.len(), 1);
        assert!(set.contains(3.0));
    }

    #[test]
    fn test_union_non_overlapping() {
        let a = DisjointIntervals::single(Interval::new(1.0, 3.0).unwrap());
        let b = DisjointIntervals::single(Interval::new(5.0, 7.0).unwrap());
        let union = a.union(&b);
        assert_eq!(union.len(), 2);
        assert!(union.contains(2.0));
        assert!(union.contains(6.0));
        assert!(!union.contains(4.0));
    }

    #[test]
    fn test_union_overlapping() {
        let a = DisjointIntervals::single(Interval::new(1.0, 5.0).unwrap());
        let b = DisjointIntervals::single(Interval::new(3.0, 7.0).unwrap());
        let union = a.union(&b);
        assert_eq!(union.len(), 1);
        assert_eq!(union.min(), Some(1.0));
        assert_eq!(union.max(), Some(7.0));
    }

    #[test]
    fn test_intersect() {
        let a = DisjointIntervals::single(Interval::new(1.0, 5.0).unwrap());
        let b = DisjointIntervals::single(Interval::new(3.0, 7.0).unwrap());
        let intersection = a.intersect(&b);
        assert_eq!(intersection.len(), 1);
        assert_eq!(intersection.min(), Some(3.0));
        assert_eq!(intersection.max(), Some(5.0));
    }

    #[test]
    fn test_intersect_no_overlap() {
        let a = DisjointIntervals::single(Interval::new(1.0, 3.0).unwrap());
        let b = DisjointIntervals::single(Interval::new(5.0, 7.0).unwrap());
        let intersection = a.intersect(&b);
        assert!(intersection.is_empty());
    }

    #[test]
    fn test_difference() {
        let a = DisjointIntervals::single(Interval::new(1.0, 10.0).unwrap());
        let b = DisjointIntervals::single(Interval::new(4.0, 6.0).unwrap());
        let diff = a.difference(&b);
        assert_eq!(diff.len(), 2);
        assert!(diff.contains(2.0));
        assert!(diff.contains(8.0));
        assert!(!diff.contains(5.0));
    }

    #[test]
    fn test_arithmetic_add() {
        let a = DisjointIntervals::single(Interval::new(1.0, 2.0).unwrap());
        let b = DisjointIntervals::single(Interval::new(3.0, 4.0).unwrap());
        let sum = a + b;
        assert_eq!(sum.min(), Some(4.0));
        assert_eq!(sum.max(), Some(6.0));
    }

    #[test]
    fn test_arithmetic_multiply() {
        let a = DisjointIntervals::single(Interval::new(2.0, 3.0).unwrap());
        let b = DisjointIntervals::single(Interval::new(4.0, 5.0).unwrap());
        let product = a * b;
        assert_eq!(product.min(), Some(8.0));
        assert_eq!(product.max(), Some(15.0));
    }

    #[test]
    fn test_reciprocal_positive() {
        let set = DisjointIntervals::single(Interval::new(2.0, 4.0).unwrap());
        let recip = set.reciprocal();
        assert_eq!(recip.len(), 1);
        assert_eq!(recip.min(), Some(0.25));
        assert_eq!(recip.max(), Some(0.5));
    }

    #[test]
    fn test_reciprocal_spanning_zero() {
        let set = DisjointIntervals::single(Interval::new(-2.0, 2.0).unwrap());
        let recip = set.reciprocal();
        // Should produce two intervals: (-∞, -0.5] ∪ [0.5, +∞)
        assert_eq!(recip.len(), 2);
    }

    #[test]
    fn test_is_subset() {
        let outer = DisjointIntervals::single(Interval::new(0.0, 10.0).unwrap());
        let inner = DisjointIntervals::single(Interval::new(2.0, 8.0).unwrap());
        assert!(inner.is_subset_of(&outer));
        assert!(!outer.is_subset_of(&inner));
    }
}
