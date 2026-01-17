//! Query builder for part database searches.
//!
//! This module provides a fluent API for building part queries that can
//! filter by component type, parameters, package, and other criteria.

use ato_domain::QuantityInterval;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The type of component being searched for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentType {
    /// Resistor.
    Resistor,
    /// Capacitor.
    Capacitor,
    /// Inductor.
    Inductor,
    /// Diode (including LEDs).
    Diode,
    /// LED.
    Led,
    /// Transistor (BJT).
    Transistor,
    /// MOSFET.
    Mosfet,
    /// Integrated circuit.
    IntegratedCircuit,
    /// Connector.
    Connector,
    /// Crystal/oscillator.
    Crystal,
    /// Fuse.
    Fuse,
    /// Other/unknown.
    Other,
}

impl ComponentType {
    /// Get the API endpoint name for this component type.
    pub fn endpoint_name(&self) -> &'static str {
        match self {
            ComponentType::Resistor => "resistors",
            ComponentType::Capacitor => "capacitors",
            ComponentType::Inductor => "inductors",
            ComponentType::Diode => "diodes",
            ComponentType::Led => "leds",
            ComponentType::Transistor => "transistors",
            ComponentType::Mosfet => "mosfets",
            ComponentType::IntegratedCircuit => "ics",
            ComponentType::Connector => "connectors",
            ComponentType::Crystal => "crystals",
            ComponentType::Fuse => "fuses",
            ComponentType::Other => "other",
        }
    }
}

/// A constraint on a parameter value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterConstraint {
    /// Exact value (with small tolerance for floating point).
    Equals(f64),
    /// Minimum value (inclusive).
    GreaterOrEqual(f64),
    /// Maximum value (inclusive).
    LessOrEqual(f64),
    /// Within a range (inclusive on both ends).
    Range { min: f64, max: f64 },
    /// Within an interval with units.
    Interval(QuantityInterval),
}

impl ParameterConstraint {
    /// Create an exact match constraint.
    pub fn equals(value: f64) -> Self {
        Self::Equals(value)
    }

    /// Create a minimum constraint.
    pub fn at_least(value: f64) -> Self {
        Self::GreaterOrEqual(value)
    }

    /// Create a maximum constraint.
    pub fn at_most(value: f64) -> Self {
        Self::LessOrEqual(value)
    }

    /// Create a range constraint.
    pub fn between(min: f64, max: f64) -> Self {
        Self::Range { min, max }
    }

    /// Create a constraint from a quantity interval.
    pub fn interval(interval: QuantityInterval) -> Self {
        Self::Interval(interval)
    }

    /// Check if a value satisfies this constraint.
    pub fn is_satisfied_by(&self, value: f64) -> bool {
        match self {
            Self::Equals(v) => (value - v).abs() < v.abs() * 1e-9 + 1e-15,
            Self::GreaterOrEqual(min) => value >= *min,
            Self::LessOrEqual(max) => value <= *max,
            Self::Range { min, max } => value >= *min && value <= *max,
            Self::Interval(interval) => {
                value >= interval.min().value() && value <= interval.max().value()
            }
        }
    }

    /// Get the minimum value of this constraint (if defined).
    pub fn min_value(&self) -> Option<f64> {
        match self {
            Self::Equals(v) => Some(*v),
            Self::GreaterOrEqual(min) => Some(*min),
            Self::LessOrEqual(_) => None,
            Self::Range { min, .. } => Some(*min),
            Self::Interval(interval) => Some(interval.min().value()),
        }
    }

    /// Get the maximum value of this constraint (if defined).
    pub fn max_value(&self) -> Option<f64> {
        match self {
            Self::Equals(v) => Some(*v),
            Self::GreaterOrEqual(_) => None,
            Self::LessOrEqual(max) => Some(*max),
            Self::Range { max, .. } => Some(*max),
            Self::Interval(interval) => Some(interval.max().value()),
        }
    }
}

/// A query for finding parts in a database.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartQuery {
    /// The component type to search for.
    pub component_type: Option<ComponentType>,
    /// Package filter (e.g., "0402", "0603").
    pub package: Option<String>,
    /// Parameter constraints.
    pub parameters: HashMap<String, ParameterConstraint>,
    /// Minimum stock required.
    pub min_stock: Option<u32>,
    /// Only include basic/preferred parts.
    pub basic_only: bool,
    /// Maximum number of results.
    pub limit: Option<usize>,
}

impl PartQuery {
    /// Create a new empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a query for a specific component type.
    pub fn for_type(component_type: ComponentType) -> Self {
        Self {
            component_type: Some(component_type),
            ..Default::default()
        }
    }

    /// Create a query for resistors.
    pub fn resistor() -> Self {
        Self::for_type(ComponentType::Resistor)
    }

    /// Create a query for capacitors.
    pub fn capacitor() -> Self {
        Self::for_type(ComponentType::Capacitor)
    }

    /// Create a query for inductors.
    pub fn inductor() -> Self {
        Self::for_type(ComponentType::Inductor)
    }

    /// Set the package filter.
    pub fn with_package(mut self, package: impl Into<String>) -> Self {
        self.package = Some(package.into());
        self
    }

    /// Add a parameter constraint.
    pub fn with_param(mut self, name: impl Into<String>, constraint: ParameterConstraint) -> Self {
        self.parameters.insert(name.into(), constraint);
        self
    }

    /// Add a resistance constraint (for resistors).
    pub fn with_resistance(self, constraint: ParameterConstraint) -> Self {
        self.with_param("resistance", constraint)
    }

    /// Add a capacitance constraint (for capacitors).
    pub fn with_capacitance(self, constraint: ParameterConstraint) -> Self {
        self.with_param("capacitance", constraint)
    }

    /// Add an inductance constraint (for inductors).
    pub fn with_inductance(self, constraint: ParameterConstraint) -> Self {
        self.with_param("inductance", constraint)
    }

    /// Add a voltage rating constraint.
    pub fn with_voltage(self, constraint: ParameterConstraint) -> Self {
        self.with_param("voltage", constraint)
    }

    /// Add a power rating constraint.
    pub fn with_power(self, constraint: ParameterConstraint) -> Self {
        self.with_param("power", constraint)
    }

    /// Add a tolerance constraint.
    pub fn with_tolerance(self, constraint: ParameterConstraint) -> Self {
        self.with_param("tolerance", constraint)
    }

    /// Set minimum stock.
    pub fn with_min_stock(mut self, stock: u32) -> Self {
        self.min_stock = Some(stock);
        self
    }

    /// Only include basic/preferred parts.
    pub fn basic_only(mut self) -> Self {
        self.basic_only = true;
        self
    }

    /// Set result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Builder for creating resistor queries with common parameters.
#[derive(Debug, Clone, Default)]
pub struct ResistorQuery {
    query: PartQuery,
}

impl ResistorQuery {
    /// Create a new resistor query.
    pub fn new() -> Self {
        Self {
            query: PartQuery::resistor(),
        }
    }

    /// Set the resistance value (exact match with tolerance).
    pub fn resistance(mut self, ohms: f64) -> Self {
        self.query = self.query.with_resistance(ParameterConstraint::equals(ohms));
        self
    }

    /// Set a resistance range.
    pub fn resistance_range(mut self, min_ohms: f64, max_ohms: f64) -> Self {
        self.query = self
            .query
            .with_resistance(ParameterConstraint::between(min_ohms, max_ohms));
        self
    }

    /// Set the package.
    pub fn package(mut self, package: impl Into<String>) -> Self {
        self.query = self.query.with_package(package);
        self
    }

    /// Set the power rating (minimum).
    pub fn power(mut self, watts: f64) -> Self {
        self.query = self.query.with_power(ParameterConstraint::at_least(watts));
        self
    }

    /// Set the tolerance (maximum).
    pub fn tolerance(mut self, percent: f64) -> Self {
        self.query = self
            .query
            .with_tolerance(ParameterConstraint::at_most(percent / 100.0));
        self
    }

    /// Build the query.
    pub fn build(self) -> PartQuery {
        self.query
    }
}

/// Builder for creating capacitor queries with common parameters.
#[derive(Debug, Clone, Default)]
pub struct CapacitorQuery {
    query: PartQuery,
}

impl CapacitorQuery {
    /// Create a new capacitor query.
    pub fn new() -> Self {
        Self {
            query: PartQuery::capacitor(),
        }
    }

    /// Set the capacitance value (exact match with tolerance).
    pub fn capacitance(mut self, farads: f64) -> Self {
        self.query = self
            .query
            .with_capacitance(ParameterConstraint::equals(farads));
        self
    }

    /// Set a capacitance range.
    pub fn capacitance_range(mut self, min_farads: f64, max_farads: f64) -> Self {
        self.query = self
            .query
            .with_capacitance(ParameterConstraint::between(min_farads, max_farads));
        self
    }

    /// Set the package.
    pub fn package(mut self, package: impl Into<String>) -> Self {
        self.query = self.query.with_package(package);
        self
    }

    /// Set the voltage rating (minimum).
    pub fn voltage(mut self, volts: f64) -> Self {
        self.query = self.query.with_voltage(ParameterConstraint::at_least(volts));
        self
    }

    /// Build the query.
    pub fn build(self) -> PartQuery {
        self.query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_constraint() {
        let eq = ParameterConstraint::equals(10000.0);
        assert!(eq.is_satisfied_by(10000.0));
        assert!(!eq.is_satisfied_by(10001.0));

        let range = ParameterConstraint::between(9500.0, 10500.0);
        assert!(range.is_satisfied_by(10000.0));
        assert!(range.is_satisfied_by(9500.0));
        assert!(range.is_satisfied_by(10500.0));
        assert!(!range.is_satisfied_by(9499.0));
        assert!(!range.is_satisfied_by(10501.0));

        let at_least = ParameterConstraint::at_least(5.0);
        assert!(at_least.is_satisfied_by(5.0));
        assert!(at_least.is_satisfied_by(100.0));
        assert!(!at_least.is_satisfied_by(4.9));
    }

    #[test]
    fn test_part_query() {
        let query = PartQuery::resistor()
            .with_package("0402")
            .with_resistance(ParameterConstraint::between(9500.0, 10500.0))
            .with_power(ParameterConstraint::at_least(0.0625))
            .with_min_stock(100)
            .with_limit(10);

        assert_eq!(query.component_type, Some(ComponentType::Resistor));
        assert_eq!(query.package, Some("0402".to_string()));
        assert!(query.parameters.contains_key("resistance"));
        assert!(query.parameters.contains_key("power"));
        assert_eq!(query.min_stock, Some(100));
        assert_eq!(query.limit, Some(10));
    }

    #[test]
    fn test_resistor_query_builder() {
        let query = ResistorQuery::new()
            .resistance_range(9500.0, 10500.0)
            .package("0402")
            .power(0.0625)
            .tolerance(1.0)
            .build();

        assert_eq!(query.component_type, Some(ComponentType::Resistor));
        assert_eq!(query.package, Some("0402".to_string()));
        assert!(query.parameters.contains_key("resistance"));
        assert!(query.parameters.contains_key("power"));
        assert!(query.parameters.contains_key("tolerance"));
    }

    #[test]
    fn test_capacitor_query_builder() {
        let query = CapacitorQuery::new()
            .capacitance(100e-9) // 100nF
            .package("0603")
            .voltage(16.0)
            .build();

        assert_eq!(query.component_type, Some(ComponentType::Capacitor));
        assert_eq!(query.package, Some("0603".to_string()));
        assert!(query.parameters.contains_key("capacitance"));
        assert!(query.parameters.contains_key("voltage"));
    }

    #[test]
    fn test_component_type() {
        assert_eq!(ComponentType::Resistor.endpoint_name(), "resistors");
        assert_eq!(ComponentType::Capacitor.endpoint_name(), "capacitors");
        assert_eq!(ComponentType::Mosfet.endpoint_name(), "mosfets");
    }
}
