//! Part database interface for the Ato electronics compiler.
//!
//! This crate provides traits and types for querying electronic component
//! databases like JLCPCB/LCSC, Digikey, and others. It enables part selection
//! based on solved parameter constraints from the constraint solver.
//!
//! # Overview
//!
//! The part selection process works as follows:
//!
//! 1. The constraint solver produces parameter bounds (e.g., resistance within 9.5k-10.5k ohm)
//! 2. A [`PartQuery`] is constructed from these constraints
//! 3. A [`PartDatabase`] implementation queries a parts API or local database
//! 4. A [`PartSelector`] scores and ranks the matching parts
//! 5. The best matching part is selected
//!
//! # Main Types
//!
//! - [`Part`] - A component part with parameters, package, and availability info
//! - [`PartId`] - Unique identifier for a part (supplier + part number)
//! - [`PartQuery`] - Query criteria for finding matching parts
//! - [`PartDatabase`] - Trait for querying part databases
//! - [`PartSelector`] - Trait for ranking/selecting parts
//!
//! # Example
//!
//! ```
//! use ato_parts::{PartQuery, ResistorQuery, ParameterConstraint, ComponentType};
//!
//! // Create a query for a 10k resistor in 0402 package
//! let query = ResistorQuery::new()
//!     .resistance_range(9500.0, 10500.0)  // 10k +/- 5%
//!     .package("0402")
//!     .power(0.0625)  // 1/16W minimum
//!     .tolerance(1.0)  // 1% or better
//!     .build();
//!
//! // Or build a query directly
//! let query = PartQuery::resistor()
//!     .with_package("0402")
//!     .with_resistance(ParameterConstraint::between(9500.0, 10500.0))
//!     .with_min_stock(100)
//!     .with_limit(10);
//! ```
//!
//! # Database Implementations
//!
//! This crate defines the traits but does not include any concrete database
//! implementations. Implementations for specific suppliers (JLCPCB, Digikey, etc.)
//! should be provided in separate crates or modules.

mod part;
mod database;
mod query;

pub use part::{
    Availability,
    Manufacturer,
    Package,
    ParameterValue,
    Part,
    PartClass,
    PartId,
    PartParameters,
    PriceTier,
};

pub use database::{
    BasicPartSelector,
    DatabaseError,
    DatabaseResult,
    PartDatabase,
    PartSelection,
    PartSelector,
    SelectionConfig,
    SelectionStrategy,
};

pub use query::{
    CapacitorQuery,
    ComponentType,
    ParameterConstraint,
    PartQuery,
    ResistorQuery,
};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::{
        ComponentType,
        Part,
        PartDatabase,
        PartId,
        PartQuery,
        PartSelector,
        ParameterConstraint,
        ResistorQuery,
        CapacitorQuery,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration() {
        // Create a part
        let part = Part::new(
            PartId::lcsc(25871),
            Manufacturer::new("Yageo", "RC0402FR-0710KL"),
            Package::new("0402"),
            "10kOhm 1% 1/16W 0402 Resistor",
        )
        .with_parameters(
            PartParameters::new()
                .with_param_unit(
                    "resistance",
                    ParameterValue::scalar(10000.0),
                    ato_domain::Unit::Ohm,
                )
                .with_param("tolerance", ParameterValue::scalar(0.01)),
        )
        .with_availability(Availability::with_stock(50000));

        // Create a query that matches
        let query = ResistorQuery::new()
            .resistance_range(9500.0, 10500.0)
            .package("0402")
            .build();

        // Verify the part would match
        assert_eq!(query.component_type, Some(ComponentType::Resistor));
        assert_eq!(query.package, Some("0402".to_string()));

        // Verify part parameters
        let resistance = part.get_param("resistance").unwrap().as_scalar().unwrap();
        let constraint = query.parameters.get("resistance").unwrap();
        assert!(constraint.is_satisfied_by(resistance));
    }

    #[test]
    fn test_selector() {
        let selector = BasicPartSelector::new();
        let config = SelectionConfig::new()
            .with_strategy(SelectionStrategy::Cheapest)
            .with_min_stock(100);

        // Create some test parts
        let parts = vec![
            Part::new(
                PartId::lcsc(1),
                Manufacturer::new("Test", "P1"),
                Package::new("0402"),
                "Part 1",
            )
            .with_availability({
                let mut a = Availability::with_stock(1000);
                a.part_class = PartClass::Basic;
                a
            }),
            Part::new(
                PartId::lcsc(2),
                Manufacturer::new("Test", "P2"),
                Package::new("0402"),
                "Part 2",
            )
            .with_availability({
                let mut a = Availability::with_stock(500);
                a.part_class = PartClass::Extended;
                a
            }),
        ];

        let selections = selector.select(parts, &config);
        assert_eq!(selections.len(), 2);

        // Basic part should be ranked higher
        assert_eq!(selections[0].part.id.supplier_id, "C1");
    }
}
