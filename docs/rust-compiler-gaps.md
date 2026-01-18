# Rust Compiler Gaps - TDD Fixing Plan

This document identifies critical gaps in the Rust atopile compiler that prevent it from building real projects. Each gap includes failing test cases and a Ralph prompt for fixing it.

---

## Gap 1: Instance Field Expansion

**Status:** NOT IMPLEMENTED
**Severity:** CRITICAL
**Files:** `crates/ato-sema/src/names.rs`, `crates/ato-ir/src/field.rs`

### Problem
When `r1 = new Resistor` is processed, the instance field is created with a type reference but the fields from `Resistor` (resistance, p1, p2, etc.) are NOT copied/expanded into the instance. This means `r1.resistance` cannot be resolved.

### Failing Test Case
```rust
// File: crates/ato-sema/src/names.rs (add to tests module)
#[test]
fn test_instance_field_expansion() {
    let source = r#"
module Inner:
    value: ohm
    pin p1

module Outer:
    inner = new Inner
"#;
    let mut analyzer = Analyzer::new();
    let result = analyzer.analyze_source(source);
    assert!(result.is_ok());

    let design = result.unwrap();
    // Find the Outer module
    let outer = design.modules().find(|m| m.name == "Outer").unwrap();

    // The 'inner' field should exist
    let inner_field_id = outer.get_field("inner").expect("inner field should exist");
    let inner_field = design.get_field(inner_field_id).unwrap();

    // The inner field should have resolved_type pointing to Inner module
    if let FieldKind::Instance { resolved_type, .. } = &inner_field.kind {
        assert!(resolved_type.is_some(), "Instance should have resolved type");
    } else {
        panic!("Expected Instance field kind");
    }

    // Critical: We should be able to access inner.value through the type
    // This requires either field expansion OR proper nested field resolution
}
```

### Ralph Prompt
```
Instance Field Expansion (Gap 1). When `x = new Type` is processed, the instance's type must be resolved and nested field access must work.

Current state: `field_kind_from_new` in names.rs creates Instance fields but ignores the scope parameter, leaving resolved_type as None. Post-creation resolution attempts to fix this but is fragile.

Requirements:
1) Fix `field_kind_from_new` to use scope and resolve type immediately
2) Ensure resolved_type is populated for all Instance fields
3) Nested field access like `x.field` must resolve through the instance's type
4) Handle instance arrays (`new Type[n]`) correctly

Key files: crates/ato-sema/src/names.rs (lines 354-388), crates/ato-sema/src/types.rs (resolve_nested_field). Tests must pass: cargo test -p ato-sema test_instance_field_expansion.
```

---

## Gap 2: Assignment-to-Constraint Conversion

**Status:** NOT IMPLEMENTED
**Severity:** CRITICAL
**Files:** `crates/ato-sema/src/lower.rs`, `crates/ato-ir/src/field.rs`

### Problem
Assignment statements like `r1.resistance = 100ohm +/- 10%` are completely discarded. The value is examined to determine field kind but never stored or converted to a constraint.

### Failing Test Case
```rust
// File: crates/ato-sema/src/lower.rs (add to tests module)
#[test]
fn test_assignment_creates_constraint() {
    let source = r#"
module Resistor:
    resistance: ohm

module App:
    r1 = new Resistor
    r1.resistance = 100ohm +/- 10%
"#;
    let mut analyzer = Analyzer::new();
    let result = analyzer.analyze_source(source);
    assert!(result.is_ok());

    let design = result.unwrap();

    // The App module should have at least one constraint
    let app = design.modules().find(|m| m.name == "App").unwrap();
    assert!(
        app.constraints.len() > 0 || design.constraint_count() > 0,
        "Assignment should create a constraint, found {} constraints",
        design.constraint_count()
    );
}

#[test]
fn test_assignment_value_stored() {
    let source = r#"
module Test:
    value: ohm
    value = 100ohm
"#;
    let mut analyzer = Analyzer::new();
    let result = analyzer.analyze_source(source);
    assert!(result.is_ok());

    let design = result.unwrap();

    // Should have a constraint for "value = 100ohm" (as value IS 100ohm)
    assert!(design.constraint_count() > 0, "Simple assignment should create constraint");
}
```

### Ralph Prompt
```
Assignment-to-Constraint Conversion (Gap 2). Assignments like `x = 100ohm +/- 10%` must create constraints in the IR.

Current state: In lower.rs, Statement::Assignment falls into the catch-all `_ => {}` case and is ignored. In names.rs, assignment values are examined for field kind detection but then discarded.

Requirements:
1) In lower.rs, handle Statement::Assignment in lower_block_statement
2) Convert assignments with physical values to Constraint with CompareOpKind::Is or Within
3) For bilateral tolerances (100ohm +/- 10%), use CompareOpKind::Within
4) Store the constraint in the module's constraints list
5) Handle nested field assignments like `r1.resistance = ...`

Key files: crates/ato-sema/src/lower.rs (lines 90-113), crates/ato-ir/src/constraint.rs. Tests must pass: cargo test -p ato-sema test_assignment_creates_constraint.
```

---

## Gap 3: Import Resolution and Stdlib Loading

**Status:** PARTIALLY IMPLEMENTED
**Severity:** HIGH
**Files:** `crates/ato-sema/src/analyzer.rs`, `crates/ato-sema/src/resolution.rs`

### Problem
The stdlib is indexed but not merged into the main scope. Imported types are not available when processing `new TypeName` expressions.

### Failing Test Case
```rust
// File: crates/ato-sema/src/analyzer.rs (add to tests module)
#[test]
fn test_stdlib_resistor_available() {
    let source = r#"
import Resistor

module App:
    r1 = new Resistor
    r1.resistance = 10kohm +/- 10%
"#;
    let mut analyzer = Analyzer::new()
        .with_stdlib(PathBuf::from("stdlib"));

    let result = analyzer.analyze_source(source);
    assert!(result.is_ok(), "Stdlib import should work: {:?}", result.err());

    let design = result.unwrap();

    // Resistor module should be in the design
    assert!(
        design.modules().any(|m| m.name == "Resistor"),
        "Resistor module should be loaded from stdlib"
    );

    // App.r1 should have resolved_type pointing to Resistor
    let app = design.modules().find(|m| m.name == "App").unwrap();
    let r1_id = app.get_field("r1").expect("r1 should exist");
    let r1 = design.get_field(r1_id).unwrap();

    if let FieldKind::Instance { resolved_type, .. } = &r1.kind {
        assert!(resolved_type.is_some(), "r1 should have resolved type");
    }
}

#[test]
fn test_import_from_file() {
    // Create a temp file with a module, then import it
    let lib_source = r#"
module MyLib:
    value: ohm
"#;
    let main_source = r#"
from "lib.ato" import MyLib

module App:
    x = new MyLib
"#;
    // This requires file-based test infrastructure
    // The test should verify that MyLib is available in App's scope
}
```

### Ralph Prompt
```
Import Resolution and Stdlib Loading (Gap 3). Imports must load modules and make them available for `new` expressions.

Current state: analyzer.rs indexes stdlib (line 142) but never merges the registry into the scope. Imported types may not be available when field_kind_from_new runs.

Requirements:
1) After indexing stdlib, add stdlib modules to the initial scope
2) When processing imports, load the file and add modules to scope BEFORE name resolution
3) Ensure imported modules are available when processing `new ImportedType`
4) Handle transitive imports (imported module imports another module)
5) Track import errors properly

Key files: crates/ato-sema/src/analyzer.rs (lines 142-267), crates/ato-sema/src/resolution.rs. Tests must pass: cargo test -p ato-sema test_stdlib_resistor_available.
```

---

## Gap 4: Netlist Component Extraction

**Status:** INCORRECT IMPLEMENTATION
**Severity:** CRITICAL
**Files:** `crates/ato-export/src/netlist.rs`

### Problem
NetlistBuilder treats ALL modules with pins as components, instead of only extracting actual instances. This creates phantom components and misses the real ones.

### Failing Test Case
```rust
// File: crates/ato-export/src/netlist.rs (add to tests module)
#[test]
fn test_netlist_extracts_instances_not_definitions() {
    // Create a design with:
    // - Resistor (module definition with pins) - should NOT be a component
    // - App containing r1 = new Resistor - r1 SHOULD be a component

    let mut design = Design::new();

    // Create Resistor module (definition only)
    let resistor_id = design.create_module("Resistor", ModuleKind::Component);
    design.add_field(resistor_id, "p1", FieldKind::pin("1"));
    design.add_field(resistor_id, "p2", FieldKind::pin("2"));
    design.add_field(resistor_id, "resistance", FieldKind::parameter_with_unit("ohm"));

    // Create App module with instance
    let app_id = design.create_module("App", ModuleKind::Module);
    let r1_kind = FieldKind::Instance {
        type_ref: QualifiedName::simple("Resistor"),
        count: None,
        resolved_type: Some(resistor_id),
    };
    design.add_field(app_id, "r1", r1_kind);

    // Build netlist
    let builder = NetlistBuilder::new(&design);
    let netlist = builder.build().unwrap();

    // Should have exactly 1 component (r1), not 2
    assert_eq!(
        netlist.component_count(), 1,
        "Should only have instance components, not module definitions"
    );

    // The component should be named R1 (from r1 instance)
    assert!(
        netlist.get_component("R1").is_some(),
        "Instance r1 should become component R1"
    );
}

#[test]
fn test_netlist_extracts_connections_through_instances() {
    let mut design = Design::new();

    // Create Resistor with pins
    let resistor_id = design.create_module("Resistor", ModuleKind::Component);
    let p1_id = design.add_field(resistor_id, "p1", FieldKind::pin("1"));
    let p2_id = design.add_field(resistor_id, "p2", FieldKind::pin("2"));

    // Create App with two resistor instances connected
    let app_id = design.create_module("App", ModuleKind::Module);
    let r1_kind = FieldKind::Instance {
        type_ref: QualifiedName::simple("Resistor"),
        count: None,
        resolved_type: Some(resistor_id),
    };
    let r2_kind = r1_kind.clone();
    let r1_id = design.add_field(app_id, "r1", r1_kind);
    let r2_id = design.add_field(app_id, "r2", r2_kind);

    // Connect r1.p2 ~ r2.p1 (would need proper connection representation)
    // design.add_connection(app_id, ...);

    let builder = NetlistBuilder::new(&design);
    let netlist = builder.build().unwrap();

    // Should have 2 components
    assert_eq!(netlist.component_count(), 2);

    // Should have a net connecting R1.p2 to R2.p1
    // (This test documents the expected behavior)
}
```

### Ralph Prompt
```
Netlist Component Extraction (Gap 4). NetlistBuilder must extract component instances, not module definitions.

Current state: collect_components() in netlist.rs iterates design.modules() and creates components for ANY module with pins. This creates phantom components for abstract modules and misses actual instances.

Requirements:
1) Start from entry module and traverse instance fields
2) For each Instance field with resolved_type, create a component
3) Track instance path for proper reference naming (r1, divider.r1, etc.)
4) Extract footprint from module properties/traits
5) Build nets by following connections through instance boundaries
6) Map instance.pin references to component.pin in netlist

Key files: crates/ato-export/src/netlist.rs (lines 240-356). Tests must pass: cargo test -p ato-export test_netlist_extracts_instances_not_definitions.
```

---

## Gap 5: Nested Field Access in Constraints

**Status:** PARTIALLY IMPLEMENTED
**Severity:** HIGH
**Files:** `crates/ato-sema/src/constraint_collector.rs`, `crates/ato-sema/src/types.rs`

### Problem
The constraint collector cannot resolve nested field paths like `r1.resistance` because it only looks at the first part of the path.

### Failing Test Case
```rust
// File: crates/ato-sema/src/constraint_collector.rs (add to tests module)
#[test]
fn test_collect_nested_field_constraint() {
    let source = r#"
module Inner:
    value: ohm

module Outer:
    inner = new Inner
    assert inner.value > 0ohm
"#;
    let mut analyzer = Analyzer::new();
    let result = analyzer.analyze_source(source);
    assert!(result.is_ok(), "Should parse: {:?}", result.err());

    let design = result.unwrap();

    // Collect constraints
    let collector = ConstraintCollector::new();
    let result = collector.collect(&design);

    assert!(result.is_ok(), "Should collect constraints: {:?}", result.err());

    let (solver, deps) = result.unwrap();

    // Should have found the inner.value parameter
    assert!(
        deps.all_parameters().iter().any(|p| p.path.to_string().contains("inner.value")),
        "Should find nested parameter inner.value"
    );
}
```

### Ralph Prompt
```
Nested Field Access in Constraints (Gap 5). Constraint collector must resolve paths like `instance.field` through instance types.

Current state: resolve_field_unit() in constraint_collector.rs only looks at the first part of a path. For `r1.resistance`, it finds `r1` but cannot traverse into its type to find `resistance`.

Requirements:
1) In constraint_collector.rs, extend resolve_field_unit to handle multi-part paths
2) When path[0] is an Instance field, get its resolved_type and look up path[1] in that module
3) Continue recursively for deeper paths (a.b.c)
4) Track the full path for constraint variable naming
5) Handle array instances (r[0].value)

Key files: crates/ato-sema/src/constraint_collector.rs (resolve_field_unit around line 277), crates/ato-sema/src/types.rs (resolve_nested_field). Tests must pass: cargo test -p ato-sema test_collect_nested_field_constraint.
```

---

## Gap 6: Connection Lowering for Instance Pins

**Status:** PARTIALLY IMPLEMENTED
**Severity:** HIGH
**Files:** `crates/ato-sema/src/lower.rs`

### Problem
Connections like `r1.p1 ~ r2.p2` are not properly resolved because the lowerer doesn't traverse through instance fields to find the actual pins.

### Failing Test Case
```rust
// File: crates/ato-sema/src/lower.rs (add to tests module)
#[test]
fn test_lower_instance_pin_connection() {
    let source = r#"
module Resistor:
    pin p1
    pin p2

module App:
    r1 = new Resistor
    r2 = new Resistor
    r1.p2 ~ r2.p1
"#;
    let mut analyzer = Analyzer::new();
    let result = analyzer.analyze_source(source);
    assert!(result.is_ok(), "Should analyze: {:?}", result.err());

    let design = result.unwrap();

    // App should have a connection
    let app = design.modules().find(|m| m.name == "App").unwrap();
    assert!(
        !app.connections.is_empty(),
        "App should have connection between r1.p2 and r2.p1"
    );

    // The connection endpoints should reference instance.pin paths
    let conn = design.get_connection(app.connections[0]).unwrap();
    // Verify the endpoints are properly formed
}
```

### Ralph Prompt
```
Connection Lowering for Instance Pins (Gap 6). Connections between instance pins must be properly lowered.

Current state: lower_connectable() creates FieldPath but doesn't verify that nested paths (r1.p1) are valid or that they reference actual pins through instance types.

Requirements:
1) When lowering a connection like `r1.p2 ~ r2.p1`, verify r1 is an instance
2) Resolve r1's type and verify p2 exists as a pin in that type
3) Create ConnectionEndpoint with proper FieldPath representing instance.pin
4) Handle inline signal definitions in connections
5) Track connection span for error reporting

Key files: crates/ato-sema/src/lower.rs (lower_connectable around line 145). Tests must pass: cargo test -p ato-sema test_lower_instance_pin_connection.
```

---

## Summary: Priority Order for Fixing

| Priority | Gap | Ralph Prompt Title | Estimated Complexity |
|----------|-----|-------------------|---------------------|
| 1 | Gap 1 | Instance Field Expansion | High |
| 2 | Gap 2 | Assignment-to-Constraint Conversion | Medium |
| 3 | Gap 3 | Import Resolution and Stdlib Loading | High |
| 4 | Gap 5 | Nested Field Access in Constraints | Medium |
| 5 | Gap 6 | Connection Lowering for Instance Pins | Medium |
| 6 | Gap 4 | Netlist Component Extraction | High |

**Recommended approach:** Fix Gaps 1-3 first as they are foundational. Then fix Gaps 5-6 for constraint and connection handling. Finally fix Gap 4 for netlist generation.

---

## Running the Tests

To verify fixes, run:

```bash
# Test specific gap
cargo test -p ato-sema test_instance_field_expansion
cargo test -p ato-sema test_assignment_creates_constraint
cargo test -p ato-sema test_stdlib_resistor_available
cargo test -p ato-export test_netlist_extracts_instances

# Test all
cargo test -p ato-sema -p ato-export
```
