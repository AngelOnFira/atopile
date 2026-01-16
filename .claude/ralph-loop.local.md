---
active: true
iteration: 2
max_iterations: 50
completion_promise: "PARSER_COMPLETE"
started_at: "2026-01-16T17:12:45Z"
---

Implement Ato Parser in Rust. Create crates/ato-parser that depends on ato-lexer. Define complete AST types with serde serialization. Implement recursive descent parser following src/atopile/parser/AtoParser.g4 grammar. Use miette crate for error messages with source spans. Handle all statement types: imports, block definitions (module/component/interface), declarations, assignments, connections, retype, assertions, for loops, traits, new expressions, pragmas. Support all expression types: arithmetic, literals with units, field references, ranges. Structure: Cargo.toml, src/lib.rs, src/ast.rs, src/error.rs, src/parse/mod.rs with expr.rs stmt.rs types.rs, and tests/. Tests must pass and parser must work on example .ato files. After completion: git add, commit, push to rust-test, then output the completion promise. Output PARSER_COMPLETE when: full grammar coverage, all tests pass, good error messages, parses examples, committed and pushed.
