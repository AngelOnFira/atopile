//! Python bindings for the Ato parser.

use ato_parser::parse;
use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

/// Parse Ato source code into an AST.
///
/// Returns the AST as a Python dictionary (JSON-compatible structure).
pub fn parse_source(source: &str) -> PyResult<PyObject> {
    let file = parse(source).map_err(|e| PyValueError::new_err(format!("Parse error: {}", e)))?;

    // Serialize to JSON and then convert to Python
    let json_str = serde_json::to_string(&file)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))?;

    Python::with_gil(|py| {
        let json_module = py.import("json")?;
        let result = json_module.call_method1("loads", (json_str,))?;
        Ok(result.into())
    })
}
