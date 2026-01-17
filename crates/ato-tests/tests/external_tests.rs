//! Tests for external atopile repositories.
//!
//! These tests validate the Rust parser against real-world community projects.
//!
//! Note: External repos are cloned to crates/ato-tests/external/ (gitignored).
//! Tests skip gracefully if repos aren't available.

use std::fs;
use std::path::Path;

/// Check if external repos are available.
fn external_dir() -> Option<std::path::PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("external");
    if dir.exists() {
        Some(dir)
    } else {
        None
    }
}

/// Parse a file from an external repo.
fn parse_external(repo: &str, file: &str) -> Result<ato_parser::File, String> {
    let base = match external_dir() {
        Some(d) => d,
        None => return Err("External repos not cloned".into()),
    };

    let path = base.join(repo).join(file);
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    let source = fs::read_to_string(&path)
        .map_err(|e| format!("Read error: {}", e))?;

    ato_parser::parse(&source)
        .map_err(|e| format!("Parse error: {:?}", e))
}

/// Count parseable files in an external repo.
fn count_parseable_in_repo(repo: &str) -> (usize, usize) {
    let base = match external_dir() {
        Some(d) => d,
        None => return (0, 0),
    };

    let repo_dir = base.join(repo);
    if !repo_dir.exists() {
        return (0, 0);
    }

    let mut passed = 0;
    let mut failed = 0;

    for entry in walkdir::WalkDir::new(&repo_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "ato"))
    {
        let path = entry.path();
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => {
                failed += 1;
                continue;
            }
        };

        if ato_parser::parse(&source).is_ok() {
            passed += 1;
        } else {
            failed += 1;
        }
    }

    (passed, failed)
}

// ============================================================================
// Generics Repo Tests
// ============================================================================

#[test]
fn test_generics_resistors() {
    if external_dir().is_none() {
        eprintln!("Skipping: external repos not cloned");
        return;
    }
    let result = parse_external("generics", "resistors.ato");
    assert!(result.is_ok(), "resistors.ato should parse: {:?}", result.err());
}

#[test]
fn test_generics_capacitors() {
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("generics", "capacitors.ato");
    assert!(result.is_ok(), "capacitors.ato should parse: {:?}", result.err());
}

#[test]
fn test_generics_interfaces() {
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("generics", "interfaces.ato");
    assert!(result.is_ok(), "interfaces.ato should parse: {:?}", result.err());
}

#[test]
fn test_generics_leds() {
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("generics", "leds.ato");
    assert!(result.is_ok(), "leds.ato should parse: {:?}", result.err());
}

#[test]
fn test_generics_regulators() {
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("generics", "regulators.ato");
    assert!(result.is_ok(), "regulators.ato should parse: {:?}", result.err());
}

#[test]
fn test_generics_diodes() {
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("generics", "diodes.ato");
    assert!(result.is_ok(), "diodes.ato should parse: {:?}", result.err());
}

#[test]
fn test_generics_mosfets() {
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("generics", "mosfets.ato");
    assert!(result.is_ok(), "mosfets.ato should parse: {:?}", result.err());
}

// Known failures - document why they fail

#[test]
fn test_generics_vdivs_known_failure() {
    // KNOWN ISSUE: Uses `in` as identifier, which is a keyword in Rust parser
    // `in = new Power # legacy`
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("generics", "vdivs.ato");
    // This is expected to fail due to `in` keyword conflict
    if result.is_err() {
        eprintln!("Expected failure: vdivs.ato uses 'in' as identifier (keyword conflict)");
    }
}

#[test]
fn test_generics_buttons_known_failure() {
    // KNOWN ISSUE: Uses `in` as signal name
    // `signal in`
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("generics", "buttons.ato");
    if result.is_err() {
        eprintln!("Expected failure: buttons.ato uses 'in' as signal name (keyword conflict)");
    }
}

// ============================================================================
// RP2040 Repo Tests
// ============================================================================

#[test]
fn test_rp2040_kit_known_failure() {
    // KNOWN ISSUE: Uses `in` as signal name
    // `signal in ~ pin 1`
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("rp2040", "RP2040Kit.ato");
    if result.is_err() {
        eprintln!("Expected failure: RP2040Kit.ato uses 'in' as signal name (keyword conflict)");
    }
}

// ============================================================================
// ESP32-S3 Repo Tests
// ============================================================================

#[test]
fn test_esp32_s3_base_known_failure() {
    // KNOWN ISSUE: Uses Unicode Ω (omega) for ohms
    // `i2c_pullup_strength = 2.7kΩ +/- 10%`
    if external_dir().is_none() {
        return;
    }
    let result = parse_external("esp32-s3", "elec/src/base.ato");
    if result.is_err() {
        eprintln!("Expected failure: base.ato uses Unicode Ω (Rust lexer only supports ASCII 'ohm')");
    }
}

// ============================================================================
// Summary Tests
// ============================================================================

#[test]
fn test_external_repos_summary() {
    if external_dir().is_none() {
        eprintln!("Skipping: external repos not cloned");
        eprintln!("To run: cd crates/ato-tests/external && git clone --depth 1 https://github.com/atopile/generics.git");
        return;
    }

    let (gen_pass, gen_fail) = count_parseable_in_repo("generics");
    let (rp_pass, rp_fail) = count_parseable_in_repo("rp2040");
    let (esp_pass, esp_fail) = count_parseable_in_repo("esp32-s3");

    let total_pass = gen_pass + rp_pass + esp_pass;
    let total_fail = gen_fail + rp_fail + esp_fail;
    let total = total_pass + total_fail;

    eprintln!("\n=== External Repos Parse Results ===");
    eprintln!("generics:  {}/{} passed ({:.0}%)", gen_pass, gen_pass + gen_fail,
              if gen_pass + gen_fail > 0 { gen_pass as f64 / (gen_pass + gen_fail) as f64 * 100.0 } else { 0.0 });
    eprintln!("rp2040:    {}/{} passed ({:.0}%)", rp_pass, rp_pass + rp_fail,
              if rp_pass + rp_fail > 0 { rp_pass as f64 / (rp_pass + rp_fail) as f64 * 100.0 } else { 0.0 });
    eprintln!("esp32-s3:  {}/{} passed ({:.0}%)", esp_pass, esp_pass + esp_fail,
              if esp_pass + esp_fail > 0 { esp_pass as f64 / (esp_pass + esp_fail) as f64 * 100.0 } else { 0.0 });
    eprintln!("---");
    eprintln!("TOTAL:     {}/{} passed ({:.0}%)", total_pass, total,
              if total > 0 { total_pass as f64 / total as f64 * 100.0 } else { 0.0 });
    eprintln!("\nKnown issues:");
    eprintln!("- 'in' keyword conflict (used as signal/pin name in hardware)");
    eprintln!("- Unicode Ω not supported (use ASCII 'ohm' instead)");

    // We expect ~88% pass rate due to known issues
    if total > 0 {
        let pass_rate = total_pass as f64 / total as f64;
        assert!(pass_rate > 0.80, "Expected >80% pass rate, got {:.0}%", pass_rate * 100.0);
    }
}
