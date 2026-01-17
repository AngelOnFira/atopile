//! Build command - full compilation pipeline.
//!
//! This command runs the full compilation pipeline:
//! 1. Parse the source file
//! 2. Run semantic analysis
//! 3. Run constraint solver
//! 4. Generate build artifacts (netlist, schematic, PCB, BOM)

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{CliError, CliResult, SemanticErrorInfo, read_file};
use ato_export::{
    NetlistBuilder, KicadNetlistExporter, KicadSchematic, KicadPcb,
    KicadProject, Bom, BomExporter, BomFormat,
};
use ato_sema::{Analyzer, ConstraintCollector, SemaError};
use ato_solver::SolverError;

/// Find the stdlib path by looking for src/faebryk/library relative to the project.
fn find_stdlib_path(file_path: &Path) -> Option<PathBuf> {
    // Try to find the stdlib by walking up from the file path
    let mut current = file_path.parent()?;

    for _ in 0..10 {  // Don't go up more than 10 levels
        // Check for src/faebryk/library
        let stdlib = current.join("src/faebryk/library");
        if stdlib.exists() {
            return Some(stdlib);
        }

        // Also check if we're already in the atopile repo
        let stdlib = current.join("../src/faebryk/library");
        if stdlib.exists() {
            return Some(stdlib.canonicalize().ok()?);
        }

        current = current.parent()?;
    }

    None
}

/// Run the build command.
pub fn run(path: &Path, output: Option<&Path>, verbose: bool) -> CliResult<()> {
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

    // Phase 3: Output generation
    if verbose {
        println!("  Phase 3: Output generation...");
    }

    // Determine output directory
    let output_dir = match output {
        Some(out_path) => out_path.to_path_buf(),
        None => path.parent().unwrap_or(Path::new(".")).join("build"),
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

    // Generate KiCad project file
    let project_path = output_dir.join(format!("{}.kicad_pro", project_name));
    let kicad_project = KicadProject::new(project_name);
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

    println!("✓ {} built successfully", file_name);

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
    use tempfile::NamedTempFile;

    #[test]
    fn test_build_valid_file() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pin p1\n    pin p2\n    p1 ~ p2").unwrap();

        let result = run(file.path(), None, false);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
    }

    #[test]
    fn test_build_with_constraints() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Resistor:\n    resistance: ohm\n    assert resistance > 0").unwrap();

        let result = run(file.path(), None, false);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
    }

    #[test]
    fn test_build_verbose() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pass").unwrap();

        let result = run(file.path(), None, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_with_error() {
        let mut file = NamedTempFile::with_suffix(".ato").unwrap();
        writeln!(file, "module Test:\n    pin p1\n    p1 ~ undefined").unwrap();

        let result = run(file.path(), None, false);
        assert!(result.is_err());
    }
}
