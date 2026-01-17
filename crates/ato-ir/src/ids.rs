//! Unique identifiers for IR elements.
//!
//! These IDs are lightweight handles that can be cheaply copied and compared.
//! They are used to reference modules, fields, and connections within a Design.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A unique identifier for a module within a Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleId(pub(crate) u32);

impl ModuleId {
    /// Create a new ModuleId with the given index.
    pub(crate) fn new(index: u32) -> Self {
        Self(index)
    }

    /// Get the raw index value.
    pub fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Module({})", self.0)
    }
}

/// A unique identifier for a field within a Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldId(pub(crate) u32);

impl FieldId {
    /// Create a new FieldId with the given index.
    pub(crate) fn new(index: u32) -> Self {
        Self(index)
    }

    /// Get the raw index value.
    pub fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Display for FieldId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Field({})", self.0)
    }
}

/// A unique identifier for a connection within a Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub(crate) u32);

impl ConnectionId {
    /// Create a new ConnectionId with the given index.
    pub(crate) fn new(index: u32) -> Self {
        Self(index)
    }

    /// Get the raw index value.
    pub fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Connection({})", self.0)
    }
}

/// A unique identifier for a constraint within a Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConstraintId(pub(crate) u32);

impl ConstraintId {
    /// Create a new ConstraintId with the given index.
    pub(crate) fn new(index: u32) -> Self {
        Self(index)
    }

    /// Get the raw index value.
    pub fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ConstraintId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Constraint({})", self.0)
    }
}

/// A qualified name path, used for import resolution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QualifiedName {
    /// The path components (e.g., ["path", "to", "Module"])
    pub parts: Vec<String>,
}

impl QualifiedName {
    /// Create a new qualified name from parts.
    pub fn new(parts: Vec<String>) -> Self {
        Self { parts }
    }

    /// Create a qualified name from a single name.
    pub fn simple(name: impl Into<String>) -> Self {
        Self {
            parts: vec![name.into()],
        }
    }

    /// Get the simple name (last component).
    pub fn name(&self) -> &str {
        self.parts.last().map(|s| s.as_str()).unwrap_or("")
    }

    /// Check if this is a simple (unqualified) name.
    pub fn is_simple(&self) -> bool {
        self.parts.len() == 1
    }

    /// Join the parts with a separator.
    pub fn join(&self, sep: &str) -> String {
        self.parts.join(sep)
    }
}

impl fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.parts.join("."))
    }
}

impl From<&str> for QualifiedName {
    fn from(s: &str) -> Self {
        Self::simple(s)
    }
}

impl From<String> for QualifiedName {
    fn from(s: String) -> Self {
        Self::simple(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let id = ModuleId::new(42);
        assert_eq!(id.index(), 42);
        assert_eq!(format!("{}", id), "Module(42)");
    }

    #[test]
    fn test_field_id() {
        let id = FieldId::new(7);
        assert_eq!(id.index(), 7);
        assert_eq!(format!("{}", id), "Field(7)");
    }

    #[test]
    fn test_connection_id() {
        let id = ConnectionId::new(123);
        assert_eq!(id.index(), 123);
        assert_eq!(format!("{}", id), "Connection(123)");
    }

    #[test]
    fn test_qualified_name() {
        let name = QualifiedName::new(vec!["path".into(), "to".into(), "Module".into()]);
        assert_eq!(name.name(), "Module");
        assert!(!name.is_simple());
        assert_eq!(name.join("."), "path.to.Module");
        assert_eq!(format!("{}", name), "path.to.Module");
    }

    #[test]
    fn test_qualified_name_simple() {
        let name = QualifiedName::simple("Module");
        assert_eq!(name.name(), "Module");
        assert!(name.is_simple());
    }
}
