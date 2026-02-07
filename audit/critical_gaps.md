# Rust Atopile Implementation: Critical Gap Audit

**Branch:** `rust-test` (21 commits)
**Audit Date:** 2026-02-07
**Scope:** ato-sema, ato-solver, ato-export, ato-parts, ato-cli/build pipeline

---

## CRITICAL -- Will Produce Wrong PCBs or Fail Silently

### C1. Non-physical assignments silently dropped (ato-sema/lower.rs:429-438)

**Impact:** Assignments like `lcsc = "C123456"`, `package = "0402"`, `designator_prefix = "R"`, or arithmetic expressions (`voltage = vref * 2`) are silently discarded. Only bare physical literals (`10kohm`) and literals wrapped in trivial `Arithmetic(Literal(Physical(...)))` are lowered. This means:
- String parameter assignments (`lcsc`, `package`, `footprint`) never reach the solver or netlist builder
- Computed parameter values (e.g. `voltage = vin / (r1 + r2) * r2`) are lost
- Boolean assignments are lost

```rust
// lower.rs:429-438
let value_literal = match &assign.value {
    Assignable::Physical(phys) => self.lower_physical_literal(phys),
    Assignable::Arithmetic(expr) => {
        if let Expression::Literal(Literal::Physical(phys)) = expr {
            self.lower_physical_literal(phys)
        } else {
            return;  // <-- SILENT DROP of all arithmetic expressions
        }
    }
    _ => return,  // <-- SILENT DROP of strings, booleans
};
```

**Workaround in place:** The build pipeline in `build.rs` handles `lcsc` and `package` through a separate path (trait-based lookups and solved_values HashMap), but this is fragile and doesn't cover user-defined string parameters.

---

### C2. Function calls silently lowered to `false` (ato-sema/lower.rs:563-564)

**Impact:** Any `name(args)` expression in an assignment or constraint is silently replaced with `Bool(false)`. In ato, the "functional" grammar rule includes calls like `abs(x)`. If users write constraints involving `abs()`, the constraint will evaluate against `false` instead of the correct value.

```rust
// lower.rs:563-564
Expression::FunctionCall(_) => {
    ValueExpr::literal(ValueLiteral::Bool(false))
}
```

---

### C3. Constraint collector only resolves top-level field units (ato-sema/constraint_collector.rs:276-298)

**Impact:** For a constraint like `assert r1.resistance within 10kohm +/- 10%`, the unit resolution only looks at the *first* part of the path (`r1`). Since `r1` is an Instance, not a Parameter, it won't find a unit and defaults to `Dimensionless`. This causes the constraint to be created with the wrong unit, potentially making the solver produce incorrect results or fail to narrow parameters.

```rust
// constraint_collector.rs:282-289 -- only checks first path part
if let Some(first_part) = path.parts.first() {
    if let FieldPathPart::Name(name) = first_part {
        if let Some(field_id) = module.get_field(name) {
            if let Some(field) = design.get_field(field_id) {
                if let ato_ir::FieldKind::Parameter { unit: Some(ref unit_str) } = field.kind {
                    return self.parse_unit(unit_str);
                }
            }
        }
    }
}
// Default to dimensionless if we can't resolve
Ok(Unit::Dimensionless)  // <-- Wrong for nested paths like r1.resistance
```

---

### C4. Dependency tracking in constraint collector is shallow (ato-sema/constraint_collector.rs:514-527)

**Impact:** `extract_parameters()` only extracts direct parameter references, not parameters nested inside arithmetic expressions. The comment even says "This is simplified - a full implementation would traverse the expression tree". This means the dependency graph is incomplete, which could cause the solver to miss constraint propagation paths.

```rust
// constraint_collector.rs:514-527
fn extract_parameters(&self, expr_id: ExpressionId) -> Vec<ParameterId> {
    let mut params = Vec::new();
    if let Some(expr) = self.solver.get_expression(expr_id) {
        if let Some(param_id) = expr.as_parameter() {
            params.push(param_id);
        }
        // For arithmetic expressions, we'd need to recursively extract
        // This is simplified - a full implementation would traverse the expression tree
    }
    params
}
```

---

### C5. Part picker parameter lookup is disconnected from solver (ato-cli/build.rs:707-742)

**Impact:** The part picker searches `solved_params` for keys like `"r1.resistance"`, but the constraint collector creates parameters with keys like `"resistance"` (module-relative paths). Unless the solver output coincidentally matches the expected path format, passive components won't get parts picked. The fallback `solved_params.get(param_name)` would match the first parameter of that name, not the correct instance.

```rust
// build.rs:715-717
let param_key = format!("{}.{}", instance_name, param_name);
let value_str = solved_params.get(&param_key)
    .or_else(|| solved_params.get(param_name));  // Ambiguous for multiple passives
```

---

### C6. Type checker is a permissive stub (ato-sema/types.rs:186-192, 462-466)

**Impact:** Sub-field resolution failures and incomplete type info are silently skipped (line 186-192). The `get_module_interface()` function always returns `None` (line 462-466), meaning interface compatibility is never actually checked. Users can connect incompatible interface types (e.g., I2C to SPI) without any error.

```rust
// types.rs:186-192
FieldRefResolution::SubFieldNotFound(_) | FieldRefResolution::TypeInfoIncomplete => {
    // Skip silently for now since we can't distinguish these cases.
}

// types.rs:462-466
pub fn get_module_interface(_design: &Design, _module_id: ModuleId) -> Option<String> {
    None  // <-- Always returns None, no interface checking
}
```

---

## MAJOR -- Affects Correctness for Common Use Cases

### M1. Imported module lowerer errors silently discarded (ato-sema/analyzer.rs:680-681)

**Impact:** When processing imported modules (e.g., from packages), any errors during connection/constraint lowering are silently discarded. This means broken connections in imported packages won't surface as errors.

```rust
// analyzer.rs:680-681
// Ignore lowerer errors for imported modules (non-critical)
let _ = lowerer.take_errors();
```

---

### M2. Power operation not folded in constant_fold.rs (ato-solver/simplify/constant_fold.rs:124)

**Impact:** Expressions using the power operator (`x**2`, `2**n`) will never be simplified. The fold_arithmetic function returns `None` for `ArithmeticOp::Power`, leaving these expressions unevaluated. This affects voltage divider calculations and other common electronics formulas.

```rust
// constant_fold.rs:124 (in fold_arithmetic match)
_ => None,  // Power not handled
```

---

### M3. `PartDatabase::query()` creates a new client per call (ato-parts/lcsc.rs:174-178)

**Impact:** The `PartDatabase` trait requires `&self` (immutable), but `LcscClient` needs `&mut self` for rate limiting. The workaround creates a brand new `LcscClient` for every query call, discarding rate-limit state. Under concurrent or rapid-fire queries, this could cause rate limiting violations or excessive API calls.

```rust
// lcsc.rs:174-178
impl PartDatabase for LcscClient {
    fn query(&self, query: &PartQuery) -> DatabaseResult<Vec<Part>> {
        let mut client = LcscClient::new()?;  // New client, no rate limit memory
        client.query_parts(query)
    }
```

---

### M4. Netlist builder falls back to empty netlist on error (ato-cli/build.rs:416-424)

**Impact:** If the netlist builder fails for any reason, the build continues with an *empty* netlist and writes that to disk. The user sees files that look valid but contain no components or nets. The warning is only printed in verbose mode.

```rust
// build.rs:416-424
let netlist = match builder.build() {
    Ok(netlist) => netlist,
    Err(e) => {
        if verbose {
            println!("    Warning: Failed to build netlist: {}", e);
        }
        ato_export::Netlist::new()  // <-- Silent empty netlist
    }
};
```

---

### M5. Solver timeout is treated as a warning, not an error (ato-cli/build.rs:318-322)

**Impact:** When the solver times out (after 1000 iterations or 30 seconds), the build continues with whatever partial results were computed. Unsatisfied constraints are not surfaced as errors unless verbose mode is enabled. The resulting design may have under-constrained parameters.

```rust
// build.rs:318-322
Err(SolverError::Timeout { iterations, elapsed }) => {
    finish_spinner(&spinner, &format!(
        "Solver timed out after {} iterations ({:?})", iterations, elapsed
    ));
}
// Build continues with partial results...
```

---

### M6. `imports.rs:319` -- resolve_file result silently discarded

**Impact:** When loading and resolving an imported file, the result of `resolve_file()` is silently ignored with `let _`. If the file parses but has resolution errors internally, those errors are lost.

```rust
// imports.rs:319
let _ = self.resolve_file(&source, &import_path);
```

---

### M7. Part picker top-level key clobbering (ato-cli/build.rs:780-802)

**Impact:** When picking parts for multiple passive instances, the code stores both instance-specific keys AND top-level keys (`"lcsc"`, `"footprint"`). With multiple passives, each overwrites the previous top-level entry, so the last picked part's LCSC/footprint "wins" for any code reading the top-level key.

```rust
// build.rs:780-781
result.insert(format!("{}.lcsc", instance_name), lcsc.to_string());
result.insert("lcsc".to_string(), lcsc.to_string());  // <-- Overwrites on each iteration
```

---

### M8. Constraint collection error is non-fatal (ato-cli/build.rs:329-332)

**Impact:** If constraint collection fails (e.g., unsupported expression types), the error is only shown as a spinner message and the build continues without any constraint solving. All parameters will be unconstrained.

```rust
// build.rs:329-332
Err(e) => {
    finish_spinner(&spinner, &format!("Constraint collection error: {}", e));
}
// Build continues with no constraints...
```

---

## MINOR -- Edge Cases and Polish

### m1. Bitwise operations in constraints raise error instead of lowering (ato-sema/constraint_collector.rs:450-453)

**Impact:** `BitOr` and `BitAnd` operations (from set assignments `|=`, `&=`) in constraints return `UnsupportedExpression` error. These operations are valid in ato for flag manipulation but would cause the entire constraint collection to fail.

---

### m2. `in` keyword conflict (ato-lexer)

**Impact:** The `in` keyword (used for `for...in` and `within` assertions) conflicts with `in` used as an identifier name in hardware designs (e.g., signal names like `in`, `input`). This causes parse failures on valid real-world .ato files. 82/83 syntax tests pass; 1 fails due to this.

---

### m3. Passive component classification only checks module name (ato-cli/build.rs:546-564)

**Impact:** `classify_passive()` walks the inheritance chain checking if any ancestor's name is literally "Resistor", "Capacitor", or "Inductor". If a package defines a different name (e.g., `component MyResistor from SomethingElse`) that doesn't include "Resistor" in the chain, it won't be detected as a passive for part picking.

---

### m4. No dimensional analysis validation (ato-solver)

**Impact:** The solver does not validate dimensional consistency of arithmetic. You can add Volts to Ohms without any error. While the constant folder handles some unit computation (V/A = Ohm), there's no enforcement of dimensional correctness.

---

### m5. KiCad netlist format uses non-standard S-expression quoting (ato-export/kicad.rs)

**Impact:** The KiCad netlist exporter quotes values with `"` around numeric fields like `code` and `tstamp`. Some KiCad versions may not parse these correctly as they expect unquoted integers in those fields.

```rust
// kicad.rs:143-147
writeln!(writer, "    (net (code \"{}\") (name \"{}\")", code, escape_string(&net.name))?;
// KiCad expects: (net (code 1) (name "VCC")
```

---

### m6. No validation of package/footprint compatibility

**Impact:** When the part picker assigns a KiCad footprint suffix based on package name (e.g., "0402" -> "0402_1005Metric"), there's no validation that the footprint actually exists in the user's KiCad library. Non-standard package names pass through as-is.

---

### m7. Solved parameter display name may not match part picker lookup path

**Impact:** Parameters in the solver use `Parameter::display_name()` as their HashMap key when extracting solved values. If this name doesn't match the `instance_name.param_name` format the part picker expects, the lookup fails silently.

---

### m8. parse_parameter_range applies 5% tolerance to singleton values (ato-cli/build.rs:604)

**Impact:** When a parameter resolves to a single value (e.g., `"10000 ohm"`), the parser applies an artificial +/- 5% tolerance window. This is a reasonable heuristic but may be surprising -- it means `resistance = 10kohm` becomes a search for 9.5k-10.5k range.

---

### m9. No unsatisfiable constraint error reporting

**Impact:** When `result.all_satisfied` is false after solving, the unsatisfied constraints are counted but not reported in detail. The user sees "N constraint(s) not fully deduced" only in verbose mode, with no indication of which constraints or which parameters are problematic.

---

### m10. Cache silently removes stale entries (ato-parts/cache.rs:155)

**Impact:** When a cached part is expired, `remove()` errors are silently ignored with `let _`. If the cache database is corrupted, this silently loses data.

---

### m11. `unwrap_or_default()` used extensively in name resolution and analyzer

**Impact:** Over 30 instances of `unwrap_or_default()` across `names.rs`, `analyzer.rs`, and `types.rs`. Each one silently converts a missing/failed lookup into an empty string or empty collection. While individually non-critical, the cumulative effect makes debugging very difficult -- errors in name resolution cascade into empty strings that propagate silently.

Key locations:
- `analyzer.rs:122` -- current directory defaults to empty PathBuf
- `names.rs:158,176,199,338,372,527,537` -- field lookups default to empty
- `types.rs:183,370,380,431` -- type resolution defaults to empty

---

## Architecture Observations

### A1. Sema-to-Solver bridge is the weakest link

The `constraint_collector.rs` is the narrowest bottleneck in the pipeline. It creates solver parameters per-module rather than per-instance, doesn't follow nested field paths for unit resolution, and has incomplete dependency tracking. Most of the "works for simple cases, breaks for real designs" issues trace back to this file.

### A2. Build pipeline error handling philosophy is too lenient

The build pipeline in `build.rs` treats almost every error as a warning and continues. While this provides a "best effort" output, it means users can get empty or incorrect netlists without realizing something went wrong. A production compiler should fail loudly on constraint collection errors, solver contradictions, and netlist building failures.

### A3. Part picking is end-to-end functional but fragile

The LCSC client, caching, query building, and selection logic are well-structured. The fragility comes from the disconnect between how the solver names parameters and how the part picker looks them up (see C5). With consistent parameter naming, the part picking pipeline would work correctly.

### A4. No integration between solver results and netlist builder

Solved parameter values are passed to the netlist builder as a flat `HashMap<String, String>`, but the netlist builder primarily uses the IR Design for component/connection information. There's no mechanism for solver results (like narrowed resistance values) to flow into component properties in the netlist. The BOM gets the right values only through the part picker's separate path.
