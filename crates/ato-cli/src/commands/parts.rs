//! Parts database commands.
//!
//! This module provides CLI commands for searching and querying the
//! LCSC component database.

use crate::error::{CliError, CliResult};
use ato_parts::{
    BasicPartSelector, ComponentType, LcscClient, ParameterConstraint, PartCache, PartDatabase,
    PartQuery, PartSelector, SelectionConfig, CachedDatabase,
};

/// Run the parts search command.
pub fn search(query: &str, component_type: Option<&str>, package: Option<&str>, limit: usize) -> CliResult<()> {
    println!("Searching for parts: {}", query);

    // Parse component type
    let comp_type = component_type.map(|t| match t.to_lowercase().as_str() {
        "resistor" | "r" => ComponentType::Resistor,
        "capacitor" | "c" => ComponentType::Capacitor,
        "inductor" | "l" => ComponentType::Inductor,
        "diode" | "d" => ComponentType::Diode,
        "led" => ComponentType::Led,
        "transistor" | "q" => ComponentType::Transistor,
        "mosfet" => ComponentType::Mosfet,
        _ => ComponentType::Other,
    });

    // Try to parse the query for common patterns
    let mut part_query = if let Some(ct) = comp_type {
        PartQuery::for_type(ct)
    } else {
        PartQuery::new()
    };

    // Set package if provided
    if let Some(pkg) = package {
        part_query = part_query.with_package(pkg);
    }

    // Parse query string for common patterns
    let parsed = parse_query_string(query);
    if let Some(ct) = parsed.component_type {
        part_query.component_type = Some(ct);
    }
    if let Some(pkg) = parsed.package {
        part_query.package = Some(pkg);
    }
    if let Some(resistance) = parsed.resistance {
        part_query = part_query.with_resistance(resistance);
    }
    if let Some(capacitance) = parsed.capacitance {
        part_query = part_query.with_capacitance(capacitance);
    }

    part_query = part_query.with_limit(limit);

    // Create LCSC client with cache
    let cache = match PartCache::default_cache() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: Could not initialize cache: {}", e);
            PartCache::in_memory().map_err(|e| CliError::io(format!("cache error: {}", e)))?
        }
    };

    let client = LcscClient::new()
        .map_err(|e| CliError::io(format!("failed to create LCSC client: {}", e)))?;

    let db = CachedDatabase::new(client, cache);

    // Search for parts
    println!("Querying LCSC database...");
    let parts = db
        .query(&part_query)
        .map_err(|e| CliError::io(format!("search failed: {}", e)))?;

    if parts.is_empty() {
        println!("No parts found matching the query.");
        return Ok(());
    }

    // Score and rank parts
    let selector = BasicPartSelector::new();
    let config = SelectionConfig::new()
        .with_max_results(limit)
        .with_min_stock(1);

    let selections = selector.select(parts, &config);

    println!("\nFound {} matching parts:\n", selections.len());

    for (i, selection) in selections.iter().enumerate() {
        let part = &selection.part;
        let stock = part
            .availability
            .as_ref()
            .map_or(0, |a| a.stock);
        let class = part
            .availability
            .as_ref()
            .map_or("Unknown", |a| match a.part_class {
                ato_parts::PartClass::Basic => "Basic",
                ato_parts::PartClass::Preferred => "Preferred",
                ato_parts::PartClass::Extended => "Extended",
            });

        println!(
            "{}. {} [{}]",
            i + 1,
            part.id.supplier_id,
            class
        );
        println!(
            "   {} {}",
            part.manufacturer.name, part.manufacturer.part_number
        );
        println!("   {}", part.description);
        println!("   Package: {} | Stock: {}", part.package.name, stock);

        // Show key parameters
        if !part.parameters.is_empty() {
            let params: Vec<String> = part
                .parameters
                .iter()
                .take(3)
                .map(|(name, value)| format!("{}: {:?}", name, value))
                .collect();
            if !params.is_empty() {
                println!("   Parameters: {}", params.join(", "));
            }
        }

        // Show price if available
        if let Some(avail) = &part.availability {
            if let Some(price) = avail.get_price(1) {
                println!("   Price: ${:.4}/ea", price);
            }
        }

        println!();
    }

    Ok(())
}

/// Run the parts fetch command (fetch by LCSC ID).
pub fn fetch(lcsc_id: &str) -> CliResult<()> {
    println!("Fetching part: {}", lcsc_id);

    // Create LCSC client with cache
    let cache = match PartCache::default_cache() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: Could not initialize cache: {}", e);
            PartCache::in_memory().map_err(|e| CliError::io(format!("cache error: {}", e)))?
        }
    };

    let client = LcscClient::new()
        .map_err(|e| CliError::io(format!("failed to create LCSC client: {}", e)))?;

    let db = CachedDatabase::new(client, cache);

    // Fetch the part
    let part = db
        .fetch_by_lcsc(lcsc_id)
        .map_err(|e| CliError::io(format!("fetch failed: {}", e)))?;

    println!("\nPart Details:\n");
    println!("LCSC: {}", part.id.supplier_id);
    println!("Manufacturer: {}", part.manufacturer.name);
    println!("MPN: {}", part.manufacturer.part_number);
    println!("Description: {}", part.description);
    println!("Package: {}", part.package.name);

    if let Some(avail) = &part.availability {
        println!("\nAvailability:");
        println!("  Stock: {}", avail.stock);
        println!("  Class: {:?}", avail.part_class);
        if !avail.price_tiers.is_empty() {
            println!("  Price Tiers:");
            for tier in &avail.price_tiers {
                let max = tier.max_qty.map_or("∞".to_string(), |m| m.to_string());
                println!("    {}-{}: ${:.4}/ea", tier.min_qty, max, tier.unit_price);
            }
        }
    }

    if !part.parameters.is_empty() {
        println!("\nParameters:");
        for (name, value) in part.parameters.iter() {
            let unit = part.parameters.get_unit(name);
            if let Some(u) = unit {
                println!("  {}: {:?} {:?}", name, value, u);
            } else {
                println!("  {}: {:?}", name, value);
            }
        }
    }

    if let Some(url) = &part.datasheet_url {
        println!("\nDatasheet: {}", url);
    }

    Ok(())
}

/// Run the cache info command.
pub fn cache_info() -> CliResult<()> {
    let cache = PartCache::default_cache()
        .map_err(|e| CliError::io(format!("failed to open cache: {}", e)))?;

    let count = cache
        .count()
        .map_err(|e| CliError::io(format!("failed to count cache: {}", e)))?;

    println!("Part Cache Info:");
    println!("  Cached parts: {}", count);

    Ok(())
}

/// Run the cache clear command.
pub fn cache_clear() -> CliResult<()> {
    let cache = PartCache::default_cache()
        .map_err(|e| CliError::io(format!("failed to open cache: {}", e)))?;

    cache
        .clear_all()
        .map_err(|e| CliError::io(format!("failed to clear cache: {}", e)))?;

    println!("Cache cleared.");

    Ok(())
}

// ===== Query Parsing =====

struct ParsedQuery {
    component_type: Option<ComponentType>,
    package: Option<String>,
    resistance: Option<ParameterConstraint>,
    capacitance: Option<ParameterConstraint>,
}

fn parse_query_string(query: &str) -> ParsedQuery {
    let mut result = ParsedQuery {
        component_type: None,
        package: None,
        resistance: None,
        capacitance: None,
    };

    let lower = query.to_lowercase();

    // Detect component type
    if lower.contains("resistor") {
        result.component_type = Some(ComponentType::Resistor);
    } else if lower.contains("capacitor") || lower.contains("cap") {
        result.component_type = Some(ComponentType::Capacitor);
    } else if lower.contains("inductor") {
        result.component_type = Some(ComponentType::Inductor);
    }

    // Detect package
    for pkg in ["0201", "0402", "0603", "0805", "1206", "1210", "sot-23", "soic-8"] {
        if lower.contains(pkg) {
            result.package = Some(pkg.to_string());
            break;
        }
    }

    // Parse resistance values (e.g., "10k", "10kohm", "1M", "100ohm")
    if let Some(ohms) = parse_resistance(&lower) {
        result.component_type = Some(ComponentType::Resistor);
        // Allow +/- 10% tolerance
        let min = ohms * 0.9;
        let max = ohms * 1.1;
        result.resistance = Some(ParameterConstraint::between(min, max));
    }

    // Parse capacitance values (e.g., "100nF", "10uF", "100pF")
    if let Some(farads) = parse_capacitance(&lower) {
        result.component_type = Some(ComponentType::Capacitor);
        // Allow +/- 20% tolerance
        let min = farads * 0.8;
        let max = farads * 1.2;
        result.capacitance = Some(ParameterConstraint::between(min, max));
    }

    result
}

fn parse_resistance(s: &str) -> Option<f64> {
    // Pattern: number followed by optional SI prefix and "ohm" or just the prefix
    // Examples: "10k", "10kohm", "1M", "100ohm", "4.7k"
    // Order matters: check longer suffixes first
    let patterns = [
        ("megohm", 1e6),
        ("kohm", 1e3),
        ("mohm", 1e-3),
        ("ohm", 1.0),
        ("meg", 1e6),
        ("k", 1e3),
        ("m", 1e6), // M for mega (uppercase in original, lowercase in normalized)
    ];

    for (suffix, multiplier) in patterns {
        if let Some(idx) = s.find(suffix) {
            // Make sure it's at the end or followed by whitespace
            let after_idx = idx + suffix.len();
            if after_idx < s.len() {
                let next_char = s.chars().nth(after_idx);
                if let Some(c) = next_char {
                    if c.is_alphabetic() {
                        continue; // Part of a longer suffix
                    }
                }
            }

            // Extract the number before the suffix
            let before = &s[..idx];
            let number_str: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<String>()
                .chars()
                .rev()
                .collect();

            if let Ok(value) = number_str.parse::<f64>() {
                return Some(value * multiplier);
            }
        }
    }

    None
}

fn parse_capacitance(s: &str) -> Option<f64> {
    // Look for patterns like "100nF", "10uF", "100pF"
    let patterns = [
        ("pf", 1e-12),
        ("nf", 1e-9),
        ("uf", 1e-6),
        ("µf", 1e-6),
        ("mf", 1e-3),
    ];

    for (suffix, multiplier) in patterns {
        if let Some(idx) = s.find(suffix) {
            // Extract the number before the suffix
            let before = &s[..idx];
            let number_str: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<String>()
                .chars()
                .rev()
                .collect();

            if let Ok(value) = number_str.parse::<f64>() {
                return Some(value * multiplier);
            }
        }
    }

    None
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_resistance() {
        assert!((parse_resistance("10kohm").unwrap() - 10000.0).abs() < 1.0);
        assert!((parse_resistance("10k").unwrap() - 10000.0).abs() < 1.0);
        assert!((parse_resistance("100ohm").unwrap() - 100.0).abs() < 0.1);
        assert!((parse_resistance("4.7k").unwrap() - 4700.0).abs() < 1.0);
    }

    #[test]
    fn test_parse_capacitance() {
        assert!((parse_capacitance("100nf").unwrap() - 100e-9).abs() < 1e-12);
        assert!((parse_capacitance("10uf").unwrap() - 10e-6).abs() < 1e-9);
        assert!((parse_capacitance("100pf").unwrap() - 100e-12).abs() < 1e-15);
    }

    #[test]
    fn test_parse_query_string() {
        let parsed = parse_query_string("10k 0402 resistor");
        assert_eq!(parsed.component_type, Some(ComponentType::Resistor));
        assert_eq!(parsed.package, Some("0402".to_string()));
        assert!(parsed.resistance.is_some());
    }
}
