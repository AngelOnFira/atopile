//! KiCad schematic file generation (.kicad_sch).
//!
//! This module generates KiCad schematic files in S-expression format.

use std::io::Write;
use uuid::Uuid;

use crate::netlist::{Netlist, NetlistComponent};
use crate::ExportError;

/// KiCad schematic version.
const KICAD_SCH_VERSION: u32 = 20231120;

/// A KiCad schematic file.
#[derive(Debug, Clone)]
pub struct KicadSchematic {
    /// Version number.
    pub version: u32,
    /// Generator name.
    pub generator: String,
    /// Generator version.
    pub generator_version: String,
    /// Schematic UUID.
    pub uuid: String,
    /// Paper size.
    pub paper: String,
    /// Title block.
    pub title_block: Option<TitleBlock>,
    /// Library symbols used in the schematic.
    pub lib_symbols: Vec<LibSymbol>,
    /// Symbols (component instances) in the schematic.
    pub symbols: Vec<SchematicSymbol>,
    /// Wires connecting symbols.
    pub wires: Vec<Wire>,
    /// Text labels.
    pub labels: Vec<Label>,
}

impl Default for KicadSchematic {
    fn default() -> Self {
        Self::new()
    }
}

impl KicadSchematic {
    /// Create a new empty schematic.
    pub fn new() -> Self {
        Self {
            version: KICAD_SCH_VERSION,
            generator: "ato-export".to_string(),
            generator_version: "0.1.0".to_string(),
            uuid: Uuid::new_v4().to_string(),
            paper: "A4".to_string(),
            title_block: None,
            lib_symbols: Vec::new(),
            symbols: Vec::new(),
            wires: Vec::new(),
            labels: Vec::new(),
        }
    }

    /// Create a schematic from a netlist.
    pub fn from_netlist(netlist: &Netlist) -> Self {
        let mut schematic = Self::new();

        // Set title if available
        if let Some(title) = &netlist.metadata.title {
            schematic.title_block = Some(TitleBlock {
                title: title.clone(),
                date: netlist.metadata.date.clone(),
                revision: None,
                company: None,
            });
        }

        // Place components in a grid layout
        let mut x = 50.8; // Start position in mm
        let mut y = 50.8;
        let x_spacing = 50.8; // Spacing between components
        let y_spacing = 25.4;
        let components_per_row = 5;

        for (i, comp) in netlist.components.iter().enumerate() {
            let symbol = SchematicSymbol::from_component(comp, x, y);
            schematic.symbols.push(symbol);

            // Move to next position
            if (i + 1) % components_per_row == 0 {
                x = 50.8;
                y += y_spacing;
            } else {
                x += x_spacing;
            }
        }

        schematic
    }

    /// Export the schematic to S-expression format.
    pub fn export<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "(kicad_sch")?;
        writeln!(writer, "  (version {})", self.version)?;
        writeln!(writer, "  (generator \"{}\")", self.generator)?;
        writeln!(writer, "  (generator_version \"{}\")", self.generator_version)?;
        writeln!(writer)?;
        writeln!(writer, "  (uuid \"{}\")", self.uuid)?;
        writeln!(writer)?;
        writeln!(writer, "  (paper \"{}\")", self.paper)?;

        // Title block
        if let Some(tb) = &self.title_block {
            writeln!(writer)?;
            writeln!(writer, "  (title_block")?;
            writeln!(writer, "    (title \"{}\")", escape_sexpr(&tb.title))?;
            if let Some(date) = &tb.date {
                writeln!(writer, "    (date \"{}\")", escape_sexpr(date))?;
            }
            if let Some(rev) = &tb.revision {
                writeln!(writer, "    (rev \"{}\")", escape_sexpr(rev))?;
            }
            if let Some(company) = &tb.company {
                writeln!(writer, "    (company \"{}\")", escape_sexpr(company))?;
            }
            writeln!(writer, "  )")?;
        }

        // Library symbols
        if !self.lib_symbols.is_empty() {
            writeln!(writer)?;
            writeln!(writer, "  (lib_symbols")?;
            for sym in &self.lib_symbols {
                sym.write(writer, 4)?;
            }
            writeln!(writer, "  )")?;
        }

        // Symbols (component instances)
        for sym in &self.symbols {
            writeln!(writer)?;
            sym.write(writer)?;
        }

        // Wires
        for wire in &self.wires {
            writeln!(writer)?;
            wire.write(writer)?;
        }

        // Labels
        for label in &self.labels {
            writeln!(writer)?;
            label.write(writer)?;
        }

        writeln!(writer, ")")?;
        Ok(())
    }

    /// Export to a string.
    pub fn export_to_string(&self) -> Result<String, ExportError> {
        let mut buffer = Vec::new();
        self.export(&mut buffer)?;
        String::from_utf8(buffer).map_err(|e| ExportError::Encoding(e.to_string()))
    }
}

/// Title block information.
#[derive(Debug, Clone)]
pub struct TitleBlock {
    /// Title.
    pub title: String,
    /// Date.
    pub date: Option<String>,
    /// Revision.
    pub revision: Option<String>,
    /// Company name.
    pub company: Option<String>,
}

/// A library symbol definition.
#[derive(Debug, Clone)]
pub struct LibSymbol {
    /// Symbol library:name.
    pub lib_id: String,
    /// Properties.
    pub properties: Vec<SymbolProperty>,
    /// Symbol units.
    pub units: Vec<SymbolUnit>,
}

impl LibSymbol {
    fn write<W: Write>(&self, writer: &mut W, indent: usize) -> Result<(), ExportError> {
        let pad = " ".repeat(indent);
        writeln!(writer, "{}(symbol \"{}\"", pad, escape_sexpr(&self.lib_id))?;
        writeln!(writer, "{}  (pin_names (offset 1.016))", pad)?;
        writeln!(writer, "{}  (in_bom yes) (on_board yes)", pad)?;

        for prop in &self.properties {
            prop.write(writer, indent + 2)?;
        }

        for unit in &self.units {
            unit.write(writer, indent + 2)?;
        }

        writeln!(writer, "{})", pad)?;
        Ok(())
    }
}

/// A symbol property.
#[derive(Debug, Clone)]
pub struct SymbolProperty {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
    /// Property ID.
    pub id: u32,
    /// Position (x, y).
    pub position: (f64, f64),
    /// Hidden.
    pub hidden: bool,
}

impl SymbolProperty {
    fn write<W: Write>(&self, writer: &mut W, indent: usize) -> Result<(), ExportError> {
        let pad = " ".repeat(indent);
        write!(writer, "{}(property \"{}\" \"{}\" (id {}) (at {} {} 0)",
            pad, escape_sexpr(&self.name), escape_sexpr(&self.value),
            self.id, self.position.0, self.position.1)?;
        writeln!(writer)?;
        writeln!(writer, "{}  (effects (font (size 1.27 1.27)){}))",
            pad, if self.hidden { " hide" } else { "" })?;
        Ok(())
    }
}

/// A symbol unit (for multi-unit symbols).
#[derive(Debug, Clone)]
pub struct SymbolUnit {
    /// Unit number.
    pub unit: u32,
    /// Pins.
    pub pins: Vec<SymbolPin>,
}

impl SymbolUnit {
    fn write<W: Write>(&self, writer: &mut W, indent: usize) -> Result<(), ExportError> {
        let pad = " ".repeat(indent);
        writeln!(writer, "{}(symbol \"_{}_{}\"))", pad, self.unit, 1)?;
        Ok(())
    }
}

/// A pin on a symbol.
#[derive(Debug, Clone)]
pub struct SymbolPin {
    /// Pin type (input, output, bidirectional, etc.).
    pub pin_type: String,
    /// Pin style (line, inverted, clock, etc.).
    pub style: String,
    /// Position.
    pub position: (f64, f64),
    /// Rotation angle.
    pub angle: f64,
    /// Length.
    pub length: f64,
    /// Pin name.
    pub name: String,
    /// Pin number.
    pub number: String,
}

/// A symbol instance in the schematic.
#[derive(Debug, Clone)]
pub struct SchematicSymbol {
    /// Library ID (e.g., "Device:R").
    pub lib_id: String,
    /// Position (x, y) in mm.
    pub position: (f64, f64),
    /// Rotation angle in degrees.
    pub angle: f64,
    /// Unit number (for multi-unit symbols).
    pub unit: u32,
    /// Convert/style number.
    pub convert: u32,
    /// Unique identifier.
    pub uuid: String,
    /// Properties (Reference, Value, Footprint, etc.).
    pub properties: Vec<InstanceProperty>,
    /// Pin assignments.
    pub pins: Vec<PinInstance>,
}

impl SchematicSymbol {
    /// Create a symbol from a netlist component.
    pub fn from_component(comp: &NetlistComponent, x: f64, y: f64) -> Self {
        let lib_id = get_lib_id_for_component(comp);

        let mut properties = vec![
            InstanceProperty {
                name: "Reference".to_string(),
                value: comp.reference.clone(),
                id: 0,
                position: (x, y - 2.54),
            },
            InstanceProperty {
                name: "Value".to_string(),
                value: comp.value.clone(),
                id: 1,
                position: (x, y + 2.54),
            },
        ];

        if let Some(fp) = &comp.footprint {
            properties.push(InstanceProperty {
                name: "Footprint".to_string(),
                value: fp.clone(),
                id: 2,
                position: (x, y + 5.08),
            });
        }

        Self {
            lib_id,
            position: (x, y),
            angle: 0.0,
            unit: 1,
            convert: 1,
            uuid: Uuid::new_v4().to_string(),
            properties,
            pins: Vec::new(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (symbol")?;
        writeln!(writer, "    (lib_id \"{}\")", escape_sexpr(&self.lib_id))?;
        writeln!(writer, "    (at {} {} {})", self.position.0, self.position.1, self.angle)?;
        writeln!(writer, "    (unit {})", self.unit)?;
        writeln!(writer, "    (uuid \"{}\")", self.uuid)?;

        for prop in &self.properties {
            writeln!(writer, "    (property \"{}\" \"{}\"",
                escape_sexpr(&prop.name), escape_sexpr(&prop.value))?;
            writeln!(writer, "      (at {} {} 0)", prop.position.0, prop.position.1)?;
            writeln!(writer, "      (effects (font (size 1.27 1.27)))")?;
            writeln!(writer, "    )")?;
        }

        writeln!(writer, "  )")?;
        Ok(())
    }
}

/// An instance property.
#[derive(Debug, Clone)]
pub struct InstanceProperty {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
    /// Property ID.
    pub id: u32,
    /// Position.
    pub position: (f64, f64),
}

/// A pin instance.
#[derive(Debug, Clone)]
pub struct PinInstance {
    /// Pin number.
    pub number: String,
    /// Connected net UUID.
    pub net_uuid: Option<String>,
}

/// A wire connection.
#[derive(Debug, Clone)]
pub struct Wire {
    /// Start point (x, y).
    pub start: (f64, f64),
    /// End point (x, y).
    pub end: (f64, f64),
    /// Unique identifier.
    pub uuid: String,
}

impl Wire {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (wire")?;
        writeln!(writer, "    (pts")?;
        writeln!(writer, "      (xy {} {})", self.start.0, self.start.1)?;
        writeln!(writer, "      (xy {} {})", self.end.0, self.end.1)?;
        writeln!(writer, "    )")?;
        writeln!(writer, "    (stroke (width 0) (type default))")?;
        writeln!(writer, "    (uuid \"{}\")", self.uuid)?;
        writeln!(writer, "  )")?;
        Ok(())
    }
}

/// A text label.
#[derive(Debug, Clone)]
pub struct Label {
    /// Label text.
    pub text: String,
    /// Position (x, y).
    pub position: (f64, f64),
    /// Rotation angle.
    pub angle: f64,
    /// Unique identifier.
    pub uuid: String,
}

impl Label {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (label \"{}\"", escape_sexpr(&self.text))?;
        writeln!(writer, "    (at {} {} {})", self.position.0, self.position.1, self.angle)?;
        writeln!(writer, "    (effects (font (size 1.27 1.27)))")?;
        writeln!(writer, "    (uuid \"{}\")", self.uuid)?;
        writeln!(writer, "  )")?;
        Ok(())
    }
}

/// Get the library ID for a component based on its reference prefix.
fn get_lib_id_for_component(comp: &NetlistComponent) -> String {
    let prefix = comp.reference.chars().take_while(|c| c.is_alphabetic()).collect::<String>();

    match prefix.as_str() {
        "R" => "Device:R".to_string(),
        "C" => "Device:C".to_string(),
        "L" => "Device:L".to_string(),
        "D" => "Device:D".to_string(),
        "Q" => "Device:Q_NPN_BCE".to_string(),
        "U" => "Device:IC".to_string(),
        "J" | "P" => "Connector:Conn_01x02".to_string(),
        "Y" => "Device:Crystal".to_string(),
        "SW" => "Switch:SW_Push".to_string(),
        _ => format!("Device:{}", comp.reference.clone()),
    }
}

/// Escape a string for S-expression output.
fn escape_sexpr(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schematic_new() {
        let sch = KicadSchematic::new();
        assert_eq!(sch.version, KICAD_SCH_VERSION);
        assert_eq!(sch.paper, "A4");
        assert!(sch.symbols.is_empty());
    }

    #[test]
    fn test_schematic_from_netlist() {
        let mut netlist = Netlist::new();
        netlist.add_component(NetlistComponent::new("R1", "10k"));
        netlist.add_component(NetlistComponent::new("C1", "100nF"));

        let sch = KicadSchematic::from_netlist(&netlist);
        assert_eq!(sch.symbols.len(), 2);
        assert_eq!(sch.symbols[0].properties[0].value, "R1");
        assert_eq!(sch.symbols[1].properties[0].value, "C1");
    }

    #[test]
    fn test_schematic_export() {
        let mut netlist = Netlist::new();
        netlist.add_component(NetlistComponent::new("R1", "10k"));

        let sch = KicadSchematic::from_netlist(&netlist);
        let output = sch.export_to_string().unwrap();

        assert!(output.contains("(kicad_sch"));
        assert!(output.contains("(version"));
        assert!(output.contains("(symbol"));
        assert!(output.contains("\"R1\""));
        assert!(output.contains("\"10k\""));
    }

    #[test]
    fn test_get_lib_id() {
        let r = NetlistComponent::new("R1", "10k");
        assert_eq!(get_lib_id_for_component(&r), "Device:R");

        let c = NetlistComponent::new("C1", "100nF");
        assert_eq!(get_lib_id_for_component(&c), "Device:C");

        let u = NetlistComponent::new("U1", "ATmega328P");
        assert_eq!(get_lib_id_for_component(&u), "Device:IC");
    }

    #[test]
    fn test_escape_sexpr() {
        assert_eq!(escape_sexpr("hello"), "hello");
        assert_eq!(escape_sexpr("hello\"world"), "hello\\\"world");
        assert_eq!(escape_sexpr("path\\to\\file"), "path\\\\to\\\\file");
    }
}
