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
| **High** | 28 - Library Mapping | Makes KiCad output actually usable |
| **High** | 24 - Complete Stdlib | Enables real projects |
| **High** | 23 - LCSC Integration | Automatic part selection |
| **Medium** | 26 - LSP | IDE developer experience |
| **Medium** | 25 - Package Manager | Dependency management |
| **Medium** | 29 - Layout Preservation | Production workflow |
| **Lower** | 27 - WASM Build | Playground/web use |

---

## Key Reference Files

- `src/atopile/parser/AtoLexer.g4` - Token definitions
- `src/atopile/parser/AtoParser.g4` - Parser grammar
- `src/atopile/front_end.py` - DSL → Faebryk compiler
- `src/atopile/config.py` - Package resolution
- `src/faebryk/exporters/` - Output format exporters
- `src/faebryk/library/` - Python stdlib modules
