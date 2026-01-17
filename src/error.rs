//! Error types for mutation testing

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during mutation testing
#[derive(Debug, Error)]
pub enum MutationError {
    /// Original code couldn't be parsed as valid Rust
    #[error("Invalid original expression: '{code}'\n  Parse error: {parse_error}")]
    InvalidOriginal { code: String, parse_error: String },

    /// Replacement code couldn't be parsed as valid Rust
    #[error("Invalid replacement expression: '{code}'\n  Parse error: {parse_error}")]
    InvalidReplacement { code: String, parse_error: String },

    /// Target file doesn't exist
    #[error("File not found: {}", file.display())]
    FileNotFound { file: PathBuf },

    /// Failed to read source file
    #[error("Failed to read file '{}': {error}", file.display())]
    FileReadError { file: PathBuf, error: String },

    /// Failed to parse source file as Rust
    #[error("Failed to parse '{}' as Rust: {error}", file.display())]
    ParseError { file: PathBuf, error: String },

    /// Target function not found in file
    #[error("Function '{function}' not found in {}\n  Available functions: {}", file.display(), available_functions.join(", "))]
    FunctionNotFound {
        file: PathBuf,
        function: String,
        available_functions: Vec<String>,
    },

    /// Original expression not found in function
    #[error("Expression '{original}' not found in function '{function}'")]
    NoMatch {
        file: PathBuf,
        function: String,
        original: String,
    },

    /// Multiple matches found (ambiguous)
    #[error("Found {match_count} matches for '{original}' in '{function}'\n  Locations: {}", format_locations(locations))]
    AmbiguousMatch {
        function: String,
        original: String,
        match_count: usize,
        locations: Vec<MatchLocation>,
    },

    /// Failed to apply mutation
    #[error("Failed to apply mutation: {reason}")]
    FailedToApply { reason: String },

    /// Failed to write mutated file
    #[error("Failed to write mutated file '{}': {error}", file.display())]
    WriteError { file: PathBuf, error: String },

    /// Test execution failed
    #[error("Test execution failed: {error}")]
    TestExecutionError { error: String },

    /// Configuration error
    #[error("Configuration error: {message}")]
    ConfigError { message: String },
}

/// A location where a match was found
#[derive(Debug, Clone)]
pub struct MatchLocation {
    pub line: usize,
    pub column: usize,
}

fn format_locations(locations: &[MatchLocation]) -> String {
    locations
        .iter()
        .map(|loc| format!("line {}, column {}", loc.line, loc.column))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Result type for mutation operations
pub type Result<T> = std::result::Result<T, MutationError>;
