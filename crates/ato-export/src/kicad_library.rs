//! KiCad library mapping for symbols and footprints.
//!
//! This module provides mappings from component types and packages to
//! KiCad standard library symbols and footprints.

use std::collections::HashMap;

/// Component type for library mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentType {
    Resistor,
    Capacitor,
    CapacitorPolarized,
    Inductor,
    Diode,
    DiodeZener,
    DiodeSchottky,
    Led,
    Transistor,
    TransistorPnp,
    TransistorMosfetN,
    TransistorMosfetP,
    Crystal,
    Connector,
    Switch,
    Ic,
    Fuse,
    Ferrite,
    Unknown,
}

impl ComponentType {
    /// Determine component type from reference prefix (e.g., "R1" -> Resistor).
    pub fn from_reference(reference: &str) -> Self {
        let prefix: String = reference.chars().take_while(|c| c.is_alphabetic()).collect();
        Self::from_prefix(&prefix)
    }

    /// Determine component type from prefix string.
    pub fn from_prefix(prefix: &str) -> Self {
        match prefix.to_uppercase().as_str() {
            "R" => ComponentType::Resistor,
            "C" => ComponentType::Capacitor,
            "CP" => ComponentType::CapacitorPolarized,
            "L" => ComponentType::Inductor,
            "D" => ComponentType::Diode,
            "DZ" => ComponentType::DiodeZener,
            "DS" => ComponentType::DiodeSchottky,
            "LED" => ComponentType::Led,
            "Q" => ComponentType::Transistor,
            "Y" | "XTAL" => ComponentType::Crystal,
            "J" | "P" | "CONN" => ComponentType::Connector,
            "SW" | "S" => ComponentType::Switch,
            "U" | "IC" => ComponentType::Ic,
            "F" => ComponentType::Fuse,
            "FB" | "L_FERRITE" => ComponentType::Ferrite,
            _ => ComponentType::Unknown,
        }
    }

    /// Get the default reference prefix for this component type.
    pub fn default_prefix(&self) -> &'static str {
        match self {
            ComponentType::Resistor => "R",
            ComponentType::Capacitor => "C",
            ComponentType::CapacitorPolarized => "C",
            ComponentType::Inductor => "L",
            ComponentType::Diode => "D",
            ComponentType::DiodeZener => "D",
            ComponentType::DiodeSchottky => "D",
            ComponentType::Led => "D",
            ComponentType::Transistor => "Q",
            ComponentType::TransistorPnp => "Q",
            ComponentType::TransistorMosfetN => "Q",
            ComponentType::TransistorMosfetP => "Q",
            ComponentType::Crystal => "Y",
            ComponentType::Connector => "J",
            ComponentType::Switch => "SW",
            ComponentType::Ic => "U",
            ComponentType::Fuse => "F",
            ComponentType::Ferrite => "FB",
            ComponentType::Unknown => "X",
        }
    }
}

/// A KiCad symbol library reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRef {
    /// Library name (e.g., "Device").
    pub library: String,
    /// Symbol name within the library (e.g., "R").
    pub symbol: String,
}

impl SymbolRef {
    /// Create a new symbol reference.
    pub fn new(library: impl Into<String>, symbol: impl Into<String>) -> Self {
        Self {
            library: library.into(),
            symbol: symbol.into(),
        }
    }

    /// Get the full lib_id string (e.g., "Device:R").
    pub fn lib_id(&self) -> String {
        format!("{}:{}", self.library, self.symbol)
    }
}

/// A KiCad footprint library reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootprintRef {
    /// Library name (e.g., "Resistor_SMD").
    pub library: String,
    /// Footprint name within the library (e.g., "R_0402_1005Metric").
    pub footprint: String,
}

impl FootprintRef {
    /// Create a new footprint reference.
    pub fn new(library: impl Into<String>, footprint: impl Into<String>) -> Self {
        Self {
            library: library.into(),
            footprint: footprint.into(),
        }
    }

    /// Get the full footprint string (e.g., "Resistor_SMD:R_0402_1005Metric").
    pub fn full_name(&self) -> String {
        format!("{}:{}", self.library, self.footprint)
    }
}

/// Standard package sizes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Package {
    /// 0201 (0603 metric)
    P0201,
    /// 0402 (1005 metric)
    P0402,
    /// 0603 (1608 metric)
    P0603,
    /// 0805 (2012 metric)
    P0805,
    /// 1206 (3216 metric)
    P1206,
    /// 1210 (3225 metric)
    P1210,
    /// 2010 (5025 metric)
    P2010,
    /// 2512 (6332 metric)
    P2512,
    /// SOT-23
    Sot23,
    /// SOT-23-5
    Sot235,
    /// SOT-23-6
    Sot236,
    /// SOT-223
    Sot223,
    /// TO-220
    To220,
    /// TO-252 (DPAK)
    To252,
    /// SOIC-8
    Soic8,
    /// SOIC-14
    Soic14,
    /// SOIC-16
    Soic16,
    /// TSSOP-8
    Tssop8,
    /// TSSOP-14
    Tssop14,
    /// TSSOP-16
    Tssop16,
    /// QFP-32
    Qfp32,
    /// QFP-48
    Qfp48,
    /// QFP-64
    Qfp64,
    /// QFN-16
    Qfn16,
    /// QFN-24
    Qfn24,
    /// QFN-32
    Qfn32,
    /// Unknown/custom package
    Unknown(String),
}

impl Package {
    /// Parse a package string into a Package enum.
    pub fn from_str(s: &str) -> Self {
        let normalized = s.to_uppercase().replace(['-', '_', ' '], "");
        match normalized.as_str() {
            "0201" => Package::P0201,
            "0402" => Package::P0402,
            "0603" => Package::P0603,
            "0805" => Package::P0805,
            "1206" => Package::P1206,
            "1210" => Package::P1210,
            "2010" => Package::P2010,
            "2512" => Package::P2512,
            "SOT23" | "SOT233" => Package::Sot23,
            "SOT235" | "SOT23E" => Package::Sot235,
            "SOT236" => Package::Sot236,
            "SOT223" => Package::Sot223,
            "TO220" | "TO220AB" => Package::To220,
            "TO252" | "DPAK" => Package::To252,
            "SOIC8" | "SO8" => Package::Soic8,
            "SOIC14" | "SO14" => Package::Soic14,
            "SOIC16" | "SO16" => Package::Soic16,
            "TSSOP8" => Package::Tssop8,
            "TSSOP14" => Package::Tssop14,
            "TSSOP16" => Package::Tssop16,
            "QFP32" | "LQFP32" | "TQFP32" => Package::Qfp32,
            "QFP48" | "LQFP48" | "TQFP48" => Package::Qfp48,
            "QFP64" | "LQFP64" | "TQFP64" => Package::Qfp64,
            "QFN16" => Package::Qfn16,
            "QFN24" => Package::Qfn24,
            "QFN32" => Package::Qfn32,
            _ => Package::Unknown(s.to_string()),
        }
    }

    /// Get the full footprint suffix for chip packages (imperial_metricMetric format).
    fn footprint_suffix(&self) -> Option<&'static str> {
        match self {
            Package::P0201 => Some("0201_0603Metric"),
            Package::P0402 => Some("0402_1005Metric"),
            Package::P0603 => Some("0603_1608Metric"),
            Package::P0805 => Some("0805_2012Metric"),
            Package::P1206 => Some("1206_3216Metric"),
            Package::P1210 => Some("1210_3225Metric"),
            Package::P2010 => Some("2010_5025Metric"),
            Package::P2512 => Some("2512_6332Metric"),
            _ => None,
        }
    }
}

/// Library mapper for KiCad symbols and footprints.
#[derive(Debug, Clone)]
pub struct LibraryMapper {
    /// Symbol mappings by component type.
    symbols: HashMap<ComponentType, SymbolRef>,
    /// Default footprints for passive chip components.
    chip_footprints: HashMap<(ComponentType, Package), FootprintRef>,
}

impl Default for LibraryMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryMapper {
    /// Create a new library mapper with default KiCad library mappings.
    pub fn new() -> Self {
        let mut mapper = Self {
            symbols: HashMap::new(),
            chip_footprints: HashMap::new(),
        };
        mapper.init_default_symbols();
        mapper.init_default_footprints();
        mapper
    }

    /// Initialize default symbol mappings.
    fn init_default_symbols(&mut self) {
        // Device library symbols
        self.symbols.insert(ComponentType::Resistor, SymbolRef::new("Device", "R"));
        self.symbols.insert(ComponentType::Capacitor, SymbolRef::new("Device", "C"));
        self.symbols.insert(ComponentType::CapacitorPolarized, SymbolRef::new("Device", "CP"));
        self.symbols.insert(ComponentType::Inductor, SymbolRef::new("Device", "L"));
        self.symbols.insert(ComponentType::Diode, SymbolRef::new("Device", "D"));
        self.symbols.insert(ComponentType::DiodeZener, SymbolRef::new("Device", "D_Zener"));
        self.symbols.insert(ComponentType::DiodeSchottky, SymbolRef::new("Device", "D_Schottky"));
        self.symbols.insert(ComponentType::Led, SymbolRef::new("Device", "LED"));
        self.symbols.insert(ComponentType::Transistor, SymbolRef::new("Device", "Q_NPN_BCE"));
        self.symbols.insert(ComponentType::TransistorPnp, SymbolRef::new("Device", "Q_PNP_BCE"));
        self.symbols.insert(ComponentType::TransistorMosfetN, SymbolRef::new("Device", "Q_NMOS_GDS"));
        self.symbols.insert(ComponentType::TransistorMosfetP, SymbolRef::new("Device", "Q_PMOS_GDS"));
        self.symbols.insert(ComponentType::Crystal, SymbolRef::new("Device", "Crystal"));
        self.symbols.insert(ComponentType::Fuse, SymbolRef::new("Device", "Fuse"));
        self.symbols.insert(ComponentType::Ferrite, SymbolRef::new("Device", "Ferrite_Bead"));

        // Connector library
        self.symbols.insert(ComponentType::Connector, SymbolRef::new("Connector", "Conn_01x02_Pin"));

        // Switch library
        self.symbols.insert(ComponentType::Switch, SymbolRef::new("Switch", "SW_Push"));

        // Generic IC
        self.symbols.insert(ComponentType::Ic, SymbolRef::new("Device", "IC"));
    }

    /// Initialize default footprint mappings.
    fn init_default_footprints(&mut self) {
        // Resistor chip footprints
        for pkg in [Package::P0201, Package::P0402, Package::P0603, Package::P0805,
                    Package::P1206, Package::P1210, Package::P2010, Package::P2512] {
            if let Some(suffix) = pkg.footprint_suffix() {
                let fp_name = format!("R_{}", suffix);
                self.chip_footprints.insert(
                    (ComponentType::Resistor, pkg),
                    FootprintRef::new("Resistor_SMD", fp_name),
                );
            }
        }

        // Capacitor chip footprints
        for pkg in [Package::P0201, Package::P0402, Package::P0603, Package::P0805,
                    Package::P1206, Package::P1210] {
            if let Some(suffix) = pkg.footprint_suffix() {
                let fp_name = format!("C_{}", suffix);
                self.chip_footprints.insert(
                    (ComponentType::Capacitor, pkg),
                    FootprintRef::new("Capacitor_SMD", fp_name),
                );
            }
        }

        // Inductor chip footprints
        for pkg in [Package::P0402, Package::P0603, Package::P0805, Package::P1206] {
            if let Some(suffix) = pkg.footprint_suffix() {
                let fp_name = format!("L_{}", suffix);
                self.chip_footprints.insert(
                    (ComponentType::Inductor, pkg),
                    FootprintRef::new("Inductor_SMD", fp_name),
                );
            }
        }

        // LED chip footprints
        for pkg in [Package::P0402, Package::P0603, Package::P0805, Package::P1206] {
            if let Some(suffix) = pkg.footprint_suffix() {
                let fp_name = format!("LED_{}", suffix);
                self.chip_footprints.insert(
                    (ComponentType::Led, pkg),
                    FootprintRef::new("LED_SMD", fp_name),
                );
            }
        }

        // Diode chip footprints (typically use SOD packages but we'll map chip sizes too)
        for pkg in [Package::P0402, Package::P0603, Package::P0805] {
            if let Some(suffix) = pkg.footprint_suffix() {
                let fp_name = format!("D_{}", suffix);
                self.chip_footprints.insert(
                    (ComponentType::Diode, pkg),
                    FootprintRef::new("Diode_SMD", fp_name),
                );
            }
        }
    }

    /// Get the symbol reference for a component type.
    pub fn get_symbol(&self, component_type: ComponentType) -> SymbolRef {
        self.symbols
            .get(&component_type)
            .cloned()
            .unwrap_or_else(|| SymbolRef::new("Device", "R"))
    }

    /// Get the symbol lib_id string for a component reference.
    pub fn get_symbol_lib_id(&self, reference: &str) -> String {
        let component_type = ComponentType::from_reference(reference);
        self.get_symbol(component_type).lib_id()
    }

    /// Get the footprint reference for a component type and package.
    pub fn get_footprint(&self, component_type: ComponentType, package: &str) -> FootprintRef {
        let pkg = Package::from_str(package);

        // Try exact match first
        if let Some(fp) = self.chip_footprints.get(&(component_type, pkg.clone())) {
            return fp.clone();
        }

        // Handle special packages
        match pkg {
            Package::Sot23 => {
                return FootprintRef::new("Package_TO_SOT_SMD", "SOT-23");
            }
            Package::Sot235 => {
                return FootprintRef::new("Package_TO_SOT_SMD", "SOT-23-5");
            }
            Package::Sot236 => {
                return FootprintRef::new("Package_TO_SOT_SMD", "SOT-23-6");
            }
            Package::Sot223 => {
                return FootprintRef::new("Package_TO_SOT_SMD", "SOT-223");
            }
            Package::To220 => {
                return FootprintRef::new("Package_TO_SOT_THT", "TO-220-3_Vertical");
            }
            Package::To252 => {
                return FootprintRef::new("Package_TO_SOT_SMD", "TO-252-2");
            }
            Package::Soic8 => {
                return FootprintRef::new("Package_SO", "SOIC-8_3.9x4.9mm_P1.27mm");
            }
            Package::Soic14 => {
                return FootprintRef::new("Package_SO", "SOIC-14_3.9x8.7mm_P1.27mm");
            }
            Package::Soic16 => {
                return FootprintRef::new("Package_SO", "SOIC-16_3.9x9.9mm_P1.27mm");
            }
            Package::Tssop8 => {
                return FootprintRef::new("Package_SO", "TSSOP-8_4.4x3mm_P0.65mm");
            }
            Package::Tssop14 => {
                return FootprintRef::new("Package_SO", "TSSOP-14_4.4x5mm_P0.65mm");
            }
            Package::Tssop16 => {
                return FootprintRef::new("Package_SO", "TSSOP-16_4.4x5mm_P0.65mm");
            }
            Package::Qfp32 => {
                return FootprintRef::new("Package_QFP", "LQFP-32_7x7mm_P0.8mm");
            }
            Package::Qfp48 => {
                return FootprintRef::new("Package_QFP", "LQFP-48_7x7mm_P0.5mm");
            }
            Package::Qfp64 => {
                return FootprintRef::new("Package_QFP", "LQFP-64_10x10mm_P0.5mm");
            }
            Package::Qfn16 => {
                return FootprintRef::new("Package_DFN_QFN", "QFN-16-1EP_3x3mm_P0.5mm_EP1.75x1.75mm");
            }
            Package::Qfn24 => {
                return FootprintRef::new("Package_DFN_QFN", "QFN-24-1EP_4x4mm_P0.5mm_EP2.6x2.6mm");
            }
            Package::Qfn32 => {
                return FootprintRef::new("Package_DFN_QFN", "QFN-32-1EP_5x5mm_P0.5mm_EP3.1x3.1mm");
            }
            _ => {}
        }

        // Default footprints based on component type
        match component_type {
            ComponentType::Resistor => FootprintRef::new("Resistor_SMD", "R_0402_1005Metric"),
            ComponentType::Capacitor => FootprintRef::new("Capacitor_SMD", "C_0402_1005Metric"),
            ComponentType::CapacitorPolarized => FootprintRef::new("Capacitor_SMD", "CP_Elec_4x5.3"),
            ComponentType::Inductor => FootprintRef::new("Inductor_SMD", "L_0603_1608Metric"),
            ComponentType::Diode => FootprintRef::new("Diode_SMD", "D_SOD-123"),
            ComponentType::DiodeZener => FootprintRef::new("Diode_SMD", "D_SOD-123"),
            ComponentType::DiodeSchottky => FootprintRef::new("Diode_SMD", "D_SOD-123"),
            ComponentType::Led => FootprintRef::new("LED_SMD", "LED_0603_1608Metric"),
            ComponentType::Transistor => FootprintRef::new("Package_TO_SOT_SMD", "SOT-23"),
            ComponentType::TransistorPnp => FootprintRef::new("Package_TO_SOT_SMD", "SOT-23"),
            ComponentType::TransistorMosfetN => FootprintRef::new("Package_TO_SOT_SMD", "SOT-23"),
            ComponentType::TransistorMosfetP => FootprintRef::new("Package_TO_SOT_SMD", "SOT-23"),
            ComponentType::Crystal => FootprintRef::new("Crystal", "Crystal_SMD_3215-2Pin_3.2x1.5mm"),
            ComponentType::Connector => FootprintRef::new("Connector_PinHeader_2.54mm", "PinHeader_1x02_P2.54mm_Vertical"),
            ComponentType::Switch => FootprintRef::new("Button_Switch_SMD", "SW_SPST_TL3342"),
            ComponentType::Fuse => FootprintRef::new("Fuse", "Fuse_0603_1608Metric"),
            ComponentType::Ferrite => FootprintRef::new("Inductor_SMD", "L_0603_1608Metric"),
            ComponentType::Ic | ComponentType::Unknown => FootprintRef::new("Package_SO", "SOIC-8_3.9x4.9mm_P1.27mm"),
        }
    }

    /// Get the footprint full name for a component reference and package.
    pub fn get_footprint_name(&self, reference: &str, package: Option<&str>) -> String {
        let component_type = ComponentType::from_reference(reference);
        let pkg = package.unwrap_or("0402");
        self.get_footprint(component_type, pkg).full_name()
    }

    /// Get a list of libraries used by the current mappings.
    pub fn get_symbol_libraries(&self) -> Vec<String> {
        let mut libs: Vec<String> = self.symbols.values()
            .map(|s| s.library.clone())
            .collect();
        libs.sort();
        libs.dedup();
        libs
    }

    /// Get a list of footprint libraries used by the current mappings.
    pub fn get_footprint_libraries(&self) -> Vec<String> {
        let mut libs: Vec<String> = self.chip_footprints.values()
            .map(|f| f.library.clone())
            .collect();

        // Add common libraries that might be used
        libs.extend([
            "Resistor_SMD".to_string(),
            "Capacitor_SMD".to_string(),
            "Inductor_SMD".to_string(),
            "LED_SMD".to_string(),
            "Diode_SMD".to_string(),
            "Package_TO_SOT_SMD".to_string(),
            "Package_SO".to_string(),
            "Package_QFP".to_string(),
            "Package_DFN_QFN".to_string(),
            "Connector_PinHeader_2.54mm".to_string(),
            "Button_Switch_SMD".to_string(),
        ]);

        libs.sort();
        libs.dedup();
        libs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_type_from_reference() {
        assert_eq!(ComponentType::from_reference("R1"), ComponentType::Resistor);
        assert_eq!(ComponentType::from_reference("R100"), ComponentType::Resistor);
        assert_eq!(ComponentType::from_reference("C1"), ComponentType::Capacitor);
        assert_eq!(ComponentType::from_reference("L1"), ComponentType::Inductor);
        assert_eq!(ComponentType::from_reference("D1"), ComponentType::Diode);
        assert_eq!(ComponentType::from_reference("LED1"), ComponentType::Led);
        assert_eq!(ComponentType::from_reference("U1"), ComponentType::Ic);
        assert_eq!(ComponentType::from_reference("Q1"), ComponentType::Transistor);
    }

    #[test]
    fn test_package_parsing() {
        assert_eq!(Package::from_str("0402"), Package::P0402);
        assert_eq!(Package::from_str("0603"), Package::P0603);
        assert_eq!(Package::from_str("SOT-23"), Package::Sot23);
        assert_eq!(Package::from_str("SOIC-8"), Package::Soic8);
        assert_eq!(Package::from_str("QFP-32"), Package::Qfp32);
    }

    #[test]
    fn test_symbol_ref() {
        let sym = SymbolRef::new("Device", "R");
        assert_eq!(sym.lib_id(), "Device:R");
    }

    #[test]
    fn test_footprint_ref() {
        let fp = FootprintRef::new("Resistor_SMD", "R_0402_1005Metric");
        assert_eq!(fp.full_name(), "Resistor_SMD:R_0402_1005Metric");
    }

    #[test]
    fn test_library_mapper_symbols() {
        let mapper = LibraryMapper::new();

        assert_eq!(mapper.get_symbol_lib_id("R1"), "Device:R");
        assert_eq!(mapper.get_symbol_lib_id("C1"), "Device:C");
        assert_eq!(mapper.get_symbol_lib_id("L1"), "Device:L");
        assert_eq!(mapper.get_symbol_lib_id("D1"), "Device:D");
        assert_eq!(mapper.get_symbol_lib_id("LED1"), "Device:LED");
        assert_eq!(mapper.get_symbol_lib_id("U1"), "Device:IC");
    }

    #[test]
    fn test_library_mapper_footprints() {
        let mapper = LibraryMapper::new();

        assert_eq!(
            mapper.get_footprint_name("R1", Some("0402")),
            "Resistor_SMD:R_0402_1005Metric"
        );
        assert_eq!(
            mapper.get_footprint_name("C1", Some("0603")),
            "Capacitor_SMD:C_0603_1608Metric"
        );
        assert_eq!(
            mapper.get_footprint_name("R1", Some("0805")),
            "Resistor_SMD:R_0805_2012Metric"
        );
        assert_eq!(
            mapper.get_footprint_name("U1", Some("SOIC-8")),
            "Package_SO:SOIC-8_3.9x4.9mm_P1.27mm"
        );
    }

    #[test]
    fn test_library_mapper_default_footprint() {
        let mapper = LibraryMapper::new();

        // Should return 0402 for unknown package
        let fp = mapper.get_footprint_name("R1", None);
        assert!(fp.contains("0402"));
    }

    #[test]
    fn test_get_libraries() {
        let mapper = LibraryMapper::new();

        let symbol_libs = mapper.get_symbol_libraries();
        assert!(symbol_libs.contains(&"Device".to_string()));

        let fp_libs = mapper.get_footprint_libraries();
        assert!(fp_libs.contains(&"Resistor_SMD".to_string()));
        assert!(fp_libs.contains(&"Capacitor_SMD".to_string()));
    }
}
