//! KiCad PCB file generation (.kicad_pcb).
//!
//! This module generates KiCad PCB files in S-expression format.

use std::io::Write;
use uuid::Uuid;

use crate::netlist::{Net, Netlist, NetlistComponent};
use crate::ExportError;

/// KiCad PCB version.
const KICAD_PCB_VERSION: u32 = 20240108;

/// A KiCad PCB file.
#[derive(Debug, Clone)]
pub struct KicadPcb {
    /// Version number.
    pub version: u32,
    /// Generator name.
    pub generator: String,
    /// Generator version.
    pub generator_version: String,
    /// General settings.
    pub general: GeneralSettings,
    /// Paper size.
    pub paper: String,
    /// Layers.
    pub layers: Vec<Layer>,
    /// Setup settings.
    pub setup: SetupSettings,
    /// Nets.
    pub nets: Vec<PcbNet>,
    /// Footprints.
    pub footprints: Vec<Footprint>,
    /// Tracks.
    pub tracks: Vec<Track>,
    /// Vias.
    pub vias: Vec<Via>,
    /// Zones.
    pub zones: Vec<Zone>,
    /// Graphics (lines, arcs, text, etc.).
    pub graphics: Vec<Graphic>,
}

impl Default for KicadPcb {
    fn default() -> Self {
        Self::new()
    }
}

impl KicadPcb {
    /// Create a new empty PCB.
    pub fn new() -> Self {
        Self {
            version: KICAD_PCB_VERSION,
            generator: "ato-export".to_string(),
            generator_version: "0.1.0".to_string(),
            general: GeneralSettings::default(),
            paper: "A4".to_string(),
            layers: default_layers(),
            setup: SetupSettings::default(),
            nets: vec![PcbNet { code: 0, name: String::new() }], // Net 0 is always empty
            footprints: Vec::new(),
            tracks: Vec::new(),
            vias: Vec::new(),
            zones: Vec::new(),
            graphics: Vec::new(),
        }
    }

    /// Create a PCB from a netlist.
    pub fn from_netlist(netlist: &Netlist) -> Self {
        let mut pcb = Self::new();

        // Add nets
        for (i, net) in netlist.nets.iter().enumerate() {
            pcb.nets.push(PcbNet {
                code: (i + 1) as u32,
                name: net.name.clone(),
            });
        }

        // Place footprints in a grid layout
        let mut x = 50.0; // Start position in mm
        let mut y = 50.0;
        let x_spacing = 15.0;
        let y_spacing = 15.0;
        let components_per_row = 5;

        for (i, comp) in netlist.components.iter().enumerate() {
            let footprint = Footprint::from_component(comp, x, y, &netlist.nets);
            pcb.footprints.push(footprint);

            // Move to next position
            if (i + 1) % components_per_row == 0 {
                x = 50.0;
                y += y_spacing;
            } else {
                x += x_spacing;
            }
        }

        // Add a simple board outline
        let board_width = 100.0;
        let board_height = 80.0;
        pcb.graphics.push(Graphic::Line {
            start: (25.0, 25.0),
            end: (25.0 + board_width, 25.0),
            layer: "Edge.Cuts".to_string(),
            width: 0.1,
            uuid: Uuid::new_v4().to_string(),
        });
        pcb.graphics.push(Graphic::Line {
            start: (25.0 + board_width, 25.0),
            end: (25.0 + board_width, 25.0 + board_height),
            layer: "Edge.Cuts".to_string(),
            width: 0.1,
            uuid: Uuid::new_v4().to_string(),
        });
        pcb.graphics.push(Graphic::Line {
            start: (25.0 + board_width, 25.0 + board_height),
            end: (25.0, 25.0 + board_height),
            layer: "Edge.Cuts".to_string(),
            width: 0.1,
            uuid: Uuid::new_v4().to_string(),
        });
        pcb.graphics.push(Graphic::Line {
            start: (25.0, 25.0 + board_height),
            end: (25.0, 25.0),
            layer: "Edge.Cuts".to_string(),
            width: 0.1,
            uuid: Uuid::new_v4().to_string(),
        });

        pcb
    }

    /// Export the PCB to S-expression format.
    pub fn export<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "(kicad_pcb")?;
        writeln!(writer, "  (version {})", self.version)?;
        writeln!(writer, "  (generator \"{}\")", self.generator)?;
        writeln!(writer, "  (generator_version \"{}\")", self.generator_version)?;

        // General
        writeln!(writer, "  (general")?;
        writeln!(writer, "    (thickness {})", self.general.thickness)?;
        writeln!(writer, "    (legacy_teardrops no)")?;
        writeln!(writer, "  )")?;

        // Paper
        writeln!(writer, "  (paper \"{}\")", self.paper)?;

        // Layers
        writeln!(writer, "  (layers")?;
        for layer in &self.layers {
            writeln!(writer, "    ({} \"{}\" {})", layer.number, layer.name, layer.layer_type)?;
        }
        writeln!(writer, "  )")?;

        // Setup (minimal)
        writeln!(writer, "  (setup")?;
        writeln!(writer, "    (pad_to_mask_clearance {})", self.setup.pad_to_mask_clearance)?;
        writeln!(writer, "  )")?;

        // Nets
        for net in &self.nets {
            writeln!(writer, "  (net {} \"{}\")", net.code, escape_sexpr(&net.name))?;
        }

        // Footprints
        for fp in &self.footprints {
            writeln!(writer)?;
            fp.write(writer)?;
        }

        // Graphics (board outline, etc.)
        for graphic in &self.graphics {
            writeln!(writer)?;
            graphic.write(writer)?;
        }

        // Tracks
        for track in &self.tracks {
            writeln!(writer)?;
            track.write(writer)?;
        }

        // Vias
        for via in &self.vias {
            writeln!(writer)?;
            via.write(writer)?;
        }

        // Zones
        for zone in &self.zones {
            writeln!(writer)?;
            zone.write(writer)?;
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

/// General PCB settings.
#[derive(Debug, Clone)]
pub struct GeneralSettings {
    /// Board thickness in mm.
    pub thickness: f64,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self { thickness: 1.6 }
    }
}

/// A PCB layer.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Layer number.
    pub number: u32,
    /// Layer name.
    pub name: String,
    /// Layer type.
    pub layer_type: String,
}

/// Get default PCB layers.
fn default_layers() -> Vec<Layer> {
    vec![
        Layer { number: 0, name: "F.Cu".to_string(), layer_type: "signal".to_string() },
        Layer { number: 31, name: "B.Cu".to_string(), layer_type: "signal".to_string() },
        Layer { number: 36, name: "B.SilkS".to_string(), layer_type: "user".to_string() },
        Layer { number: 37, name: "F.SilkS".to_string(), layer_type: "user".to_string() },
        Layer { number: 38, name: "B.Mask".to_string(), layer_type: "user".to_string() },
        Layer { number: 39, name: "F.Mask".to_string(), layer_type: "user".to_string() },
        Layer { number: 44, name: "Edge.Cuts".to_string(), layer_type: "user".to_string() },
        Layer { number: 46, name: "B.CrtYd".to_string(), layer_type: "user".to_string() },
        Layer { number: 47, name: "F.CrtYd".to_string(), layer_type: "user".to_string() },
        Layer { number: 48, name: "B.Fab".to_string(), layer_type: "user".to_string() },
        Layer { number: 49, name: "F.Fab".to_string(), layer_type: "user".to_string() },
    ]
}

/// Setup settings.
#[derive(Debug, Clone)]
pub struct SetupSettings {
    /// Pad to mask clearance.
    pub pad_to_mask_clearance: f64,
}

impl Default for SetupSettings {
    fn default() -> Self {
        Self {
            pad_to_mask_clearance: 0.0,
        }
    }
}

/// A net in the PCB.
#[derive(Debug, Clone)]
pub struct PcbNet {
    /// Net code (unique identifier).
    pub code: u32,
    /// Net name.
    pub name: String,
}

/// A footprint (component) on the PCB.
#[derive(Debug, Clone)]
pub struct Footprint {
    /// Footprint library:name.
    pub library: String,
    /// Layer (F.Cu or B.Cu).
    pub layer: String,
    /// Position (x, y).
    pub position: (f64, f64),
    /// Rotation angle.
    pub angle: f64,
    /// Unique identifier.
    pub uuid: String,
    /// Properties.
    pub properties: Vec<FootprintProperty>,
    /// Pads.
    pub pads: Vec<Pad>,
}

impl Footprint {
    /// Create a footprint from a netlist component.
    pub fn from_component(comp: &NetlistComponent, x: f64, y: f64, nets: &[Net]) -> Self {
        let library = comp.footprint.clone().unwrap_or_else(|| {
            // Generate a default footprint based on component type
            let prefix = comp.reference.chars().take_while(|c| c.is_alphabetic()).collect::<String>();
            match prefix.as_str() {
                "R" => "Resistor_SMD:R_0402_1005Metric".to_string(),
                "C" => "Capacitor_SMD:C_0402_1005Metric".to_string(),
                "L" => "Inductor_SMD:L_0402_1005Metric".to_string(),
                "D" => "Diode_SMD:D_0402_1005Metric".to_string(),
                _ => format!("Package_SO:SO-8_3.9x4.9mm_P1.27mm"),
            }
        });

        let properties = vec![
            FootprintProperty {
                name: "Reference".to_string(),
                value: comp.reference.clone(),
                layer: "F.SilkS".to_string(),
                position: (0.0, -2.0),
            },
            FootprintProperty {
                name: "Value".to_string(),
                value: comp.value.clone(),
                layer: "F.Fab".to_string(),
                position: (0.0, 2.0),
            },
        ];

        // Create basic pads (2 pads for passive components)
        let pads = create_pads_for_component(comp, nets);

        Self {
            library,
            layer: "F.Cu".to_string(),
            position: (x, y),
            angle: 0.0,
            uuid: Uuid::new_v4().to_string(),
            properties,
            pads,
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (footprint \"{}\"", escape_sexpr(&self.library))?;
        writeln!(writer, "    (layer \"{}\")", self.layer)?;
        writeln!(writer, "    (uuid \"{}\")", self.uuid)?;
        writeln!(writer, "    (at {} {} {})", self.position.0, self.position.1, self.angle)?;

        for prop in &self.properties {
            writeln!(writer, "    (property \"{}\" \"{}\"",
                escape_sexpr(&prop.name), escape_sexpr(&prop.value))?;
            writeln!(writer, "      (at {} {} 0)", prop.position.0, prop.position.1)?;
            writeln!(writer, "      (layer \"{}\")", prop.layer)?;
            writeln!(writer, "      (uuid \"{}\")", Uuid::new_v4())?;
            writeln!(writer, "      (effects (font (size 1 1) (thickness 0.15)))")?;
            writeln!(writer, "    )")?;
        }

        for pad in &self.pads {
            pad.write(writer)?;
        }

        writeln!(writer, "  )")?;
        Ok(())
    }
}

/// A footprint property.
#[derive(Debug, Clone)]
pub struct FootprintProperty {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
    /// Layer.
    pub layer: String,
    /// Position relative to footprint.
    pub position: (f64, f64),
}

/// A pad on a footprint.
#[derive(Debug, Clone)]
pub struct Pad {
    /// Pad number.
    pub number: String,
    /// Pad type (smd, thru_hole, etc.).
    pub pad_type: String,
    /// Pad shape (rect, circle, oval, etc.).
    pub shape: String,
    /// Position relative to footprint.
    pub position: (f64, f64),
    /// Size (width, height).
    pub size: (f64, f64),
    /// Drill size (for through-hole).
    pub drill: Option<f64>,
    /// Layers.
    pub layers: Vec<String>,
    /// Net code.
    pub net: Option<(u32, String)>,
    /// UUID.
    pub uuid: String,
}

impl Pad {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        write!(writer, "    (pad \"{}\" {} {}",
            self.number, self.pad_type, self.shape)?;
        writeln!(writer)?;
        writeln!(writer, "      (at {} {})", self.position.0, self.position.1)?;
        writeln!(writer, "      (size {} {})", self.size.0, self.size.1)?;

        if let Some(drill) = self.drill {
            writeln!(writer, "      (drill {})", drill)?;
        }

        let layers = self.layers.iter()
            .map(|l| format!("\"{}\"", l))
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(writer, "      (layers {})", layers)?;

        if let Some((code, name)) = &self.net {
            writeln!(writer, "      (net {} \"{}\")", code, escape_sexpr(name))?;
        }

        writeln!(writer, "      (uuid \"{}\")", self.uuid)?;
        writeln!(writer, "    )")?;
        Ok(())
    }
}

/// Create pads for a component.
fn create_pads_for_component(comp: &NetlistComponent, nets: &[Net]) -> Vec<Pad> {
    let prefix = comp.reference.chars().take_while(|c| c.is_alphabetic()).collect::<String>();

    // Find net connections for this component
    let find_net = |pin: &str| -> Option<(u32, String)> {
        for (i, net) in nets.iter().enumerate() {
            for node in &net.nodes {
                if node.component == comp.reference && node.pin == pin {
                    return Some(((i + 1) as u32, net.name.clone()));
                }
            }
        }
        None
    };

    match prefix.as_str() {
        "R" | "C" | "L" => {
            // Two-terminal passive component (0402 size)
            vec![
                Pad {
                    number: "1".to_string(),
                    pad_type: "smd".to_string(),
                    shape: "rect".to_string(),
                    position: (-0.5, 0.0),
                    size: (0.5, 0.5),
                    drill: None,
                    layers: vec!["F.Cu".to_string(), "F.Paste".to_string(), "F.Mask".to_string()],
                    net: find_net("1"),
                    uuid: Uuid::new_v4().to_string(),
                },
                Pad {
                    number: "2".to_string(),
                    pad_type: "smd".to_string(),
                    shape: "rect".to_string(),
                    position: (0.5, 0.0),
                    size: (0.5, 0.5),
                    drill: None,
                    layers: vec!["F.Cu".to_string(), "F.Paste".to_string(), "F.Mask".to_string()],
                    net: find_net("2"),
                    uuid: Uuid::new_v4().to_string(),
                },
            ]
        }
        "D" => {
            // Diode
            vec![
                Pad {
                    number: "1".to_string(),
                    pad_type: "smd".to_string(),
                    shape: "rect".to_string(),
                    position: (-0.8, 0.0),
                    size: (0.8, 0.8),
                    drill: None,
                    layers: vec!["F.Cu".to_string(), "F.Paste".to_string(), "F.Mask".to_string()],
                    net: find_net("1"),
                    uuid: Uuid::new_v4().to_string(),
                },
                Pad {
                    number: "2".to_string(),
                    pad_type: "smd".to_string(),
                    shape: "rect".to_string(),
                    position: (0.8, 0.0),
                    size: (0.8, 0.8),
                    drill: None,
                    layers: vec!["F.Cu".to_string(), "F.Paste".to_string(), "F.Mask".to_string()],
                    net: find_net("2"),
                    uuid: Uuid::new_v4().to_string(),
                },
            ]
        }
        _ => {
            // Generic IC-style component (4 pins)
            (1..=4).map(|i| {
                Pad {
                    number: i.to_string(),
                    pad_type: "smd".to_string(),
                    shape: "rect".to_string(),
                    position: (((i - 1) % 2) as f64 * 2.0 - 1.0, ((i - 1) / 2) as f64 * 2.0 - 1.0),
                    size: (0.6, 0.6),
                    drill: None,
                    layers: vec!["F.Cu".to_string(), "F.Paste".to_string(), "F.Mask".to_string()],
                    net: find_net(&i.to_string()),
                    uuid: Uuid::new_v4().to_string(),
                }
            }).collect()
        }
    }
}

/// A track (trace) on the PCB.
#[derive(Debug, Clone)]
pub struct Track {
    /// Start point.
    pub start: (f64, f64),
    /// End point.
    pub end: (f64, f64),
    /// Width.
    pub width: f64,
    /// Layer.
    pub layer: String,
    /// Net code.
    pub net: u32,
    /// UUID.
    pub uuid: String,
}

impl Track {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (segment")?;
        writeln!(writer, "    (start {} {})", self.start.0, self.start.1)?;
        writeln!(writer, "    (end {} {})", self.end.0, self.end.1)?;
        writeln!(writer, "    (width {})", self.width)?;
        writeln!(writer, "    (layer \"{}\")", self.layer)?;
        writeln!(writer, "    (net {})", self.net)?;
        writeln!(writer, "    (uuid \"{}\")", self.uuid)?;
        writeln!(writer, "  )")?;
        Ok(())
    }
}

/// A via.
#[derive(Debug, Clone)]
pub struct Via {
    /// Position.
    pub position: (f64, f64),
    /// Size.
    pub size: f64,
    /// Drill diameter.
    pub drill: f64,
    /// Layers.
    pub layers: (String, String),
    /// Net code.
    pub net: u32,
    /// UUID.
    pub uuid: String,
}

impl Via {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (via")?;
        writeln!(writer, "    (at {} {})", self.position.0, self.position.1)?;
        writeln!(writer, "    (size {})", self.size)?;
        writeln!(writer, "    (drill {})", self.drill)?;
        writeln!(writer, "    (layers \"{}\" \"{}\")", self.layers.0, self.layers.1)?;
        writeln!(writer, "    (net {})", self.net)?;
        writeln!(writer, "    (uuid \"{}\")", self.uuid)?;
        writeln!(writer, "  )")?;
        Ok(())
    }
}

/// A zone (copper pour).
#[derive(Debug, Clone)]
pub struct Zone {
    /// Net code.
    pub net: u32,
    /// Net name.
    pub net_name: String,
    /// Layer.
    pub layer: String,
    /// Polygon points.
    pub polygon: Vec<(f64, f64)>,
    /// UUID.
    pub uuid: String,
}

impl Zone {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        writeln!(writer, "  (zone")?;
        writeln!(writer, "    (net {})", self.net)?;
        writeln!(writer, "    (net_name \"{}\")", escape_sexpr(&self.net_name))?;
        writeln!(writer, "    (layer \"{}\")", self.layer)?;
        writeln!(writer, "    (uuid \"{}\")", self.uuid)?;
        writeln!(writer, "    (hatch edge 0.5)")?;
        writeln!(writer, "    (connect_pads (clearance 0.2))")?;
        writeln!(writer, "    (min_thickness 0.2)")?;
        writeln!(writer, "    (fill yes (thermal_gap 0.2) (thermal_bridge_width 0.2))")?;
        writeln!(writer, "    (polygon")?;
        writeln!(writer, "      (pts")?;
        for pt in &self.polygon {
            writeln!(writer, "        (xy {} {})", pt.0, pt.1)?;
        }
        writeln!(writer, "      )")?;
        writeln!(writer, "    )")?;
        writeln!(writer, "  )")?;
        Ok(())
    }
}

/// Graphic elements.
#[derive(Debug, Clone)]
pub enum Graphic {
    /// A line.
    Line {
        start: (f64, f64),
        end: (f64, f64),
        layer: String,
        width: f64,
        uuid: String,
    },
    /// A circle.
    Circle {
        center: (f64, f64),
        radius: f64,
        layer: String,
        width: f64,
        uuid: String,
    },
    /// An arc.
    Arc {
        start: (f64, f64),
        mid: (f64, f64),
        end: (f64, f64),
        layer: String,
        width: f64,
        uuid: String,
    },
    /// Text.
    Text {
        text: String,
        position: (f64, f64),
        layer: String,
        uuid: String,
    },
}

impl Graphic {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        match self {
            Graphic::Line { start, end, layer, width, uuid } => {
                writeln!(writer, "  (gr_line")?;
                writeln!(writer, "    (start {} {})", start.0, start.1)?;
                writeln!(writer, "    (end {} {})", end.0, end.1)?;
                writeln!(writer, "    (stroke (width {}) (type solid))", width)?;
                writeln!(writer, "    (layer \"{}\")", layer)?;
                writeln!(writer, "    (uuid \"{}\")", uuid)?;
                writeln!(writer, "  )")?;
            }
            Graphic::Circle { center, radius, layer, width, uuid } => {
                writeln!(writer, "  (gr_circle")?;
                writeln!(writer, "    (center {} {})", center.0, center.1)?;
                writeln!(writer, "    (end {} {})", center.0 + radius, center.1)?;
                writeln!(writer, "    (stroke (width {}) (type solid))", width)?;
                writeln!(writer, "    (layer \"{}\")", layer)?;
                writeln!(writer, "    (uuid \"{}\")", uuid)?;
                writeln!(writer, "  )")?;
            }
            Graphic::Arc { start, mid, end, layer, width, uuid } => {
                writeln!(writer, "  (gr_arc")?;
                writeln!(writer, "    (start {} {})", start.0, start.1)?;
                writeln!(writer, "    (mid {} {})", mid.0, mid.1)?;
                writeln!(writer, "    (end {} {})", end.0, end.1)?;
                writeln!(writer, "    (stroke (width {}) (type solid))", width)?;
                writeln!(writer, "    (layer \"{}\")", layer)?;
                writeln!(writer, "    (uuid \"{}\")", uuid)?;
                writeln!(writer, "  )")?;
            }
            Graphic::Text { text, position, layer, uuid } => {
                writeln!(writer, "  (gr_text \"{}\"", escape_sexpr(text))?;
                writeln!(writer, "    (at {} {})", position.0, position.1)?;
                writeln!(writer, "    (layer \"{}\")", layer)?;
                writeln!(writer, "    (uuid \"{}\")", uuid)?;
                writeln!(writer, "    (effects (font (size 1 1) (thickness 0.15)))")?;
                writeln!(writer, "  )")?;
            }
        }
        Ok(())
    }
}

/// Escape a string for S-expression output.
fn escape_sexpr(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::{NetNode, NetlistComponent};

    #[test]
    fn test_pcb_new() {
        let pcb = KicadPcb::new();
        assert_eq!(pcb.version, KICAD_PCB_VERSION);
        assert_eq!(pcb.paper, "A4");
        assert!(!pcb.layers.is_empty());
    }

    #[test]
    fn test_pcb_from_netlist() {
        let mut netlist = Netlist::new();
        netlist.add_component(NetlistComponent::new("R1", "10k"));
        netlist.add_component(NetlistComponent::new("C1", "100nF"));

        let mut net = Net::new("VCC");
        net.add_node(NetNode::new("R1", "1"));
        net.add_node(NetNode::new("C1", "1"));
        netlist.add_net(net);

        let pcb = KicadPcb::from_netlist(&netlist);
        assert_eq!(pcb.footprints.len(), 2);
        assert_eq!(pcb.nets.len(), 2); // Net 0 (empty) + VCC
        assert!(!pcb.graphics.is_empty()); // Board outline
    }

    #[test]
    fn test_pcb_export() {
        let mut netlist = Netlist::new();
        netlist.add_component(NetlistComponent::new("R1", "10k"));

        let pcb = KicadPcb::from_netlist(&netlist);
        let output = pcb.export_to_string().unwrap();

        assert!(output.contains("(kicad_pcb"));
        assert!(output.contains("(version"));
        assert!(output.contains("(footprint"));
        assert!(output.contains("(layers"));
        assert!(output.contains("Edge.Cuts"));
    }

    #[test]
    fn test_default_layers() {
        let layers = default_layers();
        assert!(layers.iter().any(|l| l.name == "F.Cu"));
        assert!(layers.iter().any(|l| l.name == "B.Cu"));
        assert!(layers.iter().any(|l| l.name == "Edge.Cuts"));
    }
}
