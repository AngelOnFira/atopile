# Atopile Rust Rewrite Plan

Full rewrite of atopile in Rust - standalone compiler with **no Python dependencies**.

## Current Progress

| Crate | Status | Description |
|-------|--------|-------------|
| `ato-lexer` | ✅ Complete | Tokenization with INDENT/DEDENT |
| `ato-parser` | ✅ Complete | AST generation (chumsky, error recovery) |
| `ato-domain` | ✅ Complete | Quantity/interval/set types |
| `ato-solver` | ✅ Complete | Constraint solver with simplification |
| `ato-ir` | ✅ Complete | Intermediate representation |
| `ato-sema` | ✅ Complete | Semantic analysis + import resolution |
| `ato-cli` | ✅ Complete | Main compiler CLI with solver integration |
| `ato-tests` | ✅ Complete | E2E test suite (227 tests) |
| `ato-parts` | ✅ Complete | Part database interface |
| `ato-export` | ✅ Complete | Netlist, BOM, KiCad project output |

---

## Feature Parity Status

**Completed**: Parsing, semantic analysis, import resolution, constraint solving, part queries, netlist export, BOM generation, KiCad project files

**Core compiler: 100% complete**

---

## Completed Phases (Summary)

### Phase 0-4: Core Compiler ✅
- **Lexer**: logos-based tokenization with Python-style INDENT/DEDENT
- **Parser**: chumsky with error recovery, full AST
- **Domain**: Quantities, intervals, tolerances, set operations
- **Solver**: Constraint expressions, predicates, simplification, contradiction detection
- **IR**: Module/Field/Connection graph representation
- **Sema**: Name resolution, type checking, AST→IR lowering
- **CLI**: `ato build/check/parse` commands with miette error formatting
- **Tests**: 227 tests covering examples, stdlib, external repos

### Phase 5: Import Resolution ✅
- **resolution.rs**: File path resolution (relative/absolute)
- **Package resolution**: Standard library + ato.yaml dependencies
- **Module registry**: Tracks exports, detects circular imports
- **Symbol merging**: Imported modules usable in scope, cross-file inheritance

### Phase 6: Solver Integration ✅
- **constraint_collector.rs**: Extracts constraints from IR Design
- **Parameter graph**: Tracks dependencies between parameters
- **Solver execution**: Runs in `ato build`, reports results/contradictions
- **CLI integration**: Nice error messages for unsatisfiable constraints

### Phase 7: Part Selection ✅
- **ato-parts crate**: Part/PartId/Manufacturer types, PartDatabase trait
- **Query builders**: ResistorQuery, CapacitorQuery with parameter constraints
- **Selection**: BasicPartSelector with scoring strategies

### Phase 8: Output Generation ✅
- **Netlist**: NetlistBuilder from IR Design, KiCad S-expression format
- **BOM**: JLCPCB CSV format, generic CSV format
- **KiCad Project**: .kicad_sch, .kicad_pcb, .kicad_pro generation
- **Component grouping**: Identical parts grouped with quantities
- **Designator sorting**: Proper numeric ordering (R1, R2, R10)

---

## Remaining Work - Ralph Prompts

The following prompts extend the compiler toward production readiness.

---

### Prompt 23: LCSC Part Database Integration

Connect ato-parts to real component databases for automatic part selection.

#### Current State
- ato-parts has `PartDatabase` trait and query builders
- No actual database connection implemented
- Part selection is stubbed

#### Requirements

1. **LCSC API Client**
   - HTTP client for LCSC component search
   - Parse LCSC JSON responses into Part structs
   - Handle pagination and rate limiting

2. **Local Cache**
   - SQLite cache for downloaded part data
   - Invalidation strategy (TTL or manual refresh)
   - Offline-first operation

3. **Query Execution**
   - Map ResistorQuery/CapacitorQuery to LCSC search params
   - Filter results by constraints (tolerance, package, stock)
   - Return ranked candidates

4. **CLI Integration**
   - `ato parts search "10kohm 0402"` command
   - Show matched parts during build
   - Warning when no parts match constraints

#### Key Files
- crates/ato-parts/src/database.rs - PartDatabase trait
- crates/ato-parts/src/lcsc.rs - New LCSC client
- crates/ato-parts/src/cache.rs - New SQLite cache

#### Tests
- Mock HTTP responses for unit tests
- Integration test with real LCSC (optional, skip in CI)

#### Completion Promise
```
<promise>LCSC_INTEGRATION_COMPLETE</promise>
```
When:
- LCSC API client fetches real parts
- Local cache stores/retrieves parts
- Queries return matching components
- CLI shows part selection results
- Tests pass and changes committed

---

### Prompt 24: Complete Standard Library

Create full .ato stdlib with all common electronics modules.

#### Current State
- 14 stub files in crates/ato-sema/stdlib/
- Only interface definitions, no real implementations
- Missing many common components

#### Requirements

1. **Passive Components**
   - Resistor, Capacitor, Inductor (complete with all parameters)
   - Diode, Zener, LED, TVS
   - Ferrite bead, fuse, PTC

2. **Interfaces**
   - Electrical, ElectricPower, ElectricSignal, ElectricLogic
   - I2C, SPI, UART, I2S, CAN, USB
   - JTAG, SWD, GPIO

3. **Common Modules**
   - Voltage regulators (LDO, Buck, Boost patterns)
   - Op-amp configurations
   - Crystal oscillator
   - Connector types

4. **Traits**
   - has_designator_prefix
   - can_bridge
   - has_footprint
   - needs_decoupling

#### Key Files
- crates/ato-sema/stdlib/*.ato - All stdlib modules

#### Reference
- src/faebryk/library/*.py - Python stdlib equivalents

#### Completion Promise
```
<promise>STDLIB_COMPLETE</promise>
```
When:
- All common passives defined with full parameters
- All standard interfaces implemented
- Common module patterns available
- Tests verify stdlib loads correctly
- Committed and pushed

---

### Prompt 25: Package Manager & Dependencies

Implement ato.yaml parsing and package dependency resolution.

#### Current State
- Basic path resolution exists
- No ato.yaml parsing
- No package downloading/caching

#### Requirements

1. **ato.yaml Parser**
   - Parse project configuration
   - Extract dependencies list
   - Handle paths configuration

2. **Package Resolution**
   - Resolve package names to git repos or local paths
   - Version constraint handling (semver)
   - Dependency graph construction

3. **Package Cache**
   - Download packages to ~/.ato/packages/
   - Version-specific directories
   - Lock file generation (ato.lock)

4. **CLI Commands**
   - `ato add <package>` - Add dependency
   - `ato install` - Install all dependencies
   - `ato update` - Update to latest versions

#### Key Files
- crates/ato-cli/src/commands/add.rs - New
- crates/ato-cli/src/commands/install.rs - New
- crates/ato-sema/src/packages.rs - New package resolver

#### Completion Promise
```
<promise>PACKAGE_MANAGER_COMPLETE</promise>
```
When:
- ato.yaml parsed correctly
- Dependencies downloaded and cached
- Imports resolve from packages
- CLI commands work
- Tests pass

---

### Prompt 26: Language Server Protocol (LSP)

Implement LSP server for IDE integration.

#### Current State
- No LSP implementation
- IDE support relies on Python version

#### Requirements

1. **ato-lsp Crate**
   - tower-lsp based server
   - Reuse ato-lexer, ato-parser, ato-sema

2. **Core Features**
   - Diagnostics (errors, warnings)
   - Go to definition
   - Find references
   - Hover information

3. **Completions**
   - Module/interface names
   - Field names after dot
   - Import suggestions
   - Keyword completions

4. **Document Sync**
   - Incremental parsing
   - Background analysis
   - Debounced diagnostics

#### Key Files
- crates/ato-lsp/src/main.rs - LSP server
- crates/ato-lsp/src/handlers.rs - Request handlers
- crates/ato-lsp/src/completion.rs - Completion provider

#### Completion Promise
```
<promise>LSP_COMPLETE</promise>
```
When:
- LSP server starts and handles requests
- Diagnostics show in editor
- Go to definition works
- Completions appear
- Tests pass

---

### Prompt 27: WASM Build for Browser

Compile core compiler to WebAssembly for browser/playground use.

#### Current State
- No WASM target
- All crates assume native environment

#### Requirements

1. **ato-wasm Crate**
   - wasm-bindgen exports
   - Parse, check, and analyze in browser
   - Return JSON results

2. **API Design**
   ```typescript
   interface AtoCompiler {
     parse(source: string): ParseResult;
     check(source: string): CheckResult;
     getCompletions(source: string, pos: number): Completion[];
   }
   ```

3. **Size Optimization**
   - Feature flags to exclude unused code
   - wasm-opt for binary size
   - Target < 500KB gzipped

4. **Playground Integration**
   - Web worker for non-blocking
   - Monaco editor integration
   - Live error highlighting

#### Key Files
- crates/ato-wasm/src/lib.rs - WASM bindings
- crates/ato-wasm/Cargo.toml - WASM-specific deps

#### Completion Promise
```
<promise>WASM_BUILD_COMPLETE</promise>
```
When:
- `wasm-pack build` succeeds
- Parse/check work in browser
- Reasonable binary size
- Example playground works
- Tests pass

---

### Prompt 28: Footprint & Symbol Library Mapping

Map components to actual KiCad library symbols and footprints.

#### Current State
- KiCad output generates placeholder symbols
- No real footprint references
- Schematic/PCB not usable in KiCad

#### Requirements

1. **Library Database**
   - Index of KiCad default libraries
   - Map component types to symbols/footprints
   - Package string to footprint lookup

2. **Symbol Assignment**
   - Resistor → Device:R
   - Capacitor → Device:C
   - Custom mappings via traits

3. **Footprint Assignment**
   - Package "0402" → Resistor_SMD:R_0402_1005Metric
   - Package "0603" → Resistor_SMD:R_0603_1608Metric
   - Support both imperial and metric

4. **KiCad Output Updates**
   - Schematic references real symbols
   - PCB references real footprints
   - Library paths configured in .kicad_pro

#### Key Files
- crates/ato-export/src/kicad_library.rs - New
- crates/ato-export/src/kicad_schematic.rs - Update
- crates/ato-export/src/kicad_pcb.rs - Update

#### Completion Promise
```
<promise>LIBRARY_MAPPING_COMPLETE</promise>
```
When:
- Common components map to KiCad libraries
- Generated schematic opens in KiCad
- Generated PCB has real footprints
- Tests verify library references
- Committed

---

### Prompt 29: Layout Preservation

Preserve user's manual PCB layout edits across rebuilds.

#### Current State
- PCB generation always creates fresh file
- No layout preservation
- Users lose manual placement/routing

#### Requirements

1. **Layout Parser**
   - Parse existing .kicad_pcb
   - Extract component positions
   - Extract trace routing

2. **Position Mapping**
   - Map designators to positions
   - Handle added/removed components
   - Preserve relative positions

3. **Merge Strategy**
   - New components: auto-place
   - Existing components: preserve position
   - Removed components: delete from layout

4. **Routing Preservation**
   - Keep traces for unchanged nets
   - Remove traces for changed nets
   - Mark changed nets for re-routing

#### Key Files
- crates/ato-export/src/layout_parser.rs - New
- crates/ato-export/src/layout_merge.rs - New
- crates/ato-export/src/kicad_pcb.rs - Update

#### Completion Promise
```
<promise>LAYOUT_PRESERVATION_COMPLETE</promise>
```
When:
- Existing layouts parsed correctly
- Component positions preserved
- New components placed reasonably
- Tests verify preservation
- Committed

---

## Crate Structure

```
crates/
├── ato-lexer/      # ✅ Tokenization (logos)
├── ato-parser/     # ✅ Parsing → AST (chumsky)
├── ato-domain/     # ✅ Quantity/interval types
├── ato-solver/     # ✅ Constraint solver
├── ato-ir/         # ✅ Intermediate representation
├── ato-sema/       # ✅ Semantic analysis + imports + constraints
├── ato-cli/        # ✅ Main compiler binary + solver integration
├── ato-tests/      # ✅ E2E test suite
├── ato-parts/      # ✅ Part database interface
├── ato-export/     # ✅ Netlist + BOM + KiCad export
├── ato-lsp/        # 🔲 Language server (Prompt 26)
└── ato-wasm/       # 🔲 WebAssembly build (Prompt 27)
```

---

## Key Dependencies

```toml
logos = "0.15"       # Lexer generation
chumsky = "0.9"      # Parser combinators
ariadne = "0.4"      # Error reporting
miette = "7"         # CLI error formatting
thiserror = "2"      # Error types
serde = "1"          # Serialization
clap = "4"           # CLI args
insta = "1"          # Snapshot testing
tower-lsp = "0.20"   # LSP server (future)
wasm-bindgen = "0.2" # WASM bindings (future)
```

---

## Priority Order

| Priority | Prompt | Impact |
|----------|--------|--------|
| **BLOCKER** | 30-35 - Gap Fixes | Core compiler doesn't produce correct output |
| **High** | 28 - Library Mapping | Makes KiCad output actually usable |
| **High** | 24 - Complete Stdlib | Enables real projects |
| **High** | 23 - LCSC Integration | Automatic part selection |
| **Medium** | 26 - LSP | IDE developer experience |
| **Medium** | 25 - Package Manager | Dependency management |
| **Medium** | 29 - Layout Preservation | Production workflow |
| **Lower** | 27 - WASM Build | Playground/web use |

---

## Critical Gap Fixes (Prompts 30-35)

These prompts fix critical gaps discovered during testing that prevent the compiler from producing correct output for real projects. **These must be completed before the compiler is usable.**

See `docs/rust-compiler-gaps.md` for detailed analysis and failing test cases.

---

### Prompt 30: Instance Field Expansion

When `x = new Type` is processed, the instance's type must be resolved and nested field access must work.

#### Current State
- `field_kind_from_new` in names.rs creates Instance fields but ignores the scope parameter
- `resolved_type` is left as None
- Post-creation resolution attempts to fix this but is fragile

#### Requirements

1. Fix `field_kind_from_new` to use scope and resolve type immediately
2. Ensure resolved_type is populated for all Instance fields
3. Nested field access like `x.field` must resolve through the instance's type
4. Handle instance arrays (`new Type[n]`) correctly

#### Key Files
- crates/ato-sema/src/names.rs (lines 354-388)
- crates/ato-sema/src/types.rs (resolve_nested_field)

#### Verification
```bash
cargo test -p ato-sema test_gap1_instance_field_expansion -- --ignored
```

#### Completion Promise
```
<promise>GAP1_INSTANCE_EXPANSION_COMPLETE</promise>
```
When:
- Test `test_gap1_instance_field_expansion` passes
- Instance fields have resolved_type populated
- Nested field access works for instance.field patterns

---

### Prompt 31: Assignment-to-Constraint Conversion

Assignments like `x = 100ohm +/- 10%` must create constraints in the IR.

#### Current State
- In lower.rs, Statement::Assignment falls into the catch-all `_ => {}` case and is ignored
- In names.rs, assignment values are examined for field kind detection but then discarded

#### Requirements

1. In lower.rs, handle Statement::Assignment in lower_block_statement
2. Convert assignments with physical values to Constraint with CompareOpKind::Is or Within
3. For bilateral tolerances (100ohm +/- 10%), use CompareOpKind::Within
4. Store the constraint in the module's constraints list
5. Handle nested field assignments like `r1.resistance = ...`

#### Key Files
- crates/ato-sema/src/lower.rs (lines 90-113)
- crates/ato-ir/src/constraint.rs

#### Verification
```bash
cargo test -p ato-sema test_gap2 -- --ignored
```

#### Completion Promise
```
<promise>GAP2_ASSIGNMENT_CONSTRAINTS_COMPLETE</promise>
```
When:
- Tests `test_gap2_assignment_creates_constraint` and `test_gap2_simple_assignment_creates_constraint` pass
- Assignments create IR constraints
- Solver receives assignment-derived constraints

---

### Prompt 32: Import Resolution and Stdlib Loading

Imports must load modules and make them available for `new` expressions.

#### Current State
- analyzer.rs indexes stdlib (line 142) but never merges the registry into the scope
- Imported types may not be available when field_kind_from_new runs

#### Requirements

1. After indexing stdlib, add stdlib modules to the initial scope
2. When processing imports, load the file and add modules to scope BEFORE name resolution
3. Ensure imported modules are available when processing `new ImportedType`
4. Handle transitive imports (imported module imports another module)
5. Track import errors properly

#### Key Files
- crates/ato-sema/src/analyzer.rs (lines 142-267)
- crates/ato-sema/src/resolution.rs

#### Verification
```bash
cargo test -p ato-sema test_stdlib -- --ignored
```

#### Completion Promise
```
<promise>GAP3_IMPORT_RESOLUTION_COMPLETE</promise>
```
When:
- Stdlib modules (Resistor, Capacitor, etc.) are available after import
- `new Resistor` resolves correctly after `import Resistor`
- File-based imports work

---

### Prompt 33: Nested Field Access in Constraints

Constraint collector must resolve paths like `instance.field` through instance types.

#### Current State
- resolve_field_unit() in constraint_collector.rs only looks at the first part of a path
- For `r1.resistance`, it finds `r1` but cannot traverse into its type to find `resistance`

#### Requirements

1. In constraint_collector.rs, extend resolve_field_unit to handle multi-part paths
2. When path[0] is an Instance field, get its resolved_type and look up path[1] in that module
3. Continue recursively for deeper paths (a.b.c)
4. Track the full path for constraint variable naming
5. Handle array instances (r[0].value)

#### Key Files
- crates/ato-sema/src/constraint_collector.rs (resolve_field_unit around line 277)
- crates/ato-sema/src/types.rs (resolve_nested_field)

#### Verification
```bash
cargo test -p ato-sema test_gap5 -- --ignored
```

#### Completion Promise
```
<promise>GAP5_NESTED_FIELD_ACCESS_COMPLETE</promise>
```
When:
- Test `test_gap5_nested_field_constraint` passes
- Constraints on `instance.parameter` are correctly collected
- Solver receives nested field constraints

---

### Prompt 34: Connection Lowering for Instance Pins

Connections between instance pins must be properly lowered.

#### Current State
- lower_connectable() creates FieldPath but doesn't verify that nested paths (r1.p1) are valid
- Doesn't verify they reference actual pins through instance types

#### Requirements

1. When lowering a connection like `r1.p2 ~ r2.p1`, verify r1 is an instance
2. Resolve r1's type and verify p2 exists as a pin in that type
3. Create ConnectionEndpoint with proper FieldPath representing instance.pin
4. Handle inline signal definitions in connections
5. Track connection span for error reporting

#### Key Files
- crates/ato-sema/src/lower.rs (lower_connectable around line 145)

#### Verification
```bash
cargo test -p ato-sema test_gap6 -- --ignored
```

#### Completion Promise
```
<promise>GAP6_CONNECTION_LOWERING_COMPLETE</promise>
```
When:
- Test `test_gap6_instance_pin_connection` passes
- Connections between instance pins create proper IR connections
- Connection graph is built correctly

---

### Prompt 35: Netlist Component Extraction

NetlistBuilder must extract component instances, not module definitions.

#### Current State
- collect_components() in netlist.rs iterates design.modules() and creates components for ANY module with pins
- This creates phantom components for abstract modules and misses actual instances

#### Requirements

1. Start from entry module and traverse instance fields
2. For each Instance field with resolved_type, create a component
3. Track instance path for proper reference naming (r1, divider.r1, etc.)
4. Extract footprint from module properties/traits
5. Build nets by following connections through instance boundaries
6. Map instance.pin references to component.pin in netlist

#### Key Files
- crates/ato-export/src/netlist.rs (lines 240-356)

#### Verification
```bash
cargo test -p ato-export test_gap4 -- --ignored
```

#### Completion Promise
```
<promise>GAP4_NETLIST_EXTRACTION_COMPLETE</promise>
```
When:
- Tests `test_gap4_netlist_extracts_instances_not_definitions` and `test_gap4_netlist_instance_connections` pass
- Netlist contains only instantiated components
- Nets correctly connect component pins

---

## Key Reference Files

- `src/atopile/parser/AtoLexer.g4` - Token definitions
- `src/atopile/parser/AtoParser.g4` - Parser grammar
- `src/atopile/front_end.py` - DSL → Faebryk compiler
- `src/atopile/config.py` - Package resolution
- `src/faebryk/exporters/` - Output format exporters
- `src/faebryk/library/` - Python stdlib modules
