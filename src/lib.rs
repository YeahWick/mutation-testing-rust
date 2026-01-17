//! Mutation Testing Framework for Rust
//!
//! This library provides AST-based mutation testing capabilities for Rust projects.
//! It reads mutation definitions from a YAML configuration file, applies mutations
//! to source code, runs tests, and reports which mutations were detected.
//!
//! # Example Configuration
//!
//! ```yaml
//! version: "1.0"
//! settings:
//!   timeout: 30
//!
//! mutations:
//!   - file: src/math.rs
//!     function: add
//!     original: a + b
//!     replacement: a - b
//! ```
//!
//! # Usage
//!
//! ```no_run
//! use mutation_testing_rust::{Config, run_mutation_tests, MutationReport};
//! use std::path::Path;
//!
//! let config = Config::load(Path::new("mutations.yaml")).unwrap();
//! let results = run_mutation_tests(&config, Path::new("."), false);
//! let report = MutationReport::new(results);
//! report.print();
//! ```

pub mod codegen;
pub mod config;
pub mod error;
pub mod matcher;
pub mod mutator;
pub mod report;
pub mod runner;

// Re-export main types at crate root
pub use config::{Config, MutationConfig, Settings};
pub use error::{MutationError, Result};
pub use report::MutationReport;
pub use runner::{run_mutation_tests, validate_mutations, MutationResult, MutationStatus};
