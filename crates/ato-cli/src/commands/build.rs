//! Build command - full compilation pipeline.
//!
//! This command runs the full compilation pipeline:
//! 1. Parse the source file
//! 2. Run semantic analysis
//! 3. Run constraint solver
//! 4. Part picking (for passive components)
//! 5. Generate build artifacts (netlist, schematic, PCB, BOM)

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use indicatif::{ProgressBar, ProgressStyle};

use crate::error::{CliError, CliResult, SemanticErrorInfo, read_file};
use ato_export::{
    NetlistBuilder, KicadNetlistExporter, KicadSchematic, KicadPcb,
    KicadProject, Bom, BomExporter, BomFormat,
};
use ato_ir::{Design, FieldKind};
use ato_parts::{
    BasicPartSelector, CachedDatabase, ComponentType, LcscClient,
    ParameterConstraint, PartCache, PartDatabase, PartQuery, PartSelector,
    SelectionConfig,
};
use ato_sema::{Analyzer, AtoConfig, ConstraintCollector, SemaError};
use ato_solver::SolverError;

/// Create a spinner progress bar with a consistent style.
fn make_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&[
                "\u{2800}", "\u{2801}", "\u{2809}", "\u{281b}",
                "\u{283b}", "\u{2839}", "\u{2838}", "\u{2830}",
                "\u{2834}", "\u{2836}", "\u{2837}", "\u{2827}",
                "\u{2807}", "\u{2803}", "\u{2802}", "\u{2800}",
            ]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

/// Finish a spinner with a green checkmark message.
fn finish_spinner(pb: &ProgressBar, msg: &str) {
    pb.set_style(
        ProgressStyle::with_template("{msg}").unwrap()
    );
    pb.finish_with_message(format!(
        "{} {}",
        console::style("\u{2713}").green(),
        msg
    ));
}

/// Resolved build target containing the file path and optional root module name.
struct BuildTarget {
    /// Path to the .ato file to build.
    file_path: PathBuf,
    /// Optional root module name (from "file.ato:Module" syntax).
    _root_module: Option<String>,
    /// Project root directory (where ato.yaml lives), if found.
    project_root: Option<PathBuf>,
}

/// Find the stdlib path by looking for crates/ato-sema/stdlib relative to the project.
fn find_stdlib_path(file_path: &Path) -> Option<PathBuf> {
    // Canonicalize the file path first to handle relative paths
    let canonical = file_path.canonicalize().ok()?;
    let mut current = canonical.parent()?;

    for _ in 0..10 {  // Don't go up more than 10 levels
        // Check for crates/ato-sema/stdlib (Ato stdlib stubs)
        let stdlib = current.join("crates/ato-sema/stdlib");
        if stdlib.exists() {
            return Some(stdlib);
        }

        current = current.parent()?;
    }

    None
}

/// Search for ato.yaml starting from `start_dir` and walking up.
fn find_ato_yaml(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        let yaml_path = current.join("ato.yaml");
        if yaml_path.exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Parse an entry point string like "file.ato:ModuleName" into (file_path, module_name).
fn parse_entry_point(entry: &str, base_dir: &Path) -> Result<(PathBuf, Option<String>), CliError> {
    if let Some((file_part, module_part)) = entry.split_once(':') {
        let file_path = base_dir.join(file_part);
        Ok((file_path, Some(module_part.to_string())))
    } else {
        let file_path = base_dir.join(entry);
        Ok((file_path, None))
    }
}

/// Resolve the build target from the CLI argument.
///
/// The target argument can be:
/// - None: look for ato.yaml and use the "default" build target
/// - A path ending in .ato: use it directly as a file path
/// - A name: look for ato.yaml and use the named build target
fn resolve_target(target: Option<&str>) -> CliResult<BuildTarget> {
    match target {
        // Explicit .ato file path
        Some(t) if t.ends_with(".ato") => {
            let path = PathBuf::from(t);
            if !path.exists() {
                return Err(CliError::io(format!(
                    "File not found: '{}'", path.display()
                )));
            }
            let project_root = path.parent().and_then(|p| find_ato_yaml(p));
            Ok(BuildTarget {
                file_path: path,
                _root_module: None,
                project_root,
            })
        }

        // Entry point with colon (e.g., "file.ato:Module")
        Some(t) if t.contains(':') && t.contains(".ato") => {
            let cwd = std::env::current_dir().map_err(|e| {
                CliError::io(format!("failed to get current directory: {}", e))
            })?;
            let project_root = find_ato_yaml(&cwd);
            let base_dir = project_root.as_ref().unwrap_or(&cwd);
            let (file_path, root_module) = parse_entry_point(t, base_dir)?;
            if !file_path.exists() {
                return Err(CliError::io(format!(
                    "File not found: '{}'", file_path.display()
                )));
            }
            Ok(BuildTarget {
                file_path,
                _root_module: root_module,
                project_root,
            })
        }

        // Build target name or None (use ato.yaml)
        target_name => {
            let cwd = std::env::current_dir().map_err(|e| {
                CliError::io(format!("failed to get current directory: {}", e))
            })?;

            let project_root = find_ato_yaml(&cwd).ok_or_else(|| {
                CliError::io(
                    "No ato.yaml found. Provide a .ato file path or run from a project directory."
                        .to_string(),
                )
            })?;

            let config = AtoConfig::load(&project_root).map_err(|e| {
                CliError::io(format!("Failed to load ato.yaml: {}", e))
            })?;

            let build_name = target_name.unwrap_or("default");

            let entry = config.get_entry(build_name).ok_or_else(|| {
                let available: Vec<&str> = config.builds.keys().map(|k| k.as_str()).collect();
                CliError::io(format!(
                    "Build target '{}' not found in ato.yaml. Available targets: {}",
                    build_name,
                    if available.is_empty() {
                        "(none)".to_string()
                    } else {
                        available.join(", ")
                    }
                ))
            })?;

            // Resolve entry point relative to src path
            let src_dir = project_root.join(&config.paths.src);
            let (file_path, root_module) = parse_entry_point(entry, &src_dir)?;

            if !file_path.exists() {
                return Err(CliError::io(format!(
                    "Entry file not found: '{}' (from build target '{}')",
                    file_path.display(),
                    build_name
                )));
            }

            Ok(BuildTarget {
                file_path,
                _root_module: root_module,
                project_root: Some(project_root),
            })
        }
    }
}

/// Run the build command.
///
/// `target` can be:
/// - None: use the "default" build target from ato.yaml
/// - A .ato file path
/// - A build target name from ato.yaml
/// - An entry point like "file.ato:ModuleName"
pub fn run(target: Option<&str>, output: Option<&Path>, verbose: bool) -> CliResult<()> {
    // Resolve the build target
    let build_target = resolve_target(target)?;
    let path = &build_target.file_path;

    // Check file extension
    if path.extension().map_or(true, |ext| ext != "ato") {
        return Err(CliError::io(format!(
            "Expected .ato file, got '{}'",
            path.display()
        )));
    }

    // Read the file
    let source = read_file(path)?;
    let file_name = path.display().to_string();
    let short_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file_name);

    if verbose {
        println!("Building {}...", file_name);
        if let Some(ref root) = build_target.project_root {
            println!("  Project root: {}", root.display());
        }
    }

    // Phase 1: Semantic analysis
    let spinner = make_spinner(&format!("Analyzing {}...", short_name));

    // Setup analyzer with stdlib if found
    let mut analyzer = Analyzer::new();
    if let Some(stdlib_path) = find_stdlib_path(path) {
        if verbose {
            println!("    Using stdlib: {}", stdlib_path.display());
        }
        analyzer = analyzer.with_stdlib(stdlib_path);
    }

    let design = match analyzer.analyze_file(&source, path) {
        Ok(design) => design,
        Err(errors) => {
            spinner.finish_and_clear();
            let sema_errors = convert_sema_errors(&errors);
            return Err(CliError::semantic(&file_name, sema_errors, source));
        }
    };

    finish_spinner(&spinner, &format!(
        "Analyzed: {} modules, {} fields, {} connections",
        design.module_count(),
        design.field_count(),
        design.connection_count(),
    ));

    // Phase 2: Constraint solving
    let constraint_count = design.constraint_count();
    let mut _solved_params: HashMap<String, String> = HashMap::new();
    let mut build_errors: Vec<CliError> = Vec::new();

    if constraint_count > 0 {
        let spinner = make_spinner(&format!("Solving {} constraints...", constraint_count));
        let solve_start = Instant::now();

        // Collect constraints from the IR
        let collector = ConstraintCollector::new();
        match collector.collect(&design) {
            Ok((mut solver, dependencies)) => {
                if verbose {
                    let free_count = dependencies.free_parameters().len();
                    let constrained_count = dependencies.constrained_parameters().len();
                    if free_count > 0 || constrained_count > 0 {
                        println!("    {} free, {} constrained parameter(s)", free_count, constrained_count);
                    }
                }

                match solver.solve() {
                    Ok(result) => {
                        let elapsed = solve_start.elapsed();
                        finish_spinner(&spinner, &format!(
                            "Solved {} constraints in {} iterations ({:.0?})",
                            constraint_count, result.iterations, elapsed
                        ));

                        if verbose && !result.all_satisfied {
                            println!("    {} constraint(s) not fully deduced", result.not_deduced.len());
                        }

                        // Extract solved parameter values for part picking
                        _solved_params = extract_solved_parameters(&result, verbose);
                    }
                    Err(SolverError::Contradiction(msg)) => {
                        spinner.finish_and_clear();
                        build_errors.push(CliError::solver(
                            &file_name,
                            format!("Constraint contradiction: {}", msg),
                            None,
                            source.clone(),
                        ));
                    }
                    Err(SolverError::Timeout { iterations, elapsed }) => {
                        spinner.finish_and_clear();
                        build_errors.push(CliError::solver(
                            &file_name,
                            format!(
                                "Solver timed out after {} iterations ({:?}). \
                                 Design may have circular or unsolvable constraints.",
                                iterations, elapsed
                            ),
                            None,
                            source.clone(),
                        ));
                    }
                    Err(e) => {
                        spinner.finish_and_clear();
                        build_errors.push(CliError::solver(
                            &file_name,
                            format!("Solver error: {}", e),
                            None,
                            source.clone(),
                        ));
                    }
                }
            }
            Err(e) => {
                spinner.finish_and_clear();
                build_errors.push(CliError::solver(
                    &file_name,
                    format!("Constraint collection failed: {}", e),
                    None,
                    source.clone(),
                ));
            }
        }
    }

    // Phase 2.5: Part picking
    let pick_spinner = make_spinner("Picking parts...");
    let picked_parts = pick_parts(&design, &_solved_params, verbose);
    let picked_count = picked_parts.len() / 3; // Each part generates ~3 entries (lcsc, footprint, value)
    if picked_count > 0 {
        finish_spinner(&pick_spinner, &format!("Picked {} passive components", picked_count));
    } else {
        finish_spinner(&pick_spinner, "No passive components to pick");
    }

    // Merge picked part info into solved params so the netlist builder can use them
    for (key, value) in &picked_parts {
        _solved_params.insert(key.clone(), value.clone());
    }

    // Phase 3: Output generation
    let gen_spinner = make_spinner("Generating output...");

    // Determine output directory
    let output_dir = match output {
        Some(out_path) => out_path.to_path_buf(),
        None => {
            if let Some(ref project_root) = build_target.project_root {
                project_root.join("build")
            } else {
                path.parent().unwrap_or(Path::new(".")).join("build")
            }
        }
    };

    // Create output directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&output_dir) {
        gen_spinner.finish_and_clear();
        return Err(CliError::io(format!(
            "Failed to create output directory '{}': {}",
            output_dir.display(),
            e
        )));
    }

    if verbose {
        println!("    Output directory: {}", output_dir.display());
    }

    // Determine entry module for the netlist builder.
    let entry_module = design.entry_module().or_else(|| {
        if let Some(ref root_name) = build_target._root_module {
            return design.find_module(root_name);
        }
        let file_stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .replace('-', "_");
        if !file_stem.is_empty() {
            for m in design.modules() {
                if !m.is_interface() && m.name.to_lowercase().replace('-', "_") == file_stem {
                    return Some(m.id);
                }
            }
        }
        design.modules().iter().rev()
            .find(|m| {
                !m.is_interface() && m.fields.iter().any(|&fid| {
                    design.get_field(fid).map(|f| f.is_instance()).unwrap_or(false)
                })
            })
            .map(|m| m.id)
    });

    // Build netlist from design
    let mut builder = NetlistBuilder::new(&design)
        .with_solved_values(_solved_params.clone());
    if let Some(entry_id) = entry_module {
        if verbose {
            if let Some(m) = design.get_module(entry_id) {
                println!("    Entry module: {}", m.name);
            }
        }
        builder = builder.with_entry_module(entry_id);
    }
    let netlist = match builder.build() {
        Ok(netlist) => netlist,
        Err(e) => {
            gen_spinner.finish_and_clear();
            return Err(CliError::io(format!("Failed to build netlist: {}", e)));
        }
    };

    // Get project name from file stem
    let project_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("project");

    let mut generated_files: Vec<String> = Vec::new();

    // Generate KiCad netlist
    let netlist_path = output_dir.join(format!("{}.net", project_name));
    let exporter = KicadNetlistExporter::new(&netlist);
    match exporter.export_to_string() {
        Ok(content) => {
            if let Err(e) = fs::write(&netlist_path, &content) {
                if verbose { println!("    Warning: Failed to write netlist: {}", e); }
            } else {
                generated_files.push(format!("{}.net", project_name));
            }
        }
        Err(e) => { if verbose { println!("    Warning: Failed to generate netlist: {}", e); } }
    }

    // Generate KiCad schematic
    let schematic_path = output_dir.join(format!("{}.kicad_sch", project_name));
    let schematic = KicadSchematic::from_netlist(&netlist);
    match schematic.export_to_string() {
        Ok(content) => {
            if let Err(e) = fs::write(&schematic_path, &content) {
                if verbose { println!("    Warning: Failed to write schematic: {}", e); }
            } else {
                generated_files.push(format!("{}.kicad_sch", project_name));
            }
        }
        Err(e) => { if verbose { println!("    Warning: Failed to generate schematic: {}", e); } }
    }

    // Generate KiCad PCB
    let pcb_path = output_dir.join(format!("{}.kicad_pcb", project_name));
    let pcb = KicadPcb::from_netlist(&netlist);
    match pcb.export_to_string() {
        Ok(content) => {
            if let Err(e) = fs::write(&pcb_path, &content) {
                if verbose { println!("    Warning: Failed to write PCB: {}", e); }
            } else {
                generated_files.push(format!("{}.kicad_pcb", project_name));
            }
        }
        Err(e) => { if verbose { println!("    Warning: Failed to generate PCB: {}", e); } }
    }

    // Generate KiCad project file with library configuration
    let project_path = output_dir.join(format!("{}.kicad_pro", project_name));
    let kicad_project = KicadProject::new_with_libraries(project_name);
    match kicad_project.to_json() {
        Ok(content) => {
            if let Err(e) = fs::write(&project_path, &content) {
                if verbose { println!("    Warning: Failed to write project file: {}", e); }
            } else {
                generated_files.push(format!("{}.kicad_pro", project_name));
            }
        }
        Err(e) => { if verbose { println!("    Warning: Failed to generate project file: {}", e); } }
    }

    // Generate BOM
    let bom_path = output_dir.join(format!("{}_bom.csv", project_name));
    let bom = Bom::from_netlist_grouped(&netlist);
    let bom_exporter = BomExporter::new(&bom);
    match bom_exporter.export_to_string(BomFormat::Jlcpcb) {
        Ok(content) => {
            if let Err(e) = fs::write(&bom_path, &content) {
                if verbose { println!("    Warning: Failed to write BOM: {}", e); }
            } else {
                generated_files.push(format!("{}_bom.csv", project_name));
            }
        }
        Err(e) => { if verbose { println!("    Warning: Failed to generate BOM: {}", e); } }
    }

    finish_spinner(&gen_spinner, &format!(
        "Generated {} files in build/",
        generated_files.len(),
    ));
    println!("  {} components, {} nets",
        netlist.component_count(), netlist.net_count());

    // Report all accumulated errors
    if !build_errors.is_empty() {
        let count = build_errors.len();
        return Err(CliError::BuildErrors {
            count,
            errors: build_errors,
        });
    }

    Ok(())
}

/// Extract solved parameter values from the solver result.
///
/// Returns a map from parameter name to formatted value string.
fn extract_solved_parameters(
    state: &ato_solver::SolverState,
    verbose: bool,
) -> HashMap<String, String> {
    let mut params = HashMap::new();

    for param_id in state.expressions.parameter_ids() {
        if let Some(param) = state.expressions.get_parameter(param_id) {
            let name = param.display_name();

            // Check if the parameter has a known superset (narrowed value)
            if let Some(ref superset) = param.known_superset {
                let value_str = format!("{}", superset);
                if verbose {
                    println!("    Parameter {}: {}", name, value_str);
                }
                params.insert(name, value_str);
            }
        }
    }

    params
}

/// Classify a module as a passive component type, if applicable.
///
/// Checks the module name and inheritance chain (via `super_type`) to determine
/// if it's a Resistor, Capacitor, or Inductor.
fn classify_passive(design: &Design, module_id: ato_ir::ModuleId) -> Option<ComponentType> {
    let mut current = Some(module_id);
    while let Some(mid) = current {
        if let Some(module) = design.get_module(mid) {
            let name_lower = module.name.to_lowercase();
            if name_lower == "resistor" {
                return Some(ComponentType::Resistor);
            } else if name_lower == "capacitor" {
                return Some(ComponentType::Capacitor);
            } else if name_lower == "inductor" {
                return Some(ComponentType::Inductor);
            }
            current = module.super_type;
        } else {
            break;
        }
    }
    None
}

/// Get the primary parameter name for a passive component type.
fn primary_param_name(component_type: ComponentType) -> &'static str {
    match component_type {
        ComponentType::Resistor => "resistance",
        ComponentType::Capacitor => "capacitance",
        ComponentType::Inductor => "inductance",
        _ => "",
    }
}

/// Parse a solved parameter value string into a (min, max) range in base units.
///
/// Handles formats like:
/// - "[9500, 10500] ohm"  (interval notation)
/// - "10000 ohm"          (singleton)
/// - "[100e-9, 120e-9] F" (SI notation)
fn parse_parameter_range(value_str: &str) -> Option<(f64, f64)> {
    let trimmed = value_str.trim();

    // Try interval notation: [min, max] unit
    if trimmed.starts_with('[') {
        if let Some(bracket_end) = trimmed.find(']') {
            let inner = &trimmed[1..bracket_end];
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() == 2 {
                let min: f64 = parts[0].trim().parse().ok()?;
                let max: f64 = parts[1].trim().parse().ok()?;
                return Some((min, max));
            }
        }
    }

    // Try singleton: "value unit"
    let numeric_end = trimmed.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != 'e' && c != 'E' && c != '+')
        .unwrap_or(trimmed.len());
    if numeric_end > 0 {
        if let Ok(val) = trimmed[..numeric_end].trim().parse::<f64>() {
            // Give a +/- 5% tolerance window for singleton values
            let margin = val.abs() * 0.05;
            return Some((val - margin, val + margin));
        }
    }

    None
}

/// Pick parts for passive components based on the design and solved parameters.
///
/// Walks the design IR to find instances of Resistor, Capacitor, and Inductor.
/// For each, extracts the primary parameter constraint from the solver output,
/// queries the JLCPCB/LCSC database via `CachedDatabase<LcscClient>`, and
/// uses `BasicPartSelector` to pick the best match.
///
/// Returns a map of field path keys (like "r1.lcsc", "r1.footprint") to values,
/// suitable for merging into the NetlistBuilder's solved_values.
fn pick_parts(
    design: &Design,
    solved_params: &HashMap<String, String>,
    verbose: bool,
) -> HashMap<String, String> {
    let mut result: HashMap<String, String> = HashMap::new();

    // Collect passive instances: (instance_field_name, component_type, module_name)
    let mut passives: Vec<(String, ComponentType, String)> = Vec::new();

    for module in design.modules() {
        for &field_id in &module.fields {
            if let Some(field) = design.get_field(field_id) {
                if let FieldKind::Instance { ref type_ref, resolved_type, count, .. } = field.kind {
                    // Determine the type: use resolved_type if available, else match by name
                    let component_type = resolved_type
                        .and_then(|mid| classify_passive(design, mid))
                        .or_else(|| {
                            let name_lower = type_ref.name().to_lowercase();
                            match name_lower.as_str() {
                                "resistor" => Some(ComponentType::Resistor),
                                "capacitor" => Some(ComponentType::Capacitor),
                                "inductor" => Some(ComponentType::Inductor),
                                _ => None,
                            }
                        });

                    if let Some(ct) = component_type {
                        if let Some(array_count) = count {
                            // Array instance: generate entries for each element
                            for i in 0..array_count {
                                let indexed_name = format!("{}[{}]", field.name, i);
                                passives.push((indexed_name, ct, type_ref.name().to_string()));
                            }
                        } else {
                            passives.push((field.name.clone(), ct, type_ref.name().to_string()));
                        }
                    }
                }
            }
        }
    }

    if passives.is_empty() {
        if verbose {
            println!("    No passive components found for part picking");
        }
        return result;
    }

    if verbose {
        println!("    Found {} passive instance(s) to pick parts for", passives.len());
    }

    // Initialize the parts database (with cache for offline/performance)
    let db: Box<dyn PartDatabase> = match PartCache::default_cache() {
        Ok(cache) => match LcscClient::new() {
            Ok(client) => Box::new(CachedDatabase::new(client, cache)),
            Err(e) => {
                if verbose {
                    println!("    Warning: Could not create LCSC client: {}", e);
                }
                return result;
            }
        },
        Err(e) => {
            if verbose {
                println!("    Warning: Could not create parts cache: {}; trying direct client", e);
            }
            match LcscClient::new() {
                Ok(client) => Box::new(client) as Box<dyn PartDatabase>,
                Err(e) => {
                    if verbose {
                        println!("    Warning: Could not create LCSC client: {}", e);
                    }
                    return result;
                }
            }
        }
    };

    let selector = BasicPartSelector::new();
    let selection_config = SelectionConfig::new()
        .with_min_stock(100)
        .with_max_results(5);

    for (instance_name, component_type, _type_name) in &passives {
        let param_name = primary_param_name(*component_type);
        if param_name.is_empty() {
            continue;
        }

        // Look up the solved parameter value using field path convention.
        // The constraint collector names parameters by their field path as seen
        // from the module where the constraint lives:
        // - "r1.resistance" if the constraint is in the parent (e.g., App)
        // - "resistance" if the constraint is inside the Resistor module itself
        // We try the instance-qualified name first, then fall back to the bare
        // parameter name (which would match if the constraint is inherited).
        let param_key = format!("{}.{}", instance_name, param_name);
        let value_str = solved_params.get(&param_key)
            .or_else(|| solved_params.get(param_name))
            .or_else(|| {
                // Search for any key ending with ".{param_name}" that could
                // correspond to this instance through nested paths
                let suffix = format!(".{}", param_name);
                solved_params.iter()
                    .find(|(k, _)| k.ends_with(&suffix) && instance_name.starts_with(k.trim_end_matches(&suffix)))
                    .map(|(_, v)| v)
            });

        let value_range = value_str.and_then(|v| parse_parameter_range(v));

        // Also look for a package constraint
        let package_key = format!("{}.package", instance_name);
        let package = solved_params.get(&package_key)
            .map(|s| s.trim_matches('"').to_string());

        // Build the query
        let mut query = PartQuery::for_type(*component_type);
        if let Some((min, max)) = value_range {
            if min.is_finite() && max.is_finite() && min > 0.0 && max > 0.0 {
                query = query.with_param(param_name, ParameterConstraint::between(min, max));
            } else {
                if verbose {
                    println!("    Skipping {} - parameter range not finite/positive: ({}, {})",
                        instance_name, min, max);
                }
                continue;
            }
        } else {
            if verbose {
                println!("    Skipping {} - no solved {} value found", instance_name, param_name);
            }
            continue;
        }

        if let Some(ref pkg) = package {
            query = query.with_package(pkg.clone());
        }
        query = query.with_min_stock(100).with_limit(20);

        // Query the database
        match db.query(&query) {
            Ok(candidates) => {
                if candidates.is_empty() {
                    if verbose {
                        println!("    Warning: No parts found for {} ({:?})", instance_name, component_type);
                    }
                    continue;
                }

                // Select the best part
                let selections = selector.select(candidates, &selection_config);
                if let Some(best) = selections.first() {
                    let part = &best.part;
                    if verbose {
                        println!("    {} -> {} ({}, {})",
                            instance_name,
                            part.id,
                            part.manufacturer.part_number,
                            part.package.name,
                        );
                    }

                    // Store LCSC ID and footprint for this instance
                    if let Some(lcsc) = part.lcsc_id() {
                        result.insert(
                            format!("{}.lcsc", instance_name),
                            lcsc.to_string(),
                        );
                    }

                    // Map package name to KiCad footprint naming convention
                    let footprint = match *component_type {
                        ComponentType::Resistor => {
                            format!("Resistor_SMD:R_{}", kicad_footprint_suffix(&part.package.name))
                        }
                        ComponentType::Capacitor => {
                            format!("Capacitor_SMD:C_{}", kicad_footprint_suffix(&part.package.name))
                        }
                        ComponentType::Inductor => {
                            format!("Inductor_SMD:L_{}", kicad_footprint_suffix(&part.package.name))
                        }
                        _ => part.package.name.clone(),
                    };

                    result.insert(
                        format!("{}.footprint", instance_name),
                        footprint,
                    );

                    // Store value string for BOM
                    if let Some((min, max)) = value_range {
                        let value_display = format_part_value(*component_type, (min + max) / 2.0);
                        result.insert(
                            format!("{}.value", instance_name),
                            value_display,
                        );
                    }
                }
            }
            Err(e) => {
                if verbose {
                    println!("    Warning: Database query failed for {}: {}", instance_name, e);
                }
            }
        }
    }

    if verbose && !result.is_empty() {
        println!("    Picked parts for {} instance(s)", result.len() / 3);
    }

    result
}

/// Map an imperial package name (e.g., "0402") to KiCad's footprint suffix.
fn kicad_footprint_suffix(package: &str) -> String {
    match package {
        "0201" => "0201_0603Metric".to_string(),
        "0402" => "0402_1005Metric".to_string(),
        "0603" => "0603_1608Metric".to_string(),
        "0805" => "0805_2012Metric".to_string(),
        "1206" => "1206_3216Metric".to_string(),
        "1210" => "1210_3225Metric".to_string(),
        "1812" => "1812_4532Metric".to_string(),
        "2010" => "2010_5025Metric".to_string(),
        "2512" => "2512_6332Metric".to_string(),
        other => other.to_string(),
    }
}

/// Format a parameter value for human-readable display in BOM.
fn format_part_value(component_type: ComponentType, value: f64) -> String {
    match component_type {
        ComponentType::Resistor => {
            if value >= 1_000_000.0 {
                format!("{:.1}M", value / 1_000_000.0)
            } else if value >= 1_000.0 {
                format!("{:.1}k", value / 1_000.0)
            } else {
                format!("{:.1}", value)
            }
        }
        ComponentType::Capacitor => {
            if value >= 1e-3 {
                format!("{:.1}mF", value * 1e3)
            } else if value >= 1e-6 {
                format!("{:.1}uF", value * 1e6)
            } else if value >= 1e-9 {
                format!("{:.1}nF", value * 1e9)
            } else {
                format!("{:.1}pF", value * 1e12)
            }
        }
        ComponentType::Inductor => {
            if value >= 1e-3 {
                format!("{:.1}mH", value * 1e3)
            } else if value >= 1e-6 {
                format!("{:.1}uH", value * 1e6)
            } else {
                format!("{:.1}nH", value * 1e9)
            }
        }
        _ => format!("{}", value),
    }
}

/// Convert semantic errors to CLI error info.
fn convert_sema_errors(errors: &[SemaError]) -> Vec<SemanticErrorInfo> {
    errors.iter().map(|e| {
        let (message, span, help) = match e {
            SemaError::UndefinedName { name, span } => {
                (format!("undefined name '{}'", name), span_to_tuple(span), Some(format!("Did you forget to define '{}'?", name)))
            }
            SemaError::DuplicateDefinition { name, span, .. } => {
                (format!("duplicate definition of '{}'", name), span_to_tuple(span), Some("Names must be unique within a scope".into()))
            }
            SemaError::TypeMismatch { left, right, span } => {
                (format!("cannot connect '{}' to '{}': incompatible types", left, right), span_to_tuple(span), None)
            }
            SemaError::NotConnectable { name, span } => {
                (format!("'{}' is not connectable", name), span_to_tuple(span), Some("Only pins, signals, and instances can be connected".into()))
            }
            SemaError::NotIterable { name, span } => {
                (format!("'{}' is not iterable", name), span_to_tuple(span), Some("For loops require an array instance".into()))
            }
            SemaError::IndexOutOfBounds { index, size, span } => {
                (format!("index {} out of bounds for array of size {}", index, size), span_to_tuple(span), None)
            }
            SemaError::BaseTypeNotFound { name, span } => {
                (format!("base type '{}' not found", name), span_to_tuple(span), None)
            }
            SemaError::CyclicInheritance { chain, span } => {
                (format!("cyclic inheritance detected: {}", chain), span_to_tuple(span), None)
            }
            SemaError::InvalidFieldAccess { field, parent, span } => {
                (format!("'{}' is not a field of '{}'", field, parent), span_to_tuple(span), None)
            }
            SemaError::UnresolvedImport { name, span } => {
                (format!("cannot resolve import '{}'", name), span_to_tuple(span), None)
            }
            SemaError::FileNotFound { path, span } => {
                (format!("file not found: '{}'", path), span_to_tuple(span), None)
            }
            SemaError::ParseError { file, message } => {
                (format!("parse error in '{}': {}", file, message), None, None)
            }
            SemaError::IoError { message } => {
                (message.clone(), None, None)
            }
            SemaError::CircularImport { file } => {
                (format!("circular import detected: '{}'", file), None, Some("Check for import cycles between files".into()))
            }
            SemaError::UnsupportedFeature { feature, span } => {
                (format!("unsupported feature: {}", feature), span_to_tuple(span), None)
            }
        };
        SemanticErrorInfo { message, span, help }
    }).collect()
}

/// Convert an optional Span to a tuple.
fn span_to_tuple(span: &Option<ato_lexer::Span>) -> Option<(usize, usize)> {
    span.map(|s| (s.start, s.end - s.start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_build_valid_file() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pin p1\n    pin p2\n    p1 ~ p2").unwrap();

        let path_str = file.path().display().to_string();
        let result = run(Some(&path_str), None, false);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
    }

    #[test]
    fn test_build_with_constraints() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Resistor:\n    resistance: ohm\n    assert resistance > 0").unwrap();

        let path_str = file.path().display().to_string();
        let result = run(Some(&path_str), None, false);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
    }

    #[test]
    fn test_build_verbose() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pass").unwrap();

        let path_str = file.path().display().to_string();
        let result = run(Some(&path_str), None, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_with_error() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pin p1\n    p1 ~ undefined").unwrap();

        let path_str = file.path().display().to_string();
        let result = run(Some(&path_str), None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_from_ato_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create ato.yaml
        let yaml_content = r#"
requires-atopile: '^0.9.0'
paths:
    src: '.'
    layout: ./layouts
builds:
    default:
        entry: main.ato:MyModule
    example:
        entry: example.ato:Example
"#;
        fs::write(project_dir.join("ato.yaml"), yaml_content).unwrap();

        // Create the main.ato file
        fs::write(
            project_dir.join("main.ato"),
            "module MyModule:\n    pin p1\n    pin p2\n    p1 ~ p2\n",
        )
        .unwrap();

        // Create the example.ato file
        fs::write(
            project_dir.join("example.ato"),
            "module Example:\n    pass\n",
        )
        .unwrap();

        // Test building with a direct entry point
        let entry = format!("{}:MyModule", project_dir.join("main.ato").display());
        let result = run(Some(&entry), None, false);
        assert!(result.is_ok(), "Expected success building entry point, got: {:?}", result);
    }

    #[test]
    fn test_parse_entry_point() {
        let base = Path::new("/project/src");

        let (path, module) = parse_entry_point("main.ato:MyModule", base).unwrap();
        assert_eq!(path, PathBuf::from("/project/src/main.ato"));
        assert_eq!(module, Some("MyModule".to_string()));

        let (path, module) = parse_entry_point("main.ato", base).unwrap();
        assert_eq!(path, PathBuf::from("/project/src/main.ato"));
        assert_eq!(module, None);
    }

    #[test]
    fn test_extract_solved_parameters_empty() {
        let state = ato_solver::SolverState::default();
        let params = extract_solved_parameters(&state, false);
        assert!(params.is_empty());
    }

    #[test]
    fn test_pick_parts_empty_design() {
        let design = ato_ir::Design::new();
        let params = HashMap::new();
        let parts = pick_parts(&design, &params, false);
        assert!(parts.is_empty());
    }

    #[test]
    fn test_pick_parts_no_passives() {
        let mut design = ato_ir::Design::new();
        let module_id = design.create_module("App", ato_ir::ModuleKind::Module);
        design.add_field(module_id, "p1", ato_ir::FieldKind::pin("p1"));

        let params = HashMap::new();
        let parts = pick_parts(&design, &params, false);
        assert!(parts.is_empty());
    }

    #[test]
    fn test_classify_passive() {
        let mut design = ato_ir::Design::new();
        let resistor_id = design.create_module("Resistor", ato_ir::ModuleKind::Module);
        let capacitor_id = design.create_module("Capacitor", ato_ir::ModuleKind::Module);

        assert_eq!(classify_passive(&design, resistor_id), Some(ComponentType::Resistor));
        assert_eq!(classify_passive(&design, capacitor_id), Some(ComponentType::Capacitor));
    }

    #[test]
    fn test_parse_parameter_range() {
        // Interval notation
        assert_eq!(parse_parameter_range("[9500, 10500] ohm"), Some((9500.0, 10500.0)));
        // Singleton
        let (min, max) = parse_parameter_range("10000 ohm").unwrap();
        assert!((min - 9500.0).abs() < 1.0);
        assert!((max - 10500.0).abs() < 1.0);
        // Invalid
        assert!(parse_parameter_range("").is_none());
    }

    #[test]
    fn test_kicad_footprint_suffix() {
        assert_eq!(kicad_footprint_suffix("0402"), "0402_1005Metric");
        assert_eq!(kicad_footprint_suffix("0603"), "0603_1608Metric");
        assert_eq!(kicad_footprint_suffix("SOT-23"), "SOT-23");
    }

    #[test]
    fn test_format_part_value() {
        assert_eq!(format_part_value(ComponentType::Resistor, 10000.0), "10.0k");
        assert_eq!(format_part_value(ComponentType::Capacitor, 100e-9), "100.0nF");
        assert_eq!(format_part_value(ComponentType::Inductor, 10e-6), "10.0uH");
    }
}
