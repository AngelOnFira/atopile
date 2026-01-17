//! End-to-end test harness for the Atopile Rust compiler.
//!
//! This crate provides utilities for testing the full compilation pipeline:
//! - Lexer → Parser → Semantic Analysis → Constraint Solver
//!
//! Test categories:
//! - Parse tests: Verify parsing of valid/invalid .ato files
//! - Sema tests: Verify semantic analysis catches errors
//! - Solver tests: Verify constraint solving works correctly
//! - Integration tests: Full pipeline tests on real projects

pub mod harness;

pub use harness::*;
