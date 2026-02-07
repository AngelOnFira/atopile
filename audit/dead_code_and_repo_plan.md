# Dead Code Audit & Standalone Repository Plan

## Part 1: Dead Code Audit

### 1.1 Compiler-Flagged Dead Code (cargo build warnings)

The Rust compiler reports **16 warnings** across 4 crates. Here is a categorized analysis.

---

#### Category 1: Dead Code to Remove

These items are definitively unused and can be safely deleted.

| Location | Item | Reason |
|----------|------|--------|
| `ato-domain/src/unit.rs:183` | `pub fn parse_si_prefix()` | Never called. Redundant with the parser's own quantity parsing. 65 lines. |
| `ato-sema/src/names.rs:605` | `pub fn resolve_field_ref()` | Free function never called. `types.rs` has its own method-based `resolve_field_ref()`. |
| `ato-sema/src/names.rs:613` | `pub fn resolve_connectable()` | Only caller was `resolve_field_ref` above. Both can go. |
| `ato-sema/src/types.rs:462` | `pub fn get_module_interface()` | Stub that always returns `None`. Never called from any crate. |
| `ato-sema/src/types.rs:25` | `SubFieldNotFound(String)` field | The variant is used but the `String` payload is never read. Change to `SubFieldNotFound` (unit variant). |
| `ato-sema/src/error.rs:254,259` | `ErrorCollector::len()` and `is_empty()` | Only used in `#[cfg(test)]` blocks. Mark with `#[cfg(test)]` or remove. `has_errors()` is the non-test API. |
| `ato-export/src/kicad_schematic.rs:466` | `fn get_lib_id_for_component()` | Wrapper function never called. The `LibraryMapper` it wraps is used directly elsewhere. |
| `ato-cli/src/error.rs:51` | `CliError::MultipleErrors` variant | Never constructed. Remove the variant. |

**Estimated removable code: ~120 lines**

---

#### Category 2: Unused Variables (Trivial Fixes)

| Location | Item | Fix |
|----------|------|-----|
| `ato-sema/src/lower.rs:751` | `scope` parameter | Prefix with `_scope` |
| `ato-cli/src/commands/parse.rs:81` | `assign` variable | Prefix with `_assign` |
| `ato-cli/src/commands/parse.rs:84,87` | `conn` variables (x2) | Prefix with `_conn` |

---

#### Category 3: Superseded Module (imports.rs)

**`ato-sema/src/imports.rs`** - This is the most significant dead code finding.

The entire `ImportResolver` struct and all its methods are dead outside of tests. The compiler flags:
- All 5 fields of `ImportResolver` (root_dir, files, processing, errors, loader)
- All 11 associated methods (new, with_loader, resolve, files, errors, take_errors, resolve_file, extract_imports, extract_import_stmt, extract_dep_import_stmt, resolve_import, resolve_import_path)
- `MockFileLoader` struct and its methods (new, add_file)

**What happened:** `resolution.rs` was written as a replacement for `imports.rs`. It now handles all file loading, import resolution, and dependency graphing. The only thing still used from `imports.rs` is:
- `ImportResolver::extract_imports_from_ast()` (static method)
- `FileLoader` trait + `FsFileLoader` + `ParsedFile` + `ImportInfo` types
- `MockFileLoader` (used in `resolution.rs` tests)

**Recommendation:** Extract the still-used types (`FileLoader`, `FsFileLoader`, `MockFileLoader`, `ParsedFile`, `ImportInfo`, `extract_imports_from_ast`) into a smaller module. Delete the `ImportResolver` struct and all its instance methods. This would remove ~200 lines of dead code while keeping the ~80 lines that are actually used.

---

#### Category 4: `#[allow(dead_code)]` Annotations

Five items are explicitly suppressing dead_code warnings:

| Location | Item | Assessment |
|----------|------|------------|
| `ato-sema/src/analyzer.rs:542` | `fn analyze_imported_module()` | Legacy wrapper, replaced by `analyze_imported_module_with_scope()`. **Remove.** |
| `ato-solver/src/simplify/mod.rs:205` | `pub fn try_evaluate_literal()` | Trivial helper (1 line body). Might be useful later but currently dead. **Remove.** |
| `ato-parts/src/lcsc.rs:364` | `LcscDetailResponse.msg` field | Serde deserialization field. Must exist for JSON parsing. **Keep** (this is correct usage of allow). |
| `ato-parser/src/chumsky/error.rs:63` | `ParseError::with_help()` | Builder method for error help text. Not currently used but reasonable API. **Keep or remove.** |
| `ato-export/src/netlist.rs:177` | `FootprintInfo.package` field | Set but never read. Might be needed for passive part picking. **Suspicious - see below.** |

---

#### Category 5: Suspicious Dead Code (Possible Bugs)

| Location | Item | Concern |
|----------|------|---------|
| `ato-export/src/netlist.rs:177` | `FootprintInfo.package` | This field is populated during netlist building but never used in output. The `package` parameter (e.g. "0402") should influence part selection or BOM output. This may indicate incomplete passive part picking integration. |

---

### 1.2 Unused Dependencies

| Crate | Dependency | Status | Impact |
|-------|-----------|--------|--------|
| `ato-domain` | `serde_json` | **Unused** - not referenced in any source file | Remove from Cargo.toml |
| `ato-domain` | `num-traits` | **Unused** - not referenced in any source file | Remove from Cargo.toml |
| `ato-solver` | `serde_json` | **Unused** - not referenced in any source file | Remove from Cargo.toml |
| `ato-sema` | `semver` | **Unused** - declared but never imported | Remove from Cargo.toml |
| `ato-sema` | `miette` | **Unused** - declared but never imported | Remove from Cargo.toml |
| `ato-cli` | `serde_yaml` | **Unused** - declared but never imported | Remove from Cargo.toml |

**Note on `serde_yaml`:** This crate is marked as **deprecated** upstream. The `ato-sema/packages.rs` module uses it for reading `ato.yaml` configs. Consider migrating to `serde_yml` (the maintained fork) when separating the repo.

---

### 1.3 Summary Statistics

- **Total Rust code:** ~37,700 lines across 66 source files in 10 crates
- **Dead code to remove:** ~320 lines (imports.rs bulk + scattered functions)
- **Unused dependencies to remove:** 6 entries across 4 Cargo.toml files
- **Trivial warning fixes:** 4 unused variable prefixes

---

## Part 2: Standalone Repository Plan

### 2.1 Current Repository Structure

The Rust code lives inside the Python atopile monorepo. Here's what belongs where:

#### Rust-Specific Files (would move to new repo)
```
Cargo.toml              # Workspace root
Cargo.lock              # Dependency lock
crates/                 # All 10 Rust crates
  ato-cli/              # Binary crate (the `ato` command)
  ato-domain/           # Quantity/interval/tolerance types
  ato-export/           # Netlist, BOM, KiCad output
  ato-ir/               # Intermediate representation
  ato-lexer/            # Tokenizer (logos-based)
  ato-parser/           # Parser (chumsky-based)
  ato-parts/            # JLCPCB/LCSC part database
  ato-sema/             # Semantic analysis + packages + embedded stdlib
  ato-solver/           # Constraint solver
  ato-tests/            # Integration tests
```

#### Shared Files (used by both Python and Rust)
```
examples/               # Used by Rust integration tests AND Python tests
  led_badge/            # Primary integration test target
  quickstart/           # Parse/sema test target
  equations/            # Parse/sema test target
  i2c/                  # Parse/sema test target
  ...
LICENSE                 # MIT - same for both
```

#### Python-Only Files (stay in original repo)
```
src/                    # Python source (atopile, faebryk, vscode-atopile)
pyproject.toml          # Python packaging
uv.lock                 # Python dependency lock
test/                   # Python tests
tools/                  # Python tooling
scripts/                # Python scripts
docs/                   # Python docs
dockerfiles/            # Docker configs
.github/workflows/      # All Python CI (pytest, deploy, etc.)
.pre-commit-config.yaml # Python pre-commit
.ruff_cache/            # Python linter cache
assets/                 # Python assets
artifacts/              # Build artifacts
```

### 2.2 Proposed New Repository Structure

```
atopile-rs/
|-- Cargo.toml           # Workspace manifest
|-- Cargo.lock           # Dependency lock
|-- LICENSE              # MIT
|-- README.md            # New Rust-focused README
|-- rust-toolchain.toml  # Pin Rust version (e.g. 1.82+, edition 2024)
|-- .cargo/
|   |-- config.toml      # Optional: cross-compilation settings
|-- .github/
|   |-- workflows/
|       |-- ci.yml        # Build + test + clippy + fmt
|       |-- release.yml   # Cross-platform binary releases
|-- crates/
|   |-- ato-cli/          # Binary: `ato`
|   |-- ato-domain/       # Domain types
|   |-- ato-export/       # Export formats
|   |-- ato-ir/           # IR
|   |-- ato-lexer/        # Lexer
|   |-- ato-parser/       # Parser
|   |-- ato-parts/        # Part database
|   |-- ato-sema/         # Semantic analysis
|   |   |-- stdlib/       # 52 embedded .ato files
|   |-- ato-solver/       # Constraint solver
|   |-- ato-tests/        # Integration tests
|-- examples/             # Copied from monorepo (test fixtures)
|   |-- led_badge/
|   |-- quickstart/
|   |-- equations/
|   |-- i2c/
|   |-- ...
|-- .gitignore            # Rust-focused (target/, *.swp, .DS_Store)
```

### 2.3 Crate Organization Recommendations

**Keep all 10 crates as-is.** The current factoring is clean:

```
ato-lexer      (0 internal deps)
    |
ato-parser     (depends on: ato-lexer)
    |
ato-domain     (0 internal deps)
    |
ato-ir         (depends on: ato-lexer, ato-parser, ato-domain)
    |
ato-solver     (depends on: ato-domain)
    |
ato-sema       (depends on: ato-lexer, ato-parser, ato-ir, ato-solver, ato-domain)
    |
ato-parts      (depends on: ato-domain)
    |
ato-export     (depends on: ato-ir)
    |
ato-cli        (depends on: all above)
    |
ato-tests      (depends on: most above, dev-only)
```

**Possible merges to consider (but not required):**
- `ato-lexer` + `ato-parser` could merge since the parser is the only consumer of the lexer. However, keeping them separate allows independent testing and cleaner crate boundaries.
- `ato-domain` could be absorbed into `ato-solver` since its primary consumer is the solver. But `ato-parts` and `ato-ir` also depend on it, so keeping it separate is cleaner.

**Recommendation: No merges.** The current structure is well-factored.

### 2.4 CI/CD Pipeline

#### GitHub Actions: ci.yml
```yaml
# Triggers: push to main, pull requests
jobs:
  check:
    # cargo fmt --check
    # cargo clippy --workspace -- -D warnings
    # cargo build --workspace
    # cargo test --workspace

  test-examples:
    # Run `ato install` + `ato build` on example projects
    # Compare output against golden files
```

#### GitHub Actions: release.yml
```yaml
# Trigger: tag push (v*)
# Matrix build:
#   - x86_64-unknown-linux-gnu
#   - x86_64-unknown-linux-musl (static)
#   - aarch64-unknown-linux-gnu
#   - x86_64-apple-darwin
#   - aarch64-apple-darwin
#   - x86_64-pc-windows-msvc
#
# Use: cross-rs or cargo-zigbuild for cross-compilation
# Output: GitHub Release with binary assets + SHA256 checksums
```

### 2.5 Release & Distribution Plan

#### Binary Distribution
1. **GitHub Releases** - Primary. Cross-compiled binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
2. **Homebrew tap** - `brew install atopile/tap/ato` for macOS/Linux
3. **cargo install** - `cargo install ato-cli` from crates.io (if desired)
4. **npm wrapper** - Optional: npm package that downloads the correct binary (like esbuild does)

#### crates.io Publishing
- Publish library crates: `ato-domain`, `ato-lexer`, `ato-parser`, `ato-ir`, `ato-solver`
- Publish binary crate: `ato-cli` (as `ato-cli`, binary name `ato`)
- `ato-sema` and `ato-parts` may need care around the embedded stdlib and network-dependent code
- `ato-tests` should NOT be published (it's `publish = false` already)

#### Embedded Stdlib Handling
The embedded stdlib (52 .ato files in `crates/ato-sema/stdlib/`) is compiled into the binary via `include_str!()`. This is self-contained and works perfectly for a standalone repo. No changes needed.

### 2.6 Dependency Audit

#### External Dependencies (27 unique crates)

| Dependency | Version | Purpose | Risk |
|-----------|---------|---------|------|
| `logos` | 0.15 | Lexer generator | Low - stable, well-maintained |
| `chumsky` | 0.9 | Parser combinator | Medium - v0.9 is stable but v1.0 has breaking changes |
| `ariadne` | 0.4 | Error formatting | Low - stable |
| `miette` | 7 | Error diagnostics | Low - well-maintained |
| `thiserror` | 2 | Error derive macro | Low - ubiquitous |
| `serde` | 1 | Serialization | Low - ubiquitous |
| `serde_json` | 1 | JSON | Low - ubiquitous |
| **`serde_yaml`** | **0.9** | **YAML parsing** | **Medium - DEPRECATED. Migrate to `serde_yml`** |
| `clap` | 4 | CLI args | Low - well-maintained |
| `reqwest` | 0.12 | HTTP client | Low - well-maintained, but adds TLS dep |
| `rusqlite` | 0.32 | SQLite (bundled) | Medium - bundles C code, increases binary size |
| `git2` | 0.20 | Git operations | Medium - bundles libgit2 C code |
| `uuid` | 1 | UUID generation | Low |
| `zip` | 2 | ZIP extraction | Low |
| `chrono` | 0.4 | Date/time | Low |
| `directories` | 5 | XDG dirs | Low |
| `semver` | 1 | Version parsing | Low - **but unused in ato-sema, remove** |
| `sha2` | 0.10 | SHA-256 hashing | Low |
| `hex` | 0.4 | Hex encoding | Low |
| `toml` | 0.8 | TOML parsing | Low |
| `num-traits` | 0.2 | Numeric traits | Low - **unused in ato-domain, remove** |
| `rayon` | 1.10 | Parallelism | Low |
| `indicatif` | 0.17 | Progress bars | Low |
| `console` | 0.15 | Terminal styling | Low |
| `insta` | 1 (dev) | Snapshot testing | Low |
| `walkdir` | 2 (dev) | Dir traversal | Low |
| `tempfile` | 3 (dev) | Temp files | Low |

#### Heavy Dependencies
The largest contributors to binary size and compile time:
1. **`rusqlite` (bundled)** - Compiles SQLite from C source. ~3MB binary contribution.
2. **`git2`** - Bundles libgit2. ~2MB binary contribution.
3. **`reqwest`** - Pulls in hyper + tokio + TLS. ~1.5MB binary contribution.

These are all justified (part database, package management, API calls) but are worth noting for compile time optimization.

#### Dependency Tree Size
- Total dependency tree: ~409 unique crates (including transitive)
- This is moderate for a Rust project with HTTP, SQLite, and Git functionality

### 2.7 Migration Steps

1. **Create new repo** `atopile/atopile-rs` (or `atopile/ato`)
2. **Copy Rust files:**
   - `Cargo.toml`, `Cargo.lock`
   - `crates/` directory
   - `examples/` directory (for test fixtures)
   - `LICENSE`
3. **Add new files:**
   - `README.md` (Rust-focused)
   - `rust-toolchain.toml`
   - `.github/workflows/ci.yml`
   - `.github/workflows/release.yml`
   - `.gitignore` (Rust-focused)
4. **Clean up dead code** (as identified in Part 1)
5. **Remove unused dependencies** (6 entries)
6. **Migrate `serde_yaml`** to `serde_yml`
7. **Set up CI** and verify all tests pass
8. **First release** with cross-compiled binaries
9. **Update monorepo** to remove Rust files and reference the new binary

### 2.8 Open Questions

1. **Repo naming:** `atopile-rs`, `ato-compiler`, `ato`, or something else?
2. **Package registry:** Should library crates be published to crates.io for third-party use, or is this purely an internal compiler?
3. **Stdlib source of truth:** Should the 52 stdlib `.ato` files live in the Rust repo or be fetched from the monorepo? Currently they're duplicated.
4. **Python CLI compatibility:** The Rust CLI at `/target/debug/ato` would conflict with the Python CLI at `~/.local/bin/ato`. Need a naming/installation strategy.
5. **Example projects:** Should examples be git submodules pointing to the monorepo, or copied independently?
