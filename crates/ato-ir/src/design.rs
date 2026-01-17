//! Design container for the IR.
//!
//! The Design is the root container that holds all modules, fields, connections,
//! and constraints for an Ato project.

use crate::{
    Connection, ConnectionEndpoint, ConnectionGraph, ConnectionId, ConnectionKind,
    Constraint, ConstraintId, Field, FieldId, FieldKind, Import, IrError, IrResult,
    Module, ModuleId, ModuleKind, QualifiedName,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The root container for an Ato design.
///
/// A Design holds all the modules, fields, connections, and constraints
/// that make up an Ato project. It provides methods for creating and
/// querying these elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Design {
    /// All modules in the design.
    modules: Vec<Module>,

    /// Map from qualified name to module ID.
    module_names: HashMap<String, ModuleId>,

    /// All fields in the design.
    fields: Vec<Field>,

    /// All connections in the design.
    connections: Vec<Connection>,

    /// All constraints in the design.
    constraints: Vec<Constraint>,

    /// Imports from external files.
    imports: Vec<Import>,

    /// Connection graph for efficient querying.
    #[serde(skip)]
    connection_graph: ConnectionGraph,

    /// Entry point module (if set).
    entry_module: Option<ModuleId>,
}

impl Default for Design {
    fn default() -> Self {
        Self::new()
    }
}

impl Design {
    /// Create a new empty design.
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
            module_names: HashMap::new(),
            fields: Vec::new(),
            connections: Vec::new(),
            constraints: Vec::new(),
            imports: Vec::new(),
            connection_graph: ConnectionGraph::new(),
            entry_module: None,
        }
    }

    // === Module Operations ===

    /// Create a new module and return its ID.
    pub fn create_module(&mut self, name: impl Into<String>, kind: ModuleKind) -> ModuleId {
        let id = ModuleId::new(self.modules.len() as u32);
        let name = name.into();
        let module = Module::new(id, name.clone(), kind);
        self.module_names.insert(name, id);
        self.modules.push(module);
        id
    }

    /// Create a module with a qualified name.
    pub fn create_module_qualified(
        &mut self,
        name: QualifiedName,
        kind: ModuleKind,
    ) -> ModuleId {
        let id = ModuleId::new(self.modules.len() as u32);
        let simple_name = name.name().to_string();
        let mut module = Module::new(id, simple_name.clone(), kind);
        module.qualified_name = name.clone();
        self.module_names.insert(name.join("."), id);
        self.modules.push(module);
        id
    }

    /// Get a module by ID.
    pub fn get_module(&self, id: ModuleId) -> Option<&Module> {
        self.modules.get(id.0 as usize)
    }

    /// Get a mutable reference to a module by ID.
    pub fn get_module_mut(&mut self, id: ModuleId) -> Option<&mut Module> {
        self.modules.get_mut(id.0 as usize)
    }

    /// Find a module by name.
    pub fn find_module(&self, name: &str) -> Option<ModuleId> {
        self.module_names.get(name).copied()
    }

    /// Get all modules.
    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    /// Set the entry module.
    pub fn set_entry_module(&mut self, id: ModuleId) {
        self.entry_module = Some(id);
    }

    /// Get the entry module.
    pub fn entry_module(&self) -> Option<ModuleId> {
        self.entry_module
    }

    // === Field Operations ===

    /// Add a field to a module and return its ID.
    pub fn add_field(
        &mut self,
        module_id: ModuleId,
        name: impl Into<String>,
        kind: FieldKind,
    ) -> FieldId {
        let id = FieldId::new(self.fields.len() as u32);
        let name = name.into();

        let field = Field::new(id, module_id, name.clone(), kind);
        self.fields.push(field);

        // Register in the parent module
        if let Some(module) = self.get_module_mut(module_id) {
            module.add_field(name, id);
        }

        id
    }

    /// Get a field by ID.
    pub fn get_field(&self, id: FieldId) -> Option<&Field> {
        self.fields.get(id.0 as usize)
    }

    /// Get a mutable reference to a field by ID.
    pub fn get_field_mut(&mut self, id: FieldId) -> Option<&mut Field> {
        self.fields.get_mut(id.0 as usize)
    }

    /// Get all fields.
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// Find a field by name within a module.
    pub fn find_field(&self, module_id: ModuleId, name: &str) -> Option<FieldId> {
        self.get_module(module_id)?.get_field(name)
    }

    // === Connection Operations ===

    /// Add a simple connection between two endpoints.
    pub fn add_connection(
        &mut self,
        module_id: ModuleId,
        left: ConnectionEndpoint,
        right: ConnectionEndpoint,
    ) -> ConnectionId {
        let id = ConnectionId::new(self.connections.len() as u32);
        let connection = Connection::new(id, module_id, left, right);
        self.connections.push(connection);

        // Register in the parent module
        if let Some(module) = self.get_module_mut(module_id) {
            module.add_connection(id);
        }

        id
    }

    /// Add a directed connection with multiple endpoints.
    pub fn add_directed_connection(
        &mut self,
        module_id: ModuleId,
        endpoints: Vec<ConnectionEndpoint>,
        kind: ConnectionKind,
    ) -> ConnectionId {
        let id = ConnectionId::new(self.connections.len() as u32);

        let connection = Connection {
            id,
            parent: module_id,
            endpoints,
            kind,
            span: None,
        };
        self.connections.push(connection);

        // Register in the parent module
        if let Some(module) = self.get_module_mut(module_id) {
            module.add_connection(id);
        }

        id
    }

    /// Get a connection by ID.
    pub fn get_connection(&self, id: ConnectionId) -> Option<&Connection> {
        self.connections.get(id.0 as usize)
    }

    /// Get all connections.
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Get the connection graph.
    pub fn connection_graph(&self) -> &ConnectionGraph {
        &self.connection_graph
    }

    /// Rebuild the connection graph from resolved connections.
    pub fn rebuild_connection_graph(&mut self) {
        self.connection_graph = ConnectionGraph::new();

        for connection in &self.connections {
            // For simple connections, add edge between resolved endpoints
            if connection.is_simple() {
                if let (Some(left), Some(right)) =
                    (connection.left(), connection.right())
                {
                    if let (Some(left_id), Some(right_id)) =
                        (left.resolved, right.resolved)
                    {
                        self.connection_graph.add_connection(left_id, right_id);
                    }
                }
            }
            // For directed connections, add edges between consecutive pairs
            else {
                for window in connection.endpoints.windows(2) {
                    if let (Some(a), Some(b)) =
                        (window[0].resolved, window[1].resolved)
                    {
                        self.connection_graph.add_connection(a, b);
                    }
                }
            }
        }
    }

    // === Constraint Operations ===

    /// Add a constraint to a module.
    pub fn add_constraint(
        &mut self,
        module_id: ModuleId,
        constraint: Constraint,
    ) -> ConstraintId {
        let id = constraint.id;
        self.constraints.push(constraint);

        // Register in the parent module
        if let Some(module) = self.get_module_mut(module_id) {
            module.add_constraint(id);
        }

        id
    }

    /// Create and add a constraint to a module.
    pub fn create_constraint(
        &mut self,
        module_id: ModuleId,
        expression: crate::ConstraintExpr,
    ) -> ConstraintId {
        let id = ConstraintId::new(self.constraints.len() as u32);
        let constraint = Constraint::new(id, module_id, expression);
        self.add_constraint(module_id, constraint)
    }

    /// Get a constraint by ID.
    pub fn get_constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.get(id.0 as usize)
    }

    /// Get all constraints.
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    // === Import Operations ===

    /// Add an import.
    pub fn add_import(&mut self, import: Import) {
        self.imports.push(import);
    }

    /// Get all imports.
    pub fn imports(&self) -> &[Import] {
        &self.imports
    }

    // === Validation ===

    /// Validate that a module ID is valid.
    pub fn validate_module_id(&self, id: ModuleId) -> IrResult<()> {
        if (id.0 as usize) < self.modules.len() {
            Ok(())
        } else {
            Err(IrError::ModuleNotFound(id))
        }
    }

    /// Validate that a field ID is valid.
    pub fn validate_field_id(&self, id: FieldId) -> IrResult<()> {
        if (id.0 as usize) < self.fields.len() {
            Ok(())
        } else {
            Err(IrError::FieldNotFound(id))
        }
    }

    /// Validate that a connection ID is valid.
    pub fn validate_connection_id(&self, id: ConnectionId) -> IrResult<()> {
        if (id.0 as usize) < self.connections.len() {
            Ok(())
        } else {
            Err(IrError::ConnectionNotFound(id))
        }
    }

    // === Statistics ===

    /// Get the number of modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Get the number of fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Get the number of connections.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get the number of constraints.
    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FieldPath, ConstraintExpr, ValueExpr, CompareOpKind, ValueLiteral, QuantityValue};

    #[test]
    fn test_design_creation() {
        let design = Design::new();
        assert_eq!(design.module_count(), 0);
        assert_eq!(design.field_count(), 0);
        assert_eq!(design.connection_count(), 0);
    }

    #[test]
    fn test_create_module() {
        let mut design = Design::new();

        let module_id = design.create_module("MyModule", ModuleKind::Module);

        assert_eq!(design.module_count(), 1);
        let module = design.get_module(module_id).unwrap();
        assert_eq!(module.name, "MyModule");
        assert!(module.is_module());
    }

    #[test]
    fn test_find_module() {
        let mut design = Design::new();

        let module_id = design.create_module("TestModule", ModuleKind::Module);

        assert_eq!(design.find_module("TestModule"), Some(module_id));
        assert_eq!(design.find_module("NonExistent"), None);
    }

    #[test]
    fn test_add_fields() {
        let mut design = Design::new();

        let module_id = design.create_module("M", ModuleKind::Module);

        let pin_id = design.add_field(module_id, "p1", FieldKind::pin("p1"));
        let param_id = design.add_field(
            module_id,
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );

        assert_eq!(design.field_count(), 2);

        let pin = design.get_field(pin_id).unwrap();
        assert!(pin.is_pin());

        let param = design.get_field(param_id).unwrap();
        assert!(param.is_parameter());

        // Check module has the fields registered
        let module = design.get_module(module_id).unwrap();
        assert_eq!(module.get_field("p1"), Some(pin_id));
        assert_eq!(module.get_field("resistance"), Some(param_id));
    }

    #[test]
    fn test_add_connection() {
        let mut design = Design::new();

        let module_id = design.create_module("M", ModuleKind::Module);
        let _p1 = design.add_field(module_id, "p1", FieldKind::pin("p1"));
        let _p2 = design.add_field(module_id, "p2", FieldKind::pin("p2"));

        let conn_id = design.add_connection(
            module_id,
            ConnectionEndpoint::field(FieldPath::simple("p1")),
            ConnectionEndpoint::field(FieldPath::simple("p2")),
        );

        assert_eq!(design.connection_count(), 1);

        let conn = design.get_connection(conn_id).unwrap();
        assert!(conn.is_simple());
    }

    #[test]
    fn test_add_constraint() {
        let mut design = Design::new();

        let module_id = design.create_module("M", ModuleKind::Module);
        let _resistance = design.add_field(
            module_id,
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );

        let constraint_id = design.create_constraint(
            module_id,
            ConstraintExpr::compare(
                ValueExpr::field(FieldPath::simple("resistance")),
                CompareOpKind::Within,
                ValueExpr::literal(ValueLiteral::range(
                    QuantityValue::new(9.0, Some("kohm".into())),
                    QuantityValue::new(11.0, Some("kohm".into())),
                )),
            ),
        );

        assert_eq!(design.constraint_count(), 1);
        assert!(design.get_constraint(constraint_id).is_some());
    }

    #[test]
    fn test_validation() {
        let mut design = Design::new();

        let module_id = design.create_module("M", ModuleKind::Module);
        let field_id = design.add_field(module_id, "f", FieldKind::signal());

        assert!(design.validate_module_id(module_id).is_ok());
        assert!(design.validate_field_id(field_id).is_ok());

        // Invalid IDs
        assert!(design.validate_module_id(ModuleId::new(999)).is_err());
        assert!(design.validate_field_id(FieldId::new(999)).is_err());
    }

    #[test]
    fn test_entry_module() {
        let mut design = Design::new();

        let module_id = design.create_module("App", ModuleKind::Module);
        assert!(design.entry_module().is_none());

        design.set_entry_module(module_id);
        assert_eq!(design.entry_module(), Some(module_id));
    }

    #[test]
    fn test_resistor_example() {
        // Model the Resistor module from stdlib
        let mut design = Design::new();

        // Create Resistor module
        let resistor_id = design.create_module("Resistor", ModuleKind::Module);

        // Add parameters
        design.add_field(
            resistor_id,
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );
        design.add_field(
            resistor_id,
            "max_power",
            FieldKind::parameter_with_unit("W"),
        );
        design.add_field(
            resistor_id,
            "max_voltage",
            FieldKind::parameter_with_unit("V"),
        );

        // Add unnamed pins (array of 2)
        design.add_field(
            resistor_id,
            "unnamed",
            FieldKind::instance_array(QualifiedName::simple("Electrical"), 2),
        );

        let resistor = design.get_module(resistor_id).unwrap();
        assert_eq!(resistor.fields.len(), 4);
    }
}
