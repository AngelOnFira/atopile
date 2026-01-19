//! Module types for the IR.
//!
//! Modules represent the primary building blocks of Ato designs:
//! - `module`: A hardware module with pins and parameters
//! - `component`: A specific hardware component (code-as-data)
//! - `interface`: A connectable interface definition

use crate::{ConnectionId, ConstraintId, FieldId, ModuleId, QualifiedName};
use ato_lexer::Span;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A module definition in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Unique identifier for this module.
    pub id: ModuleId,

    /// The name of this module.
    pub name: String,

    /// The fully qualified name (including path).
    pub qualified_name: QualifiedName,

    /// The kind of module (Module, Component, Interface).
    pub kind: ModuleKind,

    /// Optional super type this module extends.
    pub super_type: Option<ModuleId>,

    /// Fields defined in this module (parameters, pins, signals, instances).
    pub fields: Vec<FieldId>,

    /// Map from field name to field ID for quick lookup.
    pub field_names: HashMap<String, FieldId>,

    /// Connections defined in this module.
    pub connections: Vec<ConnectionId>,

    /// Constraints (assertions) defined in this module.
    pub constraints: Vec<ConstraintId>,

    /// Nested modules defined inside this module.
    pub nested_modules: Vec<ModuleId>,

    /// Traits applied to this module.
    pub traits: Vec<TraitRef>,

    /// Source file path (for error reporting).
    pub source_file: Option<String>,

    /// Source location for error reporting.
    pub span: Option<Span>,
}

impl Module {
    /// Create a new module.
    pub fn new(id: ModuleId, name: impl Into<String>, kind: ModuleKind) -> Self {
        let name = name.into();
        Self {
            id,
            qualified_name: QualifiedName::simple(name.clone()),
            name,
            kind,
            super_type: None,
            fields: Vec::new(),
            field_names: HashMap::new(),
            connections: Vec::new(),
            constraints: Vec::new(),
            nested_modules: Vec::new(),
            traits: Vec::new(),
            source_file: None,
            span: None,
        }
    }

    /// Set the qualified name for this module.
    pub fn with_qualified_name(mut self, name: QualifiedName) -> Self {
        self.qualified_name = name;
        self
    }

    /// Set the super type for this module.
    pub fn with_super_type(mut self, super_type: ModuleId) -> Self {
        self.super_type = Some(super_type);
        self
    }

    /// Set the source file for this module.
    pub fn with_source_file(mut self, path: impl Into<String>) -> Self {
        self.source_file = Some(path.into());
        self
    }

    /// Set the source span for this module.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Add a field to this module.
    pub fn add_field(&mut self, name: impl Into<String>, field_id: FieldId) {
        let name = name.into();
        self.field_names.insert(name, field_id);
        self.fields.push(field_id);
    }

    /// Get a field by name.
    pub fn get_field(&self, name: &str) -> Option<FieldId> {
        self.field_names.get(name).copied()
    }

    /// Add a connection to this module.
    pub fn add_connection(&mut self, connection_id: ConnectionId) {
        self.connections.push(connection_id);
    }

    /// Add a constraint to this module.
    pub fn add_constraint(&mut self, constraint_id: ConstraintId) {
        self.constraints.push(constraint_id);
    }

    /// Add a nested module.
    pub fn add_nested_module(&mut self, module_id: ModuleId) {
        self.nested_modules.push(module_id);
    }

    /// Add a trait to this module.
    pub fn add_trait(&mut self, trait_ref: TraitRef) {
        self.traits.push(trait_ref);
    }

    /// Check if this is a module.
    pub fn is_module(&self) -> bool {
        self.kind == ModuleKind::Module
    }

    /// Check if this is a component.
    pub fn is_component(&self) -> bool {
        self.kind == ModuleKind::Component
    }

    /// Check if this is an interface.
    pub fn is_interface(&self) -> bool {
        self.kind == ModuleKind::Interface
    }
}

/// The kind of module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleKind {
    /// A regular module.
    Module,
    /// A component (code-as-data).
    Component,
    /// An interface definition.
    Interface,
}

impl ModuleKind {
    /// Get the keyword for this module kind.
    pub fn keyword(&self) -> &'static str {
        match self {
            ModuleKind::Module => "module",
            ModuleKind::Component => "component",
            ModuleKind::Interface => "interface",
        }
    }
}

/// A reference to a trait applied to a module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitRef {
    /// The name of the trait.
    pub name: QualifiedName,

    /// Optional constructor name.
    pub constructor: Option<String>,

    /// Template arguments (named).
    pub template_args: Vec<NamedTemplateArg>,

    /// Source location for error reporting.
    pub span: Option<Span>,
}

impl TraitRef {
    /// Create a new trait reference.
    pub fn new(name: QualifiedName) -> Self {
        Self {
            name,
            constructor: None,
            template_args: Vec::new(),
            span: None,
        }
    }

    /// Set the constructor for this trait reference.
    pub fn with_constructor(mut self, constructor: impl Into<String>) -> Self {
        self.constructor = Some(constructor.into());
        self
    }

    /// Add a template argument.
    pub fn with_arg(mut self, name: impl Into<String>, value: TemplateArgValue) -> Self {
        self.template_args.push(NamedTemplateArg {
            name: name.into(),
            value,
        });
        self
    }

    /// Set the source span.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Get a template argument by name.
    pub fn get_arg(&self, name: &str) -> Option<&TemplateArgValue> {
        self.template_args
            .iter()
            .find(|arg| arg.name == name)
            .map(|arg| &arg.value)
    }

    /// Get a string template argument by name.
    pub fn get_string_arg(&self, name: &str) -> Option<&str> {
        self.get_arg(name).and_then(|v| {
            if let TemplateArgValue::String(s) = v {
                Some(s.as_str())
            } else {
                None
            }
        })
    }
}

/// A named template argument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedTemplateArg {
    /// The argument name.
    pub name: String,
    /// The argument value.
    pub value: TemplateArgValue,
}

/// A template argument value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TemplateArgValue {
    /// An integer value.
    Int(i64),
    /// A float value.
    Float(f64),
    /// A string value.
    String(String),
    /// A boolean value.
    Bool(bool),
}

/// Import resolution information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    /// The source file path (if `from "path"` was used).
    pub from_path: Option<String>,

    /// The imported names.
    pub names: Vec<QualifiedName>,

    /// Resolved module IDs (filled in during semantic analysis).
    pub resolved: Vec<Option<ModuleId>>,

    /// Source location for error reporting.
    pub span: Option<Span>,
}

impl Import {
    /// Create a new import.
    pub fn new(names: Vec<QualifiedName>) -> Self {
        let resolved = vec![None; names.len()];
        Self {
            from_path: None,
            names,
            resolved,
            span: None,
        }
    }

    /// Set the source path.
    pub fn with_from_path(mut self, path: impl Into<String>) -> Self {
        self.from_path = Some(path.into());
        self
    }

    /// Set the source span.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_creation() {
        let module = Module::new(ModuleId::new(0), "MyModule", ModuleKind::Module);

        assert_eq!(module.name, "MyModule");
        assert!(module.is_module());
        assert!(!module.is_component());
        assert!(!module.is_interface());
    }

    #[test]
    fn test_module_fields() {
        let mut module = Module::new(ModuleId::new(0), "MyModule", ModuleKind::Module);

        module.add_field("field1", FieldId::new(0));
        module.add_field("field2", FieldId::new(1));

        assert_eq!(module.fields.len(), 2);
        assert_eq!(module.get_field("field1"), Some(FieldId::new(0)));
        assert_eq!(module.get_field("field2"), Some(FieldId::new(1)));
        assert_eq!(module.get_field("nonexistent"), None);
    }

    #[test]
    fn test_module_kind() {
        assert_eq!(ModuleKind::Module.keyword(), "module");
        assert_eq!(ModuleKind::Component.keyword(), "component");
        assert_eq!(ModuleKind::Interface.keyword(), "interface");
    }

    #[test]
    fn test_trait_ref() {
        let trait_ref = TraitRef::new(QualifiedName::simple("has_designator"))
            .with_constructor("prefix")
            .with_arg("value", TemplateArgValue::String("R".into()));

        assert_eq!(trait_ref.name.name(), "has_designator");
        assert_eq!(trait_ref.constructor, Some("prefix".into()));
        assert_eq!(trait_ref.template_args.len(), 1);

        // Test get_arg and get_string_arg
        assert_eq!(trait_ref.get_arg("value"), Some(&TemplateArgValue::String("R".into())));
        assert_eq!(trait_ref.get_string_arg("value"), Some("R"));
        assert_eq!(trait_ref.get_arg("nonexistent"), None);
        assert_eq!(trait_ref.get_string_arg("nonexistent"), None);
    }

    #[test]
    fn test_import() {
        let import = Import::new(vec![
            QualifiedName::simple("Resistor"),
            QualifiedName::simple("Capacitor"),
        ])
        .with_from_path("path/to/components.ato");

        assert_eq!(import.names.len(), 2);
        assert_eq!(import.from_path, Some("path/to/components.ato".into()));
    }
}
