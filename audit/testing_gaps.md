# Test Coverage Audit: Rust Atopile Implementation

**Date**: 2026-02-07
**Branch**: `rust-test`
**Total `#[test]` functions**: ~756 across 67 files

---

## Executive Summary

The codebase has **extensive breadth** of testing -- every crate has inline unit tests and the `ato-tests` crate provides good integration coverage. However, there are **critical depth gaps** in several areas where bugs are most likely to hide: the solver's interaction with sema, net connectivity correctness, passive part picking, and error recovery. The led_badge end-to-end tests are the strongest asset but are fragile (require pre-installed packages).

**Risk priority**: The gaps below are ordered from "most likely to let real bugs through" to "nice to have."

---

## 1. Coverage by Crate

### ato-lexer (inline: 11 tests, integration: 6 tests)
**What exists**: Token type coverage, span tracking, nested indentation, comprehensive token check.
**What's missing**:
- **P1**: No tests for **error recovery** -- what happens with malformed UTF-8, mixed tabs/spaces, unterminated strings?
- **P2**: No tests for **edge-case numbers**: `0x`, `0b` without digits, `1.2.3`, `1e10` (scientific notation), numbers at EOF
- **P2**: No tests for **INDENT/DEDENT with blank lines** or **trailing whitespace**
- **P3**: No tests for very long lines, very deep nesting, or performance regression

### ato-parser (inline: ~70 tests via chumsky submodules, integration: 13 tests)
**What exists**: Good syntax coverage via `syntax_coverage_tests.rs` (83 tests), fixture-based parse tests, AST structure verification.
**What's missing**:
- **P1**: No tests for **error recovery** -- parser currently fails hard. No test verifying useful error messages for common mistakes (missing indentation, wrong operator, etc.)
- **P1**: No tests for **semicolon-separated statements** actually producing correct multi-statement AST (known gap, test just asserts failure)
- **P2**: No tests for **deeply nested expressions** like `((((a + b) * c) / d) ** e)`
- **P2**: No tests for **malformed physical quantities** like `10kohm5V` or `5V3.3`
- **P2**: No snapshot/golden tests for AST output -- if the AST shape changes, nothing catches it except downstream failures
- **P3**: No fuzz testing (parser should never panic on arbitrary input)

### ato-sema (inline: ~84 tests across 9 source files, integration: 24 sema_tests + 29 negative_tests)
**What exists**: Name resolution, scope handling, connection lowering, constraint collection, type system basics, imports, forward references, analyzer pipeline.
**What's missing**:
- **P1**: **No test for interface type compatibility in connections** -- `I2C ~ SPI` is accepted silently (documented as ignored bug)
- **P1**: **No test for circular inheritance detection** -- `A from B` + `B from A` doesn't error (documented as ignored bug)
- **P1**: **No test for `->` retype correctness** -- retype is parsed but the sema effect (updating resolved_type on instances) is untested. The buttons-missing bug was caused by this.
- **P1**: **No test verifying inherited fields are accessible on derived instances** -- e.g., `module D from B: pass` then `d = new D; d.inherited_field ~ something`
- **P2**: **No test for embedded stdlib loading** -- path resolution for `__embedded_stdlib__` is a known gotcha with multiple code paths
- **P2**: **No test for resolve_path fallback** -- `from "file.ato" import X` in packages needs embedded stdlib fallback
- **P2**: **No test for for-loop body lowering correctness** -- do array element connections produce the right IR?
- **P2**: **No test for trait propagation through inheritance chains** (led_badge tests check existence but not correctness)
- **P3**: **No test for cumulative/set assignments** (`+=`, `-=`, `|=`, `&=`) producing correct IR

### ato-ir (inline: ~35 tests across 6 source files)
**What exists**: ID generation, field kinds, module structure, connection representation, constraint representation, design queries.
**What's missing**:
- **P2**: No tests for **Design serialization/deserialization** -- if the IR is ever cached or transmitted
- **P3**: No tests for **design-level queries** like `find_module`, `entry_module` edge cases

### ato-solver (inline: ~47 tests across solver.rs + simplify modules, integration: 34 solver_tests)
**What exists**: Good coverage of basic constraint solving, parameter narrowing (subset, <=, >=, <, is, within), contradiction detection, constant folding (multiply, divide), multiple parameter handling.
**What's missing**:
- **P1**: **No test for the full sema-to-solver pipeline** -- constraints come from `ConstraintCollector::collect()` which transforms IR constraints into solver predicates. This transformation is tested only indirectly via led_badge.
- **P1**: **No test for arithmetic expression solving** -- e.g., `assert voltage_out is (voltage_in * r2 / (r1 + r2))` where one variable is solved from the others. This is the voltage divider equation that users will actually write.
- **P1**: **No test for interval arithmetic with physical units** -- does `10kohm +/- 5% * 1mA` produce the correct voltage interval?
- **P2**: **No test for `assert x within y +/- z%` coming from actual parsed .ato source** (unit tests construct predicates manually)
- **P2**: **No test for solver convergence** with cyclic dependencies between parameters
- **P2**: No property-based tests (e.g., proptest) verifying solver invariants (e.g., narrowing is monotonic, intersection is commutative)
- **P3**: No benchmark tests for solver performance with large constraint sets

### ato-domain (inline: ~42 tests across 5 source files)
**What exists**: Good unit test coverage -- intervals (creation, arithmetic, intersection, subset, center), disjoint intervals (union, intersection, difference, reciprocal), tolerances (relative, absolute, range, contains, overlaps, subset), units (parsing, SI prefix, formatting), quantities.
**What's missing**:
- **P2**: No tests for **division by zero** in interval arithmetic (interval spanning zero)
- **P2**: No tests for **NaN/infinity** propagation through quantity arithmetic
- **P2**: No tests for **unit conversion** between compatible units (mm to m, mV to V)
- **P3**: No tests for very small intervals (floating point precision issues)

### ato-export (inline: ~51 tests across 7 source files)
**What exists**: Netlist building, KiCad netlist/schematic/PCB/project export, BOM generation/grouping/CSV export, library symbol generation.
**What's missing**:
- **P1**: **No test for net connectivity correctness** -- are the right pins actually connected to the right nets? The tests check component counts but not that `R1.1` is on the same net as `LED1.anode`. This is the single most critical thing to verify.
- **P1**: **No test for designator assignment correctness** -- uniqueness is tested, but prefix assignment logic (R vs C vs LED vs U) is only tested via exact count matching. A bug that assigns "R" to capacitors but still produces the right count would pass.
- **P2**: **No test that exported KiCad files are actually valid** -- tests check for `kicad_sch` tag but KiCad itself might reject the output
- **P2**: **No test for empty designs** or designs with a single component
- **P2**: **No test for footprint path resolution** -- do exported footprint references actually point to existing files?
- **P3**: **No test for schematic wire generation** (mentioned as remaining work)

### ato-parts (inline: ~26 tests across 6 source files)
**What exists**: Part data structures, LCSC query building, cache operations, database lookups.
**What's missing**:
- **P1**: **No integration test for passive part picking** -- the solver narrows a resistance to [9000, 11000] ohm, does the parts query actually find a matching component from JLCPCB?
- **P1**: **No test for part selection with multiple constraints** -- `resistance = 10kohm +/- 5%` AND `package = "0402"` simultaneously
- **P2**: **No test for network failure handling** -- what happens when JLCPCB API is unreachable?
- **P2**: **No test for cache invalidation** -- stale cache returning wrong parts
- **P3**: **No test for BOM deduplication** -- do identical parts get grouped correctly?

### ato-cli (inline: ~34 tests across 6 source files)
**What exists**: Build command config parsing, parse command, parts command, package operations, check command, error formatting.
**What's missing**:
- **P1**: **No test for `ato build` producing correct output files** from ato.yaml configuration
- **P1**: **No test for `ato install`** actually downloading, extracting, and symlinking packages
- **P2**: **No test for CLI argument parsing edge cases** (missing args, conflicting flags, etc.)
- **P2**: **No test for multi-target builds** -- ato.yaml with multiple build targets
- **P3**: **No test for error display formatting** -- do users see helpful messages?

---

## 2. Types of Testing Missing

### Unit Tests
**Status**: Good coverage across all crates. Every crate has inline `#[cfg(test)]` modules.
**Gap**: Tests are often shallow -- they verify "doesn't panic" or "is_ok()" rather than checking specific output values.

### Integration Tests
**Status**: The `ato-tests` crate provides integration tests across parse/sema/solver/export.
**Critical gap**: The led_badge tests are the only true end-to-end integration tests, and they require pre-installed packages (`ato install` must have been run). There are no self-contained E2E tests that go from .ato source to valid KiCad output.

### Property-Based Tests
**Status**: **None exist.**
**Recommended**:
- Parser: random token sequences should never panic
- Solver: narrowing should be monotonic (running solve() again doesn't widen)
- Domain: interval arithmetic should satisfy algebraic properties

### Fuzz Testing
**Status**: **None exist.**
**Critical for**: The parser and lexer. Arbitrary `.ato` input should never cause a panic or infinite loop. This is especially important since users edit `.ato` files and the LSP will be parsing incomplete/malformed files.

### Regression Tests
**Status**: The led_badge exact count test (`test_led_badge_bom_exact_counts`) serves as a regression test. The negative tests document known bugs.
**Gap**: No regression tests for previously fixed bugs (e.g., "inline signal/pin definitions in connections", "imported module connections silently dropped", "constant_fold infinite re-folding").

### Snapshot Tests
**Status**: **None exist.**
**Recommended**: Snapshot the netlist output, BOM CSV, and KiCad files for a few reference designs. Use `insta` crate for automatic snapshot management.

### Performance Tests
**Status**: **None exist.**
**Recommended**: Benchmark parse time, sema time, and solve time for the led_badge project. Track regressions.

### Error Path Tests
**Status**: The negative_tests.rs covers 29 error cases, but 11 are `#[ignore]` documenting bugs.
**Gap**: No test verifies that error **messages** are helpful (correct line numbers, clear descriptions).

### Concurrency Tests
**Status**: **None exist.**
**Recommended**: The `ato install` command uses rayon for parallel downloads. No test verifies correct behavior with concurrent filesystem operations, symlink creation races, or partial download recovery.

---

## 3. Language Feature Test Coverage

| Feature | Parse Test | Sema Test | Solver Test | E2E Test |
|---------|-----------|-----------|-------------|----------|
| `import` / `from ... import` | Yes (5 tests) | Partial (fixture) | N/A | Yes (led_badge) |
| `module` definition | Yes | Yes | N/A | Yes |
| `component` definition | Yes | Yes | N/A | Yes (led_badge) |
| `interface` definition | Yes | Yes | N/A | Yes |
| `module from Base` (inheritance) | Yes (parse) | Partial | N/A | Yes (led_badge) |
| `pin` declaration (name/number/string) | Yes (3 tests) | Yes | N/A | Yes |
| `signal` declaration | Yes | Yes | N/A | Yes |
| `new` instantiation | Yes | Yes | N/A | Yes |
| `new Type[N]` (arrays) | Yes | Partial | N/A | Yes (led_badge) |
| `new Type<param=val>` (templates) | Yes (4 tests) | **No** | N/A | Partial |
| `~` connections | Yes | Yes | N/A | Yes |
| `~>` / `<~` directed connections | Yes | Partial | N/A | Yes (led_badge) |
| `->` retype | Yes (parse) | **No** | N/A | **No** |
| `assert x > val` | Yes | Yes | Yes (literal) | Yes |
| `assert x < val` | Yes | Yes | Yes | Yes |
| `assert x >= val` | Yes | Yes | Yes | Yes |
| `assert x <= val` | Yes | Yes | Yes | Yes |
| `assert x within val +/- tol` | Yes | Partial | Yes | Yes |
| `assert x is val` | Yes | Partial | Yes | Yes |
| `assert low < x < high` (chained) | Yes (parse) | **No** | **No** | **No** |
| `for item in array` | Yes | Partial | N/A | Yes (led_badge) |
| `for item in array[0:N]` (slice) | Yes (parse) | **No** | N/A | **No** |
| `for ref in [a, b, c]` (list literal) | Yes (parse) | **No** | N/A | **No** |
| `trait name` | Yes | Yes | N/A | Yes (led_badge) |
| `trait name::constructor` | Yes (parse) | Partial | N/A | Partial |
| `trait name<param=val>` | Yes (parse) | Partial | N/A | Yes |
| Arithmetic expressions | Yes (5 tests) | Partial | Partial | **No standalone** |
| Physical quantities (V, ohm, F, A) | Yes | Yes | Yes | Yes |
| Bilateral tolerance (`val +/- pct%`) | Yes | Partial | Yes | Yes |
| Bilateral tolerance (`val +/- abs`) | Yes (parse) | **No** | **No** | **No** |
| Bound quantity (`low to high`) | Yes | Partial | Yes | Yes |
| String assignments | Yes | Partial | N/A | Yes |
| Boolean assignments | Yes | Partial | N/A | Partial |
| Cumulative assignments (`+=`, `-=`) | Yes (parse) | **No** | N/A | **No** |
| Set assignments (`\|=`, `&=`) | Yes (parse) | **No** | N/A | **No** |
| Array indexing (`arr[5]`) | Yes (parse) | Partial | N/A | Yes (led_badge) |
| Array slicing (`arr[0:5]`) | Yes (parse) | **No** | N/A | **No** |
| Pragma handling | Yes | Partial | N/A | Yes |
| Semicolons (compound statements) | Yes (parse only) | **No** | N/A | **No** |
| `field: Type` (declaration with type) | Yes | Yes | N/A | Yes |
| Pin reference end (`.1`, `.GND`) | Yes (parse) | Partial | N/A | Yes |
| Hex/binary/octal literals | Yes (parse) | **No** | **No** | **No** |

---

## 4. Specific High-Risk Gaps

### 4.1 Net Connectivity Correctness (CRITICAL)
**Risk**: A bug in connection lowering or net merging could cause pins to be on wrong nets, creating electrical shorts or open circuits.
**Current state**: No test verifies that `a ~ b` actually puts `a` and `b` on the same net. Led_badge tests check component counts and designators but never check net membership.
**Recommended test**: Parse a known circuit, build the netlist, and assert specific pins are on specific nets.

### 4.2 ato.yaml Build Configuration (HIGH)
**Risk**: The `ato build` command reads `ato.yaml` for entry point, paths, and build targets. No test verifies this end-to-end.
**Current state**: CLI build tests exist inline in `build.rs` but they test config parsing, not actual file I/O.
**Recommended test**: Create a temp directory with ato.yaml + .ato file, run the build pipeline, verify output files exist and are non-empty.

### 4.3 Package Installation (HIGH)
**Risk**: `ato install` downloads packages, extracts them, and creates symlinks. A regression here breaks every project.
**Current state**: Package system is tested via inline tests in `packages.rs` (3 tests) but they mock the network. No test verifies actual download/extract/symlink.
**Recommended test**: Integration test that installs a small known package and verifies the directory structure.

### 4.4 Constraint Solver + Real Designs (HIGH)
**Risk**: The solver works on manually constructed predicates in unit tests. The pipeline from `assert x within 10kohm +/- 5%` (parsed .ato) to solver predicates to narrowed domain is only tested indirectly via led_badge.
**Current state**: `led_badge_tests::test_led_badge_solver_no_contradiction` runs the solver but accepts timeout as success. No test verifies that the solver actually narrows parameters correctly for real designs.
**Recommended test**: Small self-contained .ato file with known constraint results, verify exact narrowed values.

### 4.5 Designator Uniqueness Under Retype (MEDIUM)
**Risk**: The `->` retype operator can change what type an instance resolves to, which affects designator prefix assignment. Two buttons are known to be missing due to retype issues.
**Current state**: No test for retype effect on designators.

### 4.6 Export Format Validity (MEDIUM)
**Risk**: Exported KiCad files might have structural issues that cause KiCad to reject them.
**Current state**: Tests check for tag presence (`kicad_sch`, `kicad_pcb`) but not structural validity.
**Recommended test**: At minimum, parse the exported S-expression to verify balanced parens and required sections.

---

## 5. Test Infrastructure Issues

### 5.1 led_badge Tests Require Pre-installed Packages
The 43 led_badge tests fail if `ato install` hasn't been run in `examples/led_badge/`. This makes CI fragile and complicates first-time contributor experience.
**Fix**: Either run `ato install` as a test setup step, or create self-contained integration fixtures that don't need external packages.

### 5.2 Comparison Tests Require Python CLI
The 8 comparison tests are all `#[ignore]` or skip-if-not-available. They're valuable but never run in normal CI.
**Fix**: Run them in a CI job that has both CLIs available.

### 5.3 External Tests Require Cloned Repos
The 12 external tests skip if repos aren't cloned. No CI setup clones them.
**Fix**: Add a CI job that clones test repos.

### 5.4 No Test Output Verification
Most tests use `assert!(result.is_ok())` without checking the actual output values. This means a test can pass even if the output is completely wrong, as long as it doesn't return an error.

### 5.5 Global State from AtomicU64 Counters
Expression/Predicate IDs use global counters. Tests that check specific ID values are order-dependent and fragile.

---

## 6. Recommended Priority Actions

### P0 (Do First -- Blocks Ship)
1. **Add net connectivity test**: Verify specific pins land on specific nets for a known circuit
2. **Add self-contained E2E test**: .ato source -> netlist without requiring pre-installed packages
3. **Add sema-to-solver pipeline test**: Parse `assert x within 10kohm +/- 5%`, verify solver narrows correctly

### P1 (Do Soon -- High Bug Risk)
4. Add retype (`->`) sema test verifying instance type changes
5. Add interface type checking test (even if it documents the current broken behavior)
6. Add circular inheritance detection test
7. Add `ato build` test from ato.yaml configuration
8. Add passive part picking integration test

### P2 (Medium Priority -- Catches Edge Cases)
9. Add parser fuzz test (arbitrary input should never panic)
10. Add snapshot tests for netlist/BOM output using `insta`
11. Add tests for cumulative/set assignments in sema
12. Add tests for chained comparisons (`3V < x < 5V`) through full pipeline
13. Add tests for embedded stdlib path resolution
14. Add tests for for-loop with slicing and list literals in sema

### P3 (Nice to Have)
15. Property-based tests for interval arithmetic and solver invariants
16. Performance benchmarks for parse/sema/solve pipeline
17. Error message quality tests (line numbers, descriptions)
18. Concurrency tests for parallel package installation

---

## 7. Test Count Summary

| Test File | Test Count | Ignored | Notes |
|-----------|-----------|---------|-------|
| `syntax_coverage_tests.rs` | 83 | 0 | 1 known failure (`in` keyword) |
| `led_badge_tests.rs` | 43 | 0 | Requires installed packages |
| `solver_tests.rs` | 34 | 0 | Unit-level solver tests |
| `parse_tests.rs` | 33 | 0 | Fixture + inline parse tests |
| `negative_tests.rs` | 29 | 11 | 11 ignored = documented bugs |
| `sema_tests.rs` | 24 | 0 | Fixture + inline sema tests |
| `examples_tests.rs` | 22 | 0 | Example project parse tests |
| `integration_tests.rs` | 19 | 0 | Full pipeline inline tests |
| `external_tests.rs` | 12 | 0 | External repo parse tests |
| `comparison_tests.rs` | 8 | 2 | 2 require Python CLI |
| Inline (ato-sema) | ~84 | 0 | Across 9 source files |
| Inline (ato-export) | ~51 | 0 | Across 7 source files |
| Inline (ato-solver) | ~47 | 0 | Across 6 source files |
| Inline (ato-domain) | ~42 | 0 | Across 5 source files |
| Inline (ato-ir) | ~35 | 0 | Across 6 source files |
| Inline (ato-cli) | ~34 | 0 | Across 6 source files |
| Inline (ato-parts) | ~26 | 0 | Across 6 source files |
| Inline (ato-lexer) | ~11 | 0 | 1 source file |
| Inline (ato-parser) | ~70 | 0 | Across 5 source files |
| Lexer integration | 6 | 0 | Real file lexing |
| Parser integration | 13 | 0 | Real file parsing |
| **TOTAL** | **~756** | **13** | |

---

## 8. Conclusion

The test suite is **broad but shallow**. It covers most language features at the parse level and has good solver unit tests, but the critical gap is **lack of output correctness verification**. Tests generally assert "it didn't crash" rather than "it produced the right answer." The three most impactful improvements would be:

1. **Net connectivity tests** -- verify actual electrical connections
2. **Self-contained E2E tests** -- no external dependencies
3. **Solver integration tests** -- from .ato source to narrowed parameter values

These three additions would dramatically increase confidence that the Rust implementation produces correct results, not just that it runs without errors.
