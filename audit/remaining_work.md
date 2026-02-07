# Remaining Work — Rust Atopile Implementation

**Updated:** 2026-02-07 (session 2)
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

---

## P0 — Critical Remaining Issues

### 1. PCB layout preservation
**Issue:** Every Rust build overwrites existing component placement and routing in KiCad files. The Python implementation preserves layout across builds via faebryk's `set_pcb_position` and net/designator stability.
**Scope:** Large feature. Requires:
- Parse existing `.kicad_pcb` file to extract component positions
- Preserve component positions when regenerating
- Designator stability now partially solved (deterministic naming), but position preservation is still missing

### 2. Passive part picking end-to-end verification
**Issue:** C5 was improved (3 lookup strategies) but not fully verified end-to-end with real designs. The solver→constraint_collector→part_picker pipeline may still have naming mismatches for complex module hierarchies.
**How to verify:** Build led_badge with packages installed, check BOM output — do resistors/capacitors get correct LCSC part numbers and values?

### 3. Retype needs E2E verification
**Issue:** Retype handling improved (error reporting, non-Instance conversion), but the led_badge "2 missing buttons" issue hasn't been verified as resolved. Need to build with packages installed and confirm buttons appear.
**File:** `crates/ato-sema/src/lower.rs` (lower_retype)

---

## P1 — Important Missing Features

### 4. No LSP server
**Issue:** Required for IDE extension (VS Code/Cursor). Python implementation has LSP.
**Scope:** Large. New crate. Could use `tower-lsp`.

### 5. No `.py` build support
**Issue:** Some packages use Python entry points (`.py` files in `ato.yaml`). Rust CLI can't build these.
**Scope:** Medium. Would need embedded Python or subprocess delegation.

### 6. No schematic wiring
**Issue:** KiCad schematic export creates components but no wires between them. Python implementation generates wired schematics.
**File:** `crates/ato-export/src/kicad_schematic.rs`
**Scope:** Medium-Large. Requires wire routing algorithm.

### 7. Type checker should be stricter when type info is complete
**Issue:** The type checker is currently permissive — when both types are known but different with no inheritance relationship, it still allows the connection. This was necessary to avoid false positives from incomplete type info (external packages). Once structural type checking is implemented, the permissive fallback should be removed.
**File:** `crates/ato-sema/src/types.rs` line 322
**Scope:** Medium. Requires structural type comparison (comparing field signatures of interfaces).

---

## P2 — Nice to Have

### 8. No fuzz testing
**Issue:** Parser should never panic on arbitrary input. No fuzz tests exist.
**Scope:** Small-Medium. Add cargo-fuzz targets for lexer and parser.

### 9. No snapshot tests for output formats
**Issue:** No tests verify that output files (.net, .kicad_sch, .csv) have correct format.
**Scope:** Medium. Add insta snapshot tests for export crate.

### 10. Package/footprint compatibility validation
**Issue:** No validation that assigned KiCad footprint actually exists in user's library.
**Scope:** Small. Add check in export phase.

---

## Test Status After Session 2

```
ato-lexer:    18/18 pass
ato-parser:   80/80 pass
ato-domain:   45/45 pass
ato-solver:   67/67 pass
ato-ir:       36/36 pass
ato-sema:    105/105 pass (+2 ignored)
ato-export:   66/66 pass
ato-parts:    29/29 pass
ato-tests:    79/114 pass (35 led_badge failures need `ato install`) (+2 ignored)
ato-cli:      35/36 pass (1 led_badge failure needs `ato install`)
─────────────────────────
Total:       560 pass, 36 pre-existing failures, 4 ignored, 0 warnings
```

All 36 failures are in led_badge tests that require packages to be installed first (`ato install` in the `examples/led_badge` directory). These are not regressions.

Test improvements from session 2:
- +8 solver tests (dimensional analysis)
- +12 type checker tests (interface compatibility, inheritance, permissiveness)
- +9 E2E standalone tests (full pipeline without external deps)
- +2 sema tests (circular inheritance, retype)
- +6 export tests (deterministic output)
- Fixed test_gap_in_as_identifier (was known failure, now passes)
