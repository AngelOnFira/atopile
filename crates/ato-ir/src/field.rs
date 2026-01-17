//! Field types for the IR.
//!
//! Fields represent the members of a module: parameters, pins, signals, and instances.

use crate::{FieldId, ModuleId, QualifiedName};
use ato_lexer::Span;
use serde::{Deserialize, Serialize};

/// A field within a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    /// Unique identifier for this field.
    pub id: FieldId,

    /// The module that contains this field.
    pub parent: ModuleId,

    /// The name of this field.
    pub name: String,

    /// The kind of field (parameter, pin, signal, instance).
    pub kind: FieldKind,

    /// Source location for error reporting.
    pub span: Option<Span>,
}

impl Field {
    /// Create a new field.
    pub fn new(id: FieldId, parent: ModuleId, name: impl Into<String>, kind: FieldKind) -> Self {
        Self {
            id,
            parent,
            name: name.into(),
            kind,
            span: None,
        }
    }

    /// Set the source span for this field.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Check if this field is a parameter.
    pub fn is_parameter(&self) -> bool {
        matches!(self.kind, FieldKind::Parameter { .. })
    }

    /// Check if this field is a pin.
    pub fn is_pin(&self) -> bool {
        matches!(self.kind, FieldKind::Pin { .. })
    }

    /// Check if this field is a signal.
    pub fn is_signal(&self) -> bool {
        matches!(self.kind, FieldKind::Signal)
    }

    /// Check if this field is an instance.
    pub fn is_instance(&self) -> bool {
        matches!(self.kind, FieldKind::Instance { .. })
    }

    /// Check if this field is connectable (pin, signal, or instance).
    pub fn is_connectable(&self) -> bool {
        matches!(
            self.kind,
            FieldKind::Pin { .. } | FieldKind::Signal | FieldKind::Instance { .. }
        )
    }
}

/// The kind of a field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldKind {
    /// A parameter with optional unit (e.g., `resistance: ohm`).
    Parameter {
        /// The unit type (e.g., "ohm", "V", "F").
        unit: Option<String>,
    },

    /// A pin declaration (e.g., `pin p1`, `pin 1`, `pin "GND"`).
    Pin {
        /// The pin name/identifier.
        pin_name: PinNameKind,
    },

    /// A signal definition (e.g., `signal sig`).
    Signal,

    /// An instance of another module (e.g., `r1 = new Resistor`).
    Instance {
        /// The type being instantiated.
        type_ref: QualifiedName,
        /// Optional array count for array instances.
        count: Option<u32>,
        /// The resolved module ID (filled in during semantic analysis).
        resolved_type: Option<ModuleId>,
    },
}

impl FieldKind {
    /// Create a parameter field kind with no unit.
    pub fn parameter() -> Self {
        Self::Parameter { unit: None }
    }

    /// Create a parameter field kind with a unit.
    pub fn parameter_with_unit(unit: impl Into<String>) -> Self {
        Self::Parameter {
            unit: Some(unit.into()),
        }
    }

    /// Create a pin field kind with a named identifier.
    pub fn pin(name: impl Into<String>) -> Self {
        Self::Pin {
            pin_name: PinNameKind::Identifier(name.into()),
        }
    }

    /// Create a pin field kind with a numeric identifier.
    pub fn pin_number(num: u32) -> Self {
        Self::Pin {
            pin_name: PinNameKind::Number(num),
        }
    }

    /// Create a pin field kind with a string identifier.
    pub fn pin_string(s: impl Into<String>) -> Self {
        Self::Pin {
            pin_name: PinNameKind::String(s.into()),
        }
    }

    /// Create a signal field kind.
    pub fn signal() -> Self {
        Self::Signal
    }

    /// Create an instance field kind.
    pub fn instance(type_ref: QualifiedName) -> Self {
        Self::Instance {
            type_ref,
            count: None,
            resolved_type: None,
        }
    }

    /// Create an array instance field kind.
    pub fn instance_array(type_ref: QualifiedName, count: u32) -> Self {
        Self::Instance {
            type_ref,
            count: Some(count),
            resolved_type: None,
        }
    }
}

/// The kind of pin name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinNameKind {
    /// A named identifier (e.g., `pin p1`).
    Identifier(String),
    /// A numeric identifier (e.g., `pin 1`).
    Number(u32),
    /// A string identifier (e.g., `pin "GND"`).
    String(String),
}

impl PinNameKind {
    /// Get the display name for this pin.
    pub fn display_name(&self) -> String {
        match self {
            PinNameKind::Identifier(s) => s.clone(),
            PinNameKind::Number(n) => n.to_string(),
            PinNameKind::String(s) => format!("\"{}\"", s),
        }
    }
}

/// A reference to a field, potentially through nested instances.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldPath {
    /// The path components, from root to the referenced field.
    pub parts: Vec<FieldPathPart>,
}

impl FieldPath {
    /// Create a new field path.
    pub fn new(parts: Vec<FieldPathPart>) -> Self {
        Self { parts }
    }

    /// Create a simple field path with a single name.
    pub fn simple(name: impl Into<String>) -> Self {
        Self {
            parts: vec![FieldPathPart::Name(name.into())],
        }
    }

    /// Check if this is a simple (single-part) path.
    pub fn is_simple(&self) -> bool {
        self.parts.len() == 1 && matches!(self.parts[0], FieldPathPart::Name(_))
    }

    /// Get the first part name if it's a simple name.
    pub fn first_name(&self) -> Option<&str> {
        self.parts.first().and_then(|p| match p {
            FieldPathPart::Name(n) => Some(n.as_str()),
            _ => None,
        })
    }
}

/// A part of a field path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldPathPart {
    /// A named field access.
    Name(String),
    /// An array index access.
    Index(u32),
    /// A pin reference (e.g., `.1`).
    PinRef(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_creation() {
        let field = Field::new(
            FieldId::new(0),
            ModuleId::new(0),
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );

        assert_eq!(field.name, "resistance");
        assert!(field.is_parameter());
        assert!(!field.is_connectable());
    }

    #[test]
    fn test_pin_field() {
        let field = Field::new(
            FieldId::new(1),
            ModuleId::new(0),
            "p1",
            FieldKind::pin("p1"),
        );

        assert!(field.is_pin());
        assert!(field.is_connectable());
    }

    #[test]
    fn test_signal_field() {
        let field = Field::new(
            FieldId::new(2),
            ModuleId::new(0),
            "sig",
            FieldKind::signal(),
        );

        assert!(field.is_signal());
        assert!(field.is_connectable());
    }

    #[test]
    fn test_instance_field() {
        let field = Field::new(
            FieldId::new(3),
            ModuleId::new(0),
            "r1",
            FieldKind::instance(QualifiedName::simple("Resistor")),
        );

        assert!(field.is_instance());
        assert!(field.is_connectable());
    }

    #[test]
    fn test_field_path() {
        let path = FieldPath::new(vec![
            FieldPathPart::Name("instance".into()),
            FieldPathPart::Index(0),
            FieldPathPart::Name("pin".into()),
        ]);

        assert!(!path.is_simple());
        assert_eq!(path.first_name(), Some("instance"));
    }

    #[test]
    fn test_pin_name_kinds() {
        assert_eq!(
            PinNameKind::Identifier("p1".into()).display_name(),
            "p1"
        );
        assert_eq!(PinNameKind::Number(1).display_name(), "1");
        assert_eq!(PinNameKind::String("GND".into()).display_name(), "\"GND\"");
    }
}
