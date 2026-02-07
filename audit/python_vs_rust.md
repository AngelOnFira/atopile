# Python vs Rust Atopile Implementation Audit

**Date:** 2026-02-07
**Python version:** 0.12.4 (installed via uv at `/Users/forest/.local/bin/ato`)
**Rust branch:** `rust-test` (21 commits)
**Python source:** `/Users/forest/.local/share/uv/tools/atopile/lib/python3.13/site-packages/atopile/`

---

## 1. Architecture Overview

### Python Architecture
The Python implementation is built on top of the **faebryk** hardware description framework. Key components:

- **Parser:** ANTLR4-based (G4 grammar -> Python visitor). Located in `parser/AtoParser.py`, `parser/AtoLexer.py`.
- **Frontend (semantic analysis):** `front_end.py` (~3100 lines) - A single `Bob` class (ANTLR visitor) that walks the AST, builds faebryk graph nodes, resolves imports/types, handles connections/assertions/traits.
- **Solver:** Delegates to faebryk's `DefaultSolver` (SAT-based constraint solver using `faebryk.core.solver.solver`).
- **Part Picking:** faebryk's `pick_part_recursively()` which queries JLCPCB API.
- **Export:** faebryk's exporters for KiCad netlist, PCB, BOM, schematic, 3D models.
- **Config:** Pydantic-based `config.py` (~1270 lines) with YAML settings, env vars, build targets.
- **CLI:** Typer-based `cli/cli.py` with subcommands (build, inspect, view, validate, create, dependencies).
- **LSP:** `lsp/lsp_server.py` using pygls (Python Language Server Protocol framework).
- **MCP:** `mcp/mcp_server.py` for IDE integration.

### Rust Architecture
10 crates in `crates/`:

- **ato-lexer:** Custom hand-written lexer with indentation tracking.
- **ato-parser:** Custom recursive descent parser producing typed AST.
- **ato-domain:** Core types (Unit, QuantityIntervalDisjoint, DisjointIntervals).
- **ato-ir:** Intermediate representation (Design, Module, Field, Connection).
- **ato-sema:** Semantic analyzer with name resolution, type checking, import handling.
- **ato-solver:** Fixed-point iteration constraint solver (canonical, structural, constant_fold passes).
- **ato-export:** KiCad netlist, schematic, PCB, BOM, project file generators.
- **ato-parts:** LCSC/JLCPCB API client with SQLite cache.
- **ato-cli:** clap-based CLI (parse, check, build, parts, add, install, update, remove, list).
- **ato-tests:** Integration tests (syntax coverage, led_badge, negative, comparison).

---

## 2. Build Pipeline Comparison

### 2.1 Parsing

| Feature | Python | Rust | Notes |
|---------|--------|------|-------|
| Grammar | ANTLR4 G4 | Hand-written recursive descent | Same G4 grammar spec, different impl |
| Indentation | ANTLR4 lexer base class | Custom indentation tracker in ato-lexer | Both handle Python-style indentation |
| Pragma handling | `_parse_pragma()` regex in front_end.py | Parsed as AST nodes | Both support `#pragma experiment("X")` |
| Error recovery | ANTLR4 default error recovery | Limited - first error stops | Python handles more gracefully |
| `in` keyword | Works as identifier | **KNOWN FAILURE** - `test_gap_in_as_identifier` fails | Python's ANTLR grammar doesn't have this conflict |
| Semicolons | Full support for compound statements | Supported | Both handle `pass; pass` etc. |
| Python builds (.py entry) | Supported via `import_from_path` | **NOT SUPPORTED** | Rust only handles `.ato` files |

### 2.2 Semantic Analysis

| Feature | Python | Rust | Notes |
|---------|--------|------|-------|
| Import resolution | Two-phase: Wendy surveys, Bob resolves lazily | Two-pass: forward ref resolution + full analysis | Both search same paths |
| `from "file" import X` | Full support, .ato and .py files | .ato only | Rust can't import Python modules |
| `import X` (stdlib) | Looks up in `faebryk.library._F` | Embedded stdlib via `embedded_stdlib.rs` (51 files) | Different stdlib implementations |
| Inheritance (`from` clause) | Dynamic Python class creation hierarchy | Copies fields + traits from base, sets super_type | Structurally equivalent |
| Retype (`->`) | Full graph mutation: disconnect_parent, specialize, slot in | **PARTIALLY BROKEN** - 2 buttons missing in led_badge | Python uses faebryk's graph mutation; Rust may not update resolved_type |
| For loops | Runtime dict loop variable injection, nested loop detection | Supported but simpler | Both require `#pragma experiment("FOR_LOOP")` |
| Trait system | Full faebryk trait system (Trait/TraitImpl, named constructors) | Behavioral support with trait parameters | Both require `#pragma experiment("TRAITS")` |
| Template arguments | `new X<param=val>` -> kwargs passed to Python `__init__` | Supported | Both require `#pragma experiment("MODULE_TEMPLATING")` |
| Connection duck-typing | Fallback: connect by name when types don't match | Strict type matching only | Python allows more flexible connections |
| Signal definitions | Creates `F.Electrical()` instance | Supported | Same semantics |
| Pin declarations | `_has_ato_cmp_attrs.add_pin()` through shim system | Direct field creation | Python has more shim/legacy handling |
| Module array `new X[N]` | Creates list, each element independently initialized | Supported | Same semantics |
| Forward declarations | Partial (parameter forward refs not fully supported) | Two-pass resolution | |
| Nested for loops | **Blocked with error** | Not tested | Python explicitly checks `_in_for_loop` flag |
| `component` keyword | Treated same as `module` (code-as-data) | Supported | Both equivalent |

### 2.3 Constraint Solving

| Feature | Python | Rust | Notes |
|---------|--------|------|-------|
| Solver type | faebryk's DefaultSolver (SAT-based) | Fixed-point iteration | Fundamentally different approaches |
| Constraint types | Is, IsSubset, LessOrEqual, GreaterOrEqual, Min, Max | Equality, subset, less/greater, within | Similar constraint vocabulary |
| Toleranced values | Range(center, tolerance) with pint units | QuantityIntervalDisjoint with Unit | Both handle `10kohm +/- 10%` |
| Dimensional analysis | Full pint unit compatibility checking | Unit stored but limited dim analysis | Python uses `pint` library |
| `assert x within y` | IsSubset constraint | Subset predicate | Same semantics |
| `assert x is y` | Is constraint (equality/alias) | Equality predicate | Same semantics |
| `assert x > y` | **Downgraded warning**, treated as `>=` | Supported | Python warns `>` is not supported, use `>=` |
| `assert x < y` | **Downgraded warning**, treated as `<=` | Supported | Python warns `<` is not supported, use `<=` |
| Parameter merging | Complex `_merge_parameter_assignments()` with gospel/subset logic | Predicate-based narrowing | Python has sophisticated multi-assignment handling |
| `+=` / `-=` operators | `alias_is(value)` / `alias_is(-value)` (deprecated) | Supported | Both deprecated |
| `\|=` / `&=` operators | `constrain_superset` / `constrain_subset` (deprecated) | Supported | Both deprecated |
| Bus parameter resolution | `F.is_bus_parameter.resolve_bus_parameters()` | **NOT IMPLEMENTED** | Python resolves bus parameters before solving |
| Arithmetic functions | `min()`, `max()` | Not implemented | Python supports `min(a, b)` and `max(a, b)` in expressions |

### 2.4 Part Picking

| Feature | Python | Rust | Notes |
|---------|--------|------|-------|
| Passive picking | Full integration with faebryk solver and JLCPCB API | Basic: queries LCSC API with solved parameter ranges | Rust works for simple cases |
| Non-passive picking | Via `is_pickable_by_supplier_id`, `is_pickable_by_part_number` traits | Via `lcsc` field in solved_params (from traits) | Both extract LCSC IDs from design |
| Part database | faebryk's picker infrastructure | `ato-parts` crate: LcscClient + SQLite cache | Both query JLCPCB |
| Offline support | Depends on faebryk's caching | SQLite PartCache | Both have caching |
| Part scoring | faebryk's picker scoring | BasicPartSelector (stock > class > price) | Similar heuristics |
| Package constraint | From solved parameters | `instance.package` lookup | Both check package |
| `lcsc = "C12345"` | Trait-based pickup | Field value lookup | Different mechanisms |
| `package = "0402"` | Solver-resolved parameter | Solved value lookup | Same effect |

### 2.5 Export / Output Generation

| Feature | Python | Rust | Notes |
|---------|--------|------|-------|
| Netlist format | faebryk's `faebryk_netlist_to_kicad()` | Custom `KicadNetlistExporter` | Both produce KiCad .net files |
| Net naming | `attach_net_names()` with override support | Automatic naming from connection graph | Python has `keep_net_names` option |
| Designators | `attach_random_designators()` + optional `load_designators()` | Sequential assignment (R1, R2, ...) | Python has `keep_designators` option |
| BOM | `write_bom_jlcpcb()` | `Bom::from_netlist_grouped()` CSV export | Both JLCPCB format |
| Schematic | Through faebryk/KiCad toolchain | `KicadSchematic::from_netlist()` - component placement only | **Rust has no wiring/routing** |
| PCB | Full KiCad PCB with footprints, layout preservation | `KicadPcb::from_netlist()` - footprints placed in grid | **Rust generates from scratch, no layout preservation** |
| PCB layout preservation | Full: load existing .kicad_pcb, apply_design, sync groups | **NOT IMPLEMENTED** | Critical gap for iterative design |
| KiCad project | Generated by faebryk | `KicadProject::new_with_libraries()` | Both generate .kicad_pro |
| DXF export | Via kicad-cli wrapper | **NOT IMPLEMENTED** | |
| Gerber export | Via kicad-cli wrapper | **NOT IMPLEMENTED** | |
| Pick & Place | Via kicad-cli wrapper, JLCPCB format conversion | **NOT IMPLEMENTED** | |
| 3D model export | GLB + STEP via kicad-cli | **NOT IMPLEMENTED** | |
| SVG render | Via kicad-cli | **NOT IMPLEMENTED** | |
| Variable report | `export_parameters_to_file()` | **NOT IMPLEMENTED** | |
| I2C tree diagram | `export_i2c_tree()` | **NOT IMPLEMENTED** | |
| Manifest | JSON manifest with layout paths | **NOT IMPLEMENTED** | |
| Layout sync | `LayoutSync` with group-based placement | **NOT IMPLEMENTED** | |
| DRC checking | Via kicad-cli wrapper | **NOT IMPLEMENTED** | |
| Frozen builds | Full PCB diff comparison | **NOT IMPLEMENTED** | |
| `override_net_name` | Via `load_net_names()` | **NOT IMPLEMENTED** | |

---

## 3. Config and Project Management

| Feature | Python | Rust | Notes |
|---------|--------|------|-------|
| ato.yaml parsing | Pydantic-based `ProjectSettings` with YAML + env vars | Simple serde YAML parsing via `AtoConfig` | Python much more sophisticated |
| Build targets | Multiple named targets with options per build | Single target at a time | Python supports building multiple targets |
| Dependency types | Registry, Git, File (with specs) | Registry only (via packages API) | Python supports git:// and file:// deps |
| Lock file | `ato.lock` via faebryk ProjectDependencies | **No lock file** | Rust doesn't track exact versions |
| Version requirements | Pydantic-validated semver spec | Not validated | Python checks `requires-atopile` |
| `ato create` | Project/build target/component creation | **NOT IMPLEMENTED** | |
| `ato inspect` | Component connection inspection | **NOT IMPLEMENTED** | |
| `ato view` | Block diagram / schematic viewing | **NOT IMPLEMENTED** | |
| `ato validate` | Syntax and consistency checking | `ato check` (similar) | |
| `ato sync` / `ato add` / `ato remove` | Full dependency management | `ato add`, `ato install`, `ato update`, `ato remove` | Both have dep management |
| Standalone builds | `--standalone` flag for single-file builds | Direct file path argument | Different UX, same concept |
| Open layout on build | `open_layout_on_build` config, KiCad IPC | **NOT IMPLEMENTED** | |

---

## 4. Error Reporting

| Feature | Python | Rust | Notes |
|---------|--------|------|-------|
| Error types | Rich exception hierarchy: UserException, UserKeyError, UserTypeError, etc. | SemaError enum with variants | Python has ~20 specific error types |
| Error accumulation | `accumulate()` context manager - collects all errors before reporting | Fails on first error | Python reports ALL errors at once |
| Error tracebacks | Full ato traceback stack (like Python tracebacks) | Span-based error locations | Python shows call chain through modules |
| Downgrade warnings | `downgrade()` context manager - turn errors into warnings | Not implemented | Python can turn errors into warnings for deprecated features |
| CLI error display | Rich terminal formatting with file snippets | miette-style error display with source code | Both show source context |
| Deprecation warnings | Extensive deprecation system for old syntax | Not implemented | Python gradually migrates away from old patterns |

### LSP Support

| Feature | Python | Rust |
|---------|--------|------|
| LSP server | Full pygls-based server | **NOT IMPLEMENTED** |
| Go to definition | Supported via `from_dsl` trait | N/A |
| Find references | Supported via reference tracking | N/A |
| Hover information | Full node/type info display | N/A |
| Diagnostics | Real-time syntax + semantic errors | N/A |
| Incremental parsing | Not yet (full re-parse) | N/A |
| Watch mode | Not implemented | Not implemented |

---

## 5. Output Comparison (led_badge)

### 5.1 File Structure

Python outputs to `build/builds/default/default/`:
- **Note:** Python CLI failed to build due to a dependency resolution error (PermissionError in `resolve_dependencies`), so no fresh Python output was available for comparison.

Rust outputs to `build/`:
- `led_badge.net` (109KB) - KiCad netlist
- `led_badge.kicad_sch` (124KB) - KiCad schematic
- `led_badge.kicad_pcb` (236KB) - KiCad PCB layout
- `led_badge.kicad_pro` (1.9KB) - KiCad project
- `led_badge_bom.csv` (2.1KB) - JLCPCB BOM

### 5.2 Rust Output Characteristics

Based on the Rust build output (238 components, 142 nets):
- Components have designators (R1-R100, C1-C50, U1-U20, etc.)
- Non-passive components get footprints from LCSC part data
- Passive components get generic footprints (not from part picking)
- Schematic has components placed on grid but **no wires** connecting them
- PCB has footprints placed on grid but **no routing** or layout preservation
- BOM has LCSC part numbers for non-passive components

### 5.3 Known Differences

1. **Net naming:** Rust uses auto-generated net names; Python preserves user-specified names
2. **Designator assignment:** Rust assigns sequentially; Python can preserve existing designators
3. **PCB layout:** Rust generates fresh grid layout; Python preserves existing placement
4. **Missing components:** Rust missing 2 buttons due to retype (`->`) issues
5. **Passive parts:** Rust doesn't pick passives from JLCPCB (solver doesn't fully resolve params)
6. **Schematic wires:** Rust generates component-only schematic; Python generates full schematic via faebryk

---

## 6. Critical Gaps Summary

### P0 - Blockers for Production Use

1. **PCB layout preservation** - Without this, every build destroys manual routing/placement work
2. **Passive part picking** - Solver needs to fully resolve parameter constraints to query JLCPCB
3. **Retype (`->`) incomplete** - Missing components (2 buttons in led_badge)
4. **Error accumulation** - Failing on first error is bad UX for complex projects
5. **No Python build support** - Can't build `.py` entry points

### P1 - Important for Feature Parity

6. **LSP server** - Required for IDE extension integration
7. **Bus parameter resolution** - Required for complex bus interfaces
8. **Net name preservation** - Required for stable PCB updates
9. **Designator preservation** - Required for stable PCB updates
10. **KiCad IPC** - Required for automatic PCB reload after build
11. **Schematic wiring** - Currently component-only schematics
12. **`in` keyword conflict** - Breaks `for x in container` and other uses of `in`
13. **Min/max functions** - `min(a, b)` and `max(a, b)` in expressions

### P2 - Nice to Have

14. **Lock file** - Version pinning for reproducible builds
15. **Multiple build targets** - Building all targets in one invocation
16. **Manufacturing artifacts** - Gerber, DXF, pick & place export
17. **3D model export** - GLB, STEP
18. **Frozen builds** - PCB change detection for CI
19. **Variable report** - Parameter value documentation
20. **I2C tree diagram** - Bus topology visualization
21. **Deprecation warnings** - Gradual migration from old syntax
22. **Connection duck-typing** - Flexible type matching for connections
23. **`ato create`** - Project/component scaffolding
24. **`ato inspect`** - Component connection inspection
25. **`ato view`** - Block diagram viewing

### P3 - Future

26. **WASM build** - For browser-based tools
27. **Dimensional analysis** - Full unit compatibility checking
28. **Watch mode** - Automatic rebuild on file changes
29. **Git/File dependencies** - Only registry deps supported

---

## 7. Architectural Differences of Note

### 7.1 Faebryk Dependency
The Python implementation is **deeply integrated** with faebryk, which provides:
- A graph-based data model (`L.Node`, `L.Module`, `L.ModuleInterface`)
- The constraint solver
- The KiCad PCB file format parser/writer
- The part picker infrastructure
- Layout management (LayoutSync, apply_layouts, apply_routing)
- Design rule checking (DRC)
- Numerous exporters

The Rust implementation is **standalone** and implements everything from scratch. This gives it:
- Much faster compilation and startup
- No Python dependency at runtime
- Full control over the implementation
- But also means reimplementing a LOT of functionality

### 7.2 Solver Architecture
Python uses a **SAT-based** solver (via faebryk) that can reason about complex constraint relationships. The Rust solver uses **fixed-point iteration** with simplification passes (canonical, structural, constant_fold). The Rust approach is:
- Faster for simple constraints
- Less powerful for complex constraint networks
- May not converge for some constraint patterns that faebryk handles

### 7.3 Build Pipeline
Python has a **target system** (`Muster`) that registers build steps as a DAG and executes them in dependency order. The Rust build pipeline is a **linear sequence** of steps in `build.rs`.

### 7.4 Error Handling Philosophy
Python **accumulates all errors** and reports them at the end, allowing users to fix multiple issues at once. Rust **fails fast** on the first error. This is a significant UX difference for complex projects.

---

## 8. Lines of Code Comparison

| Component | Python (lines) | Rust (lines) | Notes |
|-----------|---------------|-------------|-------|
| Frontend/Sema | ~3,100 (front_end.py) | ~2,500 (analyzer.rs + resolution.rs) | Python includes more edge cases |
| Config | ~1,270 (config.py) | ~200 (AtoConfig in sema) | Python much more sophisticated |
| Build pipeline | ~780 (build_steps.py + buildutil.py) | ~1,100 (build.rs) | Rust has part picking inline |
| Solver | faebryk (external) | ~3,730 (ato-solver) | Different implementations |
| Export | faebryk (external) | ~5,430 (ato-export) | Rust reimplements everything |
| Parts | faebryk (external) | ~1,800+ (ato-parts) | Rust reimplements from scratch |
| Parser | ~2,000 (ANTLR generated) | ~3,000 (ato-lexer + ato-parser) | Hand-written vs generated |
| CLI | ~400 (cli/) | ~500 (ato-cli) | Similar scope |
| LSP | ~800 (lsp/) | 0 | Not implemented in Rust |
| **Total** | ~8,350 (atopile) + faebryk | ~18,260+ (all Rust crates) | Rust has more but less functionality |

The Python implementation leverages faebryk heavily (estimated 50,000+ lines of code in the faebryk library for solver, exporters, PCB handling, etc.), while Rust reimplements everything from scratch.
