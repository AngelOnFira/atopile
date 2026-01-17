//! KiCad project file generation (.kicad_pro).
//!
//! This module generates KiCad project files in JSON format.

use std::io::Write;
use serde::{Deserialize, Serialize};

use crate::kicad_library::LibraryMapper;
use crate::ExportError;

/// KiCad project file version.
const KICAD_PROJECT_VERSION: u32 = 1;

/// A complete KiCad project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KicadProject {
    /// Board settings.
    #[serde(default)]
    pub board: BoardSettings,
    /// PCBNew settings.
    #[serde(default)]
    pub pcbnew: PcbnewSettings,
    /// Library settings.
    #[serde(default)]
    pub libraries: LibrarySettings,
    /// Project metadata.
    pub meta: ProjectMeta,
    /// Net settings.
    #[serde(default)]
    pub net_settings: NetSettings,
    /// Schematic settings.
    #[serde(default)]
    pub schematic: SchematicSettings,
    /// Text variables.
    #[serde(default)]
    pub text_variables: std::collections::HashMap<String, String>,
}

impl Default for KicadProject {
    fn default() -> Self {
        Self::new("project")
    }
}

impl KicadProject {
    /// Create a new project with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            board: BoardSettings::default(),
            pcbnew: PcbnewSettings::default(),
            libraries: LibrarySettings::default(),
            meta: ProjectMeta {
                filename: format!("{}.kicad_pro", name),
                version: KICAD_PROJECT_VERSION,
            },
            net_settings: NetSettings::default(),
            schematic: SchematicSettings::default(),
            text_variables: std::collections::HashMap::new(),
        }
    }

    /// Export the project to JSON.
    pub fn to_json(&self) -> Result<String, ExportError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ExportError::Encoding(e.to_string()))
    }

    /// Export the project to a writer.
    pub fn export<W: Write>(&self, writer: &mut W) -> Result<(), ExportError> {
        let json = self.to_json()?;
        writer.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Configure the project with commonly used libraries.
    pub fn configure_libraries(&mut self) {
        let mapper = LibraryMapper::new();

        // Add commonly used symbol libraries
        self.libraries.pinned_symbol_libs = mapper.get_symbol_libraries();

        // Add commonly used footprint libraries
        self.libraries.pinned_footprint_libs = mapper.get_footprint_libraries();
    }

    /// Create a new project with default library configuration.
    pub fn new_with_libraries(name: &str) -> Self {
        let mut project = Self::new(name);
        project.configure_libraries();
        project
    }
}

/// Project metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    /// Filename.
    pub filename: String,
    /// Version number.
    pub version: u32,
}

/// Board settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoardSettings {
    /// Design settings.
    #[serde(default)]
    pub design_settings: DesignSettings,
    /// 3D viewports.
    #[serde(rename = "3dviewports", default)]
    pub viewports_3d: Vec<serde_json::Value>,
    /// Layer presets.
    #[serde(default)]
    pub layer_presets: Vec<serde_json::Value>,
    /// Viewports.
    #[serde(default)]
    pub viewports: Vec<serde_json::Value>,
}

/// Design settings for the board.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignSettings {
    /// Default settings.
    #[serde(default)]
    pub defaults: DesignDefaults,
    /// DRC exclusions.
    #[serde(default)]
    pub drc_exclusions: Vec<String>,
    /// Metadata.
    #[serde(default)]
    pub meta: DesignMeta,
    /// Design rules.
    #[serde(default)]
    pub rules: DesignRules,
    /// Track widths.
    #[serde(default)]
    pub track_widths: Vec<f64>,
    /// Via dimensions.
    #[serde(default)]
    pub via_dimensions: Vec<serde_json::Value>,
}

impl Default for DesignSettings {
    fn default() -> Self {
        Self {
            defaults: DesignDefaults::default(),
            drc_exclusions: Vec::new(),
            meta: DesignMeta { version: 2 },
            rules: DesignRules::default(),
            track_widths: Vec::new(),
            via_dimensions: Vec::new(),
        }
    }
}

/// Design metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DesignMeta {
    /// Version number.
    pub version: u32,
}

/// Default design values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignDefaults {
    /// Board outline line width.
    pub board_outline_line_width: f64,
    /// Copper line width.
    pub copper_line_width: f64,
    /// Copper text size (horizontal).
    pub copper_text_size_h: f64,
    /// Copper text size (vertical).
    pub copper_text_size_v: f64,
    /// Copper text thickness.
    pub copper_text_thickness: f64,
    /// Silk line width.
    pub silk_line_width: f64,
    /// Silk text size (horizontal).
    pub silk_text_size_h: f64,
    /// Silk text size (vertical).
    pub silk_text_size_v: f64,
    /// Silk text thickness.
    pub silk_text_thickness: f64,
}

impl Default for DesignDefaults {
    fn default() -> Self {
        Self {
            board_outline_line_width: 0.05,
            copper_line_width: 0.2,
            copper_text_size_h: 1.5,
            copper_text_size_v: 1.5,
            copper_text_thickness: 0.3,
            silk_line_width: 0.1,
            silk_text_size_h: 1.0,
            silk_text_size_v: 1.0,
            silk_text_thickness: 0.1,
        }
    }
}

/// Design rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignRules {
    /// Maximum error.
    pub max_error: f64,
    /// Minimum clearance.
    pub min_clearance: f64,
    /// Minimum track width.
    pub min_track_width: f64,
    /// Minimum via diameter.
    pub min_via_diameter: f64,
    /// Minimum via drill.
    pub min_via_annular_width: f64,
}

impl Default for DesignRules {
    fn default() -> Self {
        Self {
            max_error: 0.005,
            min_clearance: 0.0,
            min_track_width: 0.0,
            min_via_diameter: 0.5,
            min_via_annular_width: 0.1,
        }
    }
}

/// PCBNew settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PcbnewSettings {
    /// Last used paths.
    #[serde(default)]
    pub last_paths: LastPaths,
    /// Page layout description file.
    #[serde(default)]
    pub page_layout_descr_file: String,
}

/// Last used paths.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LastPaths {
    /// Netlist path.
    #[serde(default)]
    pub netlist: String,
    /// Plot output path.
    #[serde(default)]
    pub plot: String,
    /// Position files path.
    #[serde(default)]
    pub pos_files: String,
}

/// Library settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibrarySettings {
    /// Pinned footprint libraries.
    #[serde(default)]
    pub pinned_footprint_libs: Vec<String>,
    /// Pinned symbol libraries.
    #[serde(default)]
    pub pinned_symbol_libs: Vec<String>,
}

/// Net settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetSettings {
    /// Net classes.
    #[serde(default)]
    pub classes: Vec<NetClass>,
    /// Metadata.
    #[serde(default)]
    pub meta: NetSettingsMeta,
    /// Net colors.
    #[serde(default)]
    pub net_colors: Option<serde_json::Value>,
    /// Net class assignments.
    #[serde(default)]
    pub netclass_assignments: Option<serde_json::Value>,
    /// Net class patterns.
    #[serde(default)]
    pub netclass_patterns: Vec<serde_json::Value>,
}

impl Default for NetSettings {
    fn default() -> Self {
        Self {
            classes: vec![NetClass::default()],
            meta: NetSettingsMeta { version: 3 },
            net_colors: None,
            netclass_assignments: None,
            netclass_patterns: Vec::new(),
        }
    }
}

/// Net settings metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetSettingsMeta {
    /// Version.
    pub version: u32,
}

/// A net class definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetClass {
    /// Class name.
    pub name: String,
    /// Clearance.
    pub clearance: f64,
    /// Track width.
    pub track_width: f64,
    /// Via diameter.
    pub via_diameter: f64,
    /// Via drill.
    pub via_drill: f64,
    /// Differential pair width.
    pub diff_pair_width: f64,
    /// Differential pair gap.
    pub diff_pair_gap: f64,
}

impl Default for NetClass {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            clearance: 0.2,
            track_width: 0.2,
            via_diameter: 0.6,
            via_drill: 0.3,
            diff_pair_width: 0.2,
            diff_pair_gap: 0.25,
        }
    }
}

/// Schematic settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchematicSettings {
    /// Legacy library directory.
    #[serde(default)]
    pub legacy_lib_dir: String,
    /// Legacy library list.
    #[serde(default)]
    pub legacy_lib_list: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kicad_project_new() {
        let project = KicadProject::new("test_project");
        assert_eq!(project.meta.filename, "test_project.kicad_pro");
        assert_eq!(project.meta.version, 1);
    }

    #[test]
    fn test_kicad_project_to_json() {
        let project = KicadProject::new("test");
        let json = project.to_json().unwrap();

        assert!(json.contains("\"filename\": \"test.kicad_pro\""));
        assert!(json.contains("\"version\": 1"));
        assert!(json.contains("\"net_settings\""));
    }

    #[test]
    fn test_net_class_default() {
        let nc = NetClass::default();
        assert_eq!(nc.name, "Default");
        assert_eq!(nc.clearance, 0.2);
        assert_eq!(nc.track_width, 0.2);
    }

    #[test]
    fn test_kicad_project_with_libraries() {
        let project = KicadProject::new_with_libraries("test");

        // Should have symbol libraries configured
        assert!(!project.libraries.pinned_symbol_libs.is_empty());
        assert!(project.libraries.pinned_symbol_libs.contains(&"Device".to_string()));

        // Should have footprint libraries configured
        assert!(!project.libraries.pinned_footprint_libs.is_empty());
        assert!(project.libraries.pinned_footprint_libs.contains(&"Resistor_SMD".to_string()));
    }

    #[test]
    fn test_kicad_project_libraries_in_json() {
        let project = KicadProject::new_with_libraries("test");
        let json = project.to_json().unwrap();

        assert!(json.contains("pinned_symbol_libs"));
        assert!(json.contains("pinned_footprint_libs"));
        assert!(json.contains("Device"));
        assert!(json.contains("Resistor_SMD"));
    }
}
