//! Intermediate Representation (IR) for the Ato hardware description language.
//!
//! This crate provides the IR types that model an Ato design after parsing.
//! The IR is a flattened, ID-based representation suitable for semantic analysis
//! and compilation.
//!
//! # Overview
//!
//! The IR consists of:
//! - **IDs**: Unique identifiers for modules, fields, and connections
//! - **Modules**: Define hardware structure (module, component, interface)
//! - **Fields**: Parameters, pins, signals, and instances
//! - **Connections**: Links between connectable fields
//! - **Constraints**: Assertions about parameter values
//! - **Design**: Root container holding all modules
//!
//! # Example
//!
//! ```
//! use ato_ir::{Design, ModuleKind, FieldKind};
//!
//! let mut design = Design::new();
//!
//! // Create a module
//! let module_id = design.create_module("MyModule", ModuleKind::Module);
//!
//! // Add a pin field
//! let pin_id = design.add_field(module_id, "p1", FieldKind::pin("p1"));
//!
//! // Add a parameter field
//! let param_id = design.add_field(module_id, "resistance", FieldKind::parameter_with_unit("ohm"));
//! ```

mod ids;
mod field;
mod module;
mod connection;
mod constraint;
mod design;

pub use ids::*;
pub use field::*;
pub use module::*;
pub use connection::*;
pub use constraint::*;
pub use design::*;

use thiserror::Error;

/// Errors that can occur when working with the IR.
#[derive(Debug, Error)]
pub enum IrError {
    #[error("Module not found: {0:?}")]
    ModuleNotFound(ModuleId),

    #[error("Field not found: {0:?}")]
    FieldNotFound(FieldId),

    #[error("Connection not found: {0:?}")]
    ConnectionNotFound(ConnectionId),

    #[error("Duplicate definition: {0}")]
    DuplicateDefinition(String),

    #[error("Invalid connection: {0}")]
    InvalidConnection(String),

    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },

    #[error("Unresolved reference: {0}")]
    UnresolvedReference(String),
}

/// Result type for IR operations.
pub type IrResult<T> = Result<T, IrError>;
