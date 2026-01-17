# Atopile Rust Rewrite Plan

This document outlines the strategy for a **full rewrite** of atopile in Rust. The goal is a standalone Rust compiler that can build `.ato` projects **without any Python dependencies**.

This is NOT an incremental migration - we are building a completely new Rust toolchain that will eventually replace the Python implementation entirely.

## Current Progress

| Crate | Status | Description |
|-------|--------|-------------|
| `ato-lexer` | ✅ Complete | Tokenization with INDENT/DEDENT |
| `ato-parser` | ✅ Complete | AST generation with chumsky (error recovery) |
| `ato-domain` | ✅ Complete | Quantity/interval/set types |
| `ato-solver` | ✅ Complete | Constraint solver with simplification |
| `ato-ir` | ✅ Complete | Intermediate representation |
| `ato-sema` | ✅ Complete | Semantic analysis |
| `ato-cli` | ✅ Complete | Main compiler CLI |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                         ato-cli                              │
│  (main binary - orchestrates compilation, handles I/O)       │
└─────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         ▼                    ▼                    ▼
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│  ato-lexer  │      │  ato-sema   │      │  ato-solver │
│  (tokens)   │      │  (analysis) │      │ (constraints)│
└─────────────┘      └─────────────┘      └─────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│ ato-parser  │      │   ato-ir    │      │ ato-domain  │
│   (AST)     │      │   (graph)   │      │  (types)    │
└─────────────┘      └─────────────┘      └─────────────┘
```

---

## Phase 0: Codebase Cleanup ✅ COMPLETE

Before proceeding with new development, clean up dead code from earlier experimentation:

### Prompt 0: Remove Dead Code

```markdown
# Task: Clean Up Unused Code

Remove code that won't be used in the full rewrite.

## Items to Remove

### 1. Remove ato-py crate
The Python bindings (`crates/ato-py/`) were created for incremental migration but aren't needed for a full rewrite.
- Delete `crates/ato-py/` directory entirely
- Remove from workspace `Cargo.toml` members

### 2. Consolidate parser to chumsky only
The parser currently has two implementations:
- `src/parse/` - Hand-written recursive descent parser (default)
- `src/chumsky/` - Chumsky-based parser with error recovery (feature-gated)

Chumsky provides better error recovery, so make it the default:
- Move `src/chumsky/` contents to `src/parse/` (replacing old implementation)
- Remove the `chumsky` feature flag from Cargo.toml
- Make chumsky + ariadne required dependencies (not optional)
- Update lib.rs to export chumsky parser as the default
- Update all tests

## Tests Must Pass
- `cargo test -p ato-parser`
- All existing parser tests continue to work

## Completion Criteria
Output <promise>CLEANUP_COMPLETE</promise> when:
- ato-py directory deleted
- Parser consolidated to chumsky only
- No feature flags for parser selection
- All tests pass
```

---

## Phase 1: Language Frontend ✅ COMPLETE

### Prompt 1: Ato Lexer ✅

```markdown
# Task: Implement Ato Lexer in Rust

Create a lexer for the Ato DSL using the `logos` crate.

## Requirements
1. Tokenize all tokens from the grammar (see src/atopile/parser/AtoLexer.g4)
2. Handle Python-style INDENT/DEDENT with indentation tracking
3. Track source locations (line, column, byte offset)
4. Support string literals, numbers (int, float, hex, bin, oct)
5. Handle physical quantities (10kohm, 5V, 100nF)

## Tests Must Pass
- `cargo test -p ato-lexer`

## Completion: LEXER_COMPLETE ✅
```

### Prompt 2: Ato Parser ✅

```markdown
# Task: Implement Ato Parser in Rust

Create a parser that produces a typed AST.

## Requirements
1. Parse according to src/atopile/parser/AtoParser.g4
2. Use ato-lexer as input
3. Produce strongly-typed AST with serde serialization
4. Generate good error messages with source spans

## Tests Must Pass
- `cargo test -p ato-parser`

## Completion: PARSER_COMPLETE ✅
```

---

## Phase 2: Constraint System ✅ COMPLETE

### Prompt 3: Domain Types ✅

```markdown
# Task: Implement Constraint Domain Types

## Requirements
1. `Quantity` type with unit tracking
2. `Interval` for continuous ranges
3. `DisjointIntervals` for unions
4. `BilateralTolerance` (e.g., 10kohm +/- 5%)
5. Set operations: union, intersection, contains, is_subset

## Tests Must Pass
- `cargo test -p ato-domain`

## Completion: DOMAIN_COMPLETE ✅
```

### Prompt 4: Constraint Solver ✅

```markdown
# Task: Implement Constraint Solver

## Requirements
1. Expression types: Parameter, Literal, arithmetic ops
2. Predicate types: Is, LessOrEqual, GreaterOrEqual, IsSubset, Within
3. Simplification pipeline with fixed-point iteration
4. Contradiction detection

## Tests Must Pass
- `cargo test -p ato-solver`

## Completion: SOLVER_COMPLETE ✅
```

---

## Phase 3: Semantic Analysis ✅ COMPLETE

### Prompt 5: Intermediate Representation

```markdown
# Task: Implement Core IR (ato-ir crate)

Create the intermediate representation that models an Ato design after parsing.

## Requirements
1. Module/Component/Interface definitions with unique IDs
2. Field system: parameters, pins, signals, instances
3. Connection graph (which pins connect to which)
4. Constraint collection (assertions on parameters)
5. Import resolution data structures

## Key Types
- `ModuleId`, `FieldId`, `ConnectionId` - interned identifiers
- `Module` - contains fields, connections, constraints, nested modules
- `Field` - parameter, pin, signal, or instance
- `Connection` - links between connectable fields
- `Design` - root container with all modules

## Structure
- `crates/ato-ir/src/lib.rs`
- `crates/ato-ir/src/module.rs` - Module/Component/Interface
- `crates/ato-ir/src/field.rs` - Fields and their types
- `crates/ato-ir/src/connection.rs` - Connection graph
- `crates/ato-ir/src/design.rs` - Top-level design container

## Tests Must Pass
- `cargo test -p ato-ir`
- Can represent the structure of example .ato files

## Completion Criteria
Output <promise>IR_COMPLETE</promise> when:
- All core types implemented
- Can model modules, fields, connections
- Unit tests pass
```

### Prompt 6: Semantic Analysis

```markdown
# Task: Implement Semantic Analysis (ato-sema crate)

Lower parsed AST to IR with full semantic checking.

## Requirements
1. Import resolution - find and load dependent .ato files
2. Name resolution - resolve all identifiers to their definitions
3. Type checking - ensure connections are between compatible interfaces
4. Inheritance - apply `from` clauses to inherit fields
5. Template instantiation - expand `new Type<args>`
6. For-loop expansion - unroll for loops into concrete instances
7. Error collection - gather all semantic errors with source spans

## Key Passes
1. `resolve_imports()` - Build module dependency graph, load files
2. `resolve_names()` - Link identifiers to definitions
3. `check_types()` - Verify interface compatibility
4. `lower_to_ir()` - Convert AST → IR with all expansions

## Structure
- `crates/ato-sema/src/lib.rs`
- `crates/ato-sema/src/imports.rs` - Import resolution
- `crates/ato-sema/src/names.rs` - Name resolution
- `crates/ato-sema/src/types.rs` - Type checking
- `crates/ato-sema/src/lower.rs` - AST → IR lowering

## Tests Must Pass
- `cargo test -p ato-sema`
- Can analyze example .ato files without false errors
- Catches real errors (undefined names, type mismatches)

## Completion: SEMA_COMPLETE ✅
```

---

## Phase 4: CLI and End-to-End Testing 🔲 IN PROGRESS

### Prompt 7: CLI Foundation ✅

```markdown
# Task: Implement CLI (ato-cli crate)

Create the main `ato` binary that orchestrates compilation.

## Requirements
1. Parse command-line arguments (use `clap`)
2. Load and parse .ato files
3. Run semantic analysis
4. Run constraint solver
5. Report errors with nice formatting (use `miette`)
6. Output build artifacts (initially just validation pass/fail)

## Commands
- `ato build <file.ato>` - Compile a project
- `ato check <file.ato>` - Check without full build
- `ato parse <file.ato>` - Parse and dump AST (for debugging)

## Structure
- `crates/ato-cli/src/main.rs` - Entry point
- `crates/ato-cli/src/commands/build.rs`
- `crates/ato-cli/src/commands/check.rs`
- `crates/ato-cli/src/commands/parse.rs`
- `crates/ato-cli/src/error.rs` - Error formatting

## Tests Must Pass
- `cargo test -p ato-cli`
- `cargo run -p ato-cli -- check examples/*/elec/src/*.ato` passes
- Integration tests with known-good and known-bad .ato files

## Completion: CLI_COMPLETE ✅
```

### Prompt 8: End-to-End Test Suite

```markdown
# Task: Create End-to-End Test Suite

Build a comprehensive test suite that validates the full compiler pipeline.

## Requirements
1. Golden tests - parse example files, compare output
2. Error tests - verify specific errors are caught
3. Solver tests - verify constraint solving on real designs
4. Regression tests - prevent previously fixed bugs from returning

## Test Categories

### Parse Tests (`tests/parse/`)
- `valid/*.ato` - Should parse successfully
- `invalid/*.ato` - Should produce specific parse errors

### Semantic Tests (`tests/sema/`)
- `valid/*.ato` - Should pass semantic analysis
- `errors/*.ato` - Should catch specific semantic errors

### Solver Tests (`tests/solver/`)
- `satisfiable/*.ato` - Constraints should solve
- `contradictions/*.ato` - Should detect contradictions

### Integration Tests (`tests/integration/`)
- Full builds of example projects
- Comparison with Python implementation output

## Structure
- `tests/` directory at workspace root
- `tests/harness.rs` - Common test utilities
- Snapshot testing with `insta` crate

## Completion Criteria
Output <promise>E2E_TESTS_COMPLETE</promise> when:
- 50+ test cases covering major features
- All example projects pass
- CI runs tests on every commit
```

---

## Phase 5: Output Generation 🔲 FUTURE

### Prompt 9: KiCad/Netlist Output

```markdown
# Task: Generate Build Outputs

Produce actual build artifacts from compiled designs.

## Requirements
1. Netlist generation (connections between components)
2. BOM generation (bill of materials)
3. KiCad project output (or compatible format)
4. Part selection integration (JLCPCB, etc.)

## This phase depends on understanding the current output formats.
## May require porting the Zig S-expression engine or reimplementing.
```

---

## Crate Structure

The Rust rewrite consists of pure Rust crates with no Python dependencies:

```
crates/
├── ato-lexer/          # ✅ Tokenization (logos-based)
├── ato-parser/         # ✅ Parsing → AST (chumsky-based, error recovery)
├── ato-domain/         # ✅ Quantity/interval/set types
├── ato-solver/         # ✅ Constraint solver with simplification
├── ato-ir/             # ✅ Intermediate representation (design graph)
├── ato-sema/           # ✅ Semantic analysis (name resolution, type checking)
└── ato-cli/            # ✅ Main compiler binary (orchestrates everything)

tests/
├── parse/              # Parser golden tests
├── sema/               # Semantic analysis tests
├── solver/             # Constraint solver tests
└── integration/        # Full pipeline tests (CLI end-to-end)
```

**Note**: `ato-py` (Python bindings) should be deleted - it was for incremental migration which we're not doing.

---

## Key Dependencies

```toml
[workspace.dependencies]
# Lexing
logos = "0.15"           # Fast lexer generation

# Parsing
chumsky = "0.9"          # Parser combinators with error recovery
ariadne = "0.4"          # Beautiful error reporting

# Error handling
miette = { version = "7", features = ["fancy"] }
thiserror = "1"

# Data structures
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# CLI
clap = { version = "4", features = ["derive"] }

# Testing
insta = "1"              # Snapshot testing
```

---

## Testing Strategy

1. **Unit tests**: Each crate has internal tests (`cargo test -p <crate>`)
2. **Integration tests**: `tests/` directory with end-to-end scenarios
3. **Golden tests**: Compare output against known-good snapshots
4. **Example projects**: Build all `examples/` and `packages/` successfully

---

## Success Metrics

| Milestone | Metric |
|-----------|--------|
| Parse all examples | `ato check examples/**/*.ato` exits 0 |
| Semantic analysis | No false positives on valid code |
| Solver correctness | Matches Python solver results |
| Error quality | Errors point to correct source locations |
| Performance | 10x faster than Python implementation |

---

## Next Steps

### Completed
1. ✅ ~~Create ato-lexer~~ - Tokenization with INDENT/DEDENT
2. ✅ ~~Create ato-parser~~ - AST with error recovery
3. ✅ ~~Create ato-domain~~ - Quantity/interval types
4. ✅ ~~Create ato-solver~~ - Constraint solver

### Up Next (in order)
5. ✅ ~~Cleanup~~ - Remove ato-py, consolidate parser to chumsky only
6. ✅ ~~Create ato-ir~~ - Intermediate representation for designs
7. ✅ ~~Create ato-sema~~ - Semantic analysis (imports, names, types)
8. ✅ ~~Create ato-cli~~ - Main `ato` binary that ties everything together
9. 🔲 **E2E tests** - Comprehensive test suite validating full pipeline
10. 🔲 **Output generation** - KiCad, BOM, netlist (future)

---

## Key Files to Reference

### Grammar
- `src/atopile/parser/AtoLexer.g4` - Token definitions
- `src/atopile/parser/AtoParser.g4` - Parser grammar

### Frontend
- `src/atopile/front_end.py` - DSL → Faebryk compiler (3127 lines)

### Solver
- `src/faebryk/core/parameter.py` - Parameter/Expression types
- `src/faebryk/core/solver/defaultsolver.py` - Main solver

### Domain Types
- `src/faebryk/libs/sets/quantity_sets.py` - Physical quantity intervals
