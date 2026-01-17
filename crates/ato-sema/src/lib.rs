//! Semantic analysis for the Ato hardware description language.
//!
//! This crate provides semantic analysis that lowers the parsed AST to IR,
//! with import resolution, name resolution, and type checking.
//!
//! # Overview
//!
//! The semantic analysis pipeline consists of:
//!
//! 1. **Import resolution** - Load and parse imported files
//! 2. **Name resolution** - Resolve identifiers to their definitions
//! 3. **Type checking** - Verify interface compatibility for connections
//! 4. **Lowering** - Convert AST to IR with all expansions
//!
//! # Example
//!
//! ```ignore
//! use ato_parser::parse;
//! use ato_sema::Analyzer;
//!
//! let source = r#"
//! module MyModule:
//!     pin p1
//!     signal sig
//!     p1 ~ sig
//! "#;
//!
//! let ast = parse(source).unwrap();
//! let mut analyzer = Analyzer::new();
//! let design = analyzer.analyze(ast)?;
//! ```

mod error;
mod imports;
mod names;
mod types;
mod lower;
mod scope;
mod analyzer;
pub mod resolution;
pub mod constraint_collector;

pub use error::{SemaError, SemaResult};
pub use analyzer::Analyzer;
pub use scope::Scope;
pub use constraint_collector::{ConstraintCollector, CollectionError, CollectedParameter, ParameterDependencies};

// Re-export IR types for convenience
pub use ato_ir::Design;
