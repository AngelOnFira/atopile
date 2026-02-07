//! KiCad netlist format export.
//!
//! This module provides export functionality for KiCad-compatible netlist files.
//! The format follows KiCad's XML-based netlist specification.

use std::io::Write;

use crate::netlist::{Net, Netlist, NetlistComponent};
use crate::ExportError;

/// KiCad netlist file format version.
const KICAD_NETLIST_VERSION: &str = "E";

/// Export a netlist to KiCad format.
pub struct KicadNetlistExporter<'a> {
    netlist: &'a Netlist,
}

impl<'a> KicadNetlistExporter<'a> {
    /// Create a new exporter for the given netlist.
    pub fn new(netlist: &'a Netlist) -> Self {
        Self { netlist }
    }

    /// Export the netlist to a writer.
    pub fn export<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        // Write XML header
        writeln!(writer, "(export (version \"{}\")", KICAD_NETLIST_VERSION)?;

        // Write design section
        self.write_design(writer)?;

        // Write components section
        self.write_components(writer)?;

        // Write libparts section (empty for now)
        writeln!(writer, "  (libparts)")?;

        // Write libraries section (empty for now)
        writeln!(writer, "  (libraries)")?;

        // Write nets section
        self.write_nets(writer)?;

        // Close export
        writeln!(writer, ")")?;

        Ok(())
    }

    /// Export the netlist to a string.
    pub fn export_to_string(&self) -> Result<String, ExportError> {
        let mut buffer = Vec::new();
        self.export(&mut buffer)?;
        String::from_utf8(buffer).map_err(|e| ExportError::Encoding(e.to_string()))
    }

    /// Write the design metadata section.
    fn write_design<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (design")?;

        if let Some(title) = &self.netlist.metadata.title {
            writeln!(writer, "    (source \"{}\")", escape_string(title))?;
        }

        if let Some(date) = &self.netlist.metadata.date {
            writeln!(writer, "    (date \"{}\")", escape_string(date))?;
        }

        if let Some(tool) = &self.netlist.metadata.tool {
            writeln!(writer, "    (tool \"{}\")", escape_string(tool))?;
        }

        writeln!(writer, "  )")?;
        Ok(())
    }

    /// Write the components section.
    fn write_components<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (components")?;

        for (tstamp, comp) in self.netlist.components.iter().enumerate() {
            self.write_component(writer, comp, tstamp + 1)?;
        }

        writeln!(writer, "  )")?;
        Ok(())
    }

    /// Write a single component.
    fn write_component<W: Write>(
        &self,
        writer: &mut W,
        comp: &NetlistComponent,
        tstamp: usize,
    ) -> Result<(), ExportError> {
        writeln!(writer, "    (comp (ref \"{}\")", escape_string(&comp.reference))?;
        writeln!(writer, "      (value \"{}\")", escape_string(&comp.value))?;

        if let Some(footprint) = &comp.footprint {
            writeln!(writer, "      (footprint \"{}\")", escape_string(footprint))?;
        }

        // Write properties as fields
        if !comp.properties.is_empty() {
            writeln!(writer, "      (fields")?;
            for (key, value) in &comp.properties {
                writeln!(
                    writer,
                    "        (field (name \"{}\") \"{}\")",
                    escape_string(key),
                    escape_string(value)
                )?;
            }
            writeln!(writer, "      )")?;
        }

        writeln!(writer, "      (tstamps {})", tstamp)?;
        writeln!(writer, "    )")?;

        Ok(())
    }

    /// Write the nets section.
    fn write_nets<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (nets")?;

        for (code, net) in self.netlist.nets.iter().enumerate() {
            self.write_net(writer, net, code + 1)?;
        }

        writeln!(writer, "  )")?;
        Ok(())
    }

    /// Write a single net.
    fn write_net<W: Write>(
        &self,
        writer: &mut W,
        net: &Net,
        code: usize,
    ) -> Result<(), ExportError> {
        writeln!(
            writer,
            "    (net (code {}) (name \"{}\")",
            code,
            escape_string(&net.name)
        )?;

        for node in &net.nodes {
            writeln!(
                writer,
                "      (node (ref \"{}\") (pin \"{}\"))",
                escape_string(&node.component),
                escape_string(&node.pin)
            )?;
        }

        writeln!(writer, "    )")?;
        Ok(())
    }
}

/// Escape special characters in a string for KiCad format.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// KiCad netlist in the modern S-expression format.
#[derive(Debug, Clone)]
pub struct KicadNetlist {
    /// Version string.
    pub version: String,
    /// Design metadata.
    pub design: KicadDesign,
    /// Components in the design.
    pub components: Vec<KicadComponent>,
    /// Nets in the design.
    pub nets: Vec<KicadNet>,
}

/// Design metadata in KiCad format.
#[derive(Debug, Clone, Default)]
pub struct KicadDesign {
    /// Source file.
    pub source: Option<String>,
    /// Date.
    pub date: Option<String>,
    /// Tool that generated this netlist.
    pub tool: Option<String>,
}

/// A component in KiCad netlist format.
#[derive(Debug, Clone)]
pub struct KicadComponent {
    /// Reference designator.
    pub ref_: String,
    /// Value.
    pub value: String,
    /// Footprint.
    pub footprint: Option<String>,
    /// Timestamp (unique identifier).
    pub tstamp: String,
    /// Fields.
    pub fields: Vec<KicadField>,
}

/// A field on a KiCad component.
#[derive(Debug, Clone)]
pub struct KicadField {
    /// Field name.
    pub name: String,
    /// Field value.
    pub value: String,
}

/// A net in KiCad netlist format.
#[derive(Debug, Clone)]
pub struct KicadNet {
    /// Net code (unique identifier).
    pub code: u32,
    /// Net name.
    pub name: String,
    /// Nodes in this net.
    pub nodes: Vec<KicadNode>,
}

/// A node in a KiCad net.
#[derive(Debug, Clone)]
pub struct KicadNode {
    /// Component reference.
    pub ref_: String,
    /// Pin number/name.
    pub pin: String,
}

impl KicadNetlist {
    /// Create from a generic netlist.
    pub fn from_netlist(netlist: &Netlist) -> Self {
        let components = netlist
            .components
            .iter()
            .enumerate()
            .map(|(i, c)| KicadComponent {
                ref_: c.reference.clone(),
                value: c.value.clone(),
                footprint: c.footprint.clone(),
                tstamp: format!("{}", i + 1),
                fields: c
                    .properties
                    .iter()
                    .map(|(k, v)| KicadField {
                        name: k.clone(),
                        value: v.clone(),
                    })
                    .collect(),
            })
            .collect();

        let nets = netlist
            .nets
            .iter()
            .enumerate()
            .map(|(i, n)| KicadNet {
                code: (i + 1) as u32,
                name: n.name.clone(),
                nodes: n
                    .nodes
                    .iter()
                    .map(|node| KicadNode {
                        ref_: node.component.clone(),
                        pin: node.pin.clone(),
                    })
                    .collect(),
            })
            .collect();

        Self {
            version: KICAD_NETLIST_VERSION.to_string(),
            design: KicadDesign {
                source: netlist.metadata.source.clone(),
                date: netlist.metadata.date.clone(),
                tool: netlist.metadata.tool.clone(),
            },
            components,
            nets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::{NetNode, NetlistMetadata};

    fn create_test_netlist() -> Netlist {
        let netlist = Netlist {
            components: vec![
                NetlistComponent::new("R1", "10k")
                    .with_footprint("Resistor_SMD:R_0402"),
                NetlistComponent::new("C1", "100nF")
                    .with_footprint("Capacitor_SMD:C_0402"),
                NetlistComponent::new("U1", "ATmega328P")
                    .with_footprint("Package_QFP:TQFP-32"),
            ],
            nets: vec![
                Net {
                    name: "VCC".to_string(),
                    nodes: vec![
                        NetNode::new("U1", "VCC"),
                        NetNode::new("C1", "1"),
                    ],
                },
                Net {
                    name: "GND".to_string(),
                    nodes: vec![
                        NetNode::new("U1", "GND"),
                        NetNode::new("C1", "2"),
                        NetNode::new("R1", "1"),
                    ],
                },
                Net {
                    name: "Net1".to_string(),
                    nodes: vec![
                        NetNode::new("U1", "PA0"),
                        NetNode::new("R1", "2"),
                    ],
                },
            ],
            metadata: NetlistMetadata {
                title: Some("Test Design".to_string()),
                date: Some("2024-01-01".to_string()),
                tool: Some("ato-export".to_string()),
                source: None,
            },
        };
        netlist
    }

    #[test]
    fn test_kicad_exporter() {
        let netlist = create_test_netlist();
        let exporter = KicadNetlistExporter::new(&netlist);

        let output = exporter.export_to_string().unwrap();

        assert!(output.contains("(export (version \"E\")"));
        assert!(output.contains("(components"));
        assert!(output.contains("(comp (ref \"R1\")"));
        assert!(output.contains("(value \"10k\")"));
        assert!(output.contains("(nets"));
        assert!(output.contains("(net (code 1) (name \"VCC\")"));
        assert!(output.contains("(node (ref \"U1\") (pin \"VCC\")"));
    }

    #[test]
    fn test_kicad_netlist_from_netlist() {
        let netlist = create_test_netlist();
        let kicad = KicadNetlist::from_netlist(&netlist);

        assert_eq!(kicad.version, "E");
        assert_eq!(kicad.components.len(), 3);
        assert_eq!(kicad.nets.len(), 3);

        // Check component
        let r1 = &kicad.components[0];
        assert_eq!(r1.ref_, "R1");
        assert_eq!(r1.value, "10k");

        // Check net
        let vcc = &kicad.nets[0];
        assert_eq!(vcc.name, "VCC");
        assert_eq!(vcc.nodes.len(), 2);
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("hello\"world"), "hello\\\"world");
        assert_eq!(escape_string("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_string("path\\to\\file"), "path\\\\to\\\\file");
    }
}
