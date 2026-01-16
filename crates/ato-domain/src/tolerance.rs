//! Bilateral tolerance specifications.
//!
//! A bilateral tolerance represents a range around a nominal value,
//! such as "10kohm +/- 5%" or "5V +/- 0.1V".

use crate::{Quantity, QuantityInterval, Unit};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A bilateral tolerance specification.
///
/// Represents a nominal value with a tolerance that can be either:
/// - Relative (percentage): 10kohm +/- 5%
/// - Absolute: 5V +/- 0.1V
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BilateralTolerance {
    /// The nominal (center) value.
    nominal: Quantity,
    /// The tolerance specification.
    tolerance: ToleranceSpec,
}

/// The tolerance specification - either relative or absolute.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ToleranceSpec {
    /// Relative tolerance as a fraction (e.g., 0.05 for 5%).
    Relative(f64),
    /// Absolute tolerance as a value.
    Absolute(f64),
}

impl BilateralTolerance {
    /// Create a bilateral tolerance with relative (percentage) tolerance.
    ///
    /// # Arguments
    /// * `nominal` - The center value
    /// * `relative` - The relative tolerance as a fraction (e.g., 0.05 for 5%)
    ///
    /// # Example
    /// ```
    /// use ato_domain::{BilateralTolerance, Quantity, Unit};
    ///
    /// // 10kohm +/- 5%
    /// let tol = BilateralTolerance::relative(
    ///     Quantity::new(10_000.0, Unit::Ohm),
    ///     0.05
    /// );
    /// ```
    pub fn relative(nominal: Quantity, relative: f64) -> Self {
        Self {
            nominal,
            tolerance: ToleranceSpec::Relative(relative),
        }
    }

    /// Create a bilateral tolerance with absolute tolerance.
    ///
    /// # Arguments
    /// * `nominal` - The center value
    /// * `absolute` - The absolute tolerance value (must have same unit as nominal)
    ///
    /// # Example
    /// ```
    /// use ato_domain::{BilateralTolerance, Quantity, Unit};
    ///
    /// // 5V +/- 0.1V
    /// let tol = BilateralTolerance::absolute(
    ///     Quantity::new(5.0, Unit::Volt),
    ///     Quantity::new(0.1, Unit::Volt)
    /// );
    /// ```
    pub fn absolute(nominal: Quantity, absolute: Quantity) -> Result<Self, ToleranceError> {
        if !nominal.unit().is_compatible_with(&absolute.unit()) {
            return Err(ToleranceError::IncompatibleUnits {
                nominal: nominal.unit(),
                tolerance: absolute.unit(),
            });
        }
        let abs_base = absolute.to_base_units();
        Ok(Self {
            nominal,
            tolerance: ToleranceSpec::Absolute(abs_base.value()),
        })
    }

    /// Create a bilateral tolerance from minimum and maximum values.
    ///
    /// The nominal value will be the center of the range.
    pub fn from_range(min: Quantity, max: Quantity) -> Result<Self, ToleranceError> {
        if !min.unit().is_compatible_with(&max.unit()) {
            return Err(ToleranceError::IncompatibleUnits {
                nominal: min.unit(),
                tolerance: max.unit(),
            });
        }

        let min_base = min.to_base_units();
        let max_base = max.to_base_units();

        if min_base.value() > max_base.value() {
            return Err(ToleranceError::InvalidRange);
        }

        let center = (min_base.value() + max_base.value()) / 2.0;
        let half_width = (max_base.value() - min_base.value()) / 2.0;

        Ok(Self {
            nominal: Quantity::new(center, min_base.unit()),
            tolerance: ToleranceSpec::Absolute(half_width),
        })
    }

    /// Get the nominal (center) value.
    pub fn nominal(&self) -> Quantity {
        self.nominal
    }

    /// Get the tolerance specification.
    pub fn tolerance_spec(&self) -> ToleranceSpec {
        self.tolerance
    }

    /// Get the absolute tolerance value.
    pub fn absolute_tolerance(&self) -> Quantity {
        match self.tolerance {
            ToleranceSpec::Relative(rel) => {
                let base = self.nominal.to_base_units();
                Quantity::new(base.value().abs() * rel, base.unit())
            }
            ToleranceSpec::Absolute(abs) => {
                let base = self.nominal.to_base_units();
                Quantity::new(abs, base.unit())
            }
        }
    }

    /// Get the relative tolerance as a fraction.
    pub fn relative_tolerance(&self) -> f64 {
        match self.tolerance {
            ToleranceSpec::Relative(rel) => rel,
            ToleranceSpec::Absolute(abs) => {
                let base = self.nominal.to_base_units();
                if base.value() == 0.0 {
                    f64::INFINITY
                } else {
                    abs / base.value().abs()
                }
            }
        }
    }

    /// Get the minimum value.
    pub fn min(&self) -> Quantity {
        let base = self.nominal.to_base_units();
        let abs_tol = self.absolute_tolerance().value();
        Quantity::new(base.value() - abs_tol, base.unit())
    }

    /// Get the maximum value.
    pub fn max(&self) -> Quantity {
        let base = self.nominal.to_base_units();
        let abs_tol = self.absolute_tolerance().value();
        Quantity::new(base.value() + abs_tol, base.unit())
    }

    /// Get the unit.
    pub fn unit(&self) -> Unit {
        self.nominal.unit()
    }

    /// Convert to a QuantityInterval.
    pub fn to_interval(&self) -> Result<QuantityInterval, crate::quantity::QuantityError> {
        QuantityInterval::new(self.min(), self.max())
    }

    /// Check if a quantity is within this tolerance.
    pub fn contains(&self, value: &Quantity) -> bool {
        if !self.nominal.unit().is_compatible_with(&value.unit()) {
            return false;
        }
        let base = value.to_base_units();
        let min = self.min().to_base_units();
        let max = self.max().to_base_units();
        base.value() >= min.value() && base.value() <= max.value()
    }

    /// Check if this tolerance overlaps with another.
    pub fn overlaps(&self, other: &BilateralTolerance) -> bool {
        if !self.unit().is_compatible_with(&other.unit()) {
            return false;
        }
        let self_min = self.min().to_base_units().value();
        let self_max = self.max().to_base_units().value();
        let other_min = other.min().to_base_units().value();
        let other_max = other.max().to_base_units().value();

        self_max >= other_min && other_max >= self_min
    }

    /// Check if this tolerance is a subset of another (completely contained).
    pub fn is_subset_of(&self, other: &BilateralTolerance) -> bool {
        if !self.unit().is_compatible_with(&other.unit()) {
            return false;
        }
        let self_min = self.min().to_base_units().value();
        let self_max = self.max().to_base_units().value();
        let other_min = other.min().to_base_units().value();
        let other_max = other.max().to_base_units().value();

        self_min >= other_min && self_max <= other_max
    }
}

impl PartialEq for BilateralTolerance {
    fn eq(&self, other: &Self) -> bool {
        // Compare the actual ranges
        let self_min = self.min().to_base_units();
        let self_max = self.max().to_base_units();
        let other_min = other.min().to_base_units();
        let other_max = other.max().to_base_units();

        (self_min.value() - other_min.value()).abs() < 1e-15
            && (self_max.value() - other_max.value()).abs() < 1e-15
            && self_min.unit().is_compatible_with(&other_min.unit())
    }
}

impl fmt::Display for BilateralTolerance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.tolerance {
            ToleranceSpec::Relative(rel) => {
                write!(f, "{} ± {:.2}%", self.nominal, rel * 100.0)
            }
            ToleranceSpec::Absolute(abs) => {
                let tol = Quantity::new(abs, self.nominal.to_base_units().unit());
                write!(f, "{} ± {}", self.nominal, tol)
            }
        }
    }
}

/// Errors that can occur when working with tolerances.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ToleranceError {
    #[error("incompatible units: nominal {nominal} and tolerance {tolerance}")]
    IncompatibleUnits { nominal: Unit, tolerance: Unit },

    #[error("invalid range: min > max")]
    InvalidRange,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relative_tolerance() {
        let tol = BilateralTolerance::relative(
            Quantity::new(10_000.0, Unit::Ohm),
            0.05, // 5%
        );

        assert_eq!(tol.nominal().value(), 10_000.0);
        assert!((tol.min().value() - 9_500.0).abs() < 0.01);
        assert!((tol.max().value() - 10_500.0).abs() < 0.01);
        assert!((tol.relative_tolerance() - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_absolute_tolerance() {
        let tol = BilateralTolerance::absolute(
            Quantity::new(5.0, Unit::Volt),
            Quantity::new(0.1, Unit::Volt),
        )
        .unwrap();

        assert_eq!(tol.nominal().value(), 5.0);
        assert!((tol.min().value() - 4.9).abs() < 0.001);
        assert!((tol.max().value() - 5.1).abs() < 0.001);
    }

    #[test]
    fn test_from_range() {
        let tol = BilateralTolerance::from_range(
            Quantity::new(9_500.0, Unit::Ohm),
            Quantity::new(10_500.0, Unit::Ohm),
        )
        .unwrap();

        assert!((tol.nominal().value() - 10_000.0).abs() < 0.01);
        assert!((tol.min().value() - 9_500.0).abs() < 0.01);
        assert!((tol.max().value() - 10_500.0).abs() < 0.01);
    }

    #[test]
    fn test_contains() {
        let tol = BilateralTolerance::relative(
            Quantity::new(10.0, Unit::Volt),
            0.1, // 10%
        );

        assert!(tol.contains(&Quantity::new(10.0, Unit::Volt)));
        assert!(tol.contains(&Quantity::new(9.5, Unit::Volt)));
        assert!(tol.contains(&Quantity::new(10.5, Unit::Volt)));
        assert!(!tol.contains(&Quantity::new(8.0, Unit::Volt)));
        assert!(!tol.contains(&Quantity::new(12.0, Unit::Volt)));
    }

    #[test]
    fn test_overlaps() {
        let tol1 = BilateralTolerance::from_range(
            Quantity::new(5.0, Unit::Volt),
            Quantity::new(10.0, Unit::Volt),
        )
        .unwrap();

        let tol2 = BilateralTolerance::from_range(
            Quantity::new(8.0, Unit::Volt),
            Quantity::new(15.0, Unit::Volt),
        )
        .unwrap();

        let tol3 = BilateralTolerance::from_range(
            Quantity::new(20.0, Unit::Volt),
            Quantity::new(25.0, Unit::Volt),
        )
        .unwrap();

        assert!(tol1.overlaps(&tol2));
        assert!(!tol1.overlaps(&tol3));
    }

    #[test]
    fn test_is_subset() {
        let outer = BilateralTolerance::from_range(
            Quantity::new(0.0, Unit::Volt),
            Quantity::new(20.0, Unit::Volt),
        )
        .unwrap();

        let inner = BilateralTolerance::from_range(
            Quantity::new(5.0, Unit::Volt),
            Quantity::new(15.0, Unit::Volt),
        )
        .unwrap();

        assert!(inner.is_subset_of(&outer));
        assert!(!outer.is_subset_of(&inner));
    }

    #[test]
    fn test_to_interval() {
        let tol = BilateralTolerance::relative(
            Quantity::new(10.0, Unit::Ohm),
            0.1,
        );

        let interval = tol.to_interval().unwrap();
        assert!(interval.contains(&Quantity::new(10.0, Unit::Ohm)));
        assert!(interval.contains(&Quantity::new(9.5, Unit::Ohm)));
    }

    #[test]
    fn test_display() {
        let tol = BilateralTolerance::relative(
            Quantity::new(10_000.0, Unit::Ohm),
            0.05,
        );
        let display = format!("{}", tol);
        assert!(display.contains("10"));
        assert!(display.contains("5.00%"));
    }
}
