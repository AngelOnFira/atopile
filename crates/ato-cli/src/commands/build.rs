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

use crate::error::{CliError, CliResult, SemanticErrorInfo, read_file};
use ato_export::{
    NetlistBuilder, KicadNetlistExporter, KicadSchematic, KicadPcb,
    KicadProject, Bom, BomExporter, BomFormat,
};
use ato_sema::{Analyzer, AtoConfig, ConstraintCollector, SemaError};
use ato_solver::SolverError;

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

    if verbose {
        println!("Building {}...", file_name);
        if let Some(ref root) = build_target.project_root {
            println!("  Project root: {}", root.display());
        }
    }

    // Phase 1: Semantic analysis
    if verbose {
        println!("  Phase 1: Semantic analysis...");
    }

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
            let sema_errors = convert_sema_errors(&errors);
            return Err(CliError::semantic(&file_name, sema_errors, source));
        }
    };

    if verbose {
        println!("    {} module(s)", design.module_count());
        println!("    {} field(s)", design.field_count());
        println!("    {} connection(s)", design.connection_count());
        println!("    {} constraint(s)", design.constraint_count());
    }

    // Phase 2: Constraint solving
    if verbose {
        println!("  Phase 2: Constraint solving...");
    }

    let mut _solved_params: HashMap<String, String> = HashMap::new();

    // Extract constraints from the design and run the solver
    let constraint_count = design.constraint_count();
    if constraint_count > 0 {
        if verbose {
            println!("    Collecting {} constraint(s)...", constraint_count);
        }

        // Collect constraints from the IR
        let collector = ConstraintCollector::new();
        match collector.collect(&design) {
            Ok((mut solver, dependencies)) => {
                if verbose {
                    println!("    Collected {} parameter(s)", solver.predicate_count());
                    let free_count = dependencies.free_parameters().len();
                    let constrained_count = dependencies.constrained_parameters().len();
                    if free_count > 0 || constrained_count > 0 {
                        println!("    {} free, {} constrained parameter(s)", free_count, constrained_count);
                    }
                }

                // Run the solver
                if verbose {
                    println!("    Running solver...");
                }

                match solver.solve() {
                    Ok(result) => {
                        if verbose {
                            println!("    Solver completed in {} iteration(s) ({:?})", result.iterations, result.elapsed);
                            if result.all_satisfied {
                                println!("    All constraints satisfied");
                            } else {
                                println!("    {} constraint(s) not fully deduced", result.not_deduced.len());
                            }
                        }

                        // Extract solved parameter values for part picking
                        _solved_params = extract_solved_parameters(&result, verbose);
                    }
                    Err(SolverError::Contradiction(msg)) => {
                        // Contradiction is a build failure
                        return Err(CliError::solver(
                            &file_name,
                            format!("Constraint contradiction: {}", msg),
                            None,
                            source,
                        ));
                    }
                    Err(SolverError::Timeout { iterations, elapsed }) => {
                        if verbose {
                            println!("    Solver timed out after {} iterations ({:?})", iterations, elapsed);
                        }
                        // Timeout is a warning, not a failure
                    }
                    Err(e) => {
                        if verbose {
                            println!("    Solver warning: {}", e);
                        }
                        // Other solver errors are warnings
                    }
                }
            }
            Err(e) => {
                if verbose {
                    println!("    Constraint collection error: {}", e);
                }
                // For now, just warn about collection errors
            }
        }
    } else if verbose {
        println!("    No constraints to solve");
    }

    // Phase 2.5: Part picking
    if verbose {
        println!("  Phase 2.5: Part picking...");
    }
    let _selected_parts = pick_parts(&_solved_params, verbose);

    // Phase 3: Output generation
    if verbose {
        println!("  Phase 3: Output generation...");
    }

    // Determine output directory
    let output_dir = match output {
        Some(out_path) => out_path.to_path_buf(),
        None => {
            if let Some(ref project_root) = build_target.project_root {
                // Use project_root/build when building from ato.yaml
                project_root.join("build")
            } else {
                path.parent().unwrap_or(Path::new(".")).join("build")
            }
        }
    };

    // Create output directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&output_dir) {
        return Err(CliError::io(format!(
            "Failed to create output directory '{}': {}",
            output_dir.display(),
            e
        )));
    }

    if verbose {
        println!("    Output directory: {}", output_dir.display());
    }

    // Build netlist from design
    let builder = NetlistBuilder::new(&design);
    let netlist = match builder.build() {
        Ok(netlist) => netlist,
        Err(e) => {
            if verbose {
                println!("    Warning: Failed to build netlist: {}", e);
            }
            // Continue with empty netlist for now
            ato_export::Netlist::new()
        }
    };

    if verbose {
        println!("    {} component(s) in netlist", netlist.component_count());
        println!("    {} net(s) in netlist", netlist.net_count());
    }

    // Get project name from file stem
    let project_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("project");

    // Generate KiCad netlist
    let netlist_path = output_dir.join(format!("{}.net", project_name));
    let exporter = KicadNetlistExporter::new(&netlist);
    match exporter.export_to_string() {
        Ok(content) => {
            if let Err(e) = fs::write(&netlist_path, &content) {
                if verbose {
                    println!("    Warning: Failed to write netlist: {}", e);
                }
            } else if verbose {
                println!("    Generated: {}", netlist_path.display());
            }
        }
        Err(e) => {
            if verbose {
                println!("    Warning: Failed to generate netlist: {}", e);
            }
        }
    }

    // Generate KiCad schematic
    let schematic_path = output_dir.join(format!("{}.kicad_sch", project_name));
    let schematic = KicadSchematic::from_netlist(&netlist);
    match schematic.export_to_string() {
        Ok(content) => {
            if let Err(e) = fs::write(&schematic_path, &content) {
                if verbose {
                    println!("    Warning: Failed to write schematic: {}", e);
                }
            } else if verbose {
                println!("    Generated: {}", schematic_path.display());
            }
        }
        Err(e) => {
            if verbose {
                println!("    Warning: Failed to generate schematic: {}", e);
            }
        }
    }

    // Generate KiCad PCB
    let pcb_path = output_dir.join(format!("{}.kicad_pcb", project_name));
    let pcb = KicadPcb::from_netlist(&netlist);
    match pcb.export_to_string() {
        Ok(content) => {
            if let Err(e) = fs::write(&pcb_path, &content) {
                if verbose {
                    println!("    Warning: Failed to write PCB: {}", e);
                }
            } else if verbose {
                println!("    Generated: {}", pcb_path.display());
            }
        }
        Err(e) => {
            if verbose {
                println!("    Warning: Failed to generate PCB: {}", e);
            }
        }
    }

    // Generate KiCad project file with library configuration
    let project_path = output_dir.join(format!("{}.kicad_pro", project_name));
    let kicad_project = KicadProject::new_with_libraries(project_name);
    match kicad_project.to_json() {
        Ok(content) => {
            if let Err(e) = fs::write(&project_path, &content) {
                if verbose {
                    println!("    Warning: Failed to write project file: {}", e);
                }
            } else if verbose {
                println!("    Generated: {}", project_path.display());
            }
        }
        Err(e) => {
            if verbose {
                println!("    Warning: Failed to generate project file: {}", e);
            }
        }
    }

    // Generate BOM
    let bom_path = output_dir.join(format!("{}_bom.csv", project_name));
    let bom = Bom::from_netlist_grouped(&netlist);
    let bom_exporter = BomExporter::new(&bom);
    match bom_exporter.export_to_string(BomFormat::Jlcpcb) {
        Ok(content) => {
            if let Err(e) = fs::write(&bom_path, &content) {
                if verbose {
                    println!("    Warning: Failed to write BOM: {}", e);
                }
            } else if verbose {
                println!("    Generated: {}", bom_path.display());
            }
        }
        Err(e) => {
            if verbose {
                println!("    Warning: Failed to generate BOM: {}", e);
            }
        }
    }

    println!("  {} built successfully", file_name);

    if verbose {
        println!();
        println!("Summary:");
        println!("  Modules:     {}", design.module_count());
        println!("  Fields:      {}", design.field_count());
        println!("  Connections: {}", design.connection_count());
        println!("  Constraints: {}", design.constraint_count());
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

/// A selected part from the part picker.
#[derive(Debug)]
struct SelectedPart {
    /// The parameter path this part was selected for.
    _field_path: String,
    /// LCSC part number.
    _lcsc_id: String,
    /// Human-readable description.
    _description: String,
}

/// Pick parts for passive components based on solved parameter constraints.
///
/// This is a placeholder that will be connected when the solver produces
/// concrete parameter ranges that can be mapped to component queries.
fn pick_parts(
    solved_params: &HashMap<String, String>,
    verbose: bool,
) -> Vec<SelectedPart> {
    let selected = Vec::new();

    if solved_params.is_empty() {
        if verbose {
            println!("    No solved parameters for part picking");
        }
        return selected;
    }

    if verbose {
        println!("    {} solved parameter(s) available for part picking", solved_params.len());
    }

    // Part picking integration point:
    // When the solver produces concrete parameter ranges (e.g., resistance = 9.5k-10.5k),
    // we can query the parts database:
    //
    // 1. Identify components that need parts (Resistor, Capacitor, etc.)
    // 2. Map solved parameter names to component types
    // 3. Build PartQuery objects from the parameter constraints
    // 4. Query the CachedDatabase
    // 5. Use BasicPartSelector to rank results
    // 6. Return SelectedPart entries
    //
    // This requires the sema IR to expose component type information
    // alongside parameter paths, which will be added as the sema and
    // solver mature.

    selected
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
    fn test_pick_parts_empty() {
        let params = HashMap::new();
        let parts = pick_parts(&params, false);
        assert!(parts.is_empty());
    }
}
