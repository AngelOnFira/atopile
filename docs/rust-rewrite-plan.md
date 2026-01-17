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
| `ato-export` | ✅ Complete | Netlist + BOM generation |
| KiCad project | 🔲 Not Started | Full KiCad project output |

---

## Feature Parity Status

**Completed**: Parsing, semantic analysis, import resolution, constraint solving, part queries, netlist export, BOM generation

**Remaining**: Full KiCad project output (schematic, PCB, libraries)

Estimated: **~90% of core functionality complete**

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
- **Component grouping**: Identical parts grouped with quantities
- **Designator sorting**: Proper numeric ordering (R1, R2, R10)

---

## Remaining Work

### Prompt 20: KiCad Project Output 🔲

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
├── ato-sema/       # ✅ Semantic analysis + imports + constraints
├── ato-cli/        # ✅ Main compiler binary + solver integration
├── ato-tests/      # ✅ E2E test suite
├── ato-parts/      # ✅ Part database interface
└── ato-export/     # ✅ Netlist + BOM export
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

## Next Steps

1. 🔲 **KiCad project** (Prompt 20) - Full design output

---

## Key Reference Files

- `src/atopile/parser/AtoLexer.g4` - Token definitions
- `src/atopile/parser/AtoParser.g4` - Parser grammar
- `src/atopile/front_end.py` - DSL → Faebryk compiler
- `src/atopile/config.py` - Package resolution
- `src/faebryk/exporters/` - Output format exporters
