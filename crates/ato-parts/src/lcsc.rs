//! LCSC/JLCPCB part database client.
//!
//! This module provides an HTTP client for querying the LCSC component database
//! through the JLCPCB API.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::database::{DatabaseError, DatabaseResult, PartDatabase};
use crate::part::{
    Availability, Manufacturer, Package, ParameterValue, Part, PartClass, PartId, PartParameters,
    PriceTier,
};
use crate::query::{ComponentType, ParameterConstraint, PartQuery};

/// LCSC API base URL for component search.
const LCSC_API_BASE: &str = "https://wmsc.lcsc.com/wmsc";

/// Rate limit: minimum time between requests.
const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(200);

/// LCSC database client.
#[derive(Debug)]
pub struct LcscClient {
    client: Client,
    last_request: Mutex<Option<Instant>>,
}

impl LcscClient {
    /// Create a new LCSC client.
    pub fn new() -> DatabaseResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ato-parts/0.1.0")
            .build()
            .map_err(|e| DatabaseError::Api(format!("failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            last_request: Mutex::new(None),
        })
    }

    /// Apply rate limiting before making a request.
    fn rate_limit(&self) {
        let mut last = self.last_request.lock().unwrap();
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < MIN_REQUEST_INTERVAL {
                std::thread::sleep(MIN_REQUEST_INTERVAL - elapsed);
            }
        }
        *last = Some(Instant::now());
    }

    /// Search for parts using a text query.
    pub fn search(&self, query: &str, limit: usize) -> DatabaseResult<Vec<Part>> {
        self.rate_limit();

        let url = format!("{}/product/search", LCSC_API_BASE);

        let request_body = LcscSearchRequest {
            current_page: 1,
            page_size: limit.min(100) as u32,
            keyword: query.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .map_err(|e| DatabaseError::Api(format!("search request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(DatabaseError::Api(format!(
                "search failed with status: {}",
                response.status()
            )));
        }

        let result: LcscSearchResponse = response
            .json()
            .map_err(|e| DatabaseError::Api(format!("failed to parse response: {}", e)))?;

        if result.code != 200 {
            return Err(DatabaseError::Api(format!(
                "API error: {} - {}",
                result.code,
                result.msg.unwrap_or_default()
            )));
        }

        let parts = result
            .result
            .map(|r| r.product_list.into_iter().map(|p| p.into_part()).collect())
            .unwrap_or_default();

        Ok(parts)
    }

    /// Fetch a part by its LCSC number.
    pub fn fetch_part(&self, lcsc_id: &str) -> DatabaseResult<Part> {
        self.rate_limit();

        // Normalize the LCSC ID
        let lcsc_num = lcsc_id.strip_prefix('C').unwrap_or(lcsc_id);

        let url = format!("{}/product/detail", LCSC_API_BASE);

        let request_body = serde_json::json!({
            "productCode": format!("C{}", lcsc_num)
        });

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .map_err(|e| DatabaseError::Api(format!("fetch request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(DatabaseError::Api(format!(
                "fetch failed with status: {}",
                response.status()
            )));
        }

        let result: LcscDetailResponse = response
            .json()
            .map_err(|e| DatabaseError::Api(format!("failed to parse response: {}", e)))?;

        if result.code != 200 {
            return Err(DatabaseError::NotFound(format!(
                "part {} not found",
                lcsc_id
            )));
        }

        result
            .result
            .map(|p| p.into_part())
            .ok_or_else(|| DatabaseError::NotFound(format!("part {} not found", lcsc_id)))
    }

    /// Search for parts matching a query.
    pub fn query_parts(&self, query: &PartQuery) -> DatabaseResult<Vec<Part>> {
        // Build search string from query
        let search_string = build_search_string(query);
        let limit = query.limit.unwrap_or(20);

        // Search and filter results
        let parts = self.search(&search_string, limit * 2)?;

        // Filter by query constraints
        let filtered: Vec<Part> = parts
            .into_iter()
            .filter(|p| matches_query(p, query))
            .take(limit)
            .collect();

        Ok(filtered)
    }
}

impl Default for LcscClient {
    fn default() -> Self {
        Self::new().expect("failed to create LCSC client")
    }
}

impl PartDatabase for LcscClient {
    fn query(&self, query: &PartQuery) -> DatabaseResult<Vec<Part>> {
        self.query_parts(query)
    }

    fn fetch_by_id(&self, id: &PartId) -> DatabaseResult<Part> {
        if id.supplier != "lcsc" {
            return Err(DatabaseError::InvalidQuery(format!(
                "LCSC client cannot fetch {} parts",
                id.supplier
            )));
        }
        self.fetch_part(&id.supplier_id)
    }

    fn fetch_by_mpn(&self, manufacturer: &str, part_number: &str) -> DatabaseResult<Vec<Part>> {
        let search = format!("{} {}", manufacturer, part_number);
        self.search(&search, 10)
    }

    fn is_available(&self) -> bool {
        self.client.get(LCSC_API_BASE).send().is_ok()
    }

    fn name(&self) -> &str {
        "LCSC"
    }
}

/// Build a search string from a PartQuery.
fn build_search_string(query: &PartQuery) -> String {
    let mut parts = Vec::new();

    // Add component type
    if let Some(ct) = &query.component_type {
        parts.push(match ct {
            ComponentType::Resistor => "resistor".to_string(),
            ComponentType::Capacitor => "capacitor".to_string(),
            ComponentType::Inductor => "inductor".to_string(),
            ComponentType::Diode => "diode".to_string(),
            ComponentType::Led => "LED".to_string(),
            ComponentType::Transistor => "transistor".to_string(),
            ComponentType::Mosfet => "MOSFET".to_string(),
            _ => "".to_string(),
        });
    }

    // Add package
    if let Some(pkg) = &query.package {
        parts.push(pkg.clone());
    }

    // Add key parameter values
    for (name, constraint) in &query.parameters {
        if let Some(value) = format_parameter_for_search(name, constraint) {
            parts.push(value);
        }
    }

    parts.join(" ")
}

/// Format a parameter constraint for search.
fn format_parameter_for_search(name: &str, constraint: &ParameterConstraint) -> Option<String> {
    let value = match constraint {
        ParameterConstraint::Equals(v) => *v,
        ParameterConstraint::Range { min, max } => (*min + *max) / 2.0,
        ParameterConstraint::Interval(i) => (i.min().value() + i.max().value()) / 2.0,
        _ => return None,
    };

    match name {
        "resistance" => Some(format_resistance(value)),
        "capacitance" => Some(format_capacitance(value)),
        "inductance" => Some(format_inductance(value)),
        _ => None,
    }
}

/// Format resistance value for search (e.g., "10kohm").
fn format_resistance(ohms: f64) -> String {
    if ohms >= 1_000_000.0 {
        format!("{:.1}Mohm", ohms / 1_000_000.0)
    } else if ohms >= 1_000.0 {
        format!("{:.1}kohm", ohms / 1_000.0)
    } else {
        format!("{:.1}ohm", ohms)
    }
}

/// Format capacitance value for search (e.g., "100nF").
fn format_capacitance(farads: f64) -> String {
    if farads >= 1e-3 {
        format!("{:.1}mF", farads * 1e3)
    } else if farads >= 1e-6 {
        format!("{:.1}uF", farads * 1e6)
    } else if farads >= 1e-9 {
        format!("{:.1}nF", farads * 1e9)
    } else {
        format!("{:.1}pF", farads * 1e12)
    }
}

/// Format inductance value for search (e.g., "10uH").
fn format_inductance(henries: f64) -> String {
    if henries >= 1e-3 {
        format!("{:.1}mH", henries * 1e3)
    } else if henries >= 1e-6 {
        format!("{:.1}uH", henries * 1e6)
    } else {
        format!("{:.1}nH", henries * 1e9)
    }
}

/// Check if a part matches a query.
fn matches_query(part: &Part, query: &PartQuery) -> bool {
    // Check package
    if let Some(pkg) = &query.package {
        if !part.package.name.eq_ignore_ascii_case(pkg) {
            return false;
        }
    }

    // Check stock
    if let Some(min_stock) = query.min_stock {
        let stock = part.availability.as_ref().map_or(0, |a| a.stock);
        if stock < min_stock {
            return false;
        }
    }

    // Check basic only
    if query.basic_only {
        let is_basic = part
            .availability
            .as_ref()
            .map_or(false, |a| matches!(a.part_class, PartClass::Basic | PartClass::Preferred));
        if !is_basic {
            return false;
        }
    }

    // Check parameters
    for (name, constraint) in &query.parameters {
        if let Some(value) = part.get_param(name) {
            if let Some(v) = value.as_scalar() {
                if !constraint.is_satisfied_by(v) {
                    return false;
                }
            }
        }
    }

    true
}

// ===== API Request/Response Types =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LcscSearchRequest {
    current_page: u32,
    page_size: u32,
    keyword: String,
}

#[derive(Debug, Deserialize)]
struct LcscSearchResponse {
    code: i32,
    msg: Option<String>,
    result: Option<LcscSearchResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcscSearchResult {
    product_list: Vec<LcscProduct>,
}

#[derive(Debug, Deserialize)]
struct LcscDetailResponse {
    code: i32,
    #[allow(dead_code)]
    msg: Option<String>,
    result: Option<LcscProduct>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcscProduct {
    product_code: String,
    product_model: Option<String>,
    brand_name_en: Option<String>,
    product_description_en: Option<String>,
    package: Option<String>,
    stock_count: Option<i64>,
    product_price_list: Option<Vec<LcscPriceItem>>,
    product_param_value_v_o_s: Option<Vec<LcscParam>>,
    library_type: Option<String>,
    datasheet_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcscPriceItem {
    start_pieces: Option<i64>,
    end_pieces: Option<i64>,
    product_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcscParam {
    param_name_en: Option<String>,
    param_value_en: Option<String>,
}

impl LcscProduct {
    fn into_part(self) -> Part {
        let lcsc_num = self
            .product_code
            .strip_prefix('C')
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let id = PartId::lcsc(lcsc_num);

        let manufacturer = Manufacturer::new(
            self.brand_name_en.unwrap_or_else(|| "Unknown".to_string()),
            self.product_model.unwrap_or_default(),
        );

        let package = Package::new(self.package.unwrap_or_else(|| "Unknown".to_string()));

        let description = self
            .product_description_en
            .unwrap_or_else(|| "No description".to_string());

        let mut part = Part::new(id, manufacturer, package, description);

        // Parse parameters
        let mut params = PartParameters::new();
        if let Some(param_list) = self.product_param_value_v_o_s {
            for param in param_list {
                if let (Some(name), Some(value)) = (param.param_name_en, param.param_value_en) {
                    if let Some((parsed_value, unit)) = parse_parameter_value(&name, &value) {
                        if let Some(u) = unit {
                            params = params.with_param_unit(&name.to_lowercase(), parsed_value, u);
                        } else {
                            params = params.with_param(&name.to_lowercase(), parsed_value);
                        }
                    }
                }
            }
        }
        part.parameters = params;

        // Parse availability
        let stock = self.stock_count.unwrap_or(0) as u32;
        let mut availability = Availability::with_stock(stock);

        // Parse price tiers
        if let Some(prices) = self.product_price_list {
            availability.price_tiers = prices
                .into_iter()
                .filter_map(|p| {
                    Some(PriceTier::new(
                        p.start_pieces? as u32,
                        p.end_pieces.map(|e| e as u32),
                        p.product_price?,
                    ))
                })
                .collect();
        }

        // Parse part class
        availability.part_class = match self.library_type.as_deref() {
            Some("base") => PartClass::Basic,
            Some("preferred") => PartClass::Preferred,
            _ => PartClass::Extended,
        };

        part.availability = Some(availability);

        // Datasheet
        if let Some(url) = self.datasheet_url {
            part.datasheet_url = Some(url);
        }

        part
    }
}

/// Parse a parameter value string into a ParameterValue.
fn parse_parameter_value(name: &str, value: &str) -> Option<(ParameterValue, Option<ato_domain::Unit>)> {
    let name_lower = name.to_lowercase();

    // Try to parse numeric values with units
    let (numeric, unit) = parse_value_with_unit(value)?;

    let unit_type = match name_lower.as_str() {
        "resistance" => Some(ato_domain::Unit::Ohm),
        "capacitance" => Some(ato_domain::Unit::Farad),
        "inductance" => Some(ato_domain::Unit::Henry),
        "voltage" | "voltage rating" | "rated voltage" => Some(ato_domain::Unit::Volt),
        "current" | "rated current" => Some(ato_domain::Unit::Ampere),
        "power" | "power rating" => Some(ato_domain::Unit::Watt),
        _ => unit,
    };

    Some((ParameterValue::Scalar(numeric), unit_type))
}

/// Parse a value string with optional SI prefix and unit.
fn parse_value_with_unit(s: &str) -> Option<(f64, Option<ato_domain::Unit>)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try to extract numeric part
    let mut numeric_end = 0;
    let mut has_decimal = false;
    let mut has_minus = false;

    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() || (c == '.' && !has_decimal) || (c == '-' && i == 0 && !has_minus) {
            if c == '.' {
                has_decimal = true;
            }
            if c == '-' {
                has_minus = true;
            }
            numeric_end = i + c.len_utf8();
        } else if !c.is_ascii_whitespace() {
            break;
        }
    }

    if numeric_end == 0 {
        return None;
    }

    let numeric_str = &s[..numeric_end];
    let suffix = s[numeric_end..].trim();

    let base_value: f64 = numeric_str.parse().ok()?;

    // Parse SI prefix and unit
    let (multiplier, unit) = parse_si_suffix(suffix);

    Some((base_value * multiplier, unit))
}

/// Parse SI prefix and unit from a suffix string.
fn parse_si_suffix(s: &str) -> (f64, Option<ato_domain::Unit>) {
    let s = s.to_lowercase();

    // Common patterns
    let (multiplier, rest) = if s.starts_with('p') {
        (1e-12, &s[1..])
    } else if s.starts_with('n') {
        (1e-9, &s[1..])
    } else if s.starts_with("μ") || s.starts_with("µ") || s.starts_with('u') {
        let skip = if s.starts_with("μ") || s.starts_with("µ") { "μ".len() } else { 1 };
        (1e-6, &s[skip..])
    } else if s.starts_with('m') && !s.starts_with("meg") && !s.starts_with("mohm") {
        (1e-3, &s[1..])
    } else if s.starts_with('k') {
        (1e3, &s[1..])
    } else if s.starts_with("meg") || s.starts_with('M') {
        let skip = if s.starts_with("meg") { 3 } else { 1 };
        (1e6, &s[skip..])
    } else if s.starts_with('g') {
        (1e9, &s[1..])
    } else {
        (1.0, s.as_str())
    };

    // Parse unit
    let unit = if rest.starts_with("ohm") || rest.starts_with('Ω') || rest.starts_with("ω") {
        Some(ato_domain::Unit::Ohm)
    } else if rest.starts_with('f') {
        Some(ato_domain::Unit::Farad)
    } else if rest.starts_with('h') {
        Some(ato_domain::Unit::Henry)
    } else if rest.starts_with('v') {
        Some(ato_domain::Unit::Volt)
    } else if rest.starts_with('a') {
        Some(ato_domain::Unit::Ampere)
    } else if rest.starts_with('w') {
        Some(ato_domain::Unit::Watt)
    } else {
        None
    };

    (multiplier, unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_resistance() {
        assert_eq!(format_resistance(10000.0), "10.0kohm");
        assert_eq!(format_resistance(1000000.0), "1.0Mohm");
        assert_eq!(format_resistance(100.0), "100.0ohm");
    }

    #[test]
    fn test_format_capacitance() {
        assert_eq!(format_capacitance(100e-9), "100.0nF");
        assert_eq!(format_capacitance(10e-6), "10.0uF");
        assert_eq!(format_capacitance(100e-12), "100.0pF");
    }

    #[test]
    fn test_parse_value_with_unit() {
        let (val, _) = parse_value_with_unit("10kohm").unwrap();
        assert!((val - 10000.0).abs() < 0.1);

        let (val, _) = parse_value_with_unit("100nF").unwrap();
        assert!((val - 100e-9).abs() < 1e-12);

        let (val, _) = parse_value_with_unit("4.7uF").unwrap();
        assert!((val - 4.7e-6).abs() < 1e-9);
    }

    #[test]
    fn test_build_search_string() {
        let query = PartQuery::resistor()
            .with_package("0402")
            .with_resistance(ParameterConstraint::between(9500.0, 10500.0));

        let search = build_search_string(&query);
        assert!(search.contains("resistor"));
        assert!(search.contains("0402"));
        assert!(search.contains("10.0kohm"));
    }

    #[test]
    fn test_matches_query() {
        let part = Part::new(
            PartId::lcsc(123),
            Manufacturer::new("Test", "TEST-123"),
            Package::new("0402"),
            "Test resistor",
        )
        .with_parameters(
            PartParameters::new().with_param("resistance", ParameterValue::Scalar(10000.0)),
        )
        .with_availability(Availability::with_stock(1000));

        let query = PartQuery::resistor()
            .with_package("0402")
            .with_resistance(ParameterConstraint::between(9500.0, 10500.0))
            .with_min_stock(100);

        assert!(matches_query(&part, &query));

        // Non-matching package
        let query_wrong_pkg = PartQuery::resistor().with_package("0603");
        assert!(!matches_query(&part, &query_wrong_pkg));

        // Non-matching resistance
        let query_wrong_res = PartQuery::resistor()
            .with_resistance(ParameterConstraint::between(1000.0, 2000.0));
        assert!(!matches_query(&part, &query_wrong_res));
    }
}
