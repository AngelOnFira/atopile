//! Netlist data structures.
//!
//! This module defines the generic netlist format that can be exported
//! to various output formats (KiCad, etc.).

use std::collections::HashMap;

use ato_ir::{Design, EndpointKind, FieldId, FieldPathPart, ModuleId};
use serde::{Deserialize, Serialize};

use crate::kicad_library::LibraryMapper;
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

/// Extracted footprint info from module parameters.
#[derive(Debug, Default)]
struct FootprintInfo {
    footprint: Option<String>,
    #[allow(dead_code)]
    package: Option<String>,
    lcsc: Option<String>,
}

/// Builder for creating netlists from IR designs.
pub struct NetlistBuilder<'a> {
    design: &'a Design,
    /// Map from module ID to component reference (for module definitions that are components).
    module_to_ref: HashMap<ModuleId, String>,
    /// Map from instance field ID to component reference.
    instance_to_ref: HashMap<FieldId, String>,
    /// For array instances, map from (instance_field_id, array_index) to component reference.
    array_instance_to_ref: HashMap<(FieldId, u32), String>,
    /// Reference counters for generating unique designators.
    ref_counters: HashMap<String, u32>,
    /// The netlist being built.
    netlist: Netlist,
    /// Solved parameter values: maps field path strings to resolved value strings.
    solved_values: HashMap<String, String>,
}

impl<'a> NetlistBuilder<'a> {
    /// Create a new netlist builder.
    pub fn new(design: &'a Design) -> Self {
        Self {
            design,
            module_to_ref: HashMap::new(),
            instance_to_ref: HashMap::new(),
            array_instance_to_ref: HashMap::new(),
            ref_counters: HashMap::new(),
            netlist: Netlist::new(),
            solved_values: HashMap::new(),
        }
    }

    /// Create a new netlist builder with solved parameter values.
    pub fn with_solved_values(mut self, solved_values: HashMap<String, String>) -> Self {
        self.solved_values = solved_values;
        self
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
    ///
    /// First checks for `has_designator_prefix::prefix<value="X">` trait,
    /// then falls back to name-based heuristics.
    fn get_designator_prefix(&self, module_id: ModuleId) -> String {
        if let Some(module) = self.design.get_module(module_id) {
            // First, check for has_designator_prefix trait
            for trait_ref in &module.traits {
                if trait_ref.name.name() == "has_designator_prefix" {
                    // Check for constructor "prefix" and get the "value" argument
                    if trait_ref.constructor.as_deref() == Some("prefix") {
                        if let Some(value) = trait_ref.get_string_arg("value") {
                            return value.to_string();
                        }
                    }
                }
            }

            // Fall back to name-based heuristics
            let name = module.name.to_lowercase();
            if name.contains("resistor") {
                return "R".to_string();
            } else if name.contains("capacitor") {
                return "C".to_string();
            } else if name.contains("inductor") {
                return "L".to_string();
            } else if name.contains("diode") || name.contains("led") {
                return "D".to_string();
            } else if name.contains("transistor") || name.contains("mosfet") {
                return "Q".to_string();
            } else if name.contains("connector") {
                return "J".to_string();
            } else if name.contains("crystal") {
                return "Y".to_string();
            }
        }
        // Default prefix for ICs and other components
        "U".to_string()
    }

    /// Collect all components from the design.
    ///
    /// This supports two patterns:
    /// 1. Instance fields with resolved_type - each instance becomes a component
    /// 2. Modules with pins that aren't used as types - legacy/simple component model
    fn collect_components(&mut self) -> Result<(), ExportError> {
        let mapper = LibraryMapper::new();
        let mut modules_used_as_types = std::collections::HashSet::new();

        // First pass: find instance fields and create components for them
        for module in self.design.modules() {
            for &field_id in &module.fields {
                if let Some(field) = self.design.get_field(field_id) {
                    if let ato_ir::FieldKind::Instance {
                        resolved_type: Some(type_module_id),
                        count,
                        ..
                    } = &field.kind
                    {
                        modules_used_as_types.insert(*type_module_id);

                        if let Some(target_module) = self.design.get_module(*type_module_id) {
                            let has_pins = target_module.fields.iter().any(|&fid| {
                                self.design
                                    .get_field(fid)
                                    .map(|f| f.is_pin())
                                    .unwrap_or(false)
                            });

                            if has_pins {
                                let footprint_info =
                                    self.extract_footprint_info(*type_module_id, &mapper);

                                // Expand array instances
                                let instance_count = match count {
                                    Some(n) if *n > 1 => *n,
                                    _ => 1,
                                };

                                for idx in 0..instance_count {
                                    let prefix = self.get_designator_prefix(*type_module_id);
                                    let reference = self.generate_reference(&prefix);
                                    let value =
                                        self.get_instance_value(&field.name, *type_module_id);

                                    let mut component =
                                        NetlistComponent::new(&reference, &value)
                                            .with_property("module", target_module.name.clone())
                                            .with_property("instance", field.name.clone());

                                    if let Some(ref fp) = footprint_info.footprint {
                                        component = component.with_footprint(fp.clone());
                                    }
                                    if let Some(ref lcsc) = footprint_info.lcsc {
                                        component =
                                            component.with_property("lcsc", lcsc.clone());
                                    }

                                    if instance_count > 1 {
                                        component = component
                                            .with_property("array_index", idx.to_string());
                                        self.array_instance_to_ref
                                            .insert((field_id, idx), reference.clone());
                                    } else {
                                        self.instance_to_ref
                                            .insert(field_id, reference.clone());
                                    }

                                    self.netlist.add_component(component);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Second pass: for modules with pins that aren't used as instance types
        for module in self.design.modules() {
            if modules_used_as_types.contains(&module.id) {
                continue;
            }
            if module.is_interface() {
                continue;
            }

            let has_pins = module.fields.iter().any(|&field_id| {
                self.design
                    .get_field(field_id)
                    .map(|f| f.is_pin())
                    .unwrap_or(false)
            });

            if has_pins {
                let prefix = self.get_designator_prefix(module.id);
                let reference = self.generate_reference(&prefix);
                let value = self.get_module_value(module.id);
                let footprint_info = self.extract_footprint_info(module.id, &mapper);

                let mut component = NetlistComponent::new(&reference, value)
                    .with_property("module", module.name.clone());

                if let Some(ref fp) = footprint_info.footprint {
                    component = component.with_footprint(fp.clone());
                }
                if let Some(ref lcsc) = footprint_info.lcsc {
                    component = component.with_property("lcsc", lcsc.clone());
                }

                self.module_to_ref.insert(module.id, reference);
                self.netlist.add_component(component);
            }
        }

        Ok(())
    }

    /// Get the value string for a module (e.g., "10k" for a resistor).
    fn get_module_value(&self, module_id: ModuleId) -> String {
        if let Some(module) = self.design.get_module(module_id) {
            for param_name in &["resistance", "capacitance", "inductance", "value"] {
                if let Some(field_id) = module.get_field(param_name) {
                    if let Some(field) = self.design.get_field(field_id) {
                        if field.is_parameter() {
                            // Check solved_values before falling back to placeholder
                            if let Some(solved) = self.solved_values.get(*param_name) {
                                return solved.clone();
                            }
                            return format!("${}", param_name);
                        }
                    }
                }
            }
            return module.name.clone();
        }
        "?".to_string()
    }

    /// Get the value string for an instance, checking solved_values first.
    fn get_instance_value(&self, instance_name: &str, type_module_id: ModuleId) -> String {
        if let Some(module) = self.design.get_module(type_module_id) {
            for param_name in &["resistance", "capacitance", "inductance", "value"] {
                if let Some(field_id) = module.get_field(param_name) {
                    if let Some(field) = self.design.get_field(field_id) {
                        if field.is_parameter() {
                            let qualified_key = format!("{}.{}", instance_name, param_name);
                            if let Some(solved) = self.solved_values.get(&qualified_key) {
                                return solved.clone();
                            }
                            if let Some(solved) = self.solved_values.get(*param_name) {
                                return solved.clone();
                            }
                            return format!("${}", param_name);
                        }
                    }
                }
            }
            return module.name.clone();
        }
        "?".to_string()
    }

    /// Extract footprint information from a module's fields.
    fn extract_footprint_info(
        &self,
        module_id: ModuleId,
        mapper: &LibraryMapper,
    ) -> FootprintInfo {
        let mut info = FootprintInfo::default();

        if let Some(module) = self.design.get_module(module_id) {
            if let Some(field_id) = module.get_field("footprint") {
                if let Some(field) = self.design.get_field(field_id) {
                    if field.is_parameter() {
                        if let Some(solved) = self.solved_values.get("footprint") {
                            info.footprint = Some(solved.clone());
                        }
                    }
                }
            }

            if info.footprint.is_none() {
                if let Some(field_id) = module.get_field("package") {
                    if let Some(field) = self.design.get_field(field_id) {
                        if field.is_parameter() {
                            if let Some(solved) = self.solved_values.get("package") {
                                let prefix = self.get_designator_prefix(module_id);
                                let fp_name = mapper.get_footprint_name(
                                    &format!("{}1", prefix),
                                    Some(solved),
                                );
                                info.footprint = Some(fp_name);
                                info.package = Some(solved.clone());
                            }
                        }
                    }
                }
            }

            if let Some(field_id) = module.get_field("lcsc") {
                if let Some(field) = self.design.get_field(field_id) {
                    if field.is_parameter() {
                        if let Some(solved) = self.solved_values.get("lcsc") {
                            info.lcsc = Some(solved.clone());
                        }
                    }
                }
            }
        }

        info
    }

    /// Build nets from the connection graph and connections.
    fn build_nets(&mut self) -> Result<(), ExportError> {
        // Build reverse map: module_id -> list of (instance_field_id, ref_name)
        let mut module_to_instances: HashMap<ModuleId, Vec<(FieldId, String)>> = HashMap::new();
        for (&field_id, ref_name) in &self.instance_to_ref {
            if let Some(field) = self.design.get_field(field_id) {
                if let ato_ir::FieldKind::Instance {
                    resolved_type: Some(type_id),
                    ..
                } = &field.kind
                {
                    module_to_instances
                        .entry(*type_id)
                        .or_default()
                        .push((field_id, ref_name.clone()));
                }
            }
        }
        for (&(field_id, _idx), ref_name) in &self.array_instance_to_ref {
            if let Some(field) = self.design.get_field(field_id) {
                if let ato_ir::FieldKind::Instance {
                    resolved_type: Some(type_id),
                    ..
                } = &field.kind
                {
                    module_to_instances
                        .entry(*type_id)
                        .or_default()
                        .push((field_id, ref_name.clone()));
                }
            }
        }

        // Build instance_name -> component ref map
        let mut instance_name_to_ref: HashMap<String, String> = HashMap::new();
        for (&field_id, ref_name) in &self.instance_to_ref {
            if let Some(field) = self.design.get_field(field_id) {
                instance_name_to_ref.insert(field.name.clone(), ref_name.clone());
            }
        }
        // Also add array instance entries with indexed names (e.g., "resistors[0]")
        for (&(field_id, idx), ref_name) in &self.array_instance_to_ref {
            if let Some(field) = self.design.get_field(field_id) {
                let indexed_name = format!("{}[{}]", field.name, idx);
                instance_name_to_ref.insert(indexed_name, ref_name.clone());
            }
        }

        // Connection-based net building
        let mut net_groups: HashMap<String, Vec<NetNode>> = HashMap::new();
        let mut net_counter = 0u32;

        for connection in self.design.connections() {
            if connection.is_simple() {
                if let (Some(left), Some(right)) = (connection.left(), connection.right()) {
                    if let (Some(left_id), Some(right_id)) = (left.resolved, right.resolved) {
                        let left_node =
                            self.resolve_endpoint_to_node(left, left_id, &instance_name_to_ref);
                        let right_node =
                            self.resolve_endpoint_to_node(right, right_id, &instance_name_to_ref);

                        if let (Some(ln), Some(rn)) = (left_node, right_node) {
                            net_counter += 1;
                            let net_key = format!("conn_{}", net_counter);
                            let nodes = net_groups.entry(net_key).or_default();
                            if !nodes.contains(&ln) {
                                nodes.push(ln);
                            }
                            if !nodes.contains(&rn) {
                                nodes.push(rn);
                            }
                        }
                    }
                }
            } else {
                for window in connection.endpoints.windows(2) {
                    let (ep_a, ep_b) = (&window[0], &window[1]);
                    if let (Some(a_id), Some(b_id)) = (ep_a.resolved, ep_b.resolved) {
                        let node_a =
                            self.resolve_endpoint_to_node(ep_a, a_id, &instance_name_to_ref);
                        let node_b =
                            self.resolve_endpoint_to_node(ep_b, b_id, &instance_name_to_ref);

                        if let (Some(na), Some(nb)) = (node_a, node_b) {
                            net_counter += 1;
                            let net_key = format!("conn_{}", net_counter);
                            let nodes = net_groups.entry(net_key).or_default();
                            if !nodes.contains(&na) {
                                nodes.push(na);
                            }
                            if !nodes.contains(&nb) {
                                nodes.push(nb);
                            }
                        }
                    }
                }
            }
        }

        // Graph-based net building for module-based (non-instance) components
        let mut visited_fields: std::collections::HashSet<FieldId> =
            std::collections::HashSet::new();

        for field in self.design.fields() {
            if !field.is_connectable() || visited_fields.contains(&field.id) {
                continue;
            }

            let connected = self.design.connection_graph().connected_component(field.id);

            if connected.len() > 1 {
                net_counter += 1;
                let net_name = self.generate_net_name(&connected, net_counter);
                let mut net = Net::new(&net_name);

                for &field_id in &connected {
                    visited_fields.insert(field_id);

                    if let Some(field) = self.design.get_field(field_id) {
                        if field.is_pin() {
                            if let Some(ref_name) = self.module_to_ref.get(&field.parent) {
                                let node = NetNode::new(ref_name.clone(), &field.name);
                                if !net.nodes.contains(&node) {
                                    net.add_node(node);
                                }
                            } else if let Some(instances) =
                                module_to_instances.get(&field.parent)
                            {
                                if instances.len() == 1 {
                                    let ref_name = &instances[0].1;
                                    let node = NetNode::new(ref_name.clone(), &field.name);
                                    if !net.nodes.contains(&node) {
                                        net.add_node(node);
                                    }
                                }
                            }
                        }
                    }
                }

                if net.nodes.len() > 1 {
                    self.netlist.add_net(net);
                }
            } else {
                visited_fields.insert(field.id);
            }
        }

        // Convert connection-based nets
        for (_, nodes) in net_groups {
            if nodes.len() > 1 {
                net_counter += 1;
                let mut net = Net::new(format!("Net{}", net_counter));
                for node in nodes {
                    net.add_node(node);
                }
                self.netlist.add_net(net);
            }
        }

        Ok(())
    }

    /// Resolve a connection endpoint to a NetNode.
    ///
    /// Handles both simple instance paths (e.g., `r1.p1`) and array instance
    /// paths (e.g., `resistors[0].p1`).
    fn resolve_endpoint_to_node(
        &self,
        endpoint: &ato_ir::ConnectionEndpoint,
        resolved_field_id: FieldId,
        instance_name_to_ref: &HashMap<String, String>,
    ) -> Option<NetNode> {
        if let Some(field) = self.design.get_field(resolved_field_id) {
            if !field.is_pin() {
                return None;
            }

            // Check module_to_ref first (for non-instance components)
            if let Some(ref_name) = self.module_to_ref.get(&field.parent) {
                return Some(NetNode::new(ref_name.clone(), &field.name));
            }

            // Try to resolve via endpoint path (for instance components)
            if let EndpointKind::FieldRef(path) = &endpoint.kind {
                let instance_key = self.extract_instance_key_from_path(path);
                if let Some(ref_name) = instance_name_to_ref.get(&instance_key) {
                    return Some(NetNode::new(ref_name.clone(), &field.name));
                }
            }
        }
        None
    }

    /// Extract an instance lookup key from a field path.
    ///
    /// For simple paths like `[Name("r1"), Name("p1")]`, returns `"r1"`.
    /// For array paths like `[Name("resistors"), Index(0), Name("p1")]`, returns `"resistors[0]"`.
    fn extract_instance_key_from_path(&self, path: &ato_ir::FieldPath) -> String {
        match path.parts.as_slice() {
            [FieldPathPart::Name(name), FieldPathPart::Index(idx), ..] => {
                format!("{}[{}]", name, idx)
            }
            [FieldPathPart::Name(name), ..] => name.clone(),
            _ => String::new(),
        }
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
    use ato_ir::{
        ConnectionEndpoint, FieldKind, FieldPath, FieldPathPart, ModuleKind, QualifiedName,
        TemplateArgValue, TraitRef,
    };

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

    #[test]
    fn test_has_designator_prefix_trait() {
        // Test that has_designator_prefix::prefix<value="X"> trait overrides default prefix
        let mut design = Design::new();

        // Create a CustomComponent with has_designator_prefix trait
        let component_id = design.create_module("CustomComponent", ModuleKind::Component);
        design.add_field(component_id, "p1", FieldKind::pin("1"));
        design.add_field(component_id, "p2", FieldKind::pin("2"));

        // Add the has_designator_prefix::prefix<value="X"> trait
        if let Some(module) = design.get_module_mut(component_id) {
            let trait_ref = TraitRef::new(QualifiedName::simple("has_designator_prefix"))
                .with_constructor("prefix")
                .with_arg("value", TemplateArgValue::String("X".to_string()));
            module.add_trait(trait_ref);
        }

        // Create App with an instance of CustomComponent
        let app_id = design.create_module("App", ModuleKind::Module);
        let instance_kind = FieldKind::Instance {
            type_ref: QualifiedName::simple("CustomComponent"),
            count: None,
            resolved_type: Some(component_id),
        };
        design.add_field(app_id, "custom1", instance_kind);

        // Build netlist
        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        // Should have 1 component with "X" prefix
        assert_eq!(netlist.component_count(), 1);
        assert!(
            netlist.get_component("X1").is_some(),
            "Component should use 'X' prefix from trait, got components: {:?}",
            netlist.components.iter().map(|c| &c.reference).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_designator_prefix_fallback_to_name_heuristics() {
        // Test that modules without has_designator_prefix trait use name-based heuristics
        let mut design = Design::new();

        // Create a Resistor module (no trait, should use "R" prefix based on name)
        let resistor_id = design.create_module("MyResistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));

        // Create App with an instance
        let app_id = design.create_module("App", ModuleKind::Module);
        let instance_kind = FieldKind::Instance {
            type_ref: QualifiedName::simple("MyResistor"),
            count: None,
            resolved_type: Some(resistor_id),
        };
        design.add_field(app_id, "r1", instance_kind);

        // Build netlist
        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        // Should have 1 component with "R" prefix (from name heuristics)
        assert_eq!(netlist.component_count(), 1);
        assert!(
            netlist.get_component("R1").is_some(),
            "Resistor should use 'R' prefix from name heuristics, got: {:?}",
            netlist.components.iter().map(|c| &c.reference).collect::<Vec<_>>()
        );
    }

    // =========================================================================
    // Gap 4 Tests - Netlist Component Extraction
    // These tests document the expected behavior that needs to be implemented.
    // =========================================================================

    #[test]
    fn test_gap4_netlist_extracts_instances_not_definitions() {
        // Create a design with:
        // - Resistor (module definition with pins) - should NOT be a component
        // - App containing r1 = new Resistor - r1 SHOULD be a component

        let mut design = Design::new();

        // Create Resistor module (definition only)
        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));
        design.add_field(resistor_id, "resistance", FieldKind::parameter_with_unit("ohm"));

        // Create App module with instance
        let app_id = design.create_module("App", ModuleKind::Module);

        // Add an instance field referencing Resistor
        // The instance should have resolved_type pointing to resistor_id
        let r1_kind = FieldKind::Instance {
            type_ref: ato_ir::QualifiedName::simple("Resistor"),
            count: None,
            resolved_type: Some(resistor_id),
        };
        design.add_field(app_id, "r1", r1_kind);

        // Build netlist
        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        // Should have exactly 1 component (r1), not 2
        // Currently fails because it creates a component for Resistor module definition
        assert_eq!(
            netlist.component_count(), 1,
            "Should only have instance components, not module definitions. Got: {:?}",
            netlist.components.iter().map(|c| &c.reference).collect::<Vec<_>>()
        );

        // The component should be named R1 (from r1 instance)
        assert!(
            netlist.get_component("R1").is_some(),
            "Instance r1 should become component R1"
        );
    }

    #[test]
    fn test_gap4_netlist_instance_connections() {
        let mut design = Design::new();

        // Create Resistor with pins
        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        let p1_id = design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        let p2_id = design.add_field(resistor_id, "p2", FieldKind::pin("2"));

        // Create App with two resistor instances
        let app_id = design.create_module("App", ModuleKind::Module);

        let r1_kind = FieldKind::Instance {
            type_ref: ato_ir::QualifiedName::simple("Resistor"),
            count: None,
            resolved_type: Some(resistor_id),
        };
        let r2_kind = FieldKind::Instance {
            type_ref: ato_ir::QualifiedName::simple("Resistor"),
            count: None,
            resolved_type: Some(resistor_id),
        };
        let _r1_id = design.add_field(app_id, "r1", r1_kind);
        let _r2_id = design.add_field(app_id, "r2", r2_kind);

        // For connection testing, we need to use resolved endpoints
        // The real semantic analysis would resolve instance.pin paths,
        // but here we directly create connections with resolved pin IDs
        // Note: In a real scenario, r1.p2 would be a unique field for that instance
        // For this test, we're just verifying component creation works
        let mut ep1 = ConnectionEndpoint::field(FieldPath::new(vec![
            FieldPathPart::Name("r1".to_string()),
            FieldPathPart::Name("p2".to_string()),
        ]));
        ep1.resolved = Some(p2_id);  // Simulate resolved endpoint

        let mut ep2 = ConnectionEndpoint::field(FieldPath::new(vec![
            FieldPathPart::Name("r2".to_string()),
            FieldPathPart::Name("p1".to_string()),
        ]));
        ep2.resolved = Some(p1_id);  // Simulate resolved endpoint

        design.add_connection(app_id, ep1, ep2);
        design.rebuild_connection_graph();

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        // Should have 2 components (r1 and r2)
        assert_eq!(netlist.component_count(), 2, "Should have 2 instance components");

        // Now also verify net building works with instance paths
        let has_connection = netlist.nets.iter().any(|net| {
            let has_r1_p2 = net
                .nodes
                .iter()
                .any(|n| n.component == "R1" && n.pin == "p2");
            let has_r2_p1 = net
                .nodes
                .iter()
                .any(|n| n.component == "R2" && n.pin == "p1");
            has_r1_p2 && has_r2_p1
        });

        assert!(
            has_connection,
            "Should have a net connecting R1.p2 to R2.p1, got nets: {:?}",
            netlist.nets
        );
    }

    #[test]
    fn test_array_instance_expansion() {
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));

        let app_id = design.create_module("App", ModuleKind::Module);
        let array_kind = FieldKind::Instance {
            type_ref: QualifiedName::simple("Resistor"),
            count: Some(3),
            resolved_type: Some(resistor_id),
        };
        design.add_field(app_id, "resistors", array_kind);

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        assert_eq!(
            netlist.component_count(),
            3,
            "Array of 3 resistors should create 3 components, got: {:?}",
            netlist
                .components
                .iter()
                .map(|c| &c.reference)
                .collect::<Vec<_>>()
        );

        assert!(netlist.get_component("R1").is_some());
        assert!(netlist.get_component("R2").is_some());
        assert!(netlist.get_component("R3").is_some());

        for (i, ref_name) in ["R1", "R2", "R3"].iter().enumerate() {
            let comp = netlist.get_component(ref_name).unwrap();
            assert_eq!(
                comp.properties.get("array_index"),
                Some(&i.to_string()),
            );
        }
    }

    #[test]
    fn test_solved_values() {
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));
        design.add_field(
            resistor_id,
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );

        let app_id = design.create_module("App", ModuleKind::Module);
        let r1_kind = FieldKind::Instance {
            type_ref: QualifiedName::simple("Resistor"),
            count: None,
            resolved_type: Some(resistor_id),
        };
        design.add_field(app_id, "r1", r1_kind);

        let mut solved = HashMap::new();
        solved.insert("r1.resistance".to_string(), "10kohm".to_string());

        let builder = NetlistBuilder::new(&design).with_solved_values(solved);
        let netlist = builder.build().unwrap();

        assert_eq!(netlist.component_count(), 1);
        let comp = netlist.get_component("R1").unwrap();
        assert_eq!(comp.value, "10kohm");
    }

    #[test]
    fn test_footprint_propagation() {
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));
        design.add_field(
            resistor_id,
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );
        design.add_field(resistor_id, "package", FieldKind::parameter());
        design.add_field(resistor_id, "lcsc", FieldKind::parameter());

        let app_id = design.create_module("App", ModuleKind::Module);
        let r1_kind = FieldKind::Instance {
            type_ref: QualifiedName::simple("Resistor"),
            count: None,
            resolved_type: Some(resistor_id),
        };
        design.add_field(app_id, "r1", r1_kind);

        let mut solved = HashMap::new();
        solved.insert("package".to_string(), "0402".to_string());
        solved.insert("lcsc".to_string(), "C25076".to_string());

        let builder = NetlistBuilder::new(&design).with_solved_values(solved);
        let netlist = builder.build().unwrap();

        let comp = netlist.get_component("R1").unwrap();
        assert!(comp.footprint.is_some());
        assert!(comp.footprint.as_ref().unwrap().contains("R_0402_1005Metric"));
        assert_eq!(comp.properties.get("lcsc"), Some(&"C25076".to_string()));
    }

    #[test]
    fn test_module_value_uses_solved_values() {
        // Test that get_module_value uses solved_values instead of placeholders
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));
        design.add_field(
            resistor_id,
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );

        // Module without an instance (second pass picks it up)
        let mut solved = HashMap::new();
        solved.insert("resistance".to_string(), "4.7kohm".to_string());

        let builder = NetlistBuilder::new(&design).with_solved_values(solved);
        let netlist = builder.build().unwrap();

        assert_eq!(netlist.component_count(), 1);
        let comp = netlist.get_component("R1").unwrap();
        assert_eq!(
            comp.value, "4.7kohm",
            "Module value should use solved_values, got: {}",
            comp.value
        );
    }

    #[test]
    fn test_multiple_instances_same_type() {
        // Test that multiple instances of the same type each get unique components
        let mut design = Design::new();

        let cap_id = design.create_module("Capacitor", ModuleKind::Module);
        design.add_field(cap_id, "p1", FieldKind::pin("1"));
        design.add_field(cap_id, "p2", FieldKind::pin("2"));
        design.add_field(
            cap_id,
            "capacitance",
            FieldKind::parameter_with_unit("F"),
        );

        let app_id = design.create_module("App", ModuleKind::Module);
        for name in ["c1", "c2", "c3"] {
            let kind = FieldKind::Instance {
                type_ref: QualifiedName::simple("Capacitor"),
                count: None,
                resolved_type: Some(cap_id),
            };
            design.add_field(app_id, name, kind);
        }

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        assert_eq!(
            netlist.component_count(),
            3,
            "Should have 3 separate capacitor components"
        );
        assert!(netlist.get_component("C1").is_some());
        assert!(netlist.get_component("C2").is_some());
        assert!(netlist.get_component("C3").is_some());
    }

    #[test]
    fn test_mixed_component_types() {
        // Test a design with multiple different component types
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));

        let cap_id = design.create_module("Capacitor", ModuleKind::Module);
        design.add_field(cap_id, "p1", FieldKind::pin("1"));
        design.add_field(cap_id, "p2", FieldKind::pin("2"));

        let led_id = design.create_module("LED", ModuleKind::Module);
        design.add_field(led_id, "anode", FieldKind::pin("1"));
        design.add_field(led_id, "cathode", FieldKind::pin("2"));

        let app_id = design.create_module("App", ModuleKind::Module);
        design.add_field(
            app_id,
            "r1",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("Resistor"),
                count: None,
                resolved_type: Some(resistor_id),
            },
        );
        design.add_field(
            app_id,
            "c1",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("Capacitor"),
                count: None,
                resolved_type: Some(cap_id),
            },
        );
        design.add_field(
            app_id,
            "led1",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("LED"),
                count: None,
                resolved_type: Some(led_id),
            },
        );

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        assert_eq!(netlist.component_count(), 3);
        // R1 for Resistor, C1 for Capacitor, D1 for LED (name heuristic)
        assert!(
            netlist.get_component("R1").is_some(),
            "Expected R1, got: {:?}",
            netlist.components.iter().map(|c| &c.reference).collect::<Vec<_>>()
        );
        assert!(netlist.get_component("C1").is_some());
        assert!(netlist.get_component("D1").is_some());
    }

    #[test]
    fn test_array_instance_with_connections() {
        // Test that array instances can be resolved in connections
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        let p1_id = design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        let p2_id = design.add_field(resistor_id, "p2", FieldKind::pin("2"));

        let app_id = design.create_module("App", ModuleKind::Module);
        let _arr_id = design.add_field(
            app_id,
            "resistors",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("Resistor"),
                count: Some(2),
                resolved_type: Some(resistor_id),
            },
        );

        // Connect resistors[0].p2 ~ resistors[1].p1
        let mut ep1 = ConnectionEndpoint::field(FieldPath::new(vec![
            FieldPathPart::Name("resistors".to_string()),
            FieldPathPart::Index(0),
            FieldPathPart::Name("p2".to_string()),
        ]));
        ep1.resolved = Some(p2_id);

        let mut ep2 = ConnectionEndpoint::field(FieldPath::new(vec![
            FieldPathPart::Name("resistors".to_string()),
            FieldPathPart::Index(1),
            FieldPathPart::Name("p1".to_string()),
        ]));
        ep2.resolved = Some(p1_id);

        design.add_connection(app_id, ep1, ep2);
        design.rebuild_connection_graph();

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        assert_eq!(netlist.component_count(), 2);

        // Check that the connection was resolved
        let has_connection = netlist.nets.iter().any(|net| {
            let has_r1_p2 = net.nodes.iter().any(|n| n.component == "R1" && n.pin == "p2");
            let has_r2_p1 = net.nodes.iter().any(|n| n.component == "R2" && n.pin == "p1");
            has_r1_p2 && has_r2_p1
        });

        assert!(
            has_connection,
            "Should have a net connecting R1.p2 to R2.p1 (array instances), got nets: {:?}",
            netlist.nets
        );
    }

    #[test]
    fn test_instance_solved_values_per_instance() {
        // Test that different instances can have different solved values
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));
        design.add_field(
            resistor_id,
            "resistance",
            FieldKind::parameter_with_unit("ohm"),
        );

        let app_id = design.create_module("App", ModuleKind::Module);
        design.add_field(
            app_id,
            "r1",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("Resistor"),
                count: None,
                resolved_type: Some(resistor_id),
            },
        );
        design.add_field(
            app_id,
            "r2",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("Resistor"),
                count: None,
                resolved_type: Some(resistor_id),
            },
        );

        let mut solved = HashMap::new();
        solved.insert("r1.resistance".to_string(), "10kohm".to_string());
        solved.insert("r2.resistance".to_string(), "47kohm".to_string());

        let builder = NetlistBuilder::new(&design).with_solved_values(solved);
        let netlist = builder.build().unwrap();

        assert_eq!(netlist.component_count(), 2);

        let r1 = netlist.get_component("R1").unwrap();
        let r2 = netlist.get_component("R2").unwrap();

        assert_eq!(r1.value, "10kohm", "R1 should have its own solved value");
        assert_eq!(r2.value, "47kohm", "R2 should have its own solved value");
    }

    #[test]
    fn test_footprint_from_explicit_field() {
        // Test that explicit footprint field is used when available
        let mut design = Design::new();

        let ic_id = design.create_module("MyIC", ModuleKind::Module);
        design.add_field(ic_id, "p1", FieldKind::pin("1"));
        design.add_field(ic_id, "p2", FieldKind::pin("2"));
        design.add_field(ic_id, "footprint", FieldKind::parameter());

        let app_id = design.create_module("App", ModuleKind::Module);
        design.add_field(
            app_id,
            "u1",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("MyIC"),
                count: None,
                resolved_type: Some(ic_id),
            },
        );

        let mut solved = HashMap::new();
        solved.insert(
            "footprint".to_string(),
            "Package_QFP:LQFP-48_7x7mm_P0.5mm".to_string(),
        );

        let builder = NetlistBuilder::new(&design).with_solved_values(solved);
        let netlist = builder.build().unwrap();

        let comp = netlist.get_component("U1").unwrap();
        assert_eq!(
            comp.footprint.as_deref(),
            Some("Package_QFP:LQFP-48_7x7mm_P0.5mm"),
            "Should use explicit footprint from solved_values"
        );
    }

    #[test]
    fn test_empty_design_produces_empty_netlist() {
        let design = Design::new();
        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        assert_eq!(netlist.component_count(), 0);
        assert_eq!(netlist.net_count(), 0);
    }

    #[test]
    fn test_interface_modules_are_not_components() {
        // Interfaces should never become components
        let mut design = Design::new();

        let iface_id = design.create_module("Electrical", ModuleKind::Interface);
        design.add_field(iface_id, "line", FieldKind::pin("1"));

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        assert_eq!(
            netlist.component_count(),
            0,
            "Interface modules should not become components"
        );
    }
}
