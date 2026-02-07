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

/// Extracted footprint info from module traits and parameters.
#[derive(Debug, Default, Clone)]
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
    /// Optional entry module to scope component collection.
    entry_module: Option<ModuleId>,
}

/// Natural-order comparison for strings containing numbers.
/// Splits strings into alphabetic and numeric segments and compares them
/// so that "R2" < "R10" (unlike lexicographic order).
fn natord_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();

    loop {
        match (ai.peek(), bi.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(&ac), Some(&bc)) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    // Parse numeric segments
                    let mut an = 0u64;
                    while let Some(&c) = ai.peek() {
                        if c.is_ascii_digit() {
                            an = an * 10 + c.to_digit(10).unwrap() as u64;
                            ai.next();
                        } else {
                            break;
                        }
                    }
                    let mut bn = 0u64;
                    while let Some(&c) = bi.peek() {
                        if c.is_ascii_digit() {
                            bn = bn * 10 + c.to_digit(10).unwrap() as u64;
                            bi.next();
                        } else {
                            break;
                        }
                    }
                    match an.cmp(&bn) {
                        std::cmp::Ordering::Equal => continue,
                        ord => return ord,
                    }
                } else {
                    match ac.cmp(&bc) {
                        std::cmp::Ordering::Equal => {
                            ai.next();
                            bi.next();
                            continue;
                        }
                        ord => return ord,
                    }
                }
            }
        }
    }
}

/// Derive a descriptive, deterministic net name from sorted endpoint paths.
///
/// Uses the alphabetically first path as the base name, replacing dots with
/// underscores for readability (e.g., "power_3v3.hv" becomes "power_3v3_hv").
fn derive_net_name(sorted_paths: &[String]) -> String {
    if let Some(first) = sorted_paths.first() {
        // Use the first (alphabetically smallest) path, replacing dots with underscores
        first.replace('.', "_")
    } else {
        "Net".to_string()
    }
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
            entry_module: None,
        }
    }

    /// Create a new netlist builder with solved parameter values.
    pub fn with_solved_values(mut self, solved_values: HashMap<String, String>) -> Self {
        self.solved_values = solved_values;
        self
    }

    /// Set the entry module to scope component collection.
    pub fn with_entry_module(mut self, entry_module: ModuleId) -> Self {
        self.entry_module = Some(entry_module);
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

        // Sort components by reference designator for deterministic output
        self.netlist.components.sort_by(|a, b| {
            natord_cmp(&a.reference, &b.reference)
        });

        // Sort nets by name for deterministic output
        self.netlist.nets.sort_by(|a, b| a.name.cmp(&b.name));

        // Sort nodes within each net for deterministic output
        for net in &mut self.netlist.nets {
            net.nodes.sort_by(|a, b| {
                natord_cmp(&a.component, &b.component)
                    .then_with(|| a.pin.cmp(&b.pin))
            });
        }

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

    /// Check if a module is a physical component (has pins directly).
    fn module_has_pins(&self, module_id: ModuleId) -> bool {
        if let Some(module) = self.design.get_module(module_id) {
            module.fields.iter().any(|&fid| {
                self.design
                    .get_field(fid)
                    .map(|f| f.is_pin())
                    .unwrap_or(false)
            })
        } else {
            false
        }
    }

    /// Check if a module has instance sub-fields (is a container).
    fn module_has_instances(&self, module_id: ModuleId) -> bool {
        if let Some(module) = self.design.get_module(module_id) {
            module.fields.iter().any(|&fid| {
                self.design
                    .get_field(fid)
                    .map(|f| f.is_instance())
                    .unwrap_or(false)
            })
        } else {
            false
        }
    }

    /// Check if a module is a "physical component" - a leaf-level module that
    /// corresponds to a real part on the PCB. Detection criteria:
    /// 1. Has pins directly (package-level components)
    /// 2. Has `has_designator_prefix` trait (stdlib passives like Resistor, Capacitor)
    /// 3. Has a `package` parameter (passive components with solved package value)
    fn is_physical_component(&self, module_id: ModuleId) -> bool {
        if self.module_has_pins(module_id) {
            return true;
        }
        if let Some(module) = self.design.get_module(module_id) {
            // Check for has_designator_prefix trait - definitive marker for physical components
            for trait_ref in &module.traits {
                if trait_ref.name.name() == "has_designator_prefix" {
                    return true;
                }
            }
            // Check for `package` parameter (passives use this for part picking)
            for &fid in &module.fields {
                if let Some(field) = self.design.get_field(fid) {
                    if field.name == "package" && field.is_parameter() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get the designator prefix for a module, checking its package instance chain.
    fn get_designator_prefix_deep(&self, module_id: ModuleId) -> String {
        // First try from traits on this module
        if let Some(module) = self.design.get_module(module_id) {
            for trait_ref in &module.traits {
                if trait_ref.name.name() == "has_designator_prefix" {
                    if trait_ref.constructor.as_deref() == Some("prefix") {
                        if let Some(value) = trait_ref.get_string_arg("value") {
                            return value.to_string();
                        }
                    }
                }
            }
        }

        // Check the package instance chain for traits
        if let Some(module) = self.design.get_module(module_id) {
            for &fid in &module.fields {
                if let Some(field) = self.design.get_field(fid) {
                    if field.name == "package" {
                        if let ato_ir::FieldKind::Instance {
                            resolved_type: Some(pkg_id),
                            ..
                        } = &field.kind
                        {
                            // Check traits on the package module
                            if let Some(pkg_module) = self.design.get_module(*pkg_id) {
                                for trait_ref in &pkg_module.traits {
                                    if trait_ref.name.name() == "has_designator_prefix" {
                                        if trait_ref.constructor.as_deref() == Some("prefix") {
                                            if let Some(value) = trait_ref.get_string_arg("value") {
                                                return value.to_string();
                                            }
                                        }
                                    }
                                }
                                // Also check name heuristics on the package module
                                let pkg_name = pkg_module.name.to_lowercase();
                                if let Some(prefix) = Self::name_to_prefix(&pkg_name) {
                                    return prefix;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fall back to name-based heuristics on this module
        if let Some(module) = self.design.get_module(module_id) {
            let name = module.name.to_lowercase();
            if let Some(prefix) = Self::name_to_prefix(&name) {
                return prefix;
            }
        }

        "U".to_string()
    }

    /// Map a module name (lowercased) to a designator prefix using heuristics.
    fn name_to_prefix(name: &str) -> Option<String> {
        if name.contains("resistor") {
            Some("R".to_string())
        } else if name.contains("capacitor") {
            Some("C".to_string())
        } else if name.contains("inductor") {
            Some("L".to_string())
        } else if name.contains("diode") || name == "led" {
            Some("D".to_string())
        } else if name.contains("transistor") || name.contains("mosfet") {
            Some("Q".to_string())
        } else if name.contains("connector") || name.contains("molex") || name.contains("jst") {
            Some("J".to_string())
        } else if name.contains("crystal") {
            Some("Y".to_string())
        } else if name.contains("button") || name.contains("switch") || name.contains("skrpace")
            || name.contains("sktdlde")
        {
            Some("SW".to_string())
        } else if name.contains("sk6805") || name.contains("ws2812") || name.contains("sk6812")
            || name.contains("neopixel")
        {
            Some("LED".to_string())
        } else {
            None
        }
    }

    /// Collect all components from the design.
    ///
    /// Uses a two-phase approach for deterministic designator assignment:
    /// 1. Collect all pending components with their hierarchy paths and prefix info
    /// 2. Sort by (prefix, hierarchy_path) and assign designators sequentially
    ///
    /// This ensures the same component always gets the same designator regardless
    /// of HashMap iteration order or other nondeterminism.
    fn collect_components(&mut self) -> Result<(), ExportError> {
        let mapper = LibraryMapper::new();
        let mut modules_used_as_types = std::collections::HashSet::new();

        // Collect all modules used as instance types or super_types
        for module in self.design.modules() {
            if let Some(super_id) = module.super_type {
                modules_used_as_types.insert(super_id);
            }
            for &field_id in &module.fields {
                if let Some(field) = self.design.get_field(field_id) {
                    if let ato_ir::FieldKind::Instance {
                        resolved_type: Some(type_module_id),
                        ..
                    } = &field.kind
                    {
                        modules_used_as_types.insert(*type_module_id);
                    }
                }
            }
        }

        // Determine which modules to walk from
        let root_modules: Vec<ModuleId> = if let Some(entry) = self.entry_module {
            vec![entry]
        } else {
            self.design
                .modules()
                .iter()
                .filter(|m| !m.is_interface() && !modules_used_as_types.contains(&m.id))
                .map(|m| m.id)
                .collect()
        };

        // Phase 1: Collect pending components (without assigning designators yet)
        // Each entry: (instance_path, prefix, value, module_name, footprint_info, field_id, array_key, module_id_for_legacy)
        let mut pending: Vec<(
            String,                          // instance_path (sort key)
            String,                          // designator prefix
            String,                          // value
            String,                          // module_name
            FootprintInfo,                   // footprint info
            HashMap<String, String>,         // extra properties
            Option<FieldId>,                 // field_id for non-array instances
            Option<(FieldId, u32)>,          // array_key for array instances
            Option<ModuleId>,                // module_id for legacy second-pass
        )> = Vec::new();

        // Recursively collect from root modules
        for root_id in &root_modules {
            self.collect_components_pending(
                *root_id, "", &mapper, &mut std::collections::HashSet::new(), &mut pending,
            );
        }

        // Second pass: legacy modules with pins not used as instance types
        if self.entry_module.is_none() {
            // Track which module IDs were already collected
            let collected_module_ids: std::collections::HashSet<ModuleId> = pending
                .iter()
                .filter_map(|p| p.8)
                .collect();

            for module in self.design.modules() {
                if modules_used_as_types.contains(&module.id) {
                    continue;
                }
                if module.is_interface() {
                    continue;
                }
                if collected_module_ids.contains(&module.id) {
                    continue;
                }
                // Check if any pending entry already has this module via instance
                let already_has = pending.iter().any(|p| {
                    p.3 == module.name
                });
                if already_has {
                    continue;
                }

                let has_pins = self.module_has_pins(module.id);
                if has_pins {
                    let prefix = self.get_designator_prefix(module.id);
                    let value = self.get_module_value(module.id);
                    let footprint_info = self.extract_footprint_info(module.id, &mapper);
                    let mut extra = HashMap::new();
                    extra.insert("module".to_string(), module.name.clone());

                    pending.push((
                        module.name.clone(), // use module name as sort key for legacy
                        prefix,
                        value,
                        module.name.clone(),
                        footprint_info,
                        extra,
                        None,
                        None,
                        Some(module.id),
                    ));
                }
            }
        }

        // Phase 2: Sort by (prefix, instance_path) for deterministic ordering
        pending.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| natord_cmp(&a.0, &b.0))
        });

        // Phase 3: Assign designators and register components
        for (instance_path, prefix, value, _module_name, footprint_info, extra_props,
             field_id, array_key, module_id) in pending
        {
            let reference = self.generate_reference(&prefix);

            let mut component = NetlistComponent::new(&reference, &value);

            for (k, v) in &extra_props {
                component = component.with_property(k.clone(), v.clone());
            }

            // Add instance path property (for instance-based components)
            if field_id.is_some() || array_key.is_some() {
                component = component.with_property("instance", instance_path);
            }

            if let Some(ref fp) = footprint_info.footprint {
                component = component.with_footprint(fp.clone());
            }
            if let Some(ref lcsc) = footprint_info.lcsc {
                component = component.with_property("lcsc", lcsc.clone());
            }

            // Register the reference mapping
            if let Some(ak) = array_key {
                self.array_instance_to_ref.insert(ak, reference.clone());
            } else if let Some(fid) = field_id {
                self.instance_to_ref.insert(fid, reference.clone());
            } else if let Some(mid) = module_id {
                self.module_to_ref.insert(mid, reference.clone());
            }

            self.netlist.add_component(component);
        }

        Ok(())
    }

    /// Recursively collect pending components from a module's instance hierarchy.
    ///
    /// Similar to the old `collect_components_recursive` but collects into a
    /// pending list instead of directly assigning designators.
    fn collect_components_pending(
        &self,
        module_id: ModuleId,
        path_prefix: &str,
        mapper: &LibraryMapper,
        visited: &mut std::collections::HashSet<ModuleId>,
        pending: &mut Vec<(String, String, String, String, FootprintInfo, HashMap<String, String>, Option<FieldId>, Option<(FieldId, u32)>, Option<ModuleId>)>,
    ) {
        if !visited.insert(module_id) {
            return;
        }

        let module = match self.design.get_module(module_id) {
            Some(m) => m,
            None => return,
        };

        let instance_fields: Vec<(FieldId, String, Option<u32>, Option<ModuleId>)> = module
            .fields
            .iter()
            .filter_map(|&fid| {
                let field = self.design.get_field(fid)?;
                if let ato_ir::FieldKind::Instance {
                    count,
                    resolved_type,
                    ..
                } = &field.kind
                {
                    Some((fid, field.name.clone(), *count, *resolved_type))
                } else {
                    None
                }
            })
            .collect();

        for (field_id, field_name, count, resolved_type) in instance_fields {
            let type_module_id = match resolved_type {
                Some(id) => id,
                None => continue,
            };

            if let Some(target) = self.design.get_module(type_module_id) {
                if target.is_interface() {
                    continue;
                }
            }

            let instance_count = match count {
                Some(n) if n > 1 => n,
                _ => 1,
            };

            let is_leaf = self.is_physical_component(type_module_id);
            let is_container = self.module_has_instances(type_module_id);

            if is_leaf {
                let footprint_info = self.extract_footprint_info(type_module_id, mapper);
                let target_name = self.design.get_module(type_module_id)
                    .map(|m| m.name.clone())
                    .unwrap_or_default();

                for idx in 0..instance_count {
                    let prefix = self.get_designator_prefix_deep(type_module_id);

                    let instance_path = if instance_count > 1 {
                        if path_prefix.is_empty() {
                            format!("{}[{}]", field_name, idx)
                        } else {
                            format!("{}.{}[{}]", path_prefix, field_name, idx)
                        }
                    } else if path_prefix.is_empty() {
                        field_name.clone()
                    } else {
                        format!("{}.{}", path_prefix, field_name)
                    };

                    let value = self.get_instance_value(&instance_path, type_module_id);

                    let mut extra = HashMap::new();
                    extra.insert("module".to_string(), target_name.clone());
                    if instance_count > 1 {
                        extra.insert("array_index".to_string(), idx.to_string());
                    }

                    let (fid_opt, ak_opt) = if instance_count > 1 {
                        (None, Some((field_id, idx)))
                    } else {
                        (Some(field_id), None)
                    };

                    pending.push((
                        instance_path,
                        prefix,
                        value,
                        target_name.clone(),
                        footprint_info.clone(),
                        extra,
                        fid_opt,
                        ak_opt,
                        None,
                    ));
                }
            } else if is_container {
                for idx in 0..instance_count {
                    let sub_prefix = if instance_count > 1 {
                        if path_prefix.is_empty() {
                            format!("{}[{}]", field_name, idx)
                        } else {
                            format!("{}.{}[{}]", path_prefix, field_name, idx)
                        }
                    } else if path_prefix.is_empty() {
                        field_name.clone()
                    } else {
                        format!("{}.{}", path_prefix, field_name)
                    };

                    let mut branch_visited = std::collections::HashSet::new();
                    self.collect_components_pending(
                        type_module_id,
                        &sub_prefix,
                        mapper,
                        &mut branch_visited,
                        pending,
                    );
                }
            }
        }

        visited.remove(&module_id);
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

    /// Extract footprint information from a module's traits and fields.
    fn extract_footprint_info(
        &self,
        module_id: ModuleId,
        mapper: &LibraryMapper,
    ) -> FootprintInfo {
        let mut info = FootprintInfo::default();

        if let Some(module) = self.design.get_module(module_id) {
            // Check traits on this module directly
            self.extract_traits_from_chain(module_id, &mut info);

            // Check the `package` field's target module for traits
            if let Some(pkg_field_id) = module.get_field("package") {
                if let Some(field) = self.design.get_field(pkg_field_id) {
                    match &field.kind {
                        ato_ir::FieldKind::Instance {
                            resolved_type: Some(pkg_module_id), ..
                        } => {
                            self.extract_traits_from_chain(*pkg_module_id, &mut info);
                        }
                        ato_ir::FieldKind::Parameter { .. } => {
                            if let Some(solved) = self.solved_values.get("package") {
                                let prefix = self.get_designator_prefix(module_id);
                                let fp_name = mapper.get_footprint_name(
                                    &format!("{}1", prefix),
                                    Some(solved),
                                );
                                if info.footprint.is_none() {
                                    info.footprint = Some(fp_name);
                                }
                                info.package = Some(solved.clone());
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Check solved_values for explicit footprint/lcsc parameters
            if info.footprint.is_none() {
                if let Some(field_id) = module.get_field("footprint") {
                    if let Some(field) = self.design.get_field(field_id) {
                        if field.is_parameter() {
                            if let Some(solved) = self.solved_values.get("footprint") {
                                info.footprint = Some(solved.clone());
                            }
                        }
                    }
                }
            }

            if info.lcsc.is_none() {
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
        }

        info
    }

    /// Walk a module's inheritance chain to extract footprint/LCSC from traits.
    fn extract_traits_from_chain(&self, start_module_id: ModuleId, info: &mut FootprintInfo) {
        let mut current_id = Some(start_module_id);
        let mut depth = 0;
        while let Some(mid) = current_id {
            if depth > 20 { break; }
            depth += 1;
            if let Some(module) = self.design.get_module(mid) {
                for trait_ref in &module.traits {
                    let trait_name = trait_ref.name.name();

                    // is_atomic_part<footprint="X.kicad_mod">
                    if trait_name == "is_atomic_part" && info.footprint.is_none() {
                        if let Some(fp) = trait_ref.get_string_arg("footprint") {
                            info.footprint = Some(fp.to_string());
                        }
                    }

                    // has_part_picked::by_supplier<supplier_partno="C123">
                    if trait_name == "has_part_picked" && info.lcsc.is_none() {
                        if let Some(partno) = trait_ref.get_string_arg("supplier_partno") {
                            info.lcsc = Some(partno.to_string());
                        }
                    }
                }
                current_id = module.super_type;
            } else {
                break;
            }
        }
    }

    /// Build nets from connections using a path-based graph approach.
    ///
    /// Connections are module-scoped but the netlist is instance-scoped. We bridge
    /// this gap by:
    /// 1. Walking the module instance hierarchy to build prefix maps
    /// 2. For each connection, prefixing endpoint paths with instance context
    /// 3. Expanding interface connections into leaf-level sub-field equivalences
    /// 4. Building a graph of path equivalences and finding connected components
    /// 5. Resolving paths in each component to (component_ref, pin_name) pairs
    fn build_nets(&mut self) -> Result<(), ExportError> {
        // Build instance_path -> component ref map from component properties
        let mut instance_path_to_ref: HashMap<String, String> = HashMap::new();
        for comp in &self.netlist.components {
            if let Some(inst_path) = comp.properties.get("instance") {
                instance_path_to_ref.insert(inst_path.clone(), comp.reference.clone());
            }
        }

        // Build module_id -> list of instance paths for that module type
        let mut module_to_instance_paths: HashMap<ModuleId, Vec<String>> = HashMap::new();
        if let Some(entry_id) = self.entry_module {
            module_to_instance_paths.entry(entry_id).or_default().push(String::new());
        }
        self.build_module_instance_prefixes(&mut module_to_instance_paths);

        // Build the path equivalence graph.
        // Each node is a fully-qualified path string.
        // Each edge means the two paths are electrically connected.
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        let connections: Vec<_> = self.design.connections().to_vec();
        for connection in &connections {
            let parent_module = connection.parent;
            let prefixes = module_to_instance_paths
                .get(&parent_module)
                .cloned()
                .unwrap_or_default();

            let effective_prefixes = if prefixes.is_empty() {
                vec![String::new()]
            } else {
                prefixes
            };

            for prefix in &effective_prefixes {
                self.add_connection_edges_to_graph(
                    connection,
                    prefix,
                    &mut graph,
                );
            }
        }

        // Find connected components in the graph via BFS.
        // Sort graph keys for deterministic traversal order.
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

        let mut all_nodes: Vec<String> = graph.keys().cloned().collect();
        all_nodes.sort();

        for start_node in &all_nodes {
            if visited.contains(start_node) {
                continue;
            }

            // BFS to find the connected component
            let mut component: Vec<String> = Vec::new();
            let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
            queue.push_back(start_node.clone());
            visited.insert(start_node.clone());

            while let Some(current) = queue.pop_front() {
                component.push(current.clone());
                if let Some(neighbors) = graph.get(&current) {
                    // Sort neighbors for deterministic BFS order
                    let mut sorted_neighbors: Vec<&String> = neighbors.iter().collect();
                    sorted_neighbors.sort();
                    for neighbor in sorted_neighbors {
                        if visited.insert(neighbor.clone()) {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }

            // For each path in this connected component, try to resolve to a
            // (component_ref, pin_name) pair
            let mut net_nodes: Vec<NetNode> = Vec::new();
            let mut seen_refs: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
            for path in &component {
                if let Some(node) = self.resolve_path_to_component_pin(path, &instance_path_to_ref) {
                    let key = (node.component.clone(), node.pin.clone());
                    if seen_refs.insert(key) {
                        net_nodes.push(node);
                    }
                }
            }

            if net_nodes.len() >= 2 {
                // Derive a descriptive net name from the endpoint paths.
                // Use the alphabetically first path in the connected component
                // (which is stable across builds since we sorted all_nodes).
                component.sort();
                let net_name = derive_net_name(&component);
                let mut net = Net::new(net_name);
                for node in net_nodes {
                    net.add_node(node);
                }
                self.netlist.add_net(net);
            }
        }

        Ok(())
    }

    /// Add edges to the path equivalence graph for a single connection.
    ///
    /// For interface-to-interface connections, recursively expands to sub-field
    /// equivalences. For pin/signal connections, adds a direct edge.
    fn add_connection_edges_to_graph(
        &self,
        connection: &ato_ir::Connection,
        prefix: &str,
        graph: &mut HashMap<String, Vec<String>>,
    ) {
        let endpoint_paths: Vec<String> = connection.endpoints.iter().filter_map(|ep| {
            self.endpoint_to_path_string(ep, prefix)
        }).collect();

        if connection.is_simple() && endpoint_paths.len() == 2 {
            // Expand interface connections and add edges
            self.add_path_equivalences(
                &endpoint_paths[0],
                &endpoint_paths[1],
                connection.parent,
                graph,
                0,
            );
        } else if connection.is_directed() {
            // For directed connections (bridge), add edges between consecutive pairs
            for pair in endpoint_paths.windows(2) {
                self.add_path_equivalences(
                    &pair[0],
                    &pair[1],
                    connection.parent,
                    graph,
                    0,
                );
            }
        }
    }

    /// Add path equivalences between two paths, expanding interface connections.
    ///
    /// When both paths point to instances of the same interface type, we expand
    /// by adding equivalences for each matching sub-field. This handles cases like
    /// `power_3v3 ~ microcontroller.power_3v3` where both are `ElectricPower`,
    /// which means `power_3v3.hv ~ microcontroller.power_3v3.hv` and
    /// `power_3v3.lv ~ microcontroller.power_3v3.lv`.
    fn add_path_equivalences(
        &self,
        path_a: &str,
        path_b: &str,
        parent_module: ModuleId,
        graph: &mut HashMap<String, Vec<String>>,
        depth: u32,
    ) {
        if depth > 20 || path_a == path_b {
            return;
        }

        // Try to figure out if both endpoints refer to interface instances
        // by checking the resolved field types through the module hierarchy.
        // We need to resolve the *local* part of the path (without prefix) to
        // find the field's type.
        let sub_fields = self.get_interface_sub_fields_for_path(path_a, parent_module)
            .or_else(|| self.get_interface_sub_fields_for_path(path_b, parent_module));

        if let Some(fields) = sub_fields {
            // Both paths should have the same interface type - expand
            for field_name in &fields {
                let sub_a = format!("{}.{}", path_a, field_name);
                let sub_b = format!("{}.{}", path_b, field_name);
                self.add_path_equivalences(&sub_a, &sub_b, parent_module, graph, depth + 1);
            }
        } else {
            // Leaf-level connection - add direct edge
            graph.entry(path_a.to_string()).or_default().push(path_b.to_string());
            graph.entry(path_b.to_string()).or_default().push(path_a.to_string());
        }
    }

    /// Given a fully-qualified path, try to determine if it refers to an interface
    /// instance, and if so, return the connectable sub-field names of that interface.
    ///
    /// For example, if `path` refers to an ElectricPower instance, returns
    /// `Some(["hv", "lv"])`.
    fn get_interface_sub_fields_for_path(
        &self,
        path: &str,
        _parent_module: ModuleId,
    ) -> Option<Vec<String>> {
        // We need to resolve the path through the module hierarchy.
        // The path may have a prefix from instance expansion, so we need to
        // find which module context to resolve in.
        //
        // Strategy: try to resolve the path from the entry module by walking
        // the instance hierarchy.
        let parts: Vec<&str> = path.split('.').collect();
        self.resolve_path_to_interface_fields(&parts, 0)
    }

    /// Walk a dotted path from the entry module through instance hierarchy,
    /// and if the final element is an interface instance, return its sub-field names.
    fn resolve_path_to_interface_fields(
        &self,
        parts: &[&str],
        start_idx: usize,
    ) -> Option<Vec<String>> {
        let entry_id = self.entry_module?;
        let mut current_module_id = entry_id;

        for i in start_idx..parts.len() {
            let part = parts[i];

            // Handle array indices embedded in part name like "leds[0]"
            let (field_name, _array_idx) = if let Some(bracket_pos) = part.find('[') {
                let name = &part[..bracket_pos];
                let idx_str = &part[bracket_pos + 1..part.len() - 1];
                (name, idx_str.parse::<u32>().ok())
            } else {
                (part, None)
            };

            let module = self.design.get_module(current_module_id)?;
            let field_id = module.get_field(field_name)?;
            let field = self.design.get_field(field_id)?;

            match &field.kind {
                ato_ir::FieldKind::Instance { resolved_type: Some(type_id), .. } => {
                    let target_module = self.design.get_module(*type_id)?;
                    if i == parts.len() - 1 {
                        // This is the last part - check if it's an interface
                        if target_module.is_interface() {
                            // Return the connectable sub-fields of this interface
                            let sub_fields: Vec<String> = target_module.fields.iter()
                                .filter_map(|&fid| {
                                    let f = self.design.get_field(fid)?;
                                    if f.is_connectable() {
                                        Some(f.name.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if !sub_fields.is_empty() {
                                return Some(sub_fields);
                            } else {
                                return None; // Leaf interface (like Electrical) with no sub-fields
                            }
                        }
                        return None;
                    } else {
                        current_module_id = *type_id;
                    }
                }
                ato_ir::FieldKind::Pin { .. } | ato_ir::FieldKind::Signal => {
                    // Reached a pin/signal - this is a leaf
                    return None;
                }
                _ => return None,
            }
        }
        None
    }

    /// Convert a connection endpoint to a fully-qualified path string.
    fn endpoint_to_path_string(
        &self,
        endpoint: &ato_ir::ConnectionEndpoint,
        prefix: &str,
    ) -> Option<String> {
        let ep_path_str = match &endpoint.kind {
            EndpointKind::FieldRef(path) => self.format_field_path(path),
            EndpointKind::SignalDef(name) => name.clone(),
            EndpointKind::PinDef(name) => name.clone(),
            EndpointKind::Resolved => {
                if let Some(field_id) = endpoint.resolved {
                    if let Some(field) = self.design.get_field(field_id) {
                        field.name.clone()
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        };

        if prefix.is_empty() {
            Some(ep_path_str)
        } else {
            Some(format!("{}.{}", prefix, ep_path_str))
        }
    }

    /// Try to resolve a fully-qualified path to a (component_ref, pin_name) pair.
    ///
    /// For a path like "microcontroller.esp32_c3.package.VDD", tries progressively
    /// shorter prefixes against the instance_path_to_ref map:
    /// - "microcontroller.esp32_c3.package" with pin "VDD"
    /// - "microcontroller.esp32_c3" with pin "package.VDD"
    /// - etc.
    fn resolve_path_to_component_pin(
        &self,
        full_path: &str,
        instance_path_to_ref: &HashMap<String, String>,
    ) -> Option<NetNode> {
        let parts: Vec<&str> = full_path.split('.').collect();

        // Try from longest prefix to shortest
        for split_point in (1..parts.len()).rev() {
            let instance_path = parts[..split_point].join(".");
            let pin_path = parts[split_point..].join(".");

            if let Some(ref_name) = instance_path_to_ref.get(&instance_path) {
                if !pin_path.is_empty() {
                    return Some(NetNode::new(ref_name.clone(), pin_path));
                }
            }
        }

        None
    }

    /// Build a map from module_id to the list of instance path prefixes
    /// for that module type.
    fn build_module_instance_prefixes(
        &self,
        prefixes: &mut HashMap<ModuleId, Vec<String>>,
    ) {
        if let Some(entry_id) = self.entry_module {
            self.walk_module_for_prefixes(entry_id, "", prefixes, &mut std::collections::HashSet::new());
        } else {
            for module in self.design.modules() {
                if !module.is_interface() {
                    prefixes.entry(module.id).or_default().push(String::new());
                }
            }
        }
    }

    /// Recursively walk a module's instance hierarchy to build prefix map.
    /// Includes both module instances AND interface instances (since connections
    /// can reference interface fields).
    fn walk_module_for_prefixes(
        &self,
        module_id: ModuleId,
        current_prefix: &str,
        prefixes: &mut HashMap<ModuleId, Vec<String>>,
        visited: &mut std::collections::HashSet<(ModuleId, String)>,
    ) {
        let key = (module_id, current_prefix.to_string());
        if !visited.insert(key) {
            return;
        }

        prefixes.entry(module_id).or_default().push(current_prefix.to_string());

        let module = match self.design.get_module(module_id) {
            Some(m) => m,
            None => return,
        };

        let instance_fields: Vec<(String, Option<u32>, Option<ModuleId>)> = module
            .fields
            .iter()
            .filter_map(|&fid| {
                let field = self.design.get_field(fid)?;
                if let ato_ir::FieldKind::Instance {
                    count,
                    resolved_type,
                    ..
                } = &field.kind
                {
                    Some((field.name.clone(), *count, *resolved_type))
                } else {
                    None
                }
            })
            .collect();

        for (field_name, count, resolved_type) in instance_fields {
            let type_module_id = match resolved_type {
                Some(id) => id,
                None => continue,
            };

            // Only skip interfaces that have no connections (pure leaf interfaces)
            if let Some(target) = self.design.get_module(type_module_id) {
                if target.is_interface() && target.connections.is_empty() {
                    continue;
                }
            }

            let instance_count = match count {
                Some(n) if n > 1 => n,
                _ => 1,
            };

            for idx in 0..instance_count {
                let child_prefix = if instance_count > 1 {
                    if current_prefix.is_empty() {
                        format!("{}[{}]", field_name, idx)
                    } else {
                        format!("{}.{}[{}]", current_prefix, field_name, idx)
                    }
                } else if current_prefix.is_empty() {
                    field_name.clone()
                } else {
                    format!("{}.{}", current_prefix, field_name)
                };

                self.walk_module_for_prefixes(type_module_id, &child_prefix, prefixes, visited);
            }
        }
    }

    /// Format a FieldPath into a dotted string representation.
    fn format_field_path(&self, path: &ato_ir::FieldPath) -> String {
        let mut result = String::new();
        for part in path.parts.iter() {
            match part {
                FieldPathPart::Name(name) => {
                    if !result.is_empty() && !result.ends_with('[') {
                        result.push('.');
                    }
                    result.push_str(name);
                }
                FieldPathPart::Index(idx) => {
                    result.push_str(&format!("[{}]", idx));
                }
                FieldPathPart::PinRef(num) => {
                    if !result.is_empty() {
                        result.push('.');
                    }
                    result.push_str(&num.to_string());
                }
            }
        }
        result
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

    #[test]
    fn test_natord_cmp() {
        use std::cmp::Ordering;
        assert_eq!(natord_cmp("R1", "R2"), Ordering::Less);
        assert_eq!(natord_cmp("R2", "R10"), Ordering::Less);
        assert_eq!(natord_cmp("R10", "R2"), Ordering::Greater);
        assert_eq!(natord_cmp("C1", "R1"), Ordering::Less);
        assert_eq!(natord_cmp("R1", "R1"), Ordering::Equal);
        assert_eq!(natord_cmp("SW1", "U1"), Ordering::Less);
    }

    #[test]
    fn test_deterministic_designator_assignment() {
        // Build the same design twice and verify identical results
        let build_netlist = || {
            let mut design = Design::new();

            let resistor_id = design.create_module("Resistor", ModuleKind::Module);
            design.add_field(resistor_id, "p1", FieldKind::pin("1"));
            design.add_field(resistor_id, "p2", FieldKind::pin("2"));

            let cap_id = design.create_module("Capacitor", ModuleKind::Module);
            design.add_field(cap_id, "p1", FieldKind::pin("1"));
            design.add_field(cap_id, "p2", FieldKind::pin("2"));

            let app_id = design.create_module("App", ModuleKind::Module);

            // Add instances in varying order to test determinism
            design.add_field(
                app_id,
                "r2",
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
                "r1",
                FieldKind::Instance {
                    type_ref: QualifiedName::simple("Resistor"),
                    count: None,
                    resolved_type: Some(resistor_id),
                },
            );

            let builder = NetlistBuilder::new(&design);
            builder.build().unwrap()
        };

        let netlist1 = build_netlist();
        let netlist2 = build_netlist();

        // Component count should match
        assert_eq!(netlist1.component_count(), netlist2.component_count());

        // Component references and order should be identical
        let refs1: Vec<&str> = netlist1.components.iter().map(|c| c.reference.as_str()).collect();
        let refs2: Vec<&str> = netlist2.components.iter().map(|c| c.reference.as_str()).collect();
        assert_eq!(refs1, refs2, "Designators must be identical across builds");

        // Verify sorting: C1 before R1 before R2
        assert_eq!(refs1, vec!["C1", "R1", "R2"]);
    }

    #[test]
    fn test_deterministic_designator_by_hierarchy_path() {
        // Components should be numbered by their hierarchy path, not insertion order
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));

        let app_id = design.create_module("App", ModuleKind::Module);

        // Add r_z before r_a -- alphabetically r_a should get R1
        design.add_field(
            app_id,
            "r_z",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("Resistor"),
                count: None,
                resolved_type: Some(resistor_id),
            },
        );
        design.add_field(
            app_id,
            "r_a",
            FieldKind::Instance {
                type_ref: QualifiedName::simple("Resistor"),
                count: None,
                resolved_type: Some(resistor_id),
            },
        );

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        assert_eq!(netlist.component_count(), 2);

        // r_a should be R1 (alphabetically first), r_z should be R2
        let r1 = netlist.get_component("R1").unwrap();
        assert_eq!(
            r1.properties.get("instance").map(|s| s.as_str()),
            Some("r_a"),
            "R1 should be the alphabetically first instance (r_a)"
        );

        let r2 = netlist.get_component("R2").unwrap();
        assert_eq!(
            r2.properties.get("instance").map(|s| s.as_str()),
            Some("r_z"),
            "R2 should be the alphabetically second instance (r_z)"
        );
    }

    #[test]
    fn test_deterministic_net_names() {
        // Net names should be derived from endpoint paths, not sequential counters
        let mut design = Design::new();

        let resistor_id = design.create_module("Resistor", ModuleKind::Module);
        design.add_field(resistor_id, "p1", FieldKind::pin("1"));
        design.add_field(resistor_id, "p2", FieldKind::pin("2"));

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

        // Connect r1.p2 ~ r2.p1
        let ep1 = ConnectionEndpoint::field(FieldPath::new(vec![
            FieldPathPart::Name("r1".to_string()),
            FieldPathPart::Name("p2".to_string()),
        ]));
        let ep2 = ConnectionEndpoint::field(FieldPath::new(vec![
            FieldPathPart::Name("r2".to_string()),
            FieldPathPart::Name("p1".to_string()),
        ]));
        design.add_connection(app_id, ep1, ep2);
        design.rebuild_connection_graph();

        let builder = NetlistBuilder::new(&design)
            .with_entry_module(app_id);
        let netlist = builder.build().unwrap();

        // Net name should be derived from sorted paths, not "Net1"
        if !netlist.nets.is_empty() {
            let net = &netlist.nets[0];
            // Name should NOT be "Net1" - it should be derived from the endpoint paths
            assert!(
                !net.name.starts_with("Net"),
                "Net name should be descriptive, not 'Net{}'. Got: '{}'",
                1, net.name
            );
        }

        // Running again should produce the same net names
        let builder2 = NetlistBuilder::new(&design)
            .with_entry_module(app_id);
        let netlist2 = builder2.build().unwrap();

        let names1: Vec<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
        let names2: Vec<&str> = netlist2.nets.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names1, names2, "Net names must be identical across builds");
    }

    #[test]
    fn test_derive_net_name() {
        assert_eq!(derive_net_name(&["power_3v3.hv".to_string()]), "power_3v3_hv");
        assert_eq!(derive_net_name(&["r1.p2".to_string(), "r2.p1".to_string()]), "r1_p2");
        assert_eq!(derive_net_name(&[]), "Net");
    }

    #[test]
    fn test_components_sorted_in_output() {
        // Verify that the final netlist has components sorted by reference
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
        // Add in reverse alphabetical order
        design.add_field(app_id, "led1", FieldKind::Instance {
            type_ref: QualifiedName::simple("LED"),
            count: None,
            resolved_type: Some(led_id),
        });
        design.add_field(app_id, "r1", FieldKind::Instance {
            type_ref: QualifiedName::simple("Resistor"),
            count: None,
            resolved_type: Some(resistor_id),
        });
        design.add_field(app_id, "c1", FieldKind::Instance {
            type_ref: QualifiedName::simple("Capacitor"),
            count: None,
            resolved_type: Some(cap_id),
        });

        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        let refs: Vec<&str> = netlist.components.iter().map(|c| c.reference.as_str()).collect();
        // Should be sorted: C1, D1, R1
        assert_eq!(refs, vec!["C1", "D1", "R1"], "Components should be sorted by reference");
    }
}
