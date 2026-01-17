# Real-World Atopile Files Test Report

This report documents the Rust parser's compatibility with real-world atopile files.

## Summary

| Category | Tested | Passed | Failed | Pass Rate |
|----------|--------|--------|--------|-----------|
| Example Projects | 7 | 7 | 0 | **100%** |
| Standard Library | 10 | 10 | 0 | **100%** |
| Syntax Examples | 1 | 0 | 1 | 0% |
| **Total** | **18** | **17** | **1** | **94%** |

## Example Projects (7/7 ✅)

All example projects in `examples/` parse successfully:

| Example | Complexity | Statements | Features Used |
|---------|------------|------------|---------------|
| quickstart | Simple | 2 | imports, module, resistor |
| equations | Medium | 3 | parameters, constraints, voltage divider |
| layout_reuse | Medium | 6 | pragmas, arrays, bridge connections |
| pick_parts | Medium | 6 | FOR_LOOP pragma, part selection |
| i2c | Medium | 11 | MODULE_TEMPLATING, local imports |
| esp32_minimal | Complex | 8 | external packages, power rails |
| led_badge | Complex | 20 | multiple subsystems, LED matrix |

### Feature Coverage from Examples

| Feature | Present | Parses |
|---------|---------|--------|
| `module` definitions | ✅ | ✅ |
| `component` definitions | ✅ | ✅ |
| `interface` definitions | ✅ | ✅ |
| `import` statements | ✅ | ✅ |
| `from "path" import` | ✅ | ✅ |
| `pin` declarations | ✅ | ✅ |
| `signal` declarations | ✅ | ✅ |
| Parameter declarations (`x: unit`) | ✅ | ✅ |
| Connections (`~`) | ✅ | ✅ |
| Directed connections (`~>`) | ✅ | ✅ |
| `new` instantiation | ✅ | ✅ |
| Array instantiation (`new X[n]`) | ✅ | ✅ |
| Templates (`new X<param=val>`) | ✅ | ✅ |
| `for` loops | ✅ | ✅ |
| `assert` statements | ✅ | ✅ |
| Physical quantities | ✅ | ✅ |
| Tolerances (`+/- N%`) | ✅ | ✅ |
| Ranges (`X to Y`) | ✅ | ✅ |
| Pragmas | ✅ | ✅ |
| Docstrings | ✅ | ✅ |
| Inheritance (`from Base`) | ✅ | ✅ |
| Traits | ✅ | ✅ |

## Standard Library (10/10 ✅)

All standard library `.ato` files in `src/faebryk/library/` parse successfully:

| File | Purpose |
|------|---------|
| debug.ato | TestPoint component |
| diodes.ato | PowerDiodeOr, FULLBRIDGERECTIFIER |
| filters.ato | LowPassPiFilter |
| i2c_pulls_weak.ato | Weak I2C pull-ups |
| interfaces.ato | I2S, SPI, CAN, USB_PD, QSPI, etc. |
| mosfets.ato | HalfBridge, LowSideSwitch |
| oscillators.ato | Crystal, Oscillator |
| regulators.ato | Buck, Boost, LDO variants |
| resistors.ato | I2CPullup |
| vdivs.ato | Voltage divider |

## Syntax Examples (0/1 ❌)

The comprehensive syntax examples file (`src/vscode-atopile/syntax_examples.ato`) fails to parse.

### Error Details

```
× unexpected token
   ╭─[src/vscode-atopile/syntax_examples.ato:15:21]
14 │ # Multiple imports on one line (semicolon separated)
15 │ import AnotherModule; from "another/source.ato" import AnotherSpecific
   ·                     ┬
   ·                     ╰── here
   ╰────
```

### Root Cause

The Rust parser does not support **semicolon-separated statements at file (top) level**.

According to the grammar:
```antlr
simple_stmts: simple_stmt (SEMI_COLON simple_stmt)* SEMI_COLON? NEWLINE;
```

This syntax should be valid, but the Rust parser only supports it inside blocks, not at the top level.

### Affected Syntax Patterns

1. `import X; import Y` - Multiple imports on one line
2. `pass; var = 1` - Mixed statements on one line
3. Top-level compound statements

### Recommendation

This is a parser gap that should be fixed. The grammar explicitly allows semicolon-separated simple statements at any level.

## Parser Gaps Identified

| Gap | Severity | Impact |
|-----|----------|--------|
| Top-level semicolon statements | Low | Rare in practice |

## Semantic Analysis Notes

Semantic analysis runs on all examples but reports errors for:

1. **Unresolved imports** - External packages (e.g., `import Resistor`) cannot be resolved
2. **Template instantiation** - `new Module<param=val>` may not fully type-check

This is expected since the semantic analyzer doesn't have access to:
- The standard library definitions
- External package definitions
- A module resolution system

## Conclusion

The Rust parser is **highly compatible** with real-world atopile code:

- **100%** of example projects parse
- **100%** of standard library files parse
- Only 1 edge case (semicolon-separated top-level statements) fails

The compiler is ready for use on typical atopile projects, with the caveat that semantic analysis cannot resolve external imports.

## Test Command

```bash
cargo test -p ato-tests -- examples
```

## Files

- Test file: `crates/ato-tests/tests/examples_tests.rs`
- This report: `crates/ato-tests/EXAMPLES_REPORT.md`
