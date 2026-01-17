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
| `ato-sema` | ✅ Complete | Semantic analysis |
| `ato-cli` | ✅ Complete | Main compiler CLI |
| `ato-tests` | ✅ Complete | E2E test suite (227 tests) |
| `ato-parts` | ✅ Complete | Part database interface |
| `ato-export` | ✅ Complete | Netlist generation (KiCad format) |
| Import resolution | 🔲 Not Started | Package/file resolution + symbol merging |
| Solver integration | 🔲 Not Started | Connect solver to build pipeline |
| BOM generation | 🔲 Not Started | Bill of materials output |
| KiCad project | 🔲 Not Started | Full KiCad project output |

---

## Feature Parity Status

**Completed**: Parsing, semantic analysis, constraint types, part queries, netlist export
**Remaining**: Import resolution, solver pipeline, BOM, full KiCad output

Estimated: ~70% of core functionality complete

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

### Phase 7: Part Selection ✅
- **ato-parts crate**: Part/PartId/Manufacturer types, PartDatabase trait
- **Query builders**: ResistorQuery, CapacitorQuery with parameter constraints
- **Selection**: BasicPartSelector with scoring strategies

### Phase 8: Output Generation (Partial) ✅
- **ato-export crate**: Netlist/Net/NetNode/NetlistComponent types
- **NetlistBuilder**: Builds netlist from IR Design
- **KiCad export**: S-expression netlist format (.net files)

---

## Remaining Work

### Phase 5: Import Resolution 🔲

#### Prompt 13: Package/File Resolution

Create module resolution system to find and load .ato files.

**Requirements**:
1. Resolve `from "path/to/file.ato" import Module` (relative/absolute paths)
2. Resolve `import Resistor` from standard library
3. Look up packages in `ato.yaml` dependencies
4. Build module registry, detect circular imports

**Reference**: `src/atopile/config.py` - Python package resolution

**Completion**: `<promise>FILE_RESOLUTION_COMPLETE</promise>`

---

#### Prompt 14: Symbol Merging

Merge imported symbols into importing scope.

**Requirements**:
1. Track which symbols each file exports
2. `import Resistor` adds `Resistor` to current scope
3. Update name resolver to check imported symbols
4. `module Derived from ImportedBase:` inherits fields across files

**Completion**: `<promise>SYMBOL_MERGING_COMPLETE</promise>`

---

### Phase 6: Solver Integration 🔲

#### Prompt 15: Constraint Collection

Extract constraints from IR and prepare for solver.

**Requirements**:
1. Walk IR Design, collect all constraints
2. Convert IR expressions to solver expressions
3. Build parameter dependency graph
4. Handle unit conversions (mV→V, etc.)

**Completion**: `<promise>CONSTRAINT_COLLECTION_COMPLETE</promise>`

---

#### Prompt 16: Solver Execution

Run solver on collected constraints.

**Requirements**:
1. Pass constraints to ato-solver, run simplification
2. Report solved values or contradictions with source locations
3. Integrate into `ato build` and `ato check` commands

**Completion**: `<promise>SOLVER_INTEGRATION_COMPLETE</promise>`

---

### Phase 8: Output Generation (Remaining) 🔲

#### Prompt 19: BOM Generation

Produce Bill of Materials for manufacturing.

**Requirements**:
1. JLCPCB BOM format (CSV)
2. Generic BOM format
3. Include part numbers, quantities, values
4. Group identical components

**Reference**: `src/faebryk/exporters/bom/`

**Completion**: `<promise>BOM_GENERATION_COMPLETE</promise>`

---

#### Prompt 20: KiCad Project Output

Generate complete KiCad project from design.

**Requirements**:
1. Schematic file generation
2. PCB file with footprints placed
3. Symbol/footprint library references
4. Project file (.kicad_pro)

**Note**: Most complex output task - requires reverse engineering KiCad formats.

**Completion**: `<promise>KICAD_PROJECT_COMPLETE</promise>`

---

## Crate Structure

```
crates/
├── ato-lexer/      # ✅ Tokenization (logos)
├── ato-parser/     # ✅ Parsing → AST (chumsky)
├── ato-domain/     # ✅ Quantity/interval types
├── ato-solver/     # ✅ Constraint solver
├── ato-ir/         # ✅ Intermediate representation
├── ato-sema/       # ✅ Semantic analysis
├── ato-cli/        # ✅ Main compiler binary
├── ato-tests/      # ✅ E2E test suite
├── ato-parts/      # ✅ Part database interface
└── ato-export/     # ✅ Netlist/KiCad export
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
```

---

## Next Steps (Priority Order)

1. 🔲 **Import resolution** (Prompts 13-14) - Critical for real projects
2. 🔲 **Solver integration** (Prompts 15-16) - Enables constraint solving
3. 🔲 **BOM generation** (Prompt 19) - Manufacturing output
4. 🔲 **KiCad project** (Prompt 20) - Full design output

---

## Key Reference Files

- `src/atopile/parser/AtoLexer.g4` - Token definitions
- `src/atopile/parser/AtoParser.g4` - Parser grammar
- `src/atopile/front_end.py` - DSL → Faebryk compiler
- `src/atopile/config.py` - Package resolution
- `src/faebryk/exporters/` - Output format exporters
