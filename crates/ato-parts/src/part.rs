//! Part data structures.
//!
//! This module defines the core types for representing electronic components
//! from part databases like JLCPCB/LCSC.

use std::collections::HashMap;

use ato_domain::{QuantityInterval, Unit};
use serde::{Deserialize, Serialize};

/// A unique identifier for a part from a specific supplier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartId {
    /// The supplier this part comes from (e.g., "lcsc", "digikey").
    pub supplier: String,
    /// The supplier's part number (e.g., "C123456" for LCSC).
    pub supplier_id: String,
}

impl PartId {
    /// Create a new part ID.
    pub fn new(supplier: impl Into<String>, supplier_id: impl Into<String>) -> Self {
        Self {
            supplier: supplier.into(),
            supplier_id: supplier_id.into(),
        }
    }

    /// Create an LCSC part ID.
    pub fn lcsc(id: u32) -> Self {
        Self::new("lcsc", format!("C{}", id))
    }
}

impl std::fmt::Display for PartId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.supplier, self.supplier_id)
    }
}

/// Manufacturer information for a part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manufacturer {
    /// The manufacturer name.
    pub name: String,
    /// The manufacturer's part number (MPN).
    pub part_number: String,
}

impl Manufacturer {
    /// Create new manufacturer info.
    pub fn new(name: impl Into<String>, part_number: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            part_number: part_number.into(),
        }
    }
}

/// Physical package/footprint information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Package {
    /// The package name (e.g., "0402", "0603", "SOT-23").
    pub name: String,
    /// Whether this is a surface mount device (SMD).
    pub is_smd: bool,
    /// Number of pins/pads.
    pub pin_count: Option<u32>,
}

impl Package {
    /// Create a new package.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_smd: true,
            pin_count: None,
        }
    }

    /// Set whether this is an SMD package.
    pub fn with_smd(mut self, is_smd: bool) -> Self {
        self.is_smd = is_smd;
        self
    }

    /// Set the pin count.
    pub fn with_pin_count(mut self, count: u32) -> Self {
        self.pin_count = Some(count);
        self
    }

    /// Check if this is a standard passive package (0201, 0402, 0603, etc.).
    pub fn is_standard_passive(&self) -> bool {
        matches!(
            self.name.as_str(),
            "0201" | "0402" | "0603" | "0805" | "1206" | "1210" | "1812" | "2010" | "2512"
        )
    }
}

/// Part classification for pricing purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartClass {
    /// Basic part - no handling fee.
    Basic,
    /// Preferred part - no handling fee.
    Preferred,
    /// Extended part - has handling fee.
    Extended,
}

impl Default for PartClass {
    fn default() -> Self {
        Self::Extended
    }
}

/// Price tier for quantity-based pricing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceTier {
    /// Minimum quantity for this tier (inclusive).
    pub min_qty: u32,
    /// Maximum quantity for this tier (None means unlimited).
    pub max_qty: Option<u32>,
    /// Unit price at this tier.
    pub unit_price: f64,
}

impl PriceTier {
    /// Create a new price tier.
    pub fn new(min_qty: u32, max_qty: Option<u32>, unit_price: f64) -> Self {
        Self {
            min_qty,
            max_qty,
            unit_price,
        }
    }

    /// Check if a quantity falls within this tier.
    pub fn contains(&self, qty: u32) -> bool {
        qty >= self.min_qty && self.max_qty.map_or(true, |max| qty <= max)
    }
}

/// Stock and availability information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Availability {
    /// Current stock level.
    pub stock: u32,
    /// Whether the part is in stock.
    pub in_stock: bool,
    /// Lead time in days (if known).
    pub lead_time_days: Option<u32>,
    /// Price tiers for different quantities.
    pub price_tiers: Vec<PriceTier>,
    /// Part classification.
    pub part_class: PartClass,
}

impl Availability {
    /// Create availability info with just stock level.
    pub fn with_stock(stock: u32) -> Self {
        Self {
            stock,
            in_stock: stock > 0,
            lead_time_days: None,
            price_tiers: Vec::new(),
            part_class: PartClass::default(),
        }
    }

    /// Get the unit price for a given quantity.
    pub fn get_price(&self, qty: u32) -> Option<f64> {
        // Find the applicable tier
        for tier in &self.price_tiers {
            if tier.contains(qty) {
                return Some(tier.unit_price);
            }
        }
        // Fall back to highest tier if qty exceeds all tiers
        self.price_tiers.last().map(|t| t.unit_price)
    }

    /// Get the total cost for a given quantity (including handling fee).
    pub fn get_total_cost(&self, qty: u32) -> Option<f64> {
        let unit_price = self.get_price(qty)?;
        let handling_fee = match self.part_class {
            PartClass::Basic | PartClass::Preferred => 0.0,
            PartClass::Extended => 3.0,
        };
        Some(unit_price * qty as f64 + handling_fee)
    }
}

/// A parameter value that can be matched against constraints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    /// A single numeric value.
    Scalar(f64),
    /// A range of values.
    Range { min: f64, max: f64 },
    /// A string value.
    String(String),
    /// A boolean value.
    Bool(bool),
}

impl ParameterValue {
    /// Create a scalar value.
    pub fn scalar(value: f64) -> Self {
        Self::Scalar(value)
    }

    /// Create a range value.
    pub fn range(min: f64, max: f64) -> Self {
        Self::Range { min, max }
    }

    /// Check if this value satisfies an interval constraint.
    pub fn satisfies_interval(&self, interval: &QuantityInterval) -> bool {
        match self {
            ParameterValue::Scalar(v) => {
                *v >= interval.min().value() && *v <= interval.max().value()
            }
            ParameterValue::Range { min, max } => {
                // The part's range must be fully contained in the constraint
                *min >= interval.min().value() && *max <= interval.max().value()
            }
            _ => false,
        }
    }

    /// Get as scalar if possible.
    pub fn as_scalar(&self) -> Option<f64> {
        match self {
            ParameterValue::Scalar(v) => Some(*v),
            _ => None,
        }
    }
}

/// Electrical parameters for a part.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartParameters {
    /// The parameters as a map from name to value.
    params: HashMap<String, ParameterValue>,
    /// The unit for each parameter (if known).
    units: HashMap<String, Unit>,
}

impl PartParameters {
    /// Create an empty parameter set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a parameter.
    pub fn with_param(mut self, name: impl Into<String>, value: ParameterValue) -> Self {
        self.params.insert(name.into(), value);
        self
    }

    /// Add a parameter with a unit.
    pub fn with_param_unit(
        mut self,
        name: impl Into<String>,
        value: ParameterValue,
        unit: Unit,
    ) -> Self {
        let name = name.into();
        self.params.insert(name.clone(), value);
        self.units.insert(name, unit);
        self
    }

    /// Get a parameter by name.
    pub fn get(&self, name: &str) -> Option<&ParameterValue> {
        self.params.get(name)
    }

    /// Get the unit for a parameter.
    pub fn get_unit(&self, name: &str) -> Option<&Unit> {
        self.units.get(name)
    }

    /// Iterate over all parameters.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ParameterValue)> {
        self.params.iter()
    }

    /// Check if a parameter exists.
    pub fn contains(&self, name: &str) -> bool {
        self.params.contains_key(name)
    }

    /// Get the number of parameters.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Check if there are no parameters.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }
}

/// A component part from a part database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    /// Unique identifier for this part.
    pub id: PartId,
    /// Manufacturer information.
    pub manufacturer: Manufacturer,
    /// Package/footprint information.
    pub package: Package,
    /// Part description.
    pub description: String,
    /// Electrical parameters.
    pub parameters: PartParameters,
    /// Availability information (optional).
    pub availability: Option<Availability>,
    /// URL to datasheet (optional).
    pub datasheet_url: Option<String>,
}

impl Part {
    /// Create a new part with minimal information.
    pub fn new(
        id: PartId,
        manufacturer: Manufacturer,
        package: Package,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id,
            manufacturer,
            package,
            description: description.into(),
            parameters: PartParameters::new(),
            availability: None,
            datasheet_url: None,
        }
    }

    /// Set the parameters.
    pub fn with_parameters(mut self, parameters: PartParameters) -> Self {
        self.parameters = parameters;
        self
    }

    /// Set the availability.
    pub fn with_availability(mut self, availability: Availability) -> Self {
        self.availability = Some(availability);
        self
    }

    /// Set the datasheet URL.
    pub fn with_datasheet(mut self, url: impl Into<String>) -> Self {
        self.datasheet_url = Some(url.into());
        self
    }

    /// Get a parameter value.
    pub fn get_param(&self, name: &str) -> Option<&ParameterValue> {
        self.parameters.get(name)
    }

    /// Check if this part is in stock.
    pub fn is_in_stock(&self) -> bool {
        self.availability.as_ref().map_or(false, |a| a.in_stock)
    }

    /// Get the LCSC ID if this is an LCSC part.
    pub fn lcsc_id(&self) -> Option<&str> {
        if self.id.supplier == "lcsc" {
            Some(&self.id.supplier_id)
        } else {
            None
        }
    }
}

impl std::fmt::Display for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} ({}) [{}]",
            self.manufacturer.name, self.manufacturer.part_number, self.package.name, self.id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_part_id() {
        let id = PartId::lcsc(123456);
        assert_eq!(id.supplier, "lcsc");
        assert_eq!(id.supplier_id, "C123456");
        assert_eq!(id.to_string(), "lcsc:C123456");
    }

    #[test]
    fn test_manufacturer() {
        let mfr = Manufacturer::new("Yageo", "RC0402FR-0710KL");
        assert_eq!(mfr.name, "Yageo");
        assert_eq!(mfr.part_number, "RC0402FR-0710KL");
    }

    #[test]
    fn test_package() {
        let pkg = Package::new("0402").with_smd(true).with_pin_count(2);
        assert_eq!(pkg.name, "0402");
        assert!(pkg.is_smd);
        assert_eq!(pkg.pin_count, Some(2));
        assert!(pkg.is_standard_passive());
    }

    #[test]
    fn test_price_tier() {
        let tier = PriceTier::new(1, Some(100), 0.01);
        assert!(tier.contains(1));
        assert!(tier.contains(50));
        assert!(tier.contains(100));
        assert!(!tier.contains(101));
    }

    #[test]
    fn test_availability() {
        let mut avail = Availability::with_stock(1000);
        avail.price_tiers = vec![
            PriceTier::new(1, Some(9), 0.05),
            PriceTier::new(10, Some(99), 0.03),
            PriceTier::new(100, None, 0.01),
        ];
        avail.part_class = PartClass::Basic;

        assert_eq!(avail.get_price(5), Some(0.05));
        assert_eq!(avail.get_price(50), Some(0.03));
        assert_eq!(avail.get_price(500), Some(0.01));

        // Total cost with no handling fee
        assert_eq!(avail.get_total_cost(10), Some(0.30));
    }

    #[test]
    fn test_parameter_value() {
        let scalar = ParameterValue::scalar(10000.0);
        assert_eq!(scalar.as_scalar(), Some(10000.0));

        let range = ParameterValue::range(9500.0, 10500.0);
        assert!(matches!(range, ParameterValue::Range { .. }));
    }

    #[test]
    fn test_part_parameters() {
        let params = PartParameters::new()
            .with_param_unit("resistance", ParameterValue::scalar(10000.0), Unit::Ohm)
            .with_param_unit("power", ParameterValue::scalar(0.0625), Unit::Watt)
            .with_param("tolerance", ParameterValue::scalar(0.01));

        assert_eq!(params.len(), 3);
        assert!(params.contains("resistance"));
        assert_eq!(params.get_unit("resistance"), Some(&Unit::Ohm));
    }

    #[test]
    fn test_part() {
        let part = Part::new(
            PartId::lcsc(25871),
            Manufacturer::new("Yageo", "RC0402FR-0710KL"),
            Package::new("0402"),
            "10kOhm 1% 1/16W 0402 Resistor",
        )
        .with_parameters(
            PartParameters::new()
                .with_param_unit("resistance", ParameterValue::scalar(10000.0), Unit::Ohm),
        )
        .with_availability(Availability::with_stock(50000));

        assert_eq!(part.lcsc_id(), Some("C25871"));
        assert!(part.is_in_stock());
        assert!(part.description.contains("10kOhm"));
    }
}
