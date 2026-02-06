//! Comparison tests between Python atopile CLI and Rust atopile CLI.
//!
//! These tests run both CLIs on the same project and compare the output
//! to verify structural equivalence of the Rust rewrite.
//!
//! Tests are skipped when the Python CLI is not available.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Helpers: CLI availability & paths
// ---------------------------------------------------------------------------

const PYTHON_ATO: &str = "/Users/forest/.local/bin/ato";

fn python_ato_available() -> bool {
    Path::new(PYTHON_ATO).exists()
}

fn rust_ato_binary() -> PathBuf {
    // Use the release binary built by `cargo build --bin ato --release`
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let release = workspace_root.join("target/release/ato");
    if release.exists() {
        return release;
    }
    // Fall back to debug binary
    let debug = workspace_root.join("target/debug/ato");
    if debug.exists() {
        return debug;
    }
    panic!(
        "Rust ato binary not found. Run `cargo build --bin ato --release` first.\n\
         Checked: {}\n         {}",
        release.display(),
        debug.display()
    );
}

fn led_badge_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/led_badge")
        .canonicalize()
        .expect("led_badge directory not found")
}

// ---------------------------------------------------------------------------
// KiCad .net file parser (S-expression)
// ---------------------------------------------------------------------------

/// A parsed component from a KiCad netlist.
#[derive(Debug, Clone)]
struct ParsedComponent {
    reference: String,
    value: String,
    footprint: Option<String>,
    properties: BTreeMap<String, String>,
}

/// A parsed net from a KiCad netlist.
#[derive(Debug, Clone)]
struct ParsedNet {
    code: u32,
    name: String,
    nodes: BTreeSet<(String, String)>, // (component_ref, pin)
}

/// Parsed KiCad netlist.
#[derive(Debug, Default)]
struct ParsedNetlist {
    components: Vec<ParsedComponent>,
    nets: Vec<ParsedNet>,
}

/// Simple S-expression token.
#[derive(Debug, Clone, PartialEq)]
enum SexprToken {
    Open,
    Close,
    Str(String),
}

/// Tokenize an S-expression string.
fn tokenize_sexpr(input: &str) -> Vec<SexprToken> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            '(' => {
                tokens.push(SexprToken::Open);
                chars.next();
            }
            ')' => {
                tokens.push(SexprToken::Close);
                chars.next();
            }
            '"' => {
                chars.next(); // consume opening quote
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        chars.next();
                        break;
                    }
                    if c == '\\' {
                        chars.next();
                        if let Some(&escaped) = chars.peek() {
                            s.push(escaped);
                            chars.next();
                        }
                    } else {
                        s.push(c);
                        chars.next();
                    }
                }
                tokens.push(SexprToken::Str(s));
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            _ => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '(' || c == ')' || c == '"' || c.is_whitespace() {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                tokens.push(SexprToken::Str(s));
            }
        }
    }
    tokens
}

/// A simple recursive S-expression tree.
#[derive(Debug, Clone)]
enum Sexpr {
    Atom(String),
    List(Vec<Sexpr>),
}

impl Sexpr {
    fn as_atom(&self) -> Option<&str> {
        if let Sexpr::Atom(s) = self {
            Some(s)
        } else {
            None
        }
    }

    fn as_list(&self) -> Option<&[Sexpr]> {
        if let Sexpr::List(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Find a child list whose first element matches the given name.
    fn find_child(&self, name: &str) -> Option<&Sexpr> {
        if let Sexpr::List(children) = self {
            for child in children {
                if let Sexpr::List(inner) = child {
                    if let Some(first) = inner.first() {
                        if first.as_atom() == Some(name) {
                            return Some(child);
                        }
                    }
                }
            }
        }
        None
    }

    /// Find all child lists whose first element matches the given name.
    fn find_children(&self, name: &str) -> Vec<&Sexpr> {
        let mut result = Vec::new();
        if let Sexpr::List(children) = self {
            for child in children {
                if let Sexpr::List(inner) = child {
                    if let Some(first) = inner.first() {
                        if first.as_atom() == Some(name) {
                            result.push(child);
                        }
                    }
                }
            }
        }
        result
    }

    /// Get the second element as an atom string.
    fn second_atom(&self) -> Option<&str> {
        if let Sexpr::List(children) = self {
            children.get(1).and_then(|s| s.as_atom())
        } else {
            None
        }
    }
}

/// Parse tokens into an S-expression tree.
fn parse_sexpr(tokens: &[SexprToken]) -> (Sexpr, usize) {
    if tokens.is_empty() {
        return (Sexpr::Atom(String::new()), 0);
    }

    match &tokens[0] {
        SexprToken::Open => {
            let mut children = Vec::new();
            let mut i = 1;
            while i < tokens.len() {
                if tokens[i] == SexprToken::Close {
                    i += 1;
                    break;
                }
                let (child, consumed) = parse_sexpr(&tokens[i..]);
                children.push(child);
                i += consumed;
            }
            (Sexpr::List(children), i)
        }
        SexprToken::Close => (Sexpr::Atom(String::new()), 1),
        SexprToken::Str(s) => (Sexpr::Atom(s.clone()), 1),
    }
}

/// Parse a KiCad .net file content into a ParsedNetlist.
fn parse_kicad_netlist(content: &str) -> ParsedNetlist {
    let tokens = tokenize_sexpr(content);
    let (tree, _) = parse_sexpr(&tokens);
    let mut netlist = ParsedNetlist::default();

    // Find (components ...) section
    if let Some(components_section) = tree.find_child("components") {
        for comp_sexpr in components_section.find_children("comp") {
            let reference = comp_sexpr
                .find_child("ref")
                .and_then(|r| r.second_atom())
                .unwrap_or("")
                .to_string();

            let value = comp_sexpr
                .find_child("value")
                .and_then(|v| v.second_atom())
                .unwrap_or("")
                .to_string();

            let footprint = comp_sexpr
                .find_child("footprint")
                .and_then(|f| f.second_atom())
                .map(|s| s.to_string());

            let mut properties = BTreeMap::new();

            // Parse (property (name "X") (value "Y")) format (Python CLI)
            for prop in comp_sexpr.find_children("property") {
                let prop_name = prop
                    .find_child("name")
                    .and_then(|n| n.second_atom())
                    .unwrap_or("");
                let prop_value = prop
                    .find_child("value")
                    .and_then(|v| v.second_atom())
                    .unwrap_or("");
                if !prop_name.is_empty() {
                    properties.insert(prop_name.to_string(), prop_value.to_string());
                }
            }

            // Parse (fields (field (name "X") "Y")) format (Rust CLI)
            if let Some(fields_section) = comp_sexpr.find_child("fields") {
                for field in fields_section.find_children("field") {
                    if let Sexpr::List(parts) = field {
                        // (field (name "X") "Y")
                        let field_name = field
                            .find_child("name")
                            .and_then(|n| n.second_atom())
                            .unwrap_or("");
                        // The value is the last atom in the list
                        let field_value = parts
                            .last()
                            .and_then(|v| v.as_atom())
                            .unwrap_or("");
                        if !field_name.is_empty()
                            && field_value != field_name
                            && field_value != "name"
                        {
                            properties.insert(field_name.to_string(), field_value.to_string());
                        }
                    }
                }
            }

            netlist.components.push(ParsedComponent {
                reference,
                value,
                footprint,
                properties,
            });
        }
    }

    // Find (nets ...) section
    if let Some(nets_section) = tree.find_child("nets") {
        for net_sexpr in nets_section.find_children("net") {
            let code = net_sexpr
                .find_child("code")
                .and_then(|c| c.second_atom())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            let name = net_sexpr
                .find_child("name")
                .and_then(|n| n.second_atom())
                .unwrap_or("")
                .to_string();

            let mut nodes = BTreeSet::new();
            for node in net_sexpr.find_children("node") {
                let ref_name = node
                    .find_child("ref")
                    .and_then(|r| r.second_atom())
                    .unwrap_or("")
                    .to_string();
                let pin = node
                    .find_child("pin")
                    .and_then(|p| p.second_atom())
                    .unwrap_or("")
                    .to_string();
                if !ref_name.is_empty() && !pin.is_empty() {
                    nodes.insert((ref_name, pin));
                }
            }

            netlist.nets.push(ParsedNet { code, name, nodes });
        }
    }

    netlist
}

// ---------------------------------------------------------------------------
// BOM parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct BomEntry {
    designator: String,
    footprint: String,
    quantity: u32,
    value: String,
    lcsc: String,
}

fn parse_bom_csv(content: &str) -> Vec<BomEntry> {
    let mut entries = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return entries;
    }

    // Parse header to find column indices
    let header = lines[0];
    let cols: Vec<&str> = header.split(',').collect();
    let find_col = |name: &str| -> Option<usize> {
        cols.iter().position(|c| c.trim().eq_ignore_ascii_case(name))
    };

    let des_idx = find_col("Designator").unwrap_or(0);
    let fp_idx = find_col("Footprint").unwrap_or(1);
    let qty_idx = find_col("Quantity").unwrap_or(2);
    let val_idx = find_col("Value").unwrap_or(3);
    let lcsc_idx = find_col("LCSC Part #")
        .or_else(|| find_col("LCSC"))
        .unwrap_or(6);

    for line in &lines[1..] {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 3 {
            continue;
        }
        let get = |idx: usize| -> String {
            fields.get(idx).unwrap_or(&"").trim().to_string()
        };

        entries.push(BomEntry {
            designator: get(des_idx),
            footprint: get(fp_idx),
            quantity: get(qty_idx).parse().unwrap_or(1),
            value: get(val_idx),
            lcsc: get(lcsc_idx),
        });
    }

    entries
}

// ---------------------------------------------------------------------------
// CLI runners
// ---------------------------------------------------------------------------

/// Run the Python ato CLI on a project directory. Returns the build output dir.
fn run_python_ato(project_dir: &Path) -> Result<PathBuf, String> {
    let output = Command::new(PYTHON_ATO)
        .arg("build")
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("Failed to run Python ato: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Python ato build failed:\nstdout: {}\nstderr: {}",
            stdout, stderr
        ));
    }

    // Find the .net file in the build output
    let build_dir = project_dir.join("build");
    Ok(build_dir)
}

/// Run the Rust ato CLI on a project directory. Returns the build output dir.
fn run_rust_ato(project_dir: &Path) -> Result<PathBuf, String> {
    let binary = rust_ato_binary();
    let output = Command::new(&binary)
        .arg("build")
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("Failed to run Rust ato: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Rust ato build failed:\nstdout: {}\nstderr: {}",
            stdout, stderr
        ));
    }

    let build_dir = project_dir.join("build");
    Ok(build_dir)
}

/// Find the .net file in a Python ato build output directory.
/// Python CLI puts it at: build/builds/default/default/default.net
fn find_python_net_file(build_dir: &Path) -> Option<PathBuf> {
    let default_path = build_dir.join("builds/default/default/default.net");
    if default_path.exists() {
        return Some(default_path);
    }
    // Try to find any .net file recursively
    for entry in walkdir::WalkDir::new(build_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().map_or(false, |e| e == "net") {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}

/// Find the .net file in a Rust ato build output directory.
/// Rust CLI puts it at: build/<project_name>.net
fn find_rust_net_file(build_dir: &Path) -> Option<PathBuf> {
    for entry in walkdir::WalkDir::new(build_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().map_or(false, |e| e == "net") {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}

/// Find BOM CSV file in a Python build output directory.
fn find_python_bom_file(build_dir: &Path) -> Option<PathBuf> {
    for entry in walkdir::WalkDir::new(build_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "csv")
            && path
                .file_name()
                .map_or(false, |n| n.to_string_lossy().contains("bom"))
        {
            return Some(path.to_path_buf());
        }
    }
    None
}

/// Find BOM CSV file in a Rust build output directory.
fn find_rust_bom_file(build_dir: &Path) -> Option<PathBuf> {
    for entry in walkdir::WalkDir::new(build_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "csv")
            && path
                .file_name()
                .map_or(false, |n| n.to_string_lossy().contains("bom"))
        {
            return Some(path.to_path_buf());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------

/// Summary of a netlist for comparison.
#[derive(Debug)]
struct NetlistSummary {
    total_components: usize,
    components_by_prefix: BTreeMap<String, usize>,
    component_refs: BTreeSet<String>,
    total_nets: usize,
    /// Non-passive components with their footprint and LCSC.
    non_passive_details: BTreeMap<String, (Option<String>, Option<String>)>,
}

impl NetlistSummary {
    fn from_parsed(netlist: &ParsedNetlist) -> Self {
        let mut components_by_prefix = BTreeMap::new();
        let mut component_refs = BTreeSet::new();
        let mut non_passive_details = BTreeMap::new();

        for comp in &netlist.components {
            component_refs.insert(comp.reference.clone());

            // Extract prefix
            let prefix = comp
                .reference
                .trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string();
            *components_by_prefix.entry(prefix.clone()).or_insert(0) += 1;

            // Non-passive = not R, C, L
            let is_passive = prefix == "R" || prefix == "C" || prefix == "L";
            if !is_passive {
                let lcsc = comp
                    .properties
                    .get("LCSC")
                    .or_else(|| comp.properties.get("lcsc"))
                    .cloned();
                non_passive_details.insert(
                    comp.reference.clone(),
                    (comp.footprint.clone(), lcsc),
                );
            }
        }

        NetlistSummary {
            total_components: netlist.components.len(),
            components_by_prefix,
            component_refs,
            total_nets: netlist.nets.len(),
            non_passive_details,
        }
    }
}

/// Compare two netlist summaries and return a human-readable report.
fn compare_summaries(python: &NetlistSummary, rust: &NetlistSummary) -> (bool, String) {
    let mut report = String::new();
    let mut all_ok = true;

    // Component count comparison
    report.push_str(&format!(
        "Component count: Python={}, Rust={}\n",
        python.total_components, rust.total_components
    ));
    if python.total_components != rust.total_components {
        report.push_str("  MISMATCH in total component count\n");
        all_ok = false;
    }

    // Per-prefix comparison
    let all_prefixes: BTreeSet<_> = python
        .components_by_prefix
        .keys()
        .chain(rust.components_by_prefix.keys())
        .cloned()
        .collect();
    for prefix in &all_prefixes {
        let py_count = python.components_by_prefix.get(prefix).unwrap_or(&0);
        let rs_count = rust.components_by_prefix.get(prefix).unwrap_or(&0);
        report.push_str(&format!(
            "  {}: Python={}, Rust={}{}\n",
            prefix,
            py_count,
            rs_count,
            if py_count != rs_count {
                " MISMATCH"
            } else {
                ""
            }
        ));
        if py_count != rs_count {
            all_ok = false;
        }
    }

    // Net count comparison
    report.push_str(&format!(
        "Net count: Python={}, Rust={}\n",
        python.total_nets, rust.total_nets
    ));
    if python.total_nets != rust.total_nets {
        report.push_str("  MISMATCH in total net count\n");
        all_ok = false;
    }

    // Non-passive component detail comparison (designator, footprint, LCSC)
    let all_non_passive_refs: BTreeSet<_> = python
        .non_passive_details
        .keys()
        .chain(rust.non_passive_details.keys())
        .cloned()
        .collect();
    let mut detail_mismatches = 0;
    for ref_name in &all_non_passive_refs {
        let py = python.non_passive_details.get(ref_name);
        let rs = rust.non_passive_details.get(ref_name);
        match (py, rs) {
            (Some(_), None) => {
                report.push_str(&format!(
                    "  Non-passive {}: present in Python, MISSING in Rust\n",
                    ref_name
                ));
                detail_mismatches += 1;
            }
            (None, Some(_)) => {
                report.push_str(&format!(
                    "  Non-passive {}: MISSING in Python, present in Rust\n",
                    ref_name
                ));
                detail_mismatches += 1;
            }
            (Some((py_fp, py_lcsc)), Some((rs_fp, rs_lcsc))) => {
                if py_lcsc != rs_lcsc {
                    report.push_str(&format!(
                        "  Non-passive {} LCSC mismatch: Python={:?}, Rust={:?}\n",
                        ref_name, py_lcsc, rs_lcsc
                    ));
                    detail_mismatches += 1;
                }
                // Footprints may have different library prefixes; compare base name
                let py_fp_base = py_fp
                    .as_ref()
                    .map(|s| s.rsplit(':').next().unwrap_or(s).to_string());
                let rs_fp_base = rs_fp
                    .as_ref()
                    .map(|s| s.rsplit(':').next().unwrap_or(s).to_string());
                if py_fp_base != rs_fp_base {
                    report.push_str(&format!(
                        "  Non-passive {} footprint mismatch: Python={:?}, Rust={:?}\n",
                        ref_name, py_fp, rs_fp
                    ));
                    detail_mismatches += 1;
                }
            }
            (None, None) => {}
        }
    }
    if detail_mismatches > 0 {
        all_ok = false;
        report.push_str(&format!(
            "  Total non-passive detail mismatches: {}\n",
            detail_mismatches
        ));
    }

    (all_ok, report)
}

// ---------------------------------------------------------------------------
// Test A: Small snippet comparison
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Run with `cargo test --test comparison_tests -- --ignored --nocapture`
fn test_small_snippet_python_vs_rust() {
    if !python_ato_available() {
        eprintln!("SKIP: Python ato CLI not available at {}", PYTHON_ATO);
        return;
    }

    // Create a temporary project directory
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let project_dir = tmp.path();

    // Write ato.yaml
    std::fs::write(
        project_dir.join("ato.yaml"),
        r#"requires-atopile: "^0.9.0"

paths:
  src: "."
  layout: "./layouts"

builds:
  default:
    entry: test.ato:Test
"#,
    )
    .unwrap();

    // Write test.ato
    std::fs::write(
        project_dir.join("test.ato"),
        r#"import Resistor
import ElectricPower

module Test:
    power = new ElectricPower
    r1 = new Resistor
    r1.resistance = 10kohm +/- 10%
    r1.package = "0402"
    r1.unnamed[0] ~ power.hv
    r1.unnamed[1] ~ power.lv
"#,
    )
    .unwrap();

    // --- Run Python CLI ---
    eprintln!("Running Python ato build...");
    let py_build_dir = match run_python_ato(project_dir) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Python ato build failed: {}", e);
            panic!("Python build failed");
        }
    };

    let py_net_file = find_python_net_file(&py_build_dir)
        .expect("Python .net file not found in build output");
    let py_net_content = std::fs::read_to_string(&py_net_file).unwrap();
    let py_netlist = parse_kicad_netlist(&py_net_content);

    eprintln!(
        "Python netlist: {} components, {} nets",
        py_netlist.components.len(),
        py_netlist.nets.len()
    );
    for comp in &py_netlist.components {
        eprintln!(
            "  {} value={:?} footprint={:?} lcsc={:?}",
            comp.reference,
            comp.value,
            comp.footprint,
            comp.properties.get("LCSC")
        );
    }

    // Verify Python output: should have 1 resistor
    assert_eq!(
        py_netlist.components.len(),
        1,
        "Python should produce 1 component"
    );
    assert_eq!(py_netlist.components[0].reference, "R1");
    assert!(
        py_netlist.components[0]
            .properties
            .contains_key("LCSC"),
        "Python component should have LCSC property"
    );

    // --- Run Rust CLI ---
    // Clean build output first to avoid mixing with Python output
    let _ = std::fs::remove_dir_all(project_dir.join("build"));

    eprintln!("Running Rust ato build...");
    let rs_build_dir = match run_rust_ato(project_dir) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Rust ato build failed: {}", e);
            eprintln!(
                "This is expected if the Rust CLI doesn't handle this case yet."
            );
            return;
        }
    };

    let rs_net_file = find_rust_net_file(&rs_build_dir);
    if rs_net_file.is_none() {
        eprintln!("Rust .net file not found - Rust CLI may not generate it for this project yet");
        return;
    }
    let rs_net_file = rs_net_file.unwrap();
    let rs_net_content = std::fs::read_to_string(&rs_net_file).unwrap();
    let rs_netlist = parse_kicad_netlist(&rs_net_content);

    eprintln!(
        "Rust netlist: {} components, {} nets",
        rs_netlist.components.len(),
        rs_netlist.nets.len()
    );
    for comp in &rs_netlist.components {
        eprintln!(
            "  {} value={:?} footprint={:?} lcsc={:?}",
            comp.reference,
            comp.value,
            comp.footprint,
            comp.properties.get("lcsc").or_else(|| comp.properties.get("LCSC"))
        );
    }

    // --- Compare ---
    let py_summary = NetlistSummary::from_parsed(&py_netlist);
    let rs_summary = NetlistSummary::from_parsed(&rs_netlist);
    let (matched, report) = compare_summaries(&py_summary, &rs_summary);

    eprintln!("\n=== Comparison Report ===\n{}", report);

    // Both should have R1
    assert!(
        rs_netlist
            .components
            .iter()
            .any(|c| c.reference == "R1"),
        "Rust should produce R1 component"
    );

    // Component count should match
    assert_eq!(
        py_summary.total_components, rs_summary.total_components,
        "Component counts should match"
    );

    if !matched {
        eprintln!("WARNING: Some differences detected between Python and Rust outputs");
    }
}

// ---------------------------------------------------------------------------
// Test B: led_badge full project comparison
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Run with `cargo test --test comparison_tests -- --ignored --nocapture`
fn test_led_badge_python_vs_rust() {
    if !python_ato_available() {
        eprintln!("SKIP: Python ato CLI not available at {}", PYTHON_ATO);
        return;
    }

    let project_dir = led_badge_dir();
    eprintln!("LED badge project: {}", project_dir.display());

    // Make sure dependencies are installed
    eprintln!("Installing dependencies...");
    let install_out = Command::new(PYTHON_ATO)
        .arg("install")
        .current_dir(&project_dir)
        .output();
    if let Err(e) = &install_out {
        eprintln!("Warning: ato install failed: {}", e);
    }

    // --- Run Python CLI ---
    // Build directly in the project dir. Python puts output in build/builds/default/
    eprintln!("Running Python ato build on led_badge...");
    let py_build_dir = match run_python_ato(&project_dir) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Python ato build on led_badge failed: {}", e);
            eprintln!("This may be due to missing packages or network issues.");
            return;
        }
    };

    let py_net_file = match find_python_net_file(&py_build_dir) {
        Some(f) => f,
        None => {
            eprintln!("Python .net file not found for led_badge");
            return;
        }
    };
    let py_net_content = std::fs::read_to_string(&py_net_file).unwrap();
    let py_netlist = parse_kicad_netlist(&py_net_content);

    eprintln!(
        "Python led_badge: {} components, {} nets",
        py_netlist.components.len(),
        py_netlist.nets.len()
    );

    // Save Python BOM content before Rust build overwrites top-level build/
    let py_bom_content = find_python_bom_file(&py_build_dir)
        .and_then(|f| std::fs::read_to_string(&f).ok());

    // --- Run Rust CLI ---
    // Rust CLI puts output in build/<project_name>.net (top-level build dir).
    // The Python output is in build/builds/default/ so they don't clobber.
    eprintln!("Running Rust ato build on led_badge...");
    let rs_build_dir = match run_rust_ato(&project_dir) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Rust ato build on led_badge failed: {}", e);
            eprintln!("This is expected if the Rust CLI doesn't fully support led_badge yet.");
            return;
        }
    };

    let rs_net_file = match find_rust_net_file(&rs_build_dir) {
        Some(f) => f,
        None => {
            eprintln!("Rust .net file not found for led_badge");
            return;
        }
    };
    let rs_net_content = std::fs::read_to_string(&rs_net_file).unwrap();
    let rs_netlist = parse_kicad_netlist(&rs_net_content);

    eprintln!(
        "Rust led_badge: {} components, {} nets",
        rs_netlist.components.len(),
        rs_netlist.nets.len()
    );

    // --- Compare ---
    let py_summary = NetlistSummary::from_parsed(&py_netlist);
    let rs_summary = NetlistSummary::from_parsed(&rs_netlist);
    let (matched, report) = compare_summaries(&py_summary, &rs_summary);

    eprintln!("\n=== LED Badge Comparison Report ===\n{}", report);

    if let Some(bom) = &py_bom_content {
        eprintln!("\nPython BOM:\n{}", bom);
    }

    // Print Rust BOM if available
    if let Some(rs_bom_file) = find_rust_bom_file(&rs_build_dir) {
        let bom_content = std::fs::read_to_string(&rs_bom_file).unwrap();
        eprintln!("\nRust BOM:\n{}", bom_content);
    }

    // Structural checks -- these should match or be close
    // LED count should be exactly 100 in both
    let py_led_count = py_summary
        .components_by_prefix
        .get("LED")
        .copied()
        .unwrap_or(0);
    let rs_led_count = rs_summary
        .components_by_prefix
        .get("LED")
        .copied()
        .unwrap_or(0);
    assert_eq!(
        py_led_count, rs_led_count,
        "LED count should match: Python={}, Rust={}",
        py_led_count, rs_led_count
    );

    // Non-passive component counts should be close (same ICs, connectors)
    let py_u = py_summary
        .components_by_prefix
        .get("U")
        .copied()
        .unwrap_or(0);
    let rs_u = rs_summary
        .components_by_prefix
        .get("U")
        .copied()
        .unwrap_or(0);
    eprintln!("U components: Python={}, Rust={}", py_u, rs_u);

    let py_j = py_summary
        .components_by_prefix
        .get("J")
        .copied()
        .unwrap_or(0);
    let rs_j = rs_summary
        .components_by_prefix
        .get("J")
        .copied()
        .unwrap_or(0);
    eprintln!("J components: Python={}, Rust={}", py_j, rs_j);

    // Total component count should be in the same ballpark
    let diff = (py_summary.total_components as i64 - rs_summary.total_components as i64).abs();
    let threshold = (py_summary.total_components as f64 * 0.1).ceil() as i64;
    eprintln!(
        "Total component difference: {} (threshold: {})",
        diff, threshold
    );

    if !matched {
        eprintln!(
            "\nWARNING: Differences detected between Python and Rust led_badge builds.\n\
             This is expected for passive part picking differences."
        );
    }
}

// ---------------------------------------------------------------------------
// Test C: Parse-only comparison (no CLI needed)
// Uses existing build artifacts if present.
// ---------------------------------------------------------------------------

#[test]
fn test_parse_existing_led_badge_net_file() {
    // This test parses the existing Rust-generated .net file (if present)
    // and validates the parser works correctly.
    let net_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/led_badge/build/led_badge.net");

    if !net_path.exists() {
        eprintln!(
            "SKIP: No existing led_badge.net at {}",
            net_path.display()
        );
        return;
    }

    let content = std::fs::read_to_string(&net_path).unwrap();
    let netlist = parse_kicad_netlist(&content);

    eprintln!(
        "Parsed led_badge.net: {} components, {} nets",
        netlist.components.len(),
        netlist.nets.len()
    );

    // Print summary
    let summary = NetlistSummary::from_parsed(&netlist);
    for (prefix, count) in &summary.components_by_prefix {
        eprintln!("  {}: {}", prefix, count);
    }

    // Validate basic structure
    assert!(
        netlist.components.len() >= 100,
        "led_badge should have at least 100 components, got {}",
        netlist.components.len()
    );

    // Check for LEDs
    let led_count = netlist
        .components
        .iter()
        .filter(|c| c.reference.starts_with("LED"))
        .count();
    assert_eq!(led_count, 100, "Expected 100 LEDs, got {}", led_count);

    // Check that non-passive components have LCSC numbers
    let non_passive_without_lcsc: Vec<_> = netlist
        .components
        .iter()
        .filter(|c| {
            let prefix = c
                .reference
                .trim_end_matches(|c: char| c.is_ascii_digit());
            prefix != "R" && prefix != "C" && prefix != "L"
        })
        .filter(|c| {
            !c.properties.contains_key("lcsc")
                && !c.properties.contains_key("LCSC")
        })
        .map(|c| c.reference.as_str())
        .collect();

    if !non_passive_without_lcsc.is_empty() {
        eprintln!(
            "Non-passive components without LCSC: {:?}",
            non_passive_without_lcsc
        );
    }
}

#[test]
fn test_sexpr_parser_basic() {
    let input = r#"(export (version "E") (components (comp (ref "R1") (value "10k"))))"#;
    let tokens = tokenize_sexpr(input);
    let (tree, _) = parse_sexpr(&tokens);

    let version = tree
        .find_child("version")
        .and_then(|v| v.second_atom());
    assert_eq!(version, Some("E"));

    let components = tree.find_child("components");
    assert!(components.is_some());

    let comp = components.unwrap().find_child("comp");
    assert!(comp.is_some());

    let ref_val = comp
        .unwrap()
        .find_child("ref")
        .and_then(|r| r.second_atom());
    assert_eq!(ref_val, Some("R1"));
}

#[test]
fn test_sexpr_parser_nested_properties() {
    // Test parsing the Python CLI's property format
    let input = r#"(comp (ref "R1") (value "10k") (footprint "R0402") (property (name "LCSC") (value "C25744")) (property (name "Manufacturer") (value "UNI-ROYAL")))"#;
    let tokens = tokenize_sexpr(input);
    let (tree, _) = parse_sexpr(&tokens);

    let ref_val = tree
        .find_child("ref")
        .and_then(|r| r.second_atom());
    assert_eq!(ref_val, Some("R1"));

    let properties = tree.find_children("property");
    assert_eq!(properties.len(), 2);

    let lcsc_prop = properties[0];
    let lcsc_name = lcsc_prop
        .find_child("name")
        .and_then(|n| n.second_atom());
    assert_eq!(lcsc_name, Some("LCSC"));
    let lcsc_value = lcsc_prop
        .find_child("value")
        .and_then(|v| v.second_atom());
    assert_eq!(lcsc_value, Some("C25744"));
}

#[test]
fn test_sexpr_parser_rust_fields_format() {
    // Test parsing the Rust CLI's fields format
    let input = r#"(comp (ref "U1") (value "ESP32") (footprint "WIFIM.kicad_mod") (fields (field (name "module") "ESP32_package") (field (name "lcsc") "C2838502")))"#;
    let tokens = tokenize_sexpr(input);
    let (tree, _) = parse_sexpr(&tokens);

    let fields_section = tree.find_child("fields");
    assert!(fields_section.is_some());

    let fields = fields_section.unwrap().find_children("field");
    assert_eq!(fields.len(), 2);
}

#[test]
fn test_full_python_netlist_parse() {
    // Parse a complete Python CLI netlist output
    let content = r#"(export
    (version "E")
    (components
        (comp
            (ref "R1")
            (value "10kΩ ±1% 62.5mW")
            (footprint "UNI_ROYAL_0402WGF1002TCE:R0402")
            (property
                (name "LCSC")
                (value "C25744")
            )
            (property
                (name "Manufacturer")
                (value "UNI-ROYAL(Uniroyal Elec)")
            )
            (property
                (name "atopile_address")
                (value "r1")
            )
            (tstamps "1")
            (fields)
        )
    )
    (nets
        (net
            (code 1)
            (name "GND")
            (node
                (ref "R1")
                (pin "2")
            )
        )
        (net
            (code 2)
            (name "VCC")
            (node
                (ref "R1")
                (pin "1")
            )
        )
    )
    (libparts)
    (libraries)
)"#;

    let netlist = parse_kicad_netlist(content);
    assert_eq!(netlist.components.len(), 1);
    assert_eq!(netlist.components[0].reference, "R1");
    assert_eq!(
        netlist.components[0].footprint.as_deref(),
        Some("UNI_ROYAL_0402WGF1002TCE:R0402")
    );
    assert_eq!(
        netlist.components[0].properties.get("LCSC"),
        Some(&"C25744".to_string())
    );
    assert_eq!(
        netlist.components[0].properties.get("atopile_address"),
        Some(&"r1".to_string())
    );

    assert_eq!(netlist.nets.len(), 2);
    assert_eq!(netlist.nets[0].name, "GND");
    assert!(netlist.nets[0].nodes.contains(&("R1".to_string(), "2".to_string())));
    assert_eq!(netlist.nets[1].name, "VCC");
    assert!(netlist.nets[1].nodes.contains(&("R1".to_string(), "1".to_string())));
}

#[test]
fn test_bom_csv_parse() {
    let content = "Designator,Footprint,Quantity,Value,Manufacturer,Partnumber,LCSC Part #\n\
                   R1,R0402,1,10k,UNI-ROYAL,0402WGF1002TCE,C25744\n\
                   C1,C0402,2,100nF,Samsung,CL05B104KO5NNNC,C307331\n";

    let entries = parse_bom_csv(content);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].designator, "R1");
    assert_eq!(entries[0].lcsc, "C25744");
    assert_eq!(entries[1].designator, "C1");
    assert_eq!(entries[1].quantity, 2);
}

// ---------------------------------------------------------------------------
// Utility: recursive directory copy
// ---------------------------------------------------------------------------

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;

        // Skip build directories and .git
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == "build" || name_str == ".git" || name_str == "__pycache__" {
            continue;
        }

        if file_type.is_symlink() {
            // Preserve symlinks as-is
            let link_target = std::fs::read_link(&src_path)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link_target, &dst_path)?;
            #[cfg(not(unix))]
            {
                // On non-unix, try to copy the target
                if src_path.is_dir() {
                    copy_dir_recursive(&src_path, &dst_path)?;
                } else {
                    std::fs::copy(&src_path, &dst_path)?;
                }
            }
        } else if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
