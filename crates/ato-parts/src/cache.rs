//! SQLite cache for part data.
//!
//! This module provides a local cache for parts fetched from remote databases,
//! enabling offline-first operation and reducing API calls.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::database::{DatabaseError, DatabaseResult, PartDatabase};
use crate::part::{Part, PartId};
use crate::query::PartQuery;

/// Default cache TTL (time-to-live) in hours.
const DEFAULT_TTL_HOURS: i64 = 24;

/// Cached part entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedPart {
    part: Part,
    cached_at: DateTime<Utc>,
    ttl_hours: i64,
}

impl CachedPart {
    fn new(part: Part, ttl_hours: i64) -> Self {
        Self {
            part,
            cached_at: Utc::now(),
            ttl_hours,
        }
    }

    fn is_expired(&self) -> bool {
        let expiry = self.cached_at + Duration::hours(self.ttl_hours);
        Utc::now() > expiry
    }
}

/// SQLite-backed part cache.
pub struct PartCache {
    conn: Connection,
    ttl_hours: i64,
}

impl PartCache {
    /// Create a new cache at the specified path.
    pub fn new(path: impl AsRef<Path>) -> DatabaseResult<Self> {
        let conn = Connection::open(path.as_ref())
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let cache = Self {
            conn,
            ttl_hours: DEFAULT_TTL_HOURS,
        };

        cache.init_schema()?;

        Ok(cache)
    }

    /// Create a cache in the default user data directory.
    pub fn default_cache() -> DatabaseResult<Self> {
        let cache_dir = get_cache_dir()?;
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| DatabaseError::Io(e))?;

        let cache_path = cache_dir.join("parts.db");
        Self::new(cache_path)
    }

    /// Create an in-memory cache (for testing).
    pub fn in_memory() -> DatabaseResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let cache = Self {
            conn,
            ttl_hours: DEFAULT_TTL_HOURS,
        };

        cache.init_schema()?;

        Ok(cache)
    }

    /// Set the TTL for cached entries.
    pub fn with_ttl(mut self, hours: i64) -> Self {
        self.ttl_hours = hours;
        self
    }

    /// Initialize the database schema.
    fn init_schema(&self) -> DatabaseResult<()> {
        self.conn
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS parts (
                    supplier TEXT NOT NULL,
                    supplier_id TEXT NOT NULL,
                    part_json TEXT NOT NULL,
                    cached_at TEXT NOT NULL,
                    ttl_hours INTEGER NOT NULL,
                    PRIMARY KEY (supplier, supplier_id)
                );

                CREATE INDEX IF NOT EXISTS idx_parts_cached_at ON parts(cached_at);

                CREATE TABLE IF NOT EXISTS search_cache (
                    query_hash TEXT PRIMARY KEY,
                    results_json TEXT NOT NULL,
                    cached_at TEXT NOT NULL,
                    ttl_hours INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_search_cached_at ON search_cache(cached_at);
                "#,
            )
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(())
    }

    /// Get a part from the cache.
    pub fn get(&self, id: &PartId) -> DatabaseResult<Option<Part>> {
        let result: Option<(String, String, i64)> = self
            .conn
            .query_row(
                "SELECT part_json, cached_at, ttl_hours FROM parts WHERE supplier = ?1 AND supplier_id = ?2",
                params![id.supplier, id.supplier_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        if let Some((json, cached_at_str, ttl_hours)) = result {
            let cached_at: DateTime<Utc> = cached_at_str
                .parse()
                .map_err(|e| DatabaseError::Api(format!("invalid date: {}", e)))?;

            let cached = CachedPart {
                part: serde_json::from_str(&json)
                    .map_err(|e| DatabaseError::Api(format!("invalid JSON: {}", e)))?,
                cached_at,
                ttl_hours,
            };

            if !cached.is_expired() {
                return Ok(Some(cached.part));
            }

            // Remove expired entry
            let _ = self.remove(id);
        }

        Ok(None)
    }

    /// Store a part in the cache.
    pub fn put(&self, part: &Part) -> DatabaseResult<()> {
        let cached = CachedPart::new(part.clone(), self.ttl_hours);
        let json = serde_json::to_string(&cached.part)
            .map_err(|e| DatabaseError::Api(format!("failed to serialize: {}", e)))?;

        self.conn
            .execute(
                "INSERT OR REPLACE INTO parts (supplier, supplier_id, part_json, cached_at, ttl_hours) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    part.id.supplier,
                    part.id.supplier_id,
                    json,
                    cached.cached_at.to_rfc3339(),
                    cached.ttl_hours,
                ],
            )
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(())
    }

    /// Remove a part from the cache.
    pub fn remove(&self, id: &PartId) -> DatabaseResult<bool> {
        let rows = self
            .conn
            .execute(
                "DELETE FROM parts WHERE supplier = ?1 AND supplier_id = ?2",
                params![id.supplier, id.supplier_id],
            )
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(rows > 0)
    }

    /// Clear all expired entries.
    pub fn clear_expired(&self) -> DatabaseResult<usize> {
        let now = Utc::now().to_rfc3339();

        let rows = self
            .conn
            .execute(
                "DELETE FROM parts WHERE datetime(cached_at, '+' || ttl_hours || ' hours') < datetime(?1)",
                params![now],
            )
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let search_rows = self
            .conn
            .execute(
                "DELETE FROM search_cache WHERE datetime(cached_at, '+' || ttl_hours || ' hours') < datetime(?1)",
                params![now],
            )
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(rows + search_rows)
    }

    /// Clear all cache entries.
    pub fn clear_all(&self) -> DatabaseResult<()> {
        self.conn
            .execute_batch("DELETE FROM parts; DELETE FROM search_cache;")
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(())
    }

    /// Get the number of cached parts.
    pub fn count(&self) -> DatabaseResult<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM parts", [], |row| row.get(0))
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(count as usize)
    }

    /// Cache search results.
    pub fn cache_search(&self, query: &PartQuery, results: &[Part]) -> DatabaseResult<()> {
        let query_hash = compute_query_hash(query);
        let json = serde_json::to_string(results)
            .map_err(|e| DatabaseError::Api(format!("failed to serialize: {}", e)))?;

        self.conn
            .execute(
                "INSERT OR REPLACE INTO search_cache (query_hash, results_json, cached_at, ttl_hours) VALUES (?1, ?2, ?3, ?4)",
                params![
                    query_hash,
                    json,
                    Utc::now().to_rfc3339(),
                    self.ttl_hours,
                ],
            )
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(())
    }

    /// Get cached search results.
    pub fn get_search(&self, query: &PartQuery) -> DatabaseResult<Option<Vec<Part>>> {
        let query_hash = compute_query_hash(query);

        let result: Option<(String, String, i64)> = self
            .conn
            .query_row(
                "SELECT results_json, cached_at, ttl_hours FROM search_cache WHERE query_hash = ?1",
                params![query_hash],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| DatabaseError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        if let Some((json, cached_at_str, ttl_hours)) = result {
            let cached_at: DateTime<Utc> = cached_at_str
                .parse()
                .map_err(|e| DatabaseError::Api(format!("invalid date: {}", e)))?;

            let expiry = cached_at + Duration::hours(ttl_hours);
            if Utc::now() <= expiry {
                let parts: Vec<Part> = serde_json::from_str(&json)
                    .map_err(|e| DatabaseError::Api(format!("invalid JSON: {}", e)))?;
                return Ok(Some(parts));
            }

            // Remove expired entry
            let _ = self.conn.execute(
                "DELETE FROM search_cache WHERE query_hash = ?1",
                params![query_hash],
            );
        }

        Ok(None)
    }
}

/// Get the default cache directory.
fn get_cache_dir() -> DatabaseResult<PathBuf> {
    directories::ProjectDirs::from("com", "atopile", "ato")
        .map(|dirs| dirs.cache_dir().to_path_buf())
        .ok_or_else(|| DatabaseError::Unavailable("could not determine cache directory".into()))
}

/// Compute a hash of a query for caching.
fn compute_query_hash(query: &PartQuery) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Hash component type
    if let Some(ct) = &query.component_type {
        format!("{:?}", ct).hash(&mut hasher);
    }

    // Hash package
    if let Some(pkg) = &query.package {
        pkg.hash(&mut hasher);
    }

    // Hash parameters (sorted for consistency)
    let mut params: Vec<_> = query.parameters.iter().collect();
    params.sort_by_key(|(k, _)| *k);
    for (name, constraint) in params {
        name.hash(&mut hasher);
        format!("{:?}", constraint).hash(&mut hasher);
    }

    // Hash other fields
    query.min_stock.hash(&mut hasher);
    query.basic_only.hash(&mut hasher);
    query.limit.hash(&mut hasher);

    format!("{:016x}", hasher.finish())
}

/// A cached database that wraps another database and caches results.
pub struct CachedDatabase<D: PartDatabase> {
    inner: D,
    cache: PartCache,
}

impl<D: PartDatabase> CachedDatabase<D> {
    /// Create a new cached database.
    pub fn new(inner: D, cache: PartCache) -> Self {
        Self { inner, cache }
    }

    /// Create with default cache location.
    pub fn with_default_cache(inner: D) -> DatabaseResult<Self> {
        let cache = PartCache::default_cache()?;
        Ok(Self::new(inner, cache))
    }
}

impl<D: PartDatabase> PartDatabase for CachedDatabase<D> {
    fn query(&self, query: &PartQuery) -> DatabaseResult<Vec<Part>> {
        // Check cache first
        if let Some(cached) = self.cache.get_search(query)? {
            return Ok(cached);
        }

        // Query the inner database
        let results = self.inner.query(query)?;

        // Cache the results
        self.cache.cache_search(query, &results)?;

        // Also cache individual parts
        for part in &results {
            let _ = self.cache.put(part);
        }

        Ok(results)
    }

    fn fetch_by_id(&self, id: &PartId) -> DatabaseResult<Part> {
        // Check cache first
        if let Some(part) = self.cache.get(id)? {
            return Ok(part);
        }

        // Fetch from inner database
        let part = self.inner.fetch_by_id(id)?;

        // Cache the result
        self.cache.put(&part)?;

        Ok(part)
    }

    fn fetch_by_mpn(&self, manufacturer: &str, part_number: &str) -> DatabaseResult<Vec<Part>> {
        // For MPN queries, always go to inner (too many combinations to cache effectively)
        self.inner.fetch_by_mpn(manufacturer, part_number)
    }

    fn is_available(&self) -> bool {
        // We're available if cache has data or inner is available
        self.cache.count().unwrap_or(0) > 0 || self.inner.is_available()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::{Availability, Manufacturer, Package, PartParameters, ParameterValue};

    fn create_test_part(id: u32) -> Part {
        Part::new(
            PartId::lcsc(id),
            Manufacturer::new("Test", format!("TEST-{}", id)),
            Package::new("0402"),
            format!("Test part {}", id),
        )
        .with_parameters(
            PartParameters::new().with_param("resistance", ParameterValue::Scalar(10000.0)),
        )
        .with_availability(Availability::with_stock(1000))
    }

    #[test]
    fn test_cache_put_get() {
        let cache = PartCache::in_memory().unwrap();
        let part = create_test_part(123);

        // Part should not be in cache
        assert!(cache.get(&part.id).unwrap().is_none());

        // Put part in cache
        cache.put(&part).unwrap();

        // Part should now be in cache
        let cached = cache.get(&part.id).unwrap().unwrap();
        assert_eq!(cached.id, part.id);
        assert_eq!(cached.manufacturer.name, part.manufacturer.name);
    }

    #[test]
    fn test_cache_remove() {
        let cache = PartCache::in_memory().unwrap();
        let part = create_test_part(456);

        cache.put(&part).unwrap();
        assert!(cache.get(&part.id).unwrap().is_some());

        cache.remove(&part.id).unwrap();
        assert!(cache.get(&part.id).unwrap().is_none());
    }

    #[test]
    fn test_cache_count() {
        let cache = PartCache::in_memory().unwrap();

        assert_eq!(cache.count().unwrap(), 0);

        cache.put(&create_test_part(1)).unwrap();
        cache.put(&create_test_part(2)).unwrap();
        cache.put(&create_test_part(3)).unwrap();

        assert_eq!(cache.count().unwrap(), 3);
    }

    #[test]
    fn test_cache_clear_all() {
        let cache = PartCache::in_memory().unwrap();

        cache.put(&create_test_part(1)).unwrap();
        cache.put(&create_test_part(2)).unwrap();

        assert_eq!(cache.count().unwrap(), 2);

        cache.clear_all().unwrap();

        assert_eq!(cache.count().unwrap(), 0);
    }

    #[test]
    fn test_search_cache() {
        let cache = PartCache::in_memory().unwrap();

        let query = PartQuery::resistor().with_package("0402");
        let results = vec![create_test_part(1), create_test_part(2)];

        // Should not be cached
        assert!(cache.get_search(&query).unwrap().is_none());

        // Cache the search
        cache.cache_search(&query, &results).unwrap();

        // Should now be cached
        let cached = cache.get_search(&query).unwrap().unwrap();
        assert_eq!(cached.len(), 2);
    }

    #[test]
    fn test_query_hash() {
        let query1 = PartQuery::resistor().with_package("0402");
        let query2 = PartQuery::resistor().with_package("0402");
        let query3 = PartQuery::resistor().with_package("0603");

        let hash1 = compute_query_hash(&query1);
        let hash2 = compute_query_hash(&query2);
        let hash3 = compute_query_hash(&query3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
