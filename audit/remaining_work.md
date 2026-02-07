# Remaining Work — Rust Atopile Implementation

**Updated:** 2026-02-07 (session 3)
**Branch:** `rust-test`

This document tracks issues that were NOT fully resolved during the audit fix sessions,
so future agent teams can pick them up. Issues are grouped by priority.

---

## What Was Fixed — Session 1 (Audit Fixes)

| ID | Issue | Status |
|----|-------|--------|
| C1 | Non-physical assignments silently dropped | **Fixed** — strings, booleans, and full arithmetic expressions now lowered properly |
| C2 | Function calls lowered to `false` | **Fixed** — now emits `UnsupportedFeature` error instead of silent wrong value |
| C3 | Constraint collector nested unit resolution | **Fixed** — full field path traversal for `r1.resistance` etc. |
| C4 | Shallow dependency tracking | **Fixed** — recursive expression tree traversal |
| C5 | Part picker parameter lookup | **Improved** — tries 3 lookup strategies (instance.param, param, fuzzy prefix) |
| M1 | Imported module errors discarded | **Fixed** — errors collected and reported as warnings |
| M2 | Power operation not folded | **Fixed** — `f64::powf` for `**` operator |
| M3 | LCSC client creates new instance per call | **Fixed** — interior mutability with Mutex |
| M4 | Netlist builder falls back to empty netlist | **Fixed** — now hard error |
| M5 | Solver timeout treated as warning | **Fixed** — now hard error |
| M6 | Import file resolution discarded | **Fixed** — errors collected |
| M7 | Part picker top-level key clobbering | **Fixed** — removed top-level overwrites |
| M8 | Constraint collection errors non-fatal | **Fixed** — now hard error |
| — | Dead code cleanup (~320 lines) | **Done** — ImportResolver removed, unused deps removed |
| — | Compiler warnings | **Done** — 0 warnings |

## What Was Fixed — Session 2 (Remaining Items)

| Old # | Issue | Status |
|-------|-------|--------|
| P0.1 (C6) | Type checker is a permissive stub | **Fixed** — Interface type checking implemented with inheritance chain walking, category tracking (Pin/Signal/Instance/Parameter), permissive mode for unresolved types |
| P0.4 | Retype (`->`) incomplete | **Improved** — Better error reporting, non-Instance field conversion support; still needs E2E verification with led_badge |
| P0.5 | UnsupportedFeature may be too aggressive | **Verified OK** — esp32_minimal now passes; led_badge failure is pre-existing (packages not installed) |
| P1.6 | `in` keyword conflict | **Fixed** — Parser now accepts `in` as identifier; test_gap_in_as_identifier passes |
| P1.7 | No dimensional analysis | **Fixed** — Add/subtract validates unit compatibility; mul/div with dimensionless preserves units |
| P1.8 | KiCad S-expression quoting | **Fixed** — Numeric fields (code, tstamp) now unquoted |
| P1.10 | Error accumulation | **Fixed** — Build errors collected and reported together; solver errors don't short-circuit |
| P1.13 | No net/designator preservation | **Fixed** — Two-phase collection with deterministic sort by (prefix, hierarchy_path); natural-order comparison; nets/nodes sorted |
| P2.14 | Circular inheritance not detected | **Fixed** — Cycle detection in both analyzer.rs (inheritance_chain tracking) and names.rs (super_type chain walking) |
| P2.17 | `serde_yaml` deprecated | **Fixed** — Migrated to `serde_yml` across workspace |
| P2.18 | No self-contained E2E test | **Fixed** — 9 tests in e2e_standalone_tests.rs covering full pipeline without external deps |

## What Was Fixed — Session 3 (P0/P1 Items)

| Old # | Issue | Status |
|-------|-------|--------|
| P0.2 | Passive part picking E2E | **Verified** — Non-passive components get correct LCSC numbers/designators/footprints. Passives show placeholder values (`$capacitance`, `$resistance`) — root cause is string literals (`package = "R0402"`) being fed to numerical solver. Needs separate string-valued parameter path. |
| P0.3 | Retype E2E verification | **Fixed** — Analyzer Pass 2 was missing `Statement::Retype` for imported modules. Added it; led_badge now produces 240 components including SW1/SW2 buttons (was 238). |
| P1.7 | Stricter type checking | **Fixed** — Structural type checking implemented (confirm-only mode). Compares field signatures of connected interfaces. Conservative: confirms matches but never rejects to avoid false positives. |
| P2.8 | No fuzz testing | **Fixed** — 3 cargo-fuzz targets (lexer, parser, pipeline) with seed corpus of 10 .ato files each. |
| — | Auto-install packages | **Fixed** — `ensure_packages_installed()` runs before build (like cargo). `--no-install` flag to skip. |
| — | Unit aliases missing | **Fixed** — Added "current", "voltage", "dimensionless", "Ah" (AmpereHour) unit aliases for community package compatibility. |
| — | ESP32 stub type mismatch | **Fixed** — Changed `usb2 = new USB` to `usb2 = new USB2_0` in esp32_s3.ato |

---

## P0 — Critical Remaining Issues

### 1. PCB layout preservation
**Issue:** Every Rust build overwrites existing component placement and routing in KiCad files. The Python implementation preserves layout across builds via faebryk's `set_pcb_position` and net/designator stability.
**Scope:** Large feature. Requires:
- Parse existing `.kicad_pcb` file to extract component positions
- Preserve component positions when regenerating
- Designator stability now partially solved (deterministic naming), but position preservation is still missing

### 2. Passive part picking string parameters
**Issue:** Passive components (resistors, capacitors) show placeholder values like `$capacitance`, `$resistance` in BOM output instead of actual values. The root cause is that string-valued parameters (`package = "R0402"`) are being fed to the numerical solver, which only handles quantities. String parameters need a separate handling path.
**Files:** `crates/ato-solver/`, `crates/ato-parts/`
**Scope:** Medium. Needs string parameter extraction separate from numerical solving.

---

## P1 — Important Missing Features

### 3. No LSP server
**Issue:** Required for IDE extension (VS Code/Cursor). Python implementation has LSP.
**Scope:** Large. New crate. Could use `tower-lsp`.

### 4. No `.py` build support
**Issue:** Some packages use Python entry points (`.py` files in `ato.yaml`). Rust CLI can't build these.
**Scope:** Medium. Would need embedded Python or subprocess delegation.

### 5. No schematic wiring
**Issue:** KiCad schematic export creates components but no wires between them. Python implementation generates wired schematics.
**File:** `crates/ato-export/src/kicad_schematic.rs`
**Scope:** Medium-Large. Requires wire routing algorithm.

---

## P2 — Nice to Have

### 6. No snapshot tests for output formats
**Issue:** No tests verify that output files (.net, .kicad_sch, .csv) have correct format.
**Scope:** Medium. Add insta snapshot tests for export crate.

### 7. Package/footprint compatibility validation
**Issue:** No validation that assigned KiCad footprint actually exists in user's library.
**Scope:** Small. Add check in export phase.

---

## Test Status After Session 3

```
Total: 796 pass, 0 fail, 15 ignored, 0 warnings
```

All led_badge tests now pass (43/43) after retype fix and unit alias additions.

Test improvements from session 3:
- +6 type checker tests (structural type checking)
- Led_badge tests updated: 240 components (was 238), SW prefix for buttons
- All previous led_badge failures resolved (retype fix + unit aliases)

Test improvements from session 2:
- +8 solver tests (dimensional analysis)
- +12 type checker tests (interface compatibility, inheritance, permissiveness)
- +9 E2E standalone tests (full pipeline without external deps)
- +2 sema tests (circular inheritance, retype)
- +6 export tests (deterministic output)
- Fixed test_gap_in_as_identifier (was known failure, now passes)
