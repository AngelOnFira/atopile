//! Netlist data structures.
//!
//! This module defines the generic netlist format that can be exported
//! to various output formats (KiCad, etc.).

use std::collections::HashMap;

use ato_ir::{Design, FieldId, ModuleId};
use serde::{Deserialize, Serialize};

use crate::ExportError;

/// A component in the netlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetlistComponent {
    /// Unique reference designator (e.g., "R1", "C1", "U1").
    pub reference: String,
    /// Component value (e.g., "10k", "100nF").
    pub value: String,
    /// Footprint name (e.g., "Resistor_SMD:R_0402_1005Metric").
    pub footprint: Option<String>,
    /// Additional properties.
    pub properties: HashMap<String, String>,
}

impl NetlistComponent {
    /// Create a new component.
    pub fn new(reference: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            reference: reference.into(),
            value: value.into(),
            footprint: None,
            properties: HashMap::new(),
        }
    }

    /// Set the footprint.
    pub fn with_footprint(mut self, footprint: impl Into<String>) -> Self {
        self.footprint = Some(footprint.into());
        self
    }

    /// Add a property.
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

/// A node in a net (component pin connection).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetNode {
    /// The component reference.
    pub component: String,
    /// The pin name.
    pub pin: String,
}

impl NetNode {
    /// Create a new net node.
    pub fn new(component: impl Into<String>, pin: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            pin: pin.into(),
        }
    }
}

/// A net (electrical connection) in the netlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Net {
    /// The net name.
    pub name: String,
    /// The nodes (component pins) connected to this net.
    pub nodes: Vec<NetNode>,
}

impl Net {
    /// Create a new net.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
        }
    }

    /// Add a node to the net.
    pub fn add_node(&mut self, node: NetNode) {
        self.nodes.push(node);
    }

    /// Add a node with component and pin.
    pub fn with_node(mut self, component: impl Into<String>, pin: impl Into<String>) -> Self {
        self.nodes.push(NetNode::new(component, pin));
        self
    }
}

/// A complete netlist.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Netlist {
    /// All components in the design.
    pub components: Vec<NetlistComponent>,
    /// All nets in the design.
    pub nets: Vec<Net>,
    /// Design metadata.
    pub metadata: NetlistMetadata,
}

/// Metadata about the netlist.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetlistMetadata {
    /// Design title.
    pub title: Option<String>,
    /// Design date.
    pub date: Option<String>,
    /// Tool that generated the netlist.
    pub tool: Option<String>,
    /// Source file.
    pub source: Option<String>,
}

impl Netlist {
    /// Create a new empty netlist.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a component.
    pub fn add_component(&mut self, component: NetlistComponent) {
        self.components.push(component);
    }

    /// Add a net.
    pub fn add_net(&mut self, net: Net) {
        self.nets.push(net);
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.metadata.title = Some(title.into());
        self
    }

    /// Set the tool name.
    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.metadata.tool = Some(tool.into());
        self
    }

    /// Get a component by reference.
    pub fn get_component(&self, reference: &str) -> Option<&NetlistComponent> {
        self.components.iter().find(|c| c.reference == reference)
    }

    /// Get a net by name.
    pub fn get_net(&self, name: &str) -> Option<&Net> {
        self.nets.iter().find(|n| n.name == name)
    }

    /// Get the number of components.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Get the number of nets.
    pub fn net_count(&self) -> usize {
        self.nets.len()
    }
}

/// Builder for creating netlists from IR designs.
pub struct NetlistBuilder<'a> {
    design: &'a Design,
    /// Map from module ID to component reference.
    module_to_ref: HashMap<ModuleId, String>,
    /// Reference counters for generating unique designators.
    ref_counters: HashMap<String, u32>,
    /// The netlist being built.
    netlist: Netlist,
}

impl<'a> NetlistBuilder<'a> {
    /// Create a new netlist builder.
    pub fn new(design: &'a Design) -> Self {
        Self {
            design,
            module_to_ref: HashMap::new(),
            ref_counters: HashMap::new(),
            netlist: Netlist::new(),
        }
    }

    /// Build the netlist from the design.
    pub fn build(mut self) -> Result<Netlist, ExportError> {
        // Set metadata
        self.netlist.metadata.tool = Some("ato-export".to_string());

        // First pass: collect all components (instances with footprints)
        self.collect_components()?;

        // Second pass: build nets from connections
        self.build_nets()?;

        Ok(self.netlist)
    }

    /// Generate a unique reference designator.
    fn generate_reference(&mut self, prefix: &str) -> String {
        let counter = self.ref_counters.entry(prefix.to_string()).or_insert(0);
        *counter += 1;
        format!("{}{}", prefix, counter)
    }

    /// Get the reference designator prefix for a module type.
    fn get_designator_prefix(&self, module_id: ModuleId) -> &'static str {
        // Look at the module name to determine prefix
        if let Some(module) = self.design.get_module(module_id) {
            let name = module.name.to_lowercase();
            if name.contains("resistor") {
                return "R";
            } else if name.contains("capacitor") {
                return "C";
            } else if name.contains("inductor") {
                return "L";
            } else if name.contains("diode") || name.contains("led") {
                return "D";
            } else if name.contains("transistor") || name.contains("mosfet") {
                return "Q";
            } else if name.contains("connector") {
                return "J";
            } else if name.contains("crystal") {
                return "Y";
            }
        }
        // Default prefix for ICs and other components
        "U"
    }

    /// Collect all components from the design.
    fn collect_components(&mut self) -> Result<(), ExportError> {
        // For each module that is an instance with a footprint, create a component
        for module in self.design.modules() {
            // Skip interface definitions (they're just type definitions)
            if module.is_interface() {
                continue;
            }

            // Check if this module is instantiated somewhere and has pins
            // For now, we create components for any module that has pins
            let has_pins = module.fields.iter().any(|&field_id| {
                self.design
                    .get_field(field_id)
                    .map(|f| f.is_pin())
                    .unwrap_or(false)
            });

            if has_pins {
                let prefix = self.get_designator_prefix(module.id);
                let reference = self.generate_reference(prefix);

                // Get value from parameters if available
                let value = self.get_module_value(module.id);

                let component = NetlistComponent::new(&reference, value)
                    .with_property("module", module.name.clone());

                self.module_to_ref.insert(module.id, reference);
                self.netlist.add_component(component);
            }
        }

        Ok(())
    }

    /// Get the value string for a module (e.g., "10k" for a resistor).
    fn get_module_value(&self, module_id: ModuleId) -> String {
        if let Some(module) = self.design.get_module(module_id) {
            // Look for common parameter names
            for param_name in &["resistance", "capacitance", "inductance", "value"] {
                if let Some(field_id) = module.get_field(param_name) {
                    if let Some(field) = self.design.get_field(field_id) {
                        if field.is_parameter() {
                            // Return the field name as placeholder
                            // In a real implementation, we'd get the solved value
                            return format!("${}", param_name);
                        }
                    }
                }
            }
            // Default to module name
            return module.name.clone();
        }
        "?".to_string()
    }

    /// Build nets from the connection graph.
    fn build_nets(&mut self) -> Result<(), ExportError> {
        // Rebuild connection graph if needed
        let mut visited_fields: std::collections::HashSet<FieldId> = std::collections::HashSet::new();
        let mut net_counter = 0u32;

        // Iterate through all fields and find connected components
        for field in self.design.fields() {
            if !field.is_connectable() || visited_fields.contains(&field.id) {
                continue;
            }

            // Get all fields in this net (connected component)
            let connected = self.design.connection_graph().connected_component(field.id);

            if connected.len() > 1 {
                // Create a net for this connected component
                net_counter += 1;
                let net_name = self.generate_net_name(&connected, net_counter);
                let mut net = Net::new(&net_name);

                for &field_id in &connected {
                    visited_fields.insert(field_id);

                    if let Some(field) = self.design.get_field(field_id) {
                        if field.is_pin() {
                            // Find the component this pin belongs to
                            if let Some(ref_name) = self.module_to_ref.get(&field.parent) {
                                net.add_node(NetNode::new(ref_name.clone(), &field.name));
                            }
                        }
                    }
                }

                // Only add nets with actual connections
                if net.nodes.len() > 1 {
                    self.netlist.add_net(net);
                }
            } else {
                visited_fields.insert(field.id);
            }
        }

        Ok(())
    }

    /// Generate a net name from connected fields.
    fn generate_net_name(&self, fields: &[FieldId], counter: u32) -> String {
        // Look for a signal with a meaningful name
        for &field_id in fields {
            if let Some(field) = self.design.get_field(field_id) {
                if field.is_signal() && !field.name.starts_with('_') {
                    return field.name.clone();
                }
            }
        }

        // Default to auto-generated name
        format!("Net{}", counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ato_ir::{ModuleKind, FieldKind, FieldPath, ConnectionEndpoint};

    #[test]
    fn test_netlist_component() {
        let comp = NetlistComponent::new("R1", "10k")
            .with_footprint("Resistor_SMD:R_0402_1005Metric")
            .with_property("tolerance", "1%");

        assert_eq!(comp.reference, "R1");
        assert_eq!(comp.value, "10k");
        assert_eq!(comp.footprint, Some("Resistor_SMD:R_0402_1005Metric".to_string()));
        assert_eq!(comp.properties.get("tolerance"), Some(&"1%".to_string()));
    }

    #[test]
    fn test_net() {
        let net = Net::new("VCC")
            .with_node("U1", "VDD")
            .with_node("C1", "1")
            .with_node("R1", "1");

        assert_eq!(net.name, "VCC");
        assert_eq!(net.nodes.len(), 3);
    }

    #[test]
    fn test_netlist() {
        let mut netlist = Netlist::new()
            .with_title("Test Design")
            .with_tool("ato-export");

        netlist.add_component(NetlistComponent::new("R1", "10k"));
        netlist.add_component(NetlistComponent::new("C1", "100nF"));

        let mut net = Net::new("Net1");
        net.add_node(NetNode::new("R1", "1"));
        net.add_node(NetNode::new("C1", "1"));
        netlist.add_net(net);

        assert_eq!(netlist.component_count(), 2);
        assert_eq!(netlist.net_count(), 1);
        assert!(netlist.get_component("R1").is_some());
        assert!(netlist.get_net("Net1").is_some());
    }

    #[test]
    fn test_netlist_builder_simple() {
        let mut design = Design::new();

        // Create a simple module with two pins
        let module_id = design.create_module("Resistor", ModuleKind::Module);
        let p1 = design.add_field(module_id, "p1", FieldKind::pin("1"));
        let p2 = design.add_field(module_id, "p2", FieldKind::pin("2"));
        design.add_field(module_id, "resistance", FieldKind::parameter_with_unit("ohm"));

        // Add a connection
        let mut ep1 = ConnectionEndpoint::field(FieldPath::simple("p1"));
        ep1.resolved = Some(p1);
        let mut ep2 = ConnectionEndpoint::field(FieldPath::simple("p2"));
        ep2.resolved = Some(p2);
        design.add_connection(module_id, ep1, ep2);

        // Rebuild connection graph
        design.rebuild_connection_graph();

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        // Should have one component (the Resistor)
        assert_eq!(netlist.component_count(), 1);
        assert!(netlist.get_component("R1").is_some());
    }

    #[test]
    fn test_reference_generation() {
        let design = Design::new();
        let mut builder = NetlistBuilder::new(&design);

        assert_eq!(builder.generate_reference("R"), "R1");
        assert_eq!(builder.generate_reference("R"), "R2");
        assert_eq!(builder.generate_reference("C"), "C1");
        assert_eq!(builder.generate_reference("R"), "R3");
    }
}
