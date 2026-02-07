//! Self-contained end-to-end tests for the atopile pipeline.
//!
//! These tests run the full pipeline (parse -> sema -> solve -> export) from
//! .ato source strings to netlist output WITHOUT requiring external packages.
//! They use only embedded stdlib modules (Resistor, Capacitor, Electrical, etc.).

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helper: run the full pipeline on an .ato source string
// ---------------------------------------------------------------------------

/// Result of a full pipeline run.
struct PipelineResult {
    design: ato_ir::Design,
    netlist: ato_export::Netlist,
}

/// Run the full atopile pipeline on the given source code.
///
/// Steps:
///   1. Write source to a temp file
///   2. Analyze with embedded stdlib
///   3. Solve constraints (non-fatal on timeout)
///   4. Build netlist
fn run_pipeline(source: &str) -> PipelineResult {
    // Write source to a temp file so analyze_file can resolve imports
    let tmp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
    let source_path = tmp_dir.path().join("test.ato");
    std::fs::write(&source_path, source).expect("failed to write source");

    // Phase 1: Semantic analysis (with embedded stdlib - no path needed)
    let mut analyzer = ato_sema::Analyzer::new();

    let design = analyzer
        .analyze_file(source, &source_path)
        .unwrap_or_else(|errors| {
            let msgs: Vec<String> = errors.iter().map(|e| format!("{:?}", e)).collect();
            panic!(
                "Semantic analysis failed ({} errors):\n  {}",
                msgs.len(),
                msgs.join("\n  ")
            );
        });

    // Phase 2: Constraint solving
    let mut solved_params: HashMap<String, String> = HashMap::new();
    if design.constraint_count() > 0 {
        let collector = ato_sema::ConstraintCollector::new();
        if let Ok((mut solver, _deps)) = collector.collect(&design) {
            match solver.solve() {
                Ok(state) => {
                    // Extract solved parameter values
                    for param_id in state.expressions.parameter_ids() {
                        if let Some(param) = state.expressions.get_parameter(param_id) {
                            if let Some(ref superset) = param.known_superset {
                                solved_params
                                    .insert(param.display_name(), format!("{}", superset));
                            }
                        }
                    }
                }
                Err(ato_solver::SolverError::Contradiction(msg)) => {
                    panic!("Solver contradiction: {}", msg);
                }
                Err(ato_solver::SolverError::Timeout { iterations, .. }) => {
                    eprintln!(
                        "Solver timed out after {} iterations (acceptable for test)",
                        iterations
                    );
                }
                Err(e) => {
                    eprintln!("Solver warning: {}", e);
                }
            }
        }
    }

    // Phase 3: Build netlist
    let entry_module = design.entry_module().or_else(|| {
        // Pick the last non-interface module with instances as entry
        design
            .modules()
            .iter()
            .rev()
            .find(|m| {
                !m.is_interface()
                    && m.fields.iter().any(|&fid| {
                        design
                            .get_field(fid)
                            .map(|f| f.is_instance())
                            .unwrap_or(false)
                    })
            })
            .map(|m| m.id)
    });

    let mut builder =
        ato_export::NetlistBuilder::new(&design).with_solved_values(solved_params);
    if let Some(entry_id) = entry_module {
        builder = builder.with_entry_module(entry_id);
    }

    let netlist = builder
        .build()
        .unwrap_or_else(|e| panic!("Netlist build failed: {}", e));

    PipelineResult { design, netlist }
}

// ===========================================================================
// Test A: Simple voltage divider (2 resistors, 3 nets)
// ===========================================================================

#[test]
fn test_e2e_standalone_voltage_divider() {
    let source = r#"
import Resistor
import Electrical

module VoltageDivider:
    input = new Electrical
    output = new Electrical
    gnd = new Electrical

    r_top = new Resistor
    r_bot = new Resistor

    input ~ r_top.unnamed[0]
    r_top.unnamed[1] ~ output
    output ~ r_bot.unnamed[0]
    r_bot.unnamed[1] ~ gnd
"#;

    let result = run_pipeline(source);

    // -- Design checks --
    assert!(
        result.design.module_count() >= 3,
        "Design should have at least VoltageDivider + Resistor + Electrical, got {}",
        result.design.module_count()
    );

    let vdiv = result
        .design
        .modules()
        .iter()
        .find(|m| m.name == "VoltageDivider");
    assert!(vdiv.is_some(), "VoltageDivider module should exist");

    // -- Netlist checks --
    // 2 resistors should appear as components
    assert_eq!(
        result.netlist.component_count(),
        2,
        "Expected 2 components (r_top + r_bot), got {}. Components: {:?}",
        result.netlist.component_count(),
        result
            .netlist
            .components
            .iter()
            .map(|c| &c.reference)
            .collect::<Vec<_>>()
    );

    // Both should have "R" prefix
    for comp in &result.netlist.components {
        assert!(
            comp.reference.starts_with('R'),
            "Resistor component should have R prefix, got '{}'",
            comp.reference
        );
    }

    // Should have at least 1 net connecting the two resistors
    assert!(
        result.netlist.net_count() >= 1,
        "Expected at least 1 net, got {}",
        result.netlist.net_count()
    );
}

// ===========================================================================
// Test B: RC filter (1 resistor + 1 capacitor)
// ===========================================================================

#[test]
fn test_e2e_standalone_rc_filter() {
    let source = r#"
import Resistor
import Capacitor
import Electrical

module RCFilter:
    input = new Electrical
    output = new Electrical
    gnd = new Electrical

    r1 = new Resistor
    c1 = new Capacitor

    input ~ r1.unnamed[0]
    r1.unnamed[1] ~ output
    output ~ c1.unnamed[0]
    c1.unnamed[1] ~ gnd
"#;

    let result = run_pipeline(source);

    // -- Netlist: 1 resistor + 1 capacitor = 2 components --
    assert_eq!(
        result.netlist.component_count(),
        2,
        "Expected 2 components (R + C), got {}. Components: {:?}",
        result.netlist.component_count(),
        result
            .netlist
            .components
            .iter()
            .map(|c| &c.reference)
            .collect::<Vec<_>>()
    );

    // Check component prefixes
    let r_count = result
        .netlist
        .components
        .iter()
        .filter(|c| c.reference.starts_with('R'))
        .count();
    let c_count = result
        .netlist
        .components
        .iter()
        .filter(|c| c.reference.starts_with('C'))
        .count();
    assert_eq!(r_count, 1, "Expected 1 resistor, got {}", r_count);
    assert_eq!(c_count, 1, "Expected 1 capacitor, got {}", c_count);

    // Designators should be unique
    let refs: Vec<&str> = result
        .netlist
        .components
        .iter()
        .map(|c| c.reference.as_str())
        .collect();
    let mut unique_refs = refs.clone();
    unique_refs.sort();
    unique_refs.dedup();
    assert_eq!(refs.len(), unique_refs.len(), "Designators must be unique");

    // Should have nets
    assert!(
        result.netlist.net_count() >= 1,
        "Expected at least 1 net connecting R and C"
    );
}

// ===========================================================================
// Test C: Module with assertions / constraints
// ===========================================================================

#[test]
fn test_e2e_standalone_constrained_resistor() {
    let source = r#"
import Resistor

module ConstrainedDesign:
    r1 = new Resistor
    assert r1.resistance within 9kohm to 11kohm
"#;

    let result = run_pipeline(source);

    // -- Design should have constraints --
    assert!(
        result.design.constraint_count() > 0,
        "Expected constraints from assert statement, got {}",
        result.design.constraint_count()
    );

    // -- Netlist should have 1 component --
    assert_eq!(
        result.netlist.component_count(),
        1,
        "Expected 1 resistor component, got {}",
        result.netlist.component_count()
    );

    let comp = &result.netlist.components[0];
    assert!(
        comp.reference.starts_with('R'),
        "Component should be a resistor (R prefix), got '{}'",
        comp.reference
    );
}

// ===========================================================================
// Test D: BOM output correctness
// ===========================================================================

#[test]
fn test_e2e_standalone_bom_output() {
    let source = r#"
import Resistor
import Capacitor

module BomTest:
    r1 = new Resistor
    r2 = new Resistor
    c1 = new Capacitor
"#;

    let result = run_pipeline(source);

    // Verify BOM generation
    let bom = ato_export::Bom::from_netlist_grouped(&result.netlist);
    let exporter = ato_export::BomExporter::new(&bom);
    let csv = exporter
        .export_to_string(ato_export::BomFormat::Jlcpcb)
        .expect("BOM export should succeed");

    // BOM should have a header
    assert!(
        csv.contains("Comment") || csv.contains("Designator"),
        "BOM CSV should have a header row"
    );

    // Total component count in BOM should match netlist
    assert_eq!(
        bom.total_components() as usize,
        result.netlist.component_count(),
        "BOM total ({}) should match netlist component count ({})",
        bom.total_components(),
        result.netlist.component_count()
    );

    // Unique line count should be <= total (grouping can reduce it)
    assert!(
        bom.unique_count() <= result.netlist.component_count(),
        "BOM unique count ({}) should be <= total ({})",
        bom.unique_count(),
        result.netlist.component_count()
    );
}

// ===========================================================================
// Test E: KiCad netlist export format
// ===========================================================================

#[test]
fn test_e2e_standalone_kicad_netlist_export() {
    let source = r#"
import Resistor

module KicadTest:
    r1 = new Resistor
"#;

    let result = run_pipeline(source);

    // KiCad netlist export
    let exporter = ato_export::KicadNetlistExporter::new(&result.netlist);
    let output = exporter
        .export_to_string()
        .expect("KiCad netlist export should succeed");

    assert!(
        output.contains("(export"),
        "Netlist should have (export tag"
    );
    assert!(
        output.contains("(components"),
        "Netlist should have (components section"
    );

    // Schematic export
    let schematic = ato_export::KicadSchematic::from_netlist(&result.netlist);
    let sch_output = schematic
        .export_to_string()
        .expect("Schematic export should succeed");
    assert!(
        sch_output.contains("kicad_sch"),
        "Schematic should have kicad_sch tag"
    );

    // PCB export
    let pcb = ato_export::KicadPcb::from_netlist(&result.netlist);
    let pcb_output = pcb
        .export_to_string()
        .expect("PCB export should succeed");
    assert!(
        pcb_output.contains("kicad_pcb"),
        "PCB should have kicad_pcb tag"
    );
}

// ===========================================================================
// Test F: No errors during pipeline for a trivial module
// ===========================================================================

#[test]
fn test_e2e_standalone_trivial_module() {
    let source = r#"
module Empty:
    pass
"#;

    let result = run_pipeline(source);

    assert_eq!(result.design.module_count(), 1);
    assert_eq!(result.design.constraint_count(), 0);
    // Empty module: no components, no nets
    assert_eq!(result.netlist.component_count(), 0);
    assert_eq!(result.netlist.net_count(), 0);
}

// ===========================================================================
// Test G: Multiple assertions are all lowered
// ===========================================================================

#[test]
fn test_e2e_standalone_multiple_assertions() {
    let source = r#"
import Resistor
import Capacitor

module MultiAssert:
    r1 = new Resistor
    c1 = new Capacitor
    assert r1.resistance within 1kohm to 100kohm
    assert c1.capacitance within 10nF to 1uF
"#;

    let result = run_pipeline(source);

    // Should have at least 2 constraints (one per assert)
    assert!(
        result.design.constraint_count() >= 2,
        "Expected at least 2 constraints, got {}",
        result.design.constraint_count()
    );

    // 2 components
    assert_eq!(
        result.netlist.component_count(),
        2,
        "Expected 2 components, got {}",
        result.netlist.component_count()
    );
}

// ===========================================================================
// Test H: Connections produce correct nets
// ===========================================================================

#[test]
fn test_e2e_standalone_connection_nets() {
    let source = r#"
import Resistor
import Electrical

module SeriesResistors:
    input = new Electrical
    output = new Electrical

    r1 = new Resistor
    r2 = new Resistor

    input ~ r1.unnamed[0]
    r1.unnamed[1] ~ r2.unnamed[0]
    r2.unnamed[1] ~ output
"#;

    let result = run_pipeline(source);

    // 2 resistors
    assert_eq!(
        result.netlist.component_count(),
        2,
        "Expected 2 resistors"
    );

    // The connection r1.unnamed[1] ~ r2.unnamed[0] should produce a net
    // connecting a pin of R1 to a pin of R2
    let has_series_net = result.netlist.nets.iter().any(|net| {
        let refs: Vec<&str> = net.nodes.iter().map(|n| n.component.as_str()).collect();
        refs.contains(&"R1") && refs.contains(&"R2")
    });

    assert!(
        has_series_net,
        "Expected a net connecting R1 and R2 in series. Nets: {:?}",
        result
            .netlist
            .nets
            .iter()
            .map(|n| format!(
                "{}: [{}]",
                n.name,
                n.nodes
                    .iter()
                    .map(|node| format!("{}.{}", node.component, node.pin))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
            .collect::<Vec<_>>()
    );
}

// ===========================================================================
// Test I: Full pipeline summary (regression guard)
// ===========================================================================

#[test]
fn test_e2e_standalone_full_pipeline_summary() {
    let source = r#"
import Resistor
import Capacitor
import Electrical

module RegressionGuard:
    power = new Electrical
    gnd = new Electrical

    r1 = new Resistor
    r2 = new Resistor
    c1 = new Capacitor

    assert r1.resistance within 9kohm to 11kohm
    assert r2.resistance within 4kohm to 5kohm
    assert c1.capacitance within 90nF to 110nF

    power ~ r1.unnamed[0]
    r1.unnamed[1] ~ r2.unnamed[0]
    r1.unnamed[1] ~ c1.unnamed[0]
    r2.unnamed[1] ~ gnd
    c1.unnamed[1] ~ gnd
"#;

    let result = run_pipeline(source);

    // Design
    assert!(result.design.module_count() >= 4); // RegressionGuard + Resistor + Capacitor + Electrical
    assert!(result.design.constraint_count() >= 3);
    assert!(result.design.connection_count() >= 5);

    // Netlist
    assert_eq!(result.netlist.component_count(), 3);

    let r_count = result
        .netlist
        .components
        .iter()
        .filter(|c| c.reference.starts_with('R'))
        .count();
    let c_count = result
        .netlist
        .components
        .iter()
        .filter(|c| c.reference.starts_with('C'))
        .count();
    assert_eq!(r_count, 2, "Expected 2 resistors");
    assert_eq!(c_count, 1, "Expected 1 capacitor");

    // Nets: at least the junction at r1.unnamed[1] should produce a net
    assert!(
        result.netlist.net_count() >= 1,
        "Expected at least 1 net, got {}",
        result.netlist.net_count()
    );

    // BOM
    let bom = ato_export::Bom::from_netlist_grouped(&result.netlist);
    assert_eq!(bom.total_components() as usize, 3);

    eprintln!("=== E2E Standalone Pipeline Summary ===");
    eprintln!(
        "Design:  {} modules, {} fields, {} connections, {} constraints",
        result.design.module_count(),
        result.design.field_count(),
        result.design.connection_count(),
        result.design.constraint_count()
    );
    eprintln!(
        "Netlist: {} components, {} nets",
        result.netlist.component_count(),
        result.netlist.net_count()
    );
    eprintln!("BOM:    {} total, {} unique lines", bom.total_components(), bom.unique_count());
}
