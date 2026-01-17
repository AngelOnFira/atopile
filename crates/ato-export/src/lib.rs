//! Export formats for the Ato electronics compiler.
//!
//! This crate provides functionality to export designs from the IR to various
//! output formats used in electronic design automation (EDA) tools.
//!
//! # Supported Formats
//!
//! - **Netlist**: Generic netlist representation
//! - **KiCad Netlist**: KiCad-compatible netlist format (.net)
//! - **KiCad Project**: Complete KiCad project files (.kicad_pro, .kicad_sch, .kicad_pcb)
//! - **BOM**: Bill of Materials in JLCPCB and generic CSV formats
//!
//! # Example
//!
//! ```
//! use ato_ir::{Design, ModuleKind, FieldKind};
//! use ato_export::{Netlist, NetlistBuilder, KicadNetlistExporter, Bom, BomExporter, BomFormat};
//!
//! // Create a design
//! let mut design = Design::new();
//! let module_id = design.create_module("Resistor", ModuleKind::Module);
//! design.add_field(module_id, "p1", FieldKind::pin("1"));
//! design.add_field(module_id, "p2", FieldKind::pin("2"));
//! design.add_field(module_id, "resistance", FieldKind::parameter_with_unit("ohm"));
//!
//! // Build a netlist
//! let builder = NetlistBuilder::new(&design);
//! let netlist = builder.build().expect("Failed to build netlist");
//!
//! // Export to KiCad format
//! let exporter = KicadNetlistExporter::new(&netlist);
//! let output = exporter.export_to_string().expect("Failed to export");
//!
//! // Generate BOM from netlist
//! let bom = Bom::from_netlist_grouped(&netlist);
//! let bom_exporter = BomExporter::new(&bom);
//! let bom_csv = bom_exporter.export_to_string(BomFormat::Jlcpcb).expect("Failed to export BOM");
//! ```

pub mod netlist;
pub mod kicad;
pub mod bom;
pub mod kicad_project;
pub mod kicad_schematic;
pub mod kicad_pcb;
pub mod kicad_library;

pub use netlist::{
    Net,
    NetNode,
    Netlist,
    NetlistBuilder,
    NetlistComponent,
    NetlistMetadata,
};

pub use kicad::{
    KicadComponent,
    KicadDesign,
    KicadField,
    KicadNet,
    KicadNetlist,
    KicadNetlistExporter,
    KicadNode,
};

pub use bom::{
    Bom,
    BomExporter,
    BomFormat,
    BomLine,
};

pub use kicad_project::{
    KicadProject,
    BoardSettings,
    DesignSettings,
    NetSettings,
    NetClass,
    PcbnewSettings,
    SchematicSettings,
};

pub use kicad_schematic::{
    KicadSchematic,
    SchematicSymbol,
    Wire,
    Label,
    TitleBlock,
};

pub use kicad_pcb::{
    KicadPcb,
    Footprint,
    Pad,
    Track,
    Via,
    Zone,
    Graphic,
    PcbNet,
};

pub use kicad_library::{
    LibraryMapper,
    ComponentType,
    SymbolRef,
    FootprintRef,
    Package,
};

use thiserror::Error;

/// Errors that can occur during export.
#[derive(Error, Debug)]
pub enum ExportError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Encoding error.
    #[error("encoding error: {0}")]
    Encoding(String),

    /// Invalid design structure.
    #[error("invalid design: {0}")]
    InvalidDesign(String),

    /// Missing required data.
    #[error("missing required data: {0}")]
    MissingData(String),
}

/// Result type for export operations.
pub type ExportResult<T> = Result<T, ExportError>;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::{
        Bom,
        BomExporter,
        BomFormat,
        BomLine,
        ExportError,
        ExportResult,
        KicadNetlistExporter,
        KicadPcb,
        KicadProject,
        KicadSchematic,
        Net,
        NetNode,
        Netlist,
        NetlistBuilder,
        NetlistComponent,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use ato_ir::{Design, ModuleKind, FieldKind, FieldPath, ConnectionEndpoint};

    #[test]
    fn test_end_to_end_export() {
        // Create a simple design with two resistors
        let mut design = Design::new();

        // Create first resistor
        let r1_id = design.create_module("Resistor", ModuleKind::Module);
        let _r1_p1 = design.add_field(r1_id, "p1", FieldKind::pin("1"));
        let r1_p2 = design.add_field(r1_id, "p2", FieldKind::pin("2"));
        design.add_field(r1_id, "resistance", FieldKind::parameter_with_unit("ohm"));

        // Create second resistor
        let r2_id = design.create_module("Resistor2", ModuleKind::Module);
        let r2_p1 = design.add_field(r2_id, "p1", FieldKind::pin("1"));
        let _r2_p2 = design.add_field(r2_id, "p2", FieldKind::pin("2"));
        design.add_field(r2_id, "resistance", FieldKind::parameter_with_unit("ohm"));

        // Connect R1.p2 to R2.p1 (series connection)
        let mut ep1 = ConnectionEndpoint::field(FieldPath::simple("p2"));
        ep1.resolved = Some(r1_p2);
        let mut ep2 = ConnectionEndpoint::field(FieldPath::simple("p1"));
        ep2.resolved = Some(r2_p1);
        design.add_connection(r1_id, ep1, ep2);

        // Rebuild connection graph
        design.rebuild_connection_graph();

        // Build netlist
        let builder = NetlistBuilder::new(&design);
        let netlist = builder.build().unwrap();

        // Verify we have components
        assert!(netlist.component_count() >= 2);

        // Export to KiCad format
        let exporter = KicadNetlistExporter::new(&netlist);
        let output = exporter.export_to_string().unwrap();

        // Verify output contains expected elements
        assert!(output.contains("(export"));
        assert!(output.contains("(components"));
    }

    #[test]
    fn test_netlist_components() {
        let mut netlist = Netlist::new();

        netlist.add_component(
            NetlistComponent::new("R1", "10k")
                .with_footprint("Resistor_SMD:R_0402")
                .with_property("tolerance", "1%")
        );

        netlist.add_component(
            NetlistComponent::new("C1", "100nF")
                .with_footprint("Capacitor_SMD:C_0402")
        );

        assert_eq!(netlist.component_count(), 2);

        let r1 = netlist.get_component("R1").unwrap();
        assert_eq!(r1.value, "10k");
        assert_eq!(r1.footprint.as_deref(), Some("Resistor_SMD:R_0402"));
    }

    #[test]
    fn test_netlist_nets() {
        let mut netlist = Netlist::new();

        let mut vcc = Net::new("VCC");
        vcc.add_node(NetNode::new("U1", "VDD"));
        vcc.add_node(NetNode::new("C1", "1"));
        netlist.add_net(vcc);

        assert_eq!(netlist.net_count(), 1);

        let net = netlist.get_net("VCC").unwrap();
        assert_eq!(net.nodes.len(), 2);
    }
}
