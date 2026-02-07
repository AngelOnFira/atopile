# Remaining Work — Rust Atopile Implementation

**Updated:** 2026-02-07
**Branch:** `rust-test`

This document tracks issues that were NOT fully resolved during the audit fix session,
so future agent teams can pick them up. Issues are grouped by priority.

---

## What Was Fixed This Session

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

---

## P0 — Critical Remaining Issues

### 1. Type checker is a permissive stub (was C6)
**File:** `crates/ato-sema/src/types.rs`
**Issue:** `get_module_interface()` was removed as dead code (always returned `None`). Interface compatibility is never checked — you can connect I2C to SPI without error. Sub-field resolution failures are silently skipped.
**Scope:** Large feature. Requires:
- Define what interface compatibility means in ato (structural typing? nominal typing?)
- Implement interface type extraction from modules
- Add connection type checking in the sema pass
- Add clear error messages for type mismatches

### 2. PCB layout preservation
**Issue:** Every Rust build overwrites existing component placement and routing in KiCad files. The Python implementation preserves layout across builds via faebryk's `set_pcb_position` and net/designator stability.
**Scope:** Large feature. Requires:
- Parse existing `.kicad_pcb` file to extract component positions
- Preserve component positions when regenerating
- Stable designator assignment (currently sequential)
- Stable net naming

### 3. Passive part picking end-to-end verification
**Issue:** C5 was improved (3 lookup strategies) but not fully verified end-to-end with real designs. The solver→constraint_collector→part_picker pipeline may still have naming mismatches for complex module hierarchies.
**How to verify:** Build led_badge, check BOM output — do resistors/capacitors get correct LCSC part numbers and values?

### 4. Retype (`->`) incomplete
**Issue:** 2 buttons missing in led_badge output. The retype operator may not update `resolved_type` on instances correctly.
**File:** `crates/ato-sema/src/analyzer.rs` (retype handling)
**Scope:** Medium. Need to trace why retype doesn't propagate type info.

### 5. `UnsupportedFeature` errors for function calls may be too aggressive
**Issue:** The C2 fix now emits errors for function calls. If real .ato files use constructs that parse as function calls (e.g., `abs(x)` in assertions), the build may now fail where it previously succeeded silently. Need to check if this causes regressions in real projects.
**How to verify:** Build led_badge and espaper, check for unexpected `UnsupportedFeature` errors.

---

## P1 — Important Missing Features

### 6. `in` keyword conflict (was m2)
**File:** `crates/ato-lexer/src/`
**Issue:** `in` is lexed as a keyword but is used as an identifier in hardware designs (signal names like `in`, `input`). 1/83 syntax tests fails due to this.
**Scope:** Medium. Requires contextual lexing or keyword escaping.

### 7. No dimensional analysis validation (was m4)
**File:** `crates/ato-solver/`
**Issue:** You can add Volts to Ohms without error. The constant folder handles some unit computation (V/A = Ohm) but there's no enforcement of dimensional correctness.
**Scope:** Medium. Add unit checking in the solver's arithmetic operations.

### 8. KiCad netlist S-expression quoting (was m5)
**File:** `crates/ato-export/src/kicad.rs`
**Issue:** Numeric fields like `code` and `tstamp` are quoted with `""`. Some KiCad versions expect unquoted integers: `(net (code 1) ...)` not `(net (code "1") ...)`.
**Scope:** Small. Change format strings.

### 9. No LSP server
**Issue:** Required for IDE extension (VS Code/Cursor). Python implementation has LSP.
**Scope:** Large. New crate. Could use `tower-lsp`.

### 10. Error accumulation
**Issue:** Rust fails on first error in many places. Python accumulates all errors and reports them together, which is much better UX.
**Scope:** Medium-Large. Systematic change across sema and build pipeline.

### 11. No `.py` build support
**Issue:** Some packages use Python entry points (`.py` files in `ato.yaml`). Rust CLI can't build these.
**Scope:** Medium. Would need embedded Python or subprocess delegation.

### 12. No schematic wiring
**Issue:** KiCad schematic export creates components but no wires between them. Python implementation generates wired schematics.
**File:** `crates/ato-export/src/kicad_schematic.rs`
**Scope:** Medium-Large. Requires wire routing algorithm.

### 13. No net/designator preservation across builds
**Issue:** Designators are assigned sequentially on each build. Net names may change. This breaks PCB layout stability.
**Scope:** Medium. Needs deterministic naming based on hierarchy path.

---

## P2 — Nice to Have

### 14. Circular inheritance not detected
**Issue:** `module A from B` where `B from A` would cause infinite loops.
**Scope:** Small. Add cycle detection in module resolution.

### 15. No fuzz testing
**Issue:** Parser should never panic on arbitrary input. No fuzz tests exist.
**Scope:** Small-Medium. Add cargo-fuzz targets for lexer and parser.

### 16. No snapshot tests for output formats
**Issue:** No tests verify that output files (.net, .kicad_sch, .csv) have correct format.
**Scope:** Medium. Add insta snapshot tests for export crate.

### 17. `serde_yaml` deprecated
**File:** `crates/ato-cli/Cargo.toml`
**Issue:** `serde_yaml` crate is deprecated upstream. Should migrate to `serde_yml`.
**Scope:** Small. Dependency swap + minor API changes.

### 18. No self-contained E2E test
**Issue:** Led_badge tests require pre-installed packages. No test goes from .ato source to valid output without external dependencies.
**Scope:** Medium. Create a small self-contained test project with inline components.

### 19. Package/footprint compatibility validation
**Issue:** No validation that assigned KiCad footprint actually exists in user's library.
**Scope:** Small. Add check in export phase.

---

## Test Status After Fixes

```
ato-domain:  45/45 pass
ato-solver:  58/58 pass
ato-export:  60/60 pass
ato-parts:   29/29 pass
ato-parser:  80/80 pass
ato-lexer:   18/18 pass
ato-ir:      36/36 pass
ato-tests:   62/97 pass (35 led_badge failures need `ato install`)
ato-cli:     35/36 pass (1 led_badge failure needs `ato install`)
─────────────────────────
Total:       423 pass, 36 pre-existing failures, 2 ignored, 0 warnings
```

All 36 failures are in led_badge tests that require packages to be installed first (`ato install` in the `examples/led_badge` directory). These are not regressions.
