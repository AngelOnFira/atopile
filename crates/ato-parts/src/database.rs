//! Part database trait and types.
//!
//! This module defines the trait for querying part databases, which allows
//! different backends (JLCPCB, Digikey, local files, etc.) to be used
//! interchangeably.

use crate::part::{Part, PartId};
use crate::query::PartQuery;
use thiserror::Error;

/// Errors that can occur when querying a part database.
#[derive(Error, Debug)]
pub enum DatabaseError {
    /// Part was not found.
    #[error("part not found: {0}")]
    NotFound(String),

    /// Multiple parts matched when only one was expected.
    #[error("multiple parts match query: {0}")]
    AmbiguousMatch(String),

    /// Network or I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// API error from a remote service.
    #[error("API error: {0}")]
    Api(String),

    /// Invalid query parameters.
    #[error("invalid query: {0}")]
    InvalidQuery(String),

    /// The database is not available or not configured.
    #[error("database not available: {0}")]
    Unavailable(String),
}

/// Result type for database operations.
pub type DatabaseResult<T> = Result<T, DatabaseError>;

/// Trait for querying part databases.
///
/// This trait allows different part database backends to be used with the
/// same interface. Implementations might connect to:
/// - JLCPCB/LCSC API
/// - Digikey API
/// - Local SQLite database
/// - Cached/offline database
pub trait PartDatabase {
    /// Query parts matching the given criteria.
    ///
    /// Returns a list of parts that match the query, sorted by relevance
    /// or preference (implementation-defined).
    fn query(&self, query: &PartQuery) -> DatabaseResult<Vec<Part>>;

    /// Fetch a specific part by its ID.
    fn fetch_by_id(&self, id: &PartId) -> DatabaseResult<Part>;

    /// Fetch a part by its LCSC number (e.g., "C123456" or just "123456").
    fn fetch_by_lcsc(&self, lcsc: &str) -> DatabaseResult<Part> {
        // Parse the LCSC number
        let number = lcsc
            .strip_prefix('C')
            .or(Some(lcsc))
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| DatabaseError::InvalidQuery(format!("invalid LCSC number: {}", lcsc)))?;

        self.fetch_by_id(&PartId::lcsc(number))
    }

    /// Fetch a part by manufacturer and part number.
    fn fetch_by_mpn(&self, manufacturer: &str, part_number: &str) -> DatabaseResult<Vec<Part>>;

    /// Check if the database is available.
    fn is_available(&self) -> bool;

    /// Get the name/identifier of this database.
    fn name(&self) -> &str;
}

/// A part selection result from the picker.
#[derive(Debug, Clone)]
pub struct PartSelection {
    /// The selected part.
    pub part: Part,
    /// Score indicating how well this part matches (higher is better).
    pub score: f64,
    /// Reason this part was selected.
    pub reason: String,
}

impl PartSelection {
    /// Create a new part selection.
    pub fn new(part: Part, score: f64, reason: impl Into<String>) -> Self {
        Self {
            part,
            score,
            reason: reason.into(),
        }
    }
}

/// Strategy for selecting among multiple matching parts.
#[derive(Debug, Clone, Copy, Default)]
pub enum SelectionStrategy {
    /// Prefer the cheapest part.
    #[default]
    Cheapest,
    /// Prefer parts with more stock.
    MostStock,
    /// Prefer basic/preferred parts (lower handling fees).
    PreferBasic,
    /// Prefer parts from well-known manufacturers.
    PreferKnownManufacturers,
    /// Use a custom scoring function.
    Custom,
}

/// Configuration for part selection.
#[derive(Debug, Clone)]
pub struct SelectionConfig {
    /// Strategy for selecting among matches.
    pub strategy: SelectionStrategy,
    /// Minimum stock level required.
    pub min_stock: u32,
    /// Maximum number of results to return.
    pub max_results: usize,
    /// Whether to allow extended parts.
    pub allow_extended: bool,
}

impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            strategy: SelectionStrategy::default(),
            min_stock: 0,
            max_results: 10,
            allow_extended: true,
        }
    }
}

impl SelectionConfig {
    /// Create a new selection config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the selection strategy.
    pub fn with_strategy(mut self, strategy: SelectionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set the minimum stock.
    pub fn with_min_stock(mut self, stock: u32) -> Self {
        self.min_stock = stock;
        self
    }

    /// Set the maximum results.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    /// Set whether to allow extended parts.
    pub fn with_allow_extended(mut self, allow: bool) -> Self {
        self.allow_extended = allow;
        self
    }
}

/// Trait for part selection/picking logic.
///
/// This trait separates the selection algorithm from the database querying,
/// allowing different strategies to be plugged in.
pub trait PartSelector {
    /// Select parts from candidates based on constraints.
    fn select(
        &self,
        candidates: Vec<Part>,
        config: &SelectionConfig,
    ) -> Vec<PartSelection>;

    /// Score a single part.
    fn score(&self, part: &Part, config: &SelectionConfig) -> f64;
}

/// A simple part selector that uses basic scoring.
#[derive(Debug, Default)]
pub struct BasicPartSelector;

impl BasicPartSelector {
    /// Create a new basic selector.
    pub fn new() -> Self {
        Self
    }
}

impl PartSelector for BasicPartSelector {
    fn select(
        &self,
        candidates: Vec<Part>,
        config: &SelectionConfig,
    ) -> Vec<PartSelection> {
        let mut selections: Vec<PartSelection> = candidates
            .into_iter()
            .filter(|p| {
                // Filter by stock
                let stock_ok = p
                    .availability
                    .as_ref()
                    .map_or(true, |a| a.stock >= config.min_stock);

                // Filter by part class
                let class_ok = config.allow_extended
                    || p.availability
                        .as_ref()
                        .map_or(true, |a| {
                            !matches!(a.part_class, crate::part::PartClass::Extended)
                        });

                stock_ok && class_ok
            })
            .map(|p| {
                let score = self.score(&p, config);
                PartSelection::new(p, score, "basic scoring")
            })
            .collect();

        // Sort by score (descending)
        selections.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Limit results
        selections.truncate(config.max_results);

        selections
    }

    fn score(&self, part: &Part, config: &SelectionConfig) -> f64 {
        let mut score = 100.0;

        // Adjust based on availability
        if let Some(avail) = &part.availability {
            // Prefer in-stock parts
            if avail.in_stock {
                score += 50.0;
            }

            // Adjust based on stock level
            score += (avail.stock as f64).log10().min(20.0);

            // Prefer basic/preferred parts
            match avail.part_class {
                crate::part::PartClass::Basic => score += 30.0,
                crate::part::PartClass::Preferred => score += 20.0,
                crate::part::PartClass::Extended => {}
            }

            // Adjust based on price (if strategy is cheapest)
            if matches!(config.strategy, SelectionStrategy::Cheapest) {
                if let Some(price) = avail.get_price(1) {
                    // Lower price = higher score
                    score += 10.0 / (price + 0.01);
                }
            }
        }

        // Prefer standard packages
        if part.package.is_standard_passive() {
            score += 10.0;
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::{Manufacturer, Package, PartClass, Availability};

    fn create_test_part(id: u32, stock: u32, part_class: PartClass) -> Part {
        let mut part = Part::new(
            PartId::lcsc(id),
            Manufacturer::new("Test", format!("TEST-{}", id)),
            Package::new("0402"),
            "Test part",
        );
        let mut avail = Availability::with_stock(stock);
        avail.part_class = part_class;
        part.availability = Some(avail);
        part
    }

    #[test]
    fn test_basic_selector() {
        let selector = BasicPartSelector::new();
        let config = SelectionConfig::new()
            .with_min_stock(100)
            .with_max_results(5);

        let candidates = vec![
            create_test_part(1, 50, PartClass::Basic),      // Low stock, filtered out
            create_test_part(2, 1000, PartClass::Basic),    // High stock, basic
            create_test_part(3, 500, PartClass::Preferred), // Medium stock, preferred
            create_test_part(4, 2000, PartClass::Extended), // Very high stock, extended
        ];

        let selections = selector.select(candidates, &config);

        // Should filter out low stock part
        assert_eq!(selections.len(), 3);

        // Basic part with high stock should be first
        assert_eq!(selections[0].part.id.supplier_id, "C2");
    }

    #[test]
    fn test_selection_config() {
        let config = SelectionConfig::new()
            .with_strategy(SelectionStrategy::Cheapest)
            .with_min_stock(100)
            .with_max_results(5)
            .with_allow_extended(false);

        assert!(matches!(config.strategy, SelectionStrategy::Cheapest));
        assert_eq!(config.min_stock, 100);
        assert_eq!(config.max_results, 5);
        assert!(!config.allow_extended);
    }
}
