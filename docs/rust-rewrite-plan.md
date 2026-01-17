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
| `ato-tests` | ✅ Complete | E2E test suite (110 tests) |
| Real-world tests | 🔲 In Progress | Testing against examples/packages/external repos |

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

## Phase 4: CLI and End-to-End Testing ✅ COMPLETE

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

### Prompt 8: End-to-End Test Suite ✅

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
- `crates/ato-tests/` - Test crate with fixtures and tests
- `crates/ato-tests/src/harness.rs` - Common test utilities
- Snapshot testing with `insta` crate

## Completion: E2E_TESTS_COMPLETE ✅
```

---

## Phase 4.5: Real-World Project Testing 🔲 IN PROGRESS

Test the Rust compiler against real atopile projects and packages to ensure compatibility with production code.

### Prompt 9: Local Examples Testing

```markdown
# Task: Test Against Local Examples

Parse and analyze all example projects in the atopile repository.

## Test Targets (by complexity)

### Simple
1. `examples/quickstart/quickstart.ato` - Basic resistor, single import
2. `examples/layout_reuse/layout_reuse.ato` - Sub-module arrays, bridge connections

### Medium
3. `examples/equations/equations.ato` - Voltage divider, constraint equations
4. `examples/pick_parts/pick_parts.ato` - Part selection, pragmas, FOR_LOOP
5. `examples/i2c/i2c.ato` - Multi-module I2C, templating, local imports

### Complex
6. `examples/esp32_minimal/esp32_minimal.ato` - External package imports, power rails
7. `examples/led_badge/led_badge.ato` - 100+ lines, multiple subsystems, LED matrix

## Requirements

1. Create test harness that runs `ato parse` on each example
2. Track which examples parse successfully vs fail
3. For failures, categorize the error type:
   - Lexer error (unknown token)
   - Parser error (syntax)
   - Unsupported feature (pragma, template, etc.)
4. Create fixture tests for each example
5. Document gaps between Rust and Python parser capabilities

## Success Criteria
- All examples parse without lexer/parser errors
- Semantic analysis runs (even if imports fail)
- Clear error messages for unsupported features

## Completion Promise
Output <promise>LOCAL_EXAMPLES_COMPLETE</promise> when:
- All 7 examples tested
- Test results documented
- Fixture tests created
```

### Prompt 10: Standard Library Testing

```markdown
# Task: Test Standard Library Files

Parse all .ato files in the standard library.

## Test Targets
- `src/faebryk/library/interfaces.ato` - Protocol interfaces (I2S, SPI, CAN, USB_PD, etc.)
- `src/faebryk/library/resistors.ato` - I2CPullup module
- `src/faebryk/library/diodes.ato` - PowerDiodeOr, bridge rectifier
- `src/faebryk/library/mosfets.ato` - HalfBridge, LowSideSwitch
- `src/faebryk/library/regulators.ato` - Buck, Boost, LDO variants
- `src/faebryk/library/filters.ato` - LowPassPiFilter
- `src/faebryk/library/vdivs.ato` - Voltage divider
- `src/faebryk/library/oscillators.ato` - Crystal oscillator
- `src/faebryk/library/debug.ato` - TestPoint

## Requirements
1. Parse each library file
2. Verify module/interface definitions are extracted
3. Test that library patterns (interfaces, traits) are handled

## Completion Promise
Output <promise>STDLIB_TESTS_COMPLETE</promise> when all library files parse successfully.
```

### Prompt 11: External Repository Testing

```markdown
# Task: Test Against External Atopile Repos

Clone and test against real-world atopile projects from GitHub.

## Repositories to Test

### Package Repos (simpler, self-contained)
1. `atopile/generics` - Standard library (resistors, capacitors, LEDs, interfaces)
2. `atopile/rp2040` - RP2040 microcontroller module
3. `atopile/esp32-s3` - ESP32-S3 module

### Hardware Project Repos (complex, multiple files)
4. `atopile/spin-servo-drive` - BLDC servo controller (115 stars)
5. `atopile/nonos` - Smart speaker with CM5, DSP, amp (51 stars)

### Community Repos
6. `u-fire/esp32c3-ato` - ESP32-C3-MINI module

## Test Approach

1. Clone each repo to `crates/ato-tests/external/` (gitignored)
2. Find all .ato files recursively
3. Run `ato parse` on each file
4. Collect and categorize results:
   - ✅ Parses successfully
   - ⚠️ Parses with warnings
   - ❌ Parse error (with error type)
5. Create summary report

## Requirements
- Script to clone/update repos
- Test harness for external projects
- CI-friendly (can skip if repos unavailable)
- Report showing compatibility percentage

## Completion Promise
Output <promise>EXTERNAL_REPOS_COMPLETE</promise> when:
- At least 3 external repos tested
- Compatibility report generated
- Major parse failures documented as issues
```

### Prompt 12: Syntax Coverage Validation

```markdown
# Task: Validate Full Syntax Coverage

Use the comprehensive syntax example file to verify all language features.

## Test Target
- `src/vscode-atopile/syntax_examples.ato` - 200 lines covering all syntax features

## Requirements
1. Parse the full syntax examples file
2. For each syntax construct, verify:
   - Lexer produces correct tokens
   - Parser produces correct AST node
   - AST can be serialized to JSON
3. Create a syntax coverage matrix:

| Feature | Lexer | Parser | Sema | Notes |
|---------|-------|--------|------|-------|
| module/interface/component | ✅ | ✅ | ✅ | |
| pin/signal declarations | ✅ | ✅ | ✅ | |
| connections (~) | ✅ | ✅ | ✅ | |
| directed connections (~>) | ✅ | ✅ | ? | Needs BRIDGE_CONNECT pragma |
| imports | ✅ | ✅ | ⚠️ | Parsed but not resolved |
| for loops | ✅ | ✅ | ✅ | Needs FOR_LOOP pragma |
| assertions | ✅ | ✅ | ✅ | |
| quantities/tolerances | ✅ | ✅ | ✅ | |
| templates | ? | ? | ? | MODULE_TEMPLATING pragma |
| traits | ? | ? | ? | TRAITS pragma |
| pragmas | ✅ | ✅ | ? | |

## Completion Promise
Output <promise>SYNTAX_COVERAGE_COMPLETE</promise> when:
- All syntax constructs tested
- Coverage matrix documented
- Gaps identified and documented
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
├── ato-cli/            # ✅ Main compiler binary (orchestrates everything)
└── ato-tests/          # ✅ E2E test suite (110 tests)
    ├── fixtures/       # Test fixture .ato files
    │   ├── parse/      # Parser test fixtures
    │   ├── sema/       # Semantic analysis fixtures
    │   ├── solver/     # Solver test fixtures
    │   └── integration/# Integration test fixtures
    └── tests/          # Test implementations
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
9. ✅ ~~E2E tests~~ - Comprehensive test suite validating full pipeline (110 tests)
10. 🔲 **Local examples testing** - Test all 7 examples in examples/ directory
11. 🔲 **Standard library testing** - Test all .ato files in src/faebryk/library/
12. 🔲 **External repos testing** - Test against atopile/generics, rp2040, esp32-s3, etc.
13. 🔲 **Syntax coverage** - Validate full language coverage with syntax_examples.ato
14. 🔲 **Output generation** - KiCad, BOM, netlist (future)

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
