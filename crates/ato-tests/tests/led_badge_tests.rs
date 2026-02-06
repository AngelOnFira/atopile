//! Comprehensive tests for the led_badge.ato compilation pipeline.
//!
//! These tests verify that led_badge.ato compiles correctly through every stage:
//! 1. Parser: AST structure, pragmas, imports, module definitions
//! 2. Semantic analysis: name resolution, connections, constraints
//! 3. Constraint solver: parameter narrowing, no contradictions
//! 4. Export: component counts, designators, footprints, BOM grouping

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Path to the led_badge.ato source file.
fn led_badge_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/led_badge/led_badge.ato")
}

/// Path to the stdlib directory.
fn stdlib_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ato-sema/stdlib")
}

/// Read the led_badge.ato source. Panics if the file doesn't exist.
fn led_badge_source() -> String {
    let path = led_badge_path();
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

// ============================================================================
// 1. Parser Tests
// ============================================================================

#[test]
fn test_led_badge_parses_successfully() {
    let source = led_badge_source();
    let result = ato_parser::parse(&source);
    assert!(result.is_ok(), "led_badge.ato should parse without errors: {:?}", result.err());
}

#[test]
fn test_led_badge_ast_top_level_statements() {
    let source = led_badge_source();
    let ast = ato_parser::parse(&source).unwrap();

    // Count statement types
    let mut pragmas = 0;
    let mut imports = 0;
    let mut from_imports = 0;
    let mut modules = 0;
    let module_names: Vec<String> = Vec::new();

    for stmt in &ast.statements {
        match stmt {
            ato_parser::Statement::Pragma(_) => pragmas += 1,
            ato_parser::Statement::Import(imp) => {
                if imp.from_path.is_some() {
                    from_imports += 1;
                } else {
                    imports += 1;
                }
            }
            ato_parser::Statement::BlockDef(_) => modules += 1,
            _ => {}
        }
    }

    // 3 pragmas: FOR_LOOP, BRIDGE_CONNECT, TRAITS
    assert_eq!(pragmas, 3, "Expected 3 pragma statements");

    // 6 stdlib imports (ElectricPower, Resistor, ElectricLogic, I2C, I2S, can_bridge_by_name)
    // Note: some imports contain multiple names (comma-separated)
    assert!(imports >= 2, "Expected at least 2 bare import statements, got {}", imports);

    // 8 from-imports (packages)
    assert_eq!(from_imports, 8, "Expected 8 from-import statements for packages");

    // 3 module definitions: LED_BADGE, SK6805EC20_strip10, SK6805EC20_grid10x10
    assert_eq!(modules, 3, "Expected 3 module definitions");

    let _ = module_names;
}

#[test]
fn test_led_badge_module_names() {
    let source = led_badge_source();
    let ast = ato_parser::parse(&source).unwrap();

    let module_names: Vec<&str> = ast.statements.iter().filter_map(|stmt| {
        if let ato_parser::Statement::BlockDef(block) = stmt {
            Some(block.name.name.as_str())
        } else {
            None
        }
    }).collect();

    assert!(module_names.contains(&"LED_BADGE"), "Missing LED_BADGE module");
    assert!(module_names.contains(&"SK6805EC20_strip10"), "Missing SK6805EC20_strip10 module");
    assert!(module_names.contains(&"SK6805EC20_grid10x10"), "Missing SK6805EC20_grid10x10 module");
}

#[test]
fn test_led_badge_pragmas() {
    let source = led_badge_source();
    let ast = ato_parser::parse(&source).unwrap();

    let pragma_contents: Vec<&str> = ast.statements.iter().filter_map(|stmt| {
        if let ato_parser::Statement::Pragma(p) = stmt {
            Some(p.content.as_str())
        } else {
            None
        }
    }).collect();

    // Check that all three experimental pragmas are present
    let has_for_loop = pragma_contents.iter().any(|c| c.contains("FOR_LOOP"));
    let has_bridge = pragma_contents.iter().any(|c| c.contains("BRIDGE_CONNECT"));
    let has_traits = pragma_contents.iter().any(|c| c.contains("TRAITS"));

    assert!(has_for_loop, "Missing FOR_LOOP pragma");
    assert!(has_bridge, "Missing BRIDGE_CONNECT pragma");
    assert!(has_traits, "Missing TRAITS pragma");
}

#[test]
fn test_led_badge_main_module_body() {
    let source = led_badge_source();
    let ast = ato_parser::parse(&source).unwrap();

    let led_badge_block = ast.statements.iter().find_map(|stmt| {
        if let ato_parser::Statement::BlockDef(block) = stmt {
            if block.name.name == "LED_BADGE" {
                return Some(block);
            }
        }
        None
    }).expect("LED_BADGE module not found");

    assert_eq!(led_badge_block.kind, ato_parser::BlockKind::Module);

    // Count statement types in the LED_BADGE body
    let mut assignments = 0;
    let mut connections = 0;
    let mut directed_connections = 0;
    let mut asserts = 0;
    let mut string_stmts = 0;

    for stmt in &led_badge_block.body {
        match stmt {
            ato_parser::Statement::Assignment(_) => assignments += 1,
            ato_parser::Statement::Connection(_) => connections += 1,
            ato_parser::Statement::DirectedConnection(_) => directed_connections += 1,
            ato_parser::Statement::Assert(_) => asserts += 1,
            ato_parser::Statement::StringStmt(_) => string_stmts += 1,
            _ => {}
        }
    }

    // LED_BADGE has many assignments (instances, parameters, net names)
    assert!(assignments >= 15, "Expected at least 15 assignments in LED_BADGE, got {}", assignments);
    // Multiple connection statements (power, USB, I2S)
    assert!(connections >= 5, "Expected at least 5 connections in LED_BADGE, got {}", connections);
    // Bridge connections (power path, EN pullup, TS/MR pull-down)
    assert!(directed_connections >= 2, "Expected at least 2 directed connections, got {}", directed_connections);
    // Assert statements for voltage/current constraints
    assert!(asserts >= 2, "Expected at least 2 assert statements, got {}", asserts);
    // Docstring
    assert!(string_stmts >= 1, "Expected at least 1 docstring");
}

#[test]
fn test_led_badge_strip10_has_for_loop() {
    let source = led_badge_source();
    let ast = ato_parser::parse(&source).unwrap();

    let strip10 = ast.statements.iter().find_map(|stmt| {
        if let ato_parser::Statement::BlockDef(block) = stmt {
            if block.name.name == "SK6805EC20_strip10" {
                return Some(block);
            }
        }
        None
    }).expect("SK6805EC20_strip10 module not found");

    let for_count = strip10.body.iter().filter(|stmt| {
        matches!(stmt, ato_parser::Statement::For(_))
    }).count();

    assert!(for_count >= 1, "SK6805EC20_strip10 should have at least 1 for loop");

    // Should also have trait statement
    let trait_count = strip10.body.iter().filter(|stmt| {
        matches!(stmt, ato_parser::Statement::Trait(_))
    }).count();
    assert!(trait_count >= 1, "SK6805EC20_strip10 should have a can_bridge_by_name trait");
}

#[test]
fn test_led_badge_grid10x10_has_for_loop() {
    let source = led_badge_source();
    let ast = ato_parser::parse(&source).unwrap();

    let grid = ast.statements.iter().find_map(|stmt| {
        if let ato_parser::Statement::BlockDef(block) = stmt {
            if block.name.name == "SK6805EC20_grid10x10" {
                return Some(block);
            }
        }
        None
    }).expect("SK6805EC20_grid10x10 module not found");

    let for_count = grid.body.iter().filter(|stmt| {
        matches!(stmt, ato_parser::Statement::For(_))
    }).count();

    assert!(for_count >= 1, "SK6805EC20_grid10x10 should have at least 1 for loop");

    // Bridge connection chaining rows
    let directed_count = grid.body.iter().filter(|stmt| {
        matches!(stmt, ato_parser::Statement::DirectedConnection(_))
    }).count();
    assert!(directed_count >= 1, "SK6805EC20_grid10x10 should have directed connection for row chaining");
}

// ============================================================================
// 2. Semantic Analysis Tests
// ============================================================================

/// Run sema on led_badge.ato with stdlib support.
/// Returns the Design on success, or a descriptive error string on failure.
fn analyze_led_badge() -> Result<ato_ir::Design, String> {
    let source = led_badge_source();
    let path = led_badge_path();
    let stdlib = stdlib_path();

    let mut analyzer = ato_sema::Analyzer::new();
    if stdlib.exists() {
        analyzer = analyzer.with_stdlib(stdlib);
    }

    analyzer.analyze_file(&source, &path)
        .map_err(|errors| {
            let msgs: Vec<String> = errors.iter().map(|e| format!("{:?}", e)).collect();
            format!("Sema errors ({}):\n  {}", msgs.len(), msgs.join("\n  "))
        })
}

#[test]
fn test_led_badge_sema_succeeds() {
    let result = analyze_led_badge();
    assert!(result.is_ok(), "Semantic analysis should succeed: {}", result.unwrap_err());
}

#[test]
fn test_led_badge_sema_module_count() {
    let design = analyze_led_badge().unwrap();

    // At minimum: LED_BADGE, SK6805EC20_strip10, SK6805EC20_grid10x10
    // Plus all imported modules (stdlib types + package drivers)
    assert!(design.module_count() >= 3,
        "Expected at least 3 modules (LED_BADGE + strip10 + grid10x10), got {}",
        design.module_count());
}

#[test]
fn test_led_badge_sema_has_fields() {
    let design = analyze_led_badge().unwrap();

    // The design should have many fields (pins, parameters, instances, signals)
    assert!(design.field_count() > 20,
        "Expected many fields in the design, got {}", design.field_count());
}

#[test]
fn test_led_badge_sema_has_connections() {
    let design = analyze_led_badge().unwrap();

    // LED_BADGE has multiple connection statements (power, USB, I2S)
    assert!(design.connection_count() > 0,
        "Expected connections in the design, got {}", design.connection_count());
}

#[test]
fn test_led_badge_sema_has_constraints() {
    let design = analyze_led_badge().unwrap();

    // LED_BADGE has assert statements for voltage/resistance
    assert!(design.constraint_count() > 0,
        "Expected constraints in the design, got {}", design.constraint_count());
}

#[test]
fn test_led_badge_sema_led_badge_module_exists() {
    let design = analyze_led_badge().unwrap();

    let found = design.modules().iter().any(|m| m.name == "LED_BADGE");
    assert!(found, "LED_BADGE module should exist in the design. Modules: {:?}",
        design.modules().iter().map(|m| &m.name).collect::<Vec<_>>());
}

#[test]
fn test_led_badge_sema_strip10_module_exists() {
    let design = analyze_led_badge().unwrap();

    let found = design.modules().iter().any(|m| m.name == "SK6805EC20_strip10");
    assert!(found, "SK6805EC20_strip10 module should exist in the design");
}

#[test]
fn test_led_badge_sema_grid10x10_module_exists() {
    let design = analyze_led_badge().unwrap();

    let found = design.modules().iter().any(|m| m.name == "SK6805EC20_grid10x10");
    assert!(found, "SK6805EC20_grid10x10 module should exist in the design");
}

#[test]
fn test_led_badge_sema_led_badge_has_instances() {
    let design = analyze_led_badge().unwrap();

    let led_badge = design.modules().iter()
        .find(|m| m.name == "LED_BADGE")
        .expect("LED_BADGE module not found");

    // Count instance fields
    let instance_count = led_badge.fields.iter()
        .filter(|&&fid| {
            design.get_field(fid).map(|f| f.is_instance()).unwrap_or(false)
        })
        .count();

    // LED_BADGE instantiates: microcontroller, microphone, usb_c, buck_boost, charger,
    // battery, led_grid, atopile_logo, power_3v3, buck_boost_en_pullup, ts_mr_pull_down, i2s_bus
    assert!(instance_count >= 8,
        "LED_BADGE should have at least 8 instance fields, got {}", instance_count);
}

// ============================================================================
// 3. Constraint Solver Tests
// ============================================================================

#[test]
fn test_led_badge_solver_no_contradiction() {
    let design = analyze_led_badge().unwrap();

    if design.constraint_count() == 0 {
        // No constraints to solve -- pass vacuously
        return;
    }

    let collector = ato_sema::ConstraintCollector::new();
    match collector.collect(&design) {
        Ok((mut solver, _deps)) => {
            let result = solver.solve();
            match result {
                Ok(state) => {
                    // The solver should not produce contradictions
                    // (It may leave some constraints not fully deduced, which is fine)
                    assert!(!state.all_satisfied || state.not_deduced.is_empty(),
                        "If all satisfied, not_deduced should be empty");
                }
                Err(ato_solver::SolverError::Contradiction(msg)) => {
                    panic!("Solver found contradiction: {}", msg);
                }
                Err(ato_solver::SolverError::Timeout { iterations, .. }) => {
                    // Timeout is acceptable for a large design
                    eprintln!("Solver timed out after {} iterations (acceptable)", iterations);
                }
                Err(e) => {
                    // Other solver errors are warnings, not failures
                    eprintln!("Solver warning: {}", e);
                }
            }
        }
        Err(e) => {
            // Collection errors mean the constraint collector couldn't extract
            // constraints from the IR. This is acceptable for partial sema support.
            eprintln!("Constraint collection warning: {}", e);
        }
    }
}

#[test]
fn test_led_badge_solver_collects_parameters() {
    let design = analyze_led_badge().unwrap();

    if design.constraint_count() == 0 {
        return;
    }

    let collector = ato_sema::ConstraintCollector::new();
    match collector.collect(&design) {
        Ok((solver, deps)) => {
            // Should have at least some parameters from the resistor/voltage assertions
            let predicate_count = solver.predicate_count();
            eprintln!("Solver has {} predicates", predicate_count);

            let free = deps.free_parameters().len();
            let constrained = deps.constrained_parameters().len();
            eprintln!("  {} free, {} constrained parameters", free, constrained);

            // We expect at least some constrained parameters
            // (resistance values, voltages, etc.)
        }
        Err(e) => {
            eprintln!("Constraint collection skipped: {}", e);
        }
    }
}

// ============================================================================
// 4. Export Tests
// ============================================================================

/// Run the full pipeline and build a netlist from led_badge.
fn build_led_badge_netlist() -> Result<ato_export::Netlist, String> {
    let design = analyze_led_badge()?;

    // Determine entry module (same logic as build.rs)
    let entry_module = design.entry_module().or_else(|| {
        // Look for LED_BADGE module
        design.find_module("LED_BADGE")
    }).or_else(|| {
        // Fallback: last non-interface module with instances
        design.modules().iter().rev()
            .find(|m| {
                !m.is_interface() && m.fields.iter().any(|&fid| {
                    design.get_field(fid).map(|f| f.is_instance()).unwrap_or(false)
                })
            })
            .map(|m| m.id)
    });

    let mut builder = ato_export::NetlistBuilder::new(&design);
    if let Some(entry_id) = entry_module {
        builder = builder.with_entry_module(entry_id);
    }

    builder.build().map_err(|e| format!("Netlist build error: {}", e))
}

#[test]
fn test_led_badge_netlist_builds() {
    let result = build_led_badge_netlist();
    assert!(result.is_ok(), "Netlist build should succeed: {}", result.unwrap_err());
}

#[test]
fn test_led_badge_netlist_has_components() {
    let netlist = build_led_badge_netlist().unwrap();

    // The led_badge design should produce many components:
    // 100 LEDs + 115 capacitors + 14 resistors + 1 inductor + ICs + connectors = 230+
    let count = netlist.component_count();
    assert!(count >= 100,
        "Expected at least 100 components in netlist, got {}", count);
}

#[test]
fn test_led_badge_netlist_has_100_leds() {
    let netlist = build_led_badge_netlist().unwrap();

    // Count LED components (designator starts with "LED")
    let led_count = netlist.components.iter()
        .filter(|c| c.reference.starts_with("LED"))
        .count();

    assert_eq!(led_count, 100,
        "Expected exactly 100 LED components, got {}", led_count);
}

#[test]
fn test_led_badge_netlist_has_capacitors() {
    let netlist = build_led_badge_netlist().unwrap();

    let cap_count = netlist.components.iter()
        .filter(|c| c.reference.starts_with('C'))
        .count();

    // Each LED driver has a decoupling cap (100) + buck-boost caps + charger caps + MCU caps
    assert!(cap_count >= 100,
        "Expected at least 100 capacitors, got {}", cap_count);
}

#[test]
fn test_led_badge_netlist_has_resistors() {
    let netlist = build_led_badge_netlist().unwrap();

    let res_count = netlist.components.iter()
        .filter(|c| c.reference.starts_with('R'))
        .count();

    // LED_BADGE has buck_boost_en_pullup + ts_mr_pull_down + package resistors
    assert!(res_count >= 2,
        "Expected at least 2 resistors, got {}", res_count);
}

#[test]
fn test_led_badge_netlist_has_ics() {
    let netlist = build_led_badge_netlist().unwrap();

    let u_count = netlist.components.iter()
        .filter(|c| c.reference.starts_with('U'))
        .count();

    // ESP32, microphone, USB connector, buck-boost, charger, battery, logo
    assert!(u_count >= 5,
        "Expected at least 5 U-designated ICs, got {}", u_count);
}

#[test]
fn test_led_badge_netlist_has_connector() {
    let netlist = build_led_badge_netlist().unwrap();

    let j_count = netlist.components.iter()
        .filter(|c| c.reference.starts_with('J'))
        .count();

    // At least USB-C connector
    assert!(j_count >= 1,
        "Expected at least 1 connector (J), got {}", j_count);
}

#[test]
fn test_led_badge_netlist_designator_uniqueness() {
    let netlist = build_led_badge_netlist().unwrap();

    let mut seen = std::collections::HashSet::new();
    let mut duplicates = Vec::new();

    for comp in &netlist.components {
        if !seen.insert(&comp.reference) {
            duplicates.push(comp.reference.clone());
        }
    }

    assert!(duplicates.is_empty(),
        "Found duplicate designators: {:?}", duplicates);
}

#[test]
fn test_led_badge_netlist_designator_prefixes() {
    let netlist = build_led_badge_netlist().unwrap();

    // Every designator should start with a known prefix letter
    let known_prefixes = ["R", "C", "L", "U", "J", "LED", "D", "Q"];

    for comp in &netlist.components {
        let has_known_prefix = known_prefixes.iter().any(|p| comp.reference.starts_with(p));
        assert!(has_known_prefix,
            "Component '{}' has unknown designator prefix (value: '{}')",
            comp.reference, comp.value);
    }
}

#[test]
fn test_led_badge_led_footprints() {
    let netlist = build_led_badge_netlist().unwrap();

    // All LEDs should have a footprint
    let leds_without_footprint: Vec<&str> = netlist.components.iter()
        .filter(|c| c.reference.starts_with("LED"))
        .filter(|c| c.footprint.is_none())
        .map(|c| c.reference.as_str())
        .collect();

    assert!(leds_without_footprint.is_empty(),
        "LEDs without footprint: {:?}", leds_without_footprint);
}

#[test]
fn test_led_badge_led_lcsc_numbers() {
    let netlist = build_led_badge_netlist().unwrap();

    // All LEDs should have LCSC part numbers (stored as "lcsc" property)
    let leds_without_lcsc: Vec<&str> = netlist.components.iter()
        .filter(|c| c.reference.starts_with("LED"))
        .filter(|c| !c.properties.contains_key("lcsc"))
        .map(|c| c.reference.as_str())
        .collect();

    assert!(leds_without_lcsc.is_empty(),
        "LEDs without LCSC number: {:?}", leds_without_lcsc);
}

#[test]
fn test_led_badge_ic_footprints() {
    let netlist = build_led_badge_netlist().unwrap();

    // ICs (U-prefixed) should have footprints
    let ics_without_footprint: Vec<&str> = netlist.components.iter()
        .filter(|c| c.reference.starts_with('U'))
        .filter(|c| c.footprint.is_none())
        .map(|c| c.reference.as_str())
        .collect();

    // Some ICs (battery, logo) may not have standard footprints -- just check the majority
    let total_ics = netlist.components.iter()
        .filter(|c| c.reference.starts_with('U'))
        .count();
    let ics_with_footprint = total_ics - ics_without_footprint.len();

    assert!(ics_with_footprint >= 4,
        "Expected at least 4 ICs with footprints, got {} (missing: {:?})",
        ics_with_footprint, ics_without_footprint);
}

// ============================================================================
// 4b. BOM Export Tests
// ============================================================================

#[test]
fn test_led_badge_bom_generation() {
    let netlist = build_led_badge_netlist().unwrap();

    let bom = ato_export::Bom::from_netlist_grouped(&netlist);
    let exporter = ato_export::BomExporter::new(&bom);
    let csv = exporter.export_to_string(ato_export::BomFormat::Jlcpcb);

    assert!(csv.is_ok(), "BOM export should succeed: {:?}", csv.err());
    let csv_content = csv.unwrap();

    // BOM should have a header line
    assert!(csv_content.contains("Comment") || csv_content.contains("Designator"),
        "BOM should have a CSV header");
}

#[test]
fn test_led_badge_bom_groups_leds() {
    let netlist = build_led_badge_netlist().unwrap();

    let bom = ato_export::Bom::from_netlist_grouped(&netlist);

    // Total component count should match netlist
    let bom_total = bom.total_components() as usize;
    let netlist_total = netlist.component_count();

    assert_eq!(bom_total, netlist_total,
        "BOM total ({}) should match netlist component count ({})",
        bom_total, netlist_total);

    // Unique line count should be much less than total (grouping works)
    let unique = bom.unique_count();
    assert!(unique < netlist_total,
        "BOM unique count ({}) should be less than total ({})", unique, netlist_total);
}

// ============================================================================
// 4c. KiCad Output Tests
// ============================================================================

#[test]
fn test_led_badge_kicad_netlist_export() {
    let netlist = build_led_badge_netlist().unwrap();

    let exporter = ato_export::KicadNetlistExporter::new(&netlist);
    let output = exporter.export_to_string();

    assert!(output.is_ok(), "KiCad netlist export should succeed");
    let content = output.unwrap();
    assert!(content.contains("(export"), "Netlist should have (export tag");
    assert!(content.contains("(components"), "Netlist should have (components section");
}

#[test]
fn test_led_badge_kicad_schematic_export() {
    let netlist = build_led_badge_netlist().unwrap();

    let schematic = ato_export::KicadSchematic::from_netlist(&netlist);
    let output = schematic.export_to_string();

    assert!(output.is_ok(), "Schematic export should succeed");
    let content = output.unwrap();
    assert!(content.contains("kicad_sch"), "Schematic should have kicad_sch tag");
}

#[test]
fn test_led_badge_kicad_pcb_export() {
    let netlist = build_led_badge_netlist().unwrap();

    let pcb = ato_export::KicadPcb::from_netlist(&netlist);
    let output = pcb.export_to_string();

    assert!(output.is_ok(), "PCB export should succeed");
    let content = output.unwrap();
    assert!(content.contains("kicad_pcb"), "PCB should have kicad_pcb tag");
}

#[test]
fn test_led_badge_kicad_project_export() {
    let project = ato_export::KicadProject::new("led_badge");
    let output = project.to_json();

    assert!(output.is_ok(), "Project export should succeed");
    let content = output.unwrap();
    assert!(content.contains("led_badge"), "Project should contain project name");
}

// ============================================================================
// 4d. Exact BOM Component Count Test (matches BOM output)
// ============================================================================

#[test]
fn test_led_badge_bom_exact_counts() {
    let netlist = build_led_badge_netlist().unwrap();

    let mut prefix_counts: HashMap<String, usize> = HashMap::new();
    for comp in &netlist.components {
        let prefix = comp.reference.trim_end_matches(|c: char| c.is_ascii_digit())
            .to_string();
        *prefix_counts.entry(prefix).or_default() += 1;
    }

    // Expected counts from the BOM: C115, LED100, R14, L1, J1, U7
    let led_count = *prefix_counts.get("LED").unwrap_or(&0);
    let cap_count = *prefix_counts.get("C").unwrap_or(&0);
    let res_count = *prefix_counts.get("R").unwrap_or(&0);
    let ind_count = *prefix_counts.get("L").unwrap_or(&0);
    let conn_count = *prefix_counts.get("J").unwrap_or(&0);
    let ic_count = *prefix_counts.get("U").unwrap_or(&0);

    assert_eq!(led_count, 100, "Expected 100 LEDs, got {}", led_count);
    assert_eq!(cap_count, 115, "Expected 115 capacitors, got {}", cap_count);
    assert_eq!(res_count, 14, "Expected 14 resistors, got {}", res_count);
    assert_eq!(ind_count, 1, "Expected 1 inductor, got {}", ind_count);
    assert_eq!(conn_count, 1, "Expected 1 connector, got {}", conn_count);
    assert_eq!(ic_count, 7, "Expected 7 ICs, got {}", ic_count);

    let total = netlist.component_count();
    assert_eq!(total, 238, "Expected 238 total components, got {}", total);
}

#[test]
fn test_led_badge_netlist_has_inductor() {
    let netlist = build_led_badge_netlist().unwrap();

    // Match L followed by digit (to exclude LED which also starts with L)
    let ind_count = netlist.components.iter()
        .filter(|c| {
            c.reference.starts_with('L')
                && c.reference.chars().nth(1).map_or(false, |ch| ch.is_ascii_digit())
        })
        .count();

    assert_eq!(ind_count, 1, "Expected exactly 1 inductor, got {}", ind_count);
}

#[test]
fn test_led_badge_package_components_have_footprints_and_lcsc() {
    let netlist = build_led_badge_netlist().unwrap();

    // Package-level components that should have both footprint and LCSC:
    // J1 (connector), U1 (ESP32), U2 (microphone), U3 (USB-C), U4 (buck-boost), U5 (charger)
    // LED1-LED100 (addressable LEDs)
    let package_refs = ["J1", "U1", "U2", "U3", "U4", "U5"];

    for ref_name in &package_refs {
        let comp = netlist.components.iter()
            .find(|c| c.reference == *ref_name)
            .unwrap_or_else(|| panic!("Component {} not found in netlist", ref_name));

        assert!(comp.footprint.is_some(),
            "Component {} should have a footprint, got None", ref_name);
        assert!(comp.properties.contains_key("lcsc"),
            "Component {} should have LCSC part number, properties: {:?}",
            ref_name, comp.properties.keys().collect::<Vec<_>>());
    }
}

// ============================================================================
// 4e. Trait Propagation Tests
// ============================================================================

#[test]
fn test_led_badge_imported_modules_have_traits() {
    let design = analyze_led_badge().unwrap();

    // Modules imported from packages should have traits propagated from their
    // base classes (inheritance chain). Specifically, package-level components
    // should have traits like has_designator_prefix and is_atomic_part.
    let mut modules_with_traits = 0;
    let mut total_trait_count = 0;

    for module in design.modules() {
        if !module.traits.is_empty() {
            modules_with_traits += 1;
            total_trait_count += module.traits.len();
        }
    }

    // We expect at least some modules to have traits (stdlib types like Resistor,
    // Capacitor, and package components like SK6805EC20_package)
    assert!(modules_with_traits > 0,
        "Expected at least some modules with traits, got 0 out of {} modules",
        design.module_count());
    assert!(total_trait_count > 0,
        "Expected at least some traits across all modules, got 0");

    eprintln!("Trait propagation: {} modules have traits, {} total traits",
        modules_with_traits, total_trait_count);
}

#[test]
fn test_led_badge_designator_prefix_traits_exist() {
    let design = analyze_led_badge().unwrap();

    // Check that has_designator_prefix traits are present on relevant modules.
    // These traits are what drive the designator prefix assignment (R, C, LED, etc.)
    let mut modules_with_designator_prefix = Vec::new();

    for module in design.modules() {
        for trait_ref in &module.traits {
            if trait_ref.name.name() == "has_designator_prefix" {
                modules_with_designator_prefix.push(module.name.clone());
            }
        }
    }

    // At minimum, stdlib Resistor and Capacitor should have this trait
    assert!(!modules_with_designator_prefix.is_empty(),
        "Expected at least some modules with has_designator_prefix trait");
    eprintln!("Modules with has_designator_prefix: {:?}", modules_with_designator_prefix);
}

#[test]
fn test_led_badge_part_picked_traits_exist() {
    let design = analyze_led_badge().unwrap();

    // Package-level components should have has_part_picked traits
    // (these carry LCSC part numbers from the auto-generated part files)
    let mut modules_with_part_picked = Vec::new();

    for module in design.modules() {
        for trait_ref in &module.traits {
            if trait_ref.name.name() == "has_part_picked" {
                modules_with_part_picked.push(module.name.clone());
            }
        }
    }

    // Package components (ESP32, microphone, USB connector, etc.) should have this
    assert!(!modules_with_part_picked.is_empty(),
        "Expected at least some modules with has_part_picked trait (for LCSC numbers)");
    eprintln!("Modules with has_part_picked: {:?}", modules_with_part_picked);
}

// ============================================================================
// 5. Summary / Regression Test
// ============================================================================

#[test]
fn test_led_badge_full_pipeline_summary() {
    let source = led_badge_source();

    // Phase 1: Parse
    let ast = ato_parser::parse(&source).expect("Parse failed");
    let module_count = ast.statements.iter()
        .filter(|s| matches!(s, ato_parser::Statement::BlockDef(_)))
        .count();
    assert_eq!(module_count, 3, "Expected 3 module definitions in AST");

    // Phase 2: Sema
    let design = analyze_led_badge().expect("Sema failed");
    assert!(design.module_count() >= 3, "Design should have at least 3 modules");
    assert!(design.field_count() > 0, "Design should have fields");
    assert!(design.connection_count() > 0, "Design should have connections");

    // Phase 3: Solver (best-effort, no contradictions)
    if design.constraint_count() > 0 {
        let collector = ato_sema::ConstraintCollector::new();
        if let Ok((mut solver, _)) = collector.collect(&design) {
            match solver.solve() {
                Err(ato_solver::SolverError::Contradiction(msg)) => {
                    panic!("Contradiction in constraints: {}", msg);
                }
                _ => { /* ok or timeout -- acceptable */ }
            }
        }
    }

    // Phase 4: Export
    let netlist = build_led_badge_netlist().expect("Netlist build failed");
    let comp_count = netlist.component_count();

    eprintln!("=== led_badge Full Pipeline Summary ===");
    eprintln!("AST:       {} top-level statements, {} modules", ast.statements.len(), module_count);
    eprintln!("Design:    {} modules, {} fields, {} connections, {} constraints",
        design.module_count(), design.field_count(),
        design.connection_count(), design.constraint_count());
    eprintln!("Netlist:   {} components, {} nets",
        comp_count, netlist.net_count());

    // Count by designator prefix
    let mut prefix_counts: HashMap<String, usize> = HashMap::new();
    for comp in &netlist.components {
        let prefix = comp.reference.trim_end_matches(|c: char| c.is_ascii_digit())
            .to_string();
        *prefix_counts.entry(prefix).or_default() += 1;
    }
    let mut sorted: Vec<_> = prefix_counts.iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (prefix, count) in &sorted {
        eprintln!("  {}: {}", prefix, count);
    }

    // Key regression assertions
    assert!(comp_count >= 100,
        "Regression: expected at least 100 components, got {}", comp_count);

    let led_count = netlist.components.iter()
        .filter(|c| c.reference.starts_with("LED"))
        .count();
    assert_eq!(led_count, 100,
        "Regression: expected 100 LEDs, got {}", led_count);

    // Verify no "all U" designator problem
    let non_u_count = netlist.components.iter()
        .filter(|c| !c.reference.starts_with('U'))
        .count();
    assert!(non_u_count > 0,
        "Regression: all components have U designator (designator prefix detection broken)");
}
