# Atopile Rust Rewrite Plan

This document outlines a strategy for rewriting performance-critical parts of atopile in Rust, using the Ralph Wiggum continuous iteration methodology.

## Atopile Architecture Summary

The project has several distinct layers:

### 1. Language Frontend (`src/atopile/`)

| Component | Current Tech | Files | Complexity |
|-----------|-------------|-------|------------|
| Lexer/Parser | ANTLR4 → Python | `parser/Ato*.g4`, `*.py` | Medium |
| AST Visitor | Python visitor pattern | `front_end.py` (3127 lines) | High |
| Error handling | Custom exceptions | Scattered | Medium |

### 2. Core Engine (`src/faebryk/core/`)

| Component | Current Tech | Files | Complexity |
|-----------|-------------|-------|------------|
| Node/Graph system | Python + C++ bindings | `node.py`, `graph.py`, `cpp/` | High |
| Parameter system | Python | `parameter.py` (1947 lines) | High |
| Constraint solver | Python | `solver/` (6+ files, 1500+ lines) | Very High |
| Trait system | Python metaclasses | `trait.py` | Medium |

### 3. S-Expression Engine (`src/faebryk/core/zig/`)

| Component | Current Tech | Files | Complexity |
|-----------|-------------|-------|------------|
| Tokenizer | Zig | `tokenizer.zig` | Medium |
| AST/Parser | Zig | `ast.zig` | Medium |
| KiCad models | Zig | `kicad/*.zig` | Medium |
| Python bindings | Zig | `pyzig/*.zig` | High |

### 4. Sets/Domains (`src/faebryk/libs/sets/`)

| Component | Current Tech | Files | Complexity |
|-----------|-------------|-------|------------|
| Quantity intervals | Python | `quantity_sets.py` | Medium |
| Numeric sets | Python | `numeric_sets.py` | Medium |
| Generic sets | Python | `sets.py` | Medium |

---

## Proposed Rust Rewrite Strategy

### Phase 1: Language Core (Most Valuable First)

**1a. Ato Lexer/Parser in Rust**
- Self-contained, clear spec (G4 grammar exists)
- Can use `logos` for lexing + `chumsky` or hand-written recursive descent
- Benefits: 10-100x faster parsing, WASM for browser IDE, better errors

**1b. AST Types**
- Define the complete AST in Rust with `serde` support
- JSON/MessagePack output for Python interop
- This becomes the "contract" between Rust frontend and Python backend

### Phase 2: Constraint Solver

**2a. Set/Domain System**
- Port `Quantity_Interval`, `Quantity_Interval_Disjoint`, `P_Set`
- Rust's type system is perfect for this
- Use `uom` crate for unit handling

**2b. Expression/Parameter System**
- Port `Parameter`, `Expression`, `Predicate` types
- Algebraic data types work naturally here

**2c. Solver Algorithms**
- Port simplification passes (canonical, structural, expression-wise)
- Fixed-point iteration loop
- Contradiction detection

### Phase 3: Optional Extensions

**3a. S-Expression Engine**
- Could stay in Zig or port to Rust
- Zig is already fast, may not be worth the effort

**3b. Graph System**
- Replace C++ bindings with Rust
- PyO3 for Python interop

---

## Ralph Wiggum Prompts

These prompts are designed for the Ralph loop continuous iteration approach.
Run with: `/ralph-loop "<prompt>" --max-iterations 50 --completion-promise "<PROMISE>"`

### Prompt 1: Ato Lexer

```markdown
# Task: Implement Ato Lexer in Rust

Create a lexer for the Ato DSL in Rust using the `logos` crate.

## Requirements
1. Tokenize all tokens from the grammar (see src/atopile/parser/AtoLexer.g4)
2. Handle Python-style INDENT/DEDENT with indentation tracking
3. Track source locations (line, column, byte offset)
4. Support string literals, numbers (int, float, hex, bin, oct)
5. Handle physical quantities (10kohm, 5V, 100nF)
6. Generate helpful error messages for invalid tokens

## Structure
- `crates/ato-lexer/src/lib.rs` - Main lexer implementation
- `crates/ato-lexer/src/tokens.rs` - Token enum
- `crates/ato-lexer/src/span.rs` - Source location tracking
- `crates/ato-lexer/tests/` - Test suite

## Tests Must Pass
- `cargo test -p ato-lexer`
- Parse all .ato files in examples/ without panic
- Benchmark: lex 10,000 lines in < 10ms

## Completion Criteria
Output <promise>LEXER_COMPLETE</promise> when:
- All tokens from grammar are handled
- INDENT/DEDENT works correctly
- All tests pass
- Examples parse without error
```

### Prompt 2: Ato Parser

```markdown
# Task: Implement Ato Parser in Rust

Create a parser for the Ato DSL that produces a typed AST.

## Requirements
1. Parse according to src/atopile/parser/AtoParser.g4
2. Use the ato-lexer crate as input
3. Produce strongly-typed AST with serde serialization
4. Generate excellent error messages with source spans
5. Handle all statement types: imports, blockdefs, assignments, connections, assertions, for-loops, traits

## Structure
- `crates/ato-parser/src/lib.rs` - Parser entry point
- `crates/ato-parser/src/ast.rs` - AST type definitions
- `crates/ato-parser/src/parse/*.rs` - Parse functions per node type
- `crates/ato-parser/src/error.rs` - Error types with nice formatting

## Tests Must Pass
- `cargo test -p ato-parser`
- Parse all .ato files in examples/ and packages/
- Round-trip test: parse → serialize → matches expected JSON

## Completion Criteria
Output <promise>PARSER_COMPLETE</promise> when:
- Full grammar coverage
- All tests pass
- Error messages include source context
- JSON output matches expected schema
```

### Prompt 3: Quantity/Set System

```markdown
# Task: Implement Constraint Domain Types in Rust

Port the faebryk set/domain system for constraint solving.

## Requirements
1. `Quantity` type with unit tracking (use `uom` crate)
2. `Interval<T>` for continuous ranges (e.g., 5V to 6V)
3. `DisjointIntervals<T>` for unions of intervals
4. `BilateralTolerance` (e.g., 10kohm +/- 5%)
5. Set operations: union, intersection, contains, is_subset
6. Serde serialization for Python interop

## Reference Implementation
See: src/faebryk/libs/sets/quantity_sets.py

## Structure
- `crates/ato-domain/src/lib.rs`
- `crates/ato-domain/src/quantity.rs` - Physical quantities with units
- `crates/ato-domain/src/interval.rs` - Interval types
- `crates/ato-domain/src/sets.rs` - Set operations

## Tests Must Pass
- `cargo test -p ato-domain`
- Property tests for set algebra laws
- All Python test cases ported and passing

## Completion Criteria
Output <promise>DOMAIN_COMPLETE</promise> when:
- All types implemented
- Set algebra is correct (verified by property tests)
- Serialization works
```

### Prompt 4: Constraint Solver Core

```markdown
# Task: Implement Constraint Solver in Rust

Create the core constraint solver for Ato parameter resolution.

## Requirements
1. Expression types: `Parameter`, `Literal`, arithmetic operations, predicates
2. Predicate types: Is, LessOrEqual, GreaterOrEqual, IsSubset, Within
3. Solver with simplification pipeline:
   - Canonical form conversion
   - Structural simplifications (contradiction detection, transitive subset)
   - Expression-wise simplifications (constant folding, algebraic)
   - Fixed-point iteration until no changes
4. Contradiction detection with clear error messages
5. Timeout handling

## Reference Implementation
See: src/faebryk/core/solver/defaultsolver.py

## Structure
- `crates/ato-solver/src/lib.rs`
- `crates/ato-solver/src/expression.rs` - Expression types
- `crates/ato-solver/src/predicate.rs` - Constraint predicates
- `crates/ato-solver/src/simplify/*.rs` - Simplification passes
- `crates/ato-solver/src/solver.rs` - Main solver loop

## Tests Must Pass
- `cargo test -p ato-solver`
- Solve constraints from example projects
- Performance: 1000 constraints in < 100ms

## Completion Criteria
Output <promise>SOLVER_COMPLETE</promise> when:
- All predicate types work
- Simplification converges correctly
- Contradictions are detected with good errors
- Example constraints solve correctly
```

---

## Implementation Recommendations

### Crate Structure

```
crates/
├── ato-lexer/          # Tokenization
├── ato-parser/         # Parsing → AST
├── ato-ast/            # Shared AST types (if separated)
├── ato-domain/         # Quantity/interval/set types
├── ato-solver/         # Constraint solver
└── ato-py/             # PyO3 bindings for Python interop
```

### Key Crate Dependencies

```toml
[workspace.dependencies]
logos = "0.14"           # Fast lexer generator
chumsky = "0.9"          # Parser combinators (optional)
miette = "7"             # Beautiful error reporting
ariadne = "0.4"          # Alternative error reporting
serde = "1"              # Serialization
serde_json = "1"         # JSON output
uom = "0.36"             # Units of measure
proptest = "1"           # Property-based testing
pyo3 = "0.21"            # Python bindings
```

### Python Integration Strategy

1. **Start with JSON IPC**: Rust CLI outputs JSON, Python reads it
2. **Graduate to PyO3**: Direct Python bindings when stable
3. **Incremental replacement**: Each Rust crate can replace its Python equivalent one at a time

### Testing Strategy

1. **Unit tests**: Per-function correctness
2. **Property tests**: Set algebra laws, parser round-trips
3. **Golden tests**: Parse all existing .ato files, compare output
4. **Benchmark tests**: Ensure performance targets are met

---

## Key Files to Reference

### Grammar
- `src/atopile/parser/AtoLexer.g4` - Token definitions
- `src/atopile/parser/AtoParser.g4` - Parser grammar

### Frontend
- `src/atopile/front_end.py` - DSL → Faebryk compiler (3127 lines)
- `src/atopile/parse.py` - ANTLR4 wrapper

### Solver
- `src/faebryk/core/parameter.py` - Parameter/Expression types (1947 lines)
- `src/faebryk/core/solver/defaultsolver.py` - Main solver (471 lines)
- `src/faebryk/core/solver/symbolic/` - Simplification algorithms

### Domain Types
- `src/faebryk/libs/sets/quantity_sets.py` - Physical quantity intervals
- `src/faebryk/libs/sets/sets.py` - Generic set operations

---

## Success Metrics

| Component | Metric | Target |
|-----------|--------|--------|
| Lexer | Throughput | 100k lines/sec |
| Parser | Throughput | 50k lines/sec |
| Solver | 1000 constraints | < 100ms |
| Memory | Large project | < 100MB |
| Errors | Quality | Source spans, suggestions |

---

## Next Steps

1. Create the `crates/` workspace structure
2. Start with `ato-lexer` using Ralph loop
3. Validate against existing .ato files
4. Proceed to parser, then solver
5. Add PyO3 bindings for Python integration
