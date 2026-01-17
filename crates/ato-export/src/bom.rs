//! Bill of Materials (BOM) generation.
//!
//! This module provides functionality to generate BOMs from netlists in various formats:
//! - JLCPCB format for PCB assembly
//! - Generic CSV format for general use

use std::collections::HashMap;
use std::io::Write;

use crate::netlist::{Netlist, NetlistComponent};
use crate::ExportError;

/// A line item in a BOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BomLine {
    /// Component designator(s), comma-separated if grouped (e.g., "R1, R2, R3").
    pub designator: String,
    /// Component value (e.g., "10k", "100nF").
    pub value: String,
    /// Footprint name.
    pub footprint: String,
    /// Quantity of this component.
    pub quantity: u32,
    /// Manufacturer name.
    pub manufacturer: String,
    /// Manufacturer part number.
    pub part_number: String,
    /// LCSC part number (for JLCPCB).
    pub lcsc_part_number: String,
}

impl BomLine {
    /// Create a new BOM line from a netlist component.
    pub fn from_component(comp: &NetlistComponent) -> Self {
        Self {
            designator: comp.reference.clone(),
            value: comp.value.clone(),
            footprint: comp.footprint.clone().unwrap_or_default(),
            quantity: 1,
            manufacturer: comp.properties.get("manufacturer").cloned().unwrap_or_default(),
            part_number: comp.properties.get("part_number").cloned().unwrap_or_default(),
            lcsc_part_number: comp.properties.get("lcsc").cloned().unwrap_or_default(),
        }
    }

    /// Merge another BOM line into this one (for grouping identical components).
    pub fn merge(&mut self, other: &BomLine) {
        // Append designator
        self.designator = format!("{}, {}", self.designator, other.designator);
        self.quantity += other.quantity;
    }

    /// Get a grouping key for this BOM line.
    /// Components with the same key can be grouped together.
    fn grouping_key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.value, self.footprint, self.part_number, self.lcsc_part_number
        )
    }
}

/// A complete Bill of Materials.
#[derive(Debug, Clone, Default)]
pub struct Bom {
    /// Lines in the BOM.
    pub lines: Vec<BomLine>,
    /// Optional title/project name.
    pub title: Option<String>,
    /// Optional date.
    pub date: Option<String>,
}

impl Bom {
    /// Create a new empty BOM.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a BOM from a netlist.
    pub fn from_netlist(netlist: &Netlist) -> Self {
        let lines: Vec<BomLine> = netlist
            .components
            .iter()
            .map(BomLine::from_component)
            .collect();

        Self {
            lines,
            title: netlist.metadata.title.clone(),
            date: netlist.metadata.date.clone(),
        }
    }

    /// Create a BOM from a netlist with component grouping.
    /// Components with identical value, footprint, and part numbers are grouped together.
    pub fn from_netlist_grouped(netlist: &Netlist) -> Self {
        let mut groups: HashMap<String, BomLine> = HashMap::new();

        for comp in &netlist.components {
            let line = BomLine::from_component(comp);
            let key = line.grouping_key();

            if let Some(existing) = groups.get_mut(&key) {
                existing.merge(&line);
            } else {
                groups.insert(key, line);
            }
        }

        // Collect and sort by designator
        let mut lines: Vec<BomLine> = groups.into_values().collect();
        lines.sort_by(|a, b| {
            let (prefix_a, num_a) = split_designator(&a.designator);
            let (prefix_b, num_b) = split_designator(&b.designator);
            prefix_a.cmp(&prefix_b).then(num_a.cmp(&num_b))
        });

        // Sort designators within each line
        for line in &mut lines {
            let mut designators: Vec<&str> = line.designator.split(", ").collect();
            designators.sort_by(|a, b| {
                let (prefix_a, num_a) = split_designator(a);
                let (prefix_b, num_b) = split_designator(b);
                prefix_a.cmp(&prefix_b).then(num_a.cmp(&num_b))
            });
            line.designator = designators.join(", ");
        }

        Self {
            lines,
            title: netlist.metadata.title.clone(),
            date: netlist.metadata.date.clone(),
        }
    }

    /// Add a line to the BOM.
    pub fn add_line(&mut self, line: BomLine) {
        self.lines.push(line);
    }

    /// Get the total number of components (sum of quantities).
    pub fn total_components(&self) -> u32 {
        self.lines.iter().map(|l| l.quantity).sum()
    }

    /// Get the number of unique line items.
    pub fn unique_count(&self) -> usize {
        self.lines.len()
    }
}

/// Split a designator into prefix and number (e.g., "R12" -> ("R", 12)).
fn split_designator(designator: &str) -> (&str, u32) {
    // Find where the number starts at the end
    let num_start = designator
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, _)| i)
        .unwrap_or(designator.len());

    let prefix = &designator[..num_start];
    let num: u32 = designator[num_start..].parse().unwrap_or(0);
    (prefix, num)
}

/// BOM output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomFormat {
    /// JLCPCB format: Comment, Designator, Footprint, LCSC Part #
    Jlcpcb,
    /// Generic CSV format with all fields.
    GenericCsv,
}

/// Export a BOM to a writer in the specified format.
pub struct BomExporter<'a> {
    bom: &'a Bom,
}

impl<'a> BomExporter<'a> {
    /// Create a new BOM exporter.
    pub fn new(bom: &'a Bom) -> Self {
        Self { bom }
    }

    /// Export the BOM to a writer in the specified format.
    pub fn export<W: Write>(&self, writer: &mut W, format: BomFormat) -> Result<(), ExportError> {
        match format {
            BomFormat::Jlcpcb => self.export_jlcpcb(writer),
            BomFormat::GenericCsv => self.export_generic_csv(writer),
        }
    }

    /// Export to a string in the specified format.
    pub fn export_to_string(&self, format: BomFormat) -> Result<String, ExportError> {
        let mut buffer = Vec::new();
        self.export(&mut buffer, format)?;
        String::from_utf8(buffer).map_err(|e| ExportError::Encoding(e.to_string()))
    }

    /// Export to JLCPCB format.
    fn export_jlcpcb<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        // Write header
        writeln!(writer, "Comment,Designator,Footprint,LCSC Part #")?;

        // Write lines
        for line in &self.bom.lines {
            writeln!(
                writer,
                "{},{},{},{}",
                escape_csv(&line.value),
                escape_csv(&line.designator),
                escape_csv(&line.footprint),
                escape_csv(&line.lcsc_part_number),
            )?;
        }

        Ok(())
    }

    /// Export to generic CSV format.
    fn export_generic_csv<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        // Write header
        writeln!(
            writer,
            "Reference,Value,Footprint,Quantity,Part Number,Manufacturer,LCSC"
        )?;

        // Write lines
        for line in &self.bom.lines {
            writeln!(
                writer,
                "{},{},{},{},{},{},{}",
                escape_csv(&line.designator),
                escape_csv(&line.value),
                escape_csv(&line.footprint),
                line.quantity,
                escape_csv(&line.part_number),
                escape_csv(&line.manufacturer),
                escape_csv(&line.lcsc_part_number),
            )?;
        }

        Ok(())
    }
}

/// Escape a string for CSV output.
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::NetlistComponent;

    fn create_test_netlist() -> Netlist {
        let mut netlist = Netlist::new();

        // Add some resistors
        netlist.add_component(
            NetlistComponent::new("R1", "10k")
                .with_footprint("Resistor_SMD:R_0402")
                .with_property("lcsc", "C25744")
                .with_property("manufacturer", "YAGEO"),
        );

        netlist.add_component(
            NetlistComponent::new("R2", "10k")
                .with_footprint("Resistor_SMD:R_0402")
                .with_property("lcsc", "C25744")
                .with_property("manufacturer", "YAGEO"),
        );

        netlist.add_component(
            NetlistComponent::new("R3", "4.7k")
                .with_footprint("Resistor_SMD:R_0402")
                .with_property("lcsc", "C25900")
                .with_property("manufacturer", "YAGEO"),
        );

        // Add a capacitor
        netlist.add_component(
            NetlistComponent::new("C1", "100nF")
                .with_footprint("Capacitor_SMD:C_0402")
                .with_property("lcsc", "C1525")
                .with_property("manufacturer", "Samsung"),
        );

        // Add an IC
        netlist.add_component(
            NetlistComponent::new("U1", "ATmega328P")
                .with_footprint("Package_QFP:TQFP-32")
                .with_property("lcsc", "C14877")
                .with_property("manufacturer", "Microchip")
                .with_property("part_number", "ATMEGA328P-AU"),
        );

        netlist
    }

    #[test]
    fn test_bom_from_netlist() {
        let netlist = create_test_netlist();
        let bom = Bom::from_netlist(&netlist);

        assert_eq!(bom.lines.len(), 5);
        assert_eq!(bom.total_components(), 5);
    }

    #[test]
    fn test_bom_from_netlist_grouped() {
        let netlist = create_test_netlist();
        let bom = Bom::from_netlist_grouped(&netlist);

        // R1 and R2 should be grouped (same 10k, same footprint, same LCSC)
        assert_eq!(bom.lines.len(), 4);
        assert_eq!(bom.total_components(), 5);

        // Find the 10k resistor line
        let r10k = bom.lines.iter().find(|l| l.value == "10k").unwrap();
        assert_eq!(r10k.quantity, 2);
        assert!(r10k.designator.contains("R1"));
        assert!(r10k.designator.contains("R2"));
    }

    #[test]
    fn test_bom_export_jlcpcb() {
        let netlist = create_test_netlist();
        let bom = Bom::from_netlist_grouped(&netlist);
        let exporter = BomExporter::new(&bom);

        let output = exporter.export_to_string(BomFormat::Jlcpcb).unwrap();

        // Check header
        assert!(output.starts_with("Comment,Designator,Footprint,LCSC Part #"));

        // Check content
        assert!(output.contains("10k"));
        assert!(output.contains("C25744"));
        assert!(output.contains("Resistor_SMD:R_0402"));
    }

    #[test]
    fn test_bom_export_generic_csv() {
        let netlist = create_test_netlist();
        let bom = Bom::from_netlist(&netlist);
        let exporter = BomExporter::new(&bom);

        let output = exporter.export_to_string(BomFormat::GenericCsv).unwrap();

        // Check header
        assert!(output.starts_with("Reference,Value,Footprint,Quantity,Part Number,Manufacturer,LCSC"));

        // Check content
        assert!(output.contains("R1"));
        assert!(output.contains("10k"));
        assert!(output.contains("YAGEO"));
        assert!(output.contains("ATmega328P"));
        assert!(output.contains("ATMEGA328P-AU"));
    }

    #[test]
    fn test_split_designator() {
        assert_eq!(split_designator("R1"), ("R", 1));
        assert_eq!(split_designator("R12"), ("R", 12));
        assert_eq!(split_designator("C100"), ("C", 100));
        assert_eq!(split_designator("U1"), ("U", 1));
        assert_eq!(split_designator("SW10"), ("SW", 10));
        assert_eq!(split_designator("NoNumber"), ("NoNumber", 0));
    }

    #[test]
    fn test_bom_line_grouping_key() {
        let line1 = BomLine {
            designator: "R1".to_string(),
            value: "10k".to_string(),
            footprint: "0402".to_string(),
            quantity: 1,
            manufacturer: "YAGEO".to_string(),
            part_number: "".to_string(),
            lcsc_part_number: "C25744".to_string(),
        };

        let line2 = BomLine {
            designator: "R2".to_string(),
            value: "10k".to_string(),
            footprint: "0402".to_string(),
            quantity: 1,
            manufacturer: "YAGEO".to_string(),
            part_number: "".to_string(),
            lcsc_part_number: "C25744".to_string(),
        };

        let line3 = BomLine {
            designator: "R3".to_string(),
            value: "4.7k".to_string(),
            footprint: "0402".to_string(),
            quantity: 1,
            manufacturer: "YAGEO".to_string(),
            part_number: "".to_string(),
            lcsc_part_number: "C25900".to_string(),
        };

        assert_eq!(line1.grouping_key(), line2.grouping_key());
        assert_ne!(line1.grouping_key(), line3.grouping_key());
    }

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("hello"), "hello");
        assert_eq!(escape_csv("hello,world"), "\"hello,world\"");
        assert_eq!(escape_csv("hello\"world"), "\"hello\"\"world\"");
        assert_eq!(escape_csv("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_bom_designator_sorting() {
        let mut netlist = Netlist::new();

        // Add components in non-sorted order
        netlist.add_component(
            NetlistComponent::new("R10", "10k")
                .with_footprint("0402")
                .with_property("lcsc", "C1"),
        );
        netlist.add_component(
            NetlistComponent::new("R2", "10k")
                .with_footprint("0402")
                .with_property("lcsc", "C1"),
        );
        netlist.add_component(
            NetlistComponent::new("R1", "10k")
                .with_footprint("0402")
                .with_property("lcsc", "C1"),
        );

        let bom = Bom::from_netlist_grouped(&netlist);

        assert_eq!(bom.lines.len(), 1);
        assert_eq!(bom.lines[0].quantity, 3);
        // Designators should be sorted
        assert_eq!(bom.lines[0].designator, "R1, R2, R10");
    }
}
