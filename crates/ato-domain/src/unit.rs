//! Physical units for quantity tracking.
//!
//! This module provides a simple unit system for common electrical units.
//! For more complex unit handling, consider using the `uom` crate.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A physical unit for quantities.
///
/// Units are represented with base SI dimensions and a scaling factor.
/// This allows for unit conversion and compatibility checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    /// Dimensionless quantity (no unit).
    Dimensionless,

    // Electrical units
    /// Volts (V) - electric potential
    Volt,
    /// Amperes (A) - electric current
    Ampere,
    /// Ohms (Ω) - electrical resistance
    Ohm,
    /// Farads (F) - electrical capacitance
    Farad,
    /// Henrys (H) - electrical inductance
    Henry,
    /// Watts (W) - power
    Watt,
    /// Hertz (Hz) - frequency
    Hertz,

    // Metric prefixes (applied to base units)
    // Time units
    /// Seconds (s)
    Second,

    // Length units
    /// Meters (m)
    Meter,
    /// Millimeters (mm)
    Millimeter,
}

impl Unit {
    /// Get the unit symbol as a string.
    pub fn symbol(&self) -> &'static str {
        match self {
            Unit::Dimensionless => "",
            Unit::Volt => "V",
            Unit::Ampere => "A",
            Unit::Ohm => "Ω",
            Unit::Farad => "F",
            Unit::Henry => "H",
            Unit::Watt => "W",
            Unit::Hertz => "Hz",
            Unit::Second => "s",
            Unit::Meter => "m",
            Unit::Millimeter => "mm",
        }
    }

    /// Get the full name of the unit.
    pub fn name(&self) -> &'static str {
        match self {
            Unit::Dimensionless => "dimensionless",
            Unit::Volt => "volt",
            Unit::Ampere => "ampere",
            Unit::Ohm => "ohm",
            Unit::Farad => "farad",
            Unit::Henry => "henry",
            Unit::Watt => "watt",
            Unit::Hertz => "hertz",
            Unit::Second => "second",
            Unit::Meter => "meter",
            Unit::Millimeter => "millimeter",
        }
    }

    /// Check if two units are compatible (can be converted between).
    ///
    /// For simplicity, units are compatible if they are the same.
    /// A more complete implementation would check dimensional compatibility.
    pub fn is_compatible_with(&self, other: &Unit) -> bool {
        match (self, other) {
            // Same unit is always compatible
            (a, b) if a == b => true,
            // Dimensionless is compatible with dimensionless
            (Unit::Dimensionless, Unit::Dimensionless) => true,
            // Length units are compatible
            (Unit::Meter, Unit::Millimeter) | (Unit::Millimeter, Unit::Meter) => true,
            // Everything else is incompatible
            _ => false,
        }
    }

    /// Get the conversion factor to the base unit.
    ///
    /// For example, millimeters have a factor of 0.001 to meters.
    pub fn to_base_factor(&self) -> f64 {
        match self {
            Unit::Millimeter => 0.001, // mm to m
            _ => 1.0,
        }
    }

    /// Get the base unit for this unit.
    pub fn base_unit(&self) -> Unit {
        match self {
            Unit::Millimeter => Unit::Meter,
            other => *other,
        }
    }

    /// Check if this unit is dimensionless.
    pub fn is_dimensionless(&self) -> bool {
        matches!(self, Unit::Dimensionless)
    }

    /// Compute the result unit of multiplying two units.
    ///
    /// Returns None if the multiplication is not supported.
    pub fn multiply(left: &Unit, right: &Unit) -> Option<Unit> {
        match (left, right) {
            (u, Unit::Dimensionless) | (Unit::Dimensionless, u) => Some(*u),
            (Unit::Volt, Unit::Ampere) | (Unit::Ampere, Unit::Volt) => Some(Unit::Watt),
            (Unit::Ohm, Unit::Ampere) | (Unit::Ampere, Unit::Ohm) => Some(Unit::Volt),
            _ => None,
        }
    }

    /// Compute the result unit of dividing two units.
    ///
    /// Returns None if the division is not supported.
    pub fn divide(left: &Unit, right: &Unit) -> Option<Unit> {
        match (left, right) {
            (u, Unit::Dimensionless) => Some(*u),
            (a, b) if a == b => Some(Unit::Dimensionless),
            (Unit::Volt, Unit::Ampere) => Some(Unit::Ohm),
            (Unit::Volt, Unit::Ohm) => Some(Unit::Ampere),
            (Unit::Watt, Unit::Volt) => Some(Unit::Ampere),
            (Unit::Watt, Unit::Ampere) => Some(Unit::Volt),
            _ => None,
        }
    }

    /// Parse a unit from a string.
    pub fn from_str(s: &str) -> Option<Unit> {
        match s.to_lowercase().as_str() {
            "" | "1" => Some(Unit::Dimensionless),
            "v" | "volt" | "volts" => Some(Unit::Volt),
            "a" | "amp" | "ampere" | "amperes" => Some(Unit::Ampere),
            "ohm" | "ohms" | "ω" => Some(Unit::Ohm),
            "f" | "farad" | "farads" => Some(Unit::Farad),
            "h" | "henry" | "henrys" | "henries" => Some(Unit::Henry),
            "w" | "watt" | "watts" => Some(Unit::Watt),
            "hz" | "hertz" => Some(Unit::Hertz),
            "s" | "sec" | "second" | "seconds" => Some(Unit::Second),
            "m" | "meter" | "meters" => Some(Unit::Meter),
            "mm" | "millimeter" | "millimeters" => Some(Unit::Millimeter),
            _ => None,
        }
    }
}

impl Default for Unit {
    fn default() -> Self {
        Unit::Dimensionless
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

/// Parse a number string with SI prefix.
///
/// Returns the numeric value with the prefix applied.
/// For example, "10k" returns 10000.0, "5m" returns 0.005.
#[cfg(test)]
fn parse_si_prefix(s: &str) -> Option<(f64, &str)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try to find where the number ends and the unit/prefix begins
    let mut num_end = 0;
    let mut has_dot = false;
    let mut has_e = false;

    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            num_end = i + 1;
        } else if c == '.' && !has_dot {
            has_dot = true;
            num_end = i + 1;
        } else if (c == 'e' || c == 'E') && !has_e && i > 0 {
            has_e = true;
            num_end = i + 1;
        } else if (c == '+' || c == '-') && has_e && (s.as_bytes().get(i - 1) == Some(&b'e') || s.as_bytes().get(i - 1) == Some(&b'E')) {
            num_end = i + 1;
        } else {
            break;
        }
    }

    if num_end == 0 {
        return None;
    }

    let num_str = &s[..num_end];
    let suffix = &s[num_end..];

    let base_value: f64 = num_str.parse().ok()?;

    // Apply SI prefix if present
    let (multiplier, unit_start) = if let Some(first_char) = suffix.chars().next() {
        match first_char {
            'T' => (1e12, 1),
            'G' => (1e9, 1),
            'M' => (1e6, 1),
            'k' | 'K' => (1e3, 1),
            'm' if suffix.len() > 1 && suffix.chars().nth(1).map(|c| c.is_alphabetic()).unwrap_or(false) => (1e-3, 1),
            'u' | 'μ' | 'µ' => (1e-6, 1),
            'n' => (1e-9, 1),
            'p' => (1e-12, 1),
            'f' if suffix.len() > 1 => (1e-15, 1),
            _ => (1.0, 0),
        }
    } else {
        (1.0, 0)
    };

    let unit_str = &suffix[unit_start..];
    Some((base_value * multiplier, unit_str))
}

/// Format a number with SI prefix.
pub fn format_si(value: f64, unit: &Unit, precision: usize) -> String {
    if value == 0.0 {
        return format!("0{}", unit.symbol());
    }

    let abs_value = value.abs();
    let (prefix, divisor) = if abs_value >= 1e12 {
        ("T", 1e12)
    } else if abs_value >= 1e9 {
        ("G", 1e9)
    } else if abs_value >= 1e6 {
        ("M", 1e6)
    } else if abs_value >= 1e3 {
        ("k", 1e3)
    } else if abs_value >= 1.0 {
        ("", 1.0)
    } else if abs_value >= 1e-3 {
        ("m", 1e-3)
    } else if abs_value >= 1e-6 {
        ("μ", 1e-6)
    } else if abs_value >= 1e-9 {
        ("n", 1e-9)
    } else if abs_value >= 1e-12 {
        ("p", 1e-12)
    } else {
        ("f", 1e-15)
    };

    let scaled = value / divisor;
    format!("{:.prec$}{}{}", scaled, prefix, unit.symbol(), prec = precision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_symbol() {
        assert_eq!(Unit::Volt.symbol(), "V");
        assert_eq!(Unit::Ohm.symbol(), "Ω");
        assert_eq!(Unit::Farad.symbol(), "F");
    }

    #[test]
    fn test_unit_compatibility() {
        assert!(Unit::Volt.is_compatible_with(&Unit::Volt));
        assert!(!Unit::Volt.is_compatible_with(&Unit::Ampere));
        assert!(Unit::Meter.is_compatible_with(&Unit::Millimeter));
    }

    #[test]
    fn test_parse_unit() {
        assert_eq!(Unit::from_str("V"), Some(Unit::Volt));
        assert_eq!(Unit::from_str("ohm"), Some(Unit::Ohm));
        assert_eq!(Unit::from_str("Hz"), Some(Unit::Hertz));
    }

    #[test]
    fn test_parse_si_prefix() {
        let (value, unit) = parse_si_prefix("10k").unwrap();
        assert!((value - 10000.0).abs() < 1e-10);
        assert_eq!(unit, "");

        let (value, unit) = parse_si_prefix("5.5mV").unwrap();
        assert!((value - 0.0055).abs() < 1e-10);
        assert_eq!(unit, "V");

        let (value, unit) = parse_si_prefix("100nF").unwrap();
        assert!((value - 100e-9).abs() < 1e-20);
        assert_eq!(unit, "F");
    }

    #[test]
    fn test_format_si() {
        assert_eq!(format_si(10000.0, &Unit::Ohm, 0), "10kΩ");
        assert_eq!(format_si(0.0055, &Unit::Volt, 1), "5.5mV");
        assert_eq!(format_si(100e-9, &Unit::Farad, 0), "100nF");
    }
}
