//! Test runner for mutation testing
//!
//! This module coordinates the mutation testing process:
//! - Applies each mutation
//! - Runs tests
//! - Collects results

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::codegen::apply_mutation_to_file;
use crate::config::{Config, MutationConfig};
use crate::error::{MutationError, Result};

/// Status of a mutation after testing
#[derive(Debug, Clone, PartialEq)]
pub enum MutationStatus {
    /// Tests failed - mutation was detected (good!)
    Killed,
    /// Tests passed - mutation was NOT detected (bad!)
    Survived,
    /// Tests timed out
    Timeout,
    /// Mutated code failed to compile
    CompileError,
    /// Configuration error (couldn't apply mutation)
    ConfigError(String),
}

/// Result of running a single mutation
#[derive(Debug)]
pub struct MutationResult {
    pub mutation_id: String,
    pub file: std::path::PathBuf,
    pub function: String,
    pub original: String,
    pub replacement: String,
    pub status: MutationStatus,
    pub duration: Duration,
    pub line: Option<usize>,
    pub details: Option<String>,
}

impl MutationResult {
    pub fn description(&self) -> String {
        format!(
            "{} -> {} in {}::{}",
            self.original,
            self.replacement,
            self.file.display(),
            self.function
        )
    }
}

/// Run mutation testing with the given configuration
pub fn run_mutation_tests(
    config: &Config,
    project_dir: &Path,
    verbose: bool,
) -> Vec<MutationResult> {
    let mut results = Vec::new();

    for mutation in &config.mutations {
        if verbose {
            eprintln!(
                "Testing mutation: {} -> {} in {}::{}",
                mutation.original, mutation.replacement, mutation.file.display(), mutation.function
            );
        }

        let result = run_single_mutation(mutation, project_dir, config.settings.timeout, verbose);
        results.push(result);
    }

    results
}

/// Run a single mutation test
fn run_single_mutation(
    mutation: &MutationConfig,
    project_dir: &Path,
    timeout_secs: u64,
    verbose: bool,
) -> MutationResult {
    let start = Instant::now();

    // Resolve file path relative to project directory
    let file_path = project_dir.join(&mutation.file);

    // Check file exists
    if !file_path.exists() {
        return MutationResult {
            mutation_id: mutation.id.clone(),
            file: mutation.file.clone(),
            function: mutation.function.clone(),
            original: mutation.original.clone(),
            replacement: mutation.replacement.clone(),
            status: MutationStatus::ConfigError(format!(
                "File not found: {}",
                file_path.display()
            )),
            duration: start.elapsed(),
            line: None,
            details: None,
        };
    }

    // Read original file content for restoration
    let original_content = match std::fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            return MutationResult {
                mutation_id: mutation.id.clone(),
                file: mutation.file.clone(),
                function: mutation.function.clone(),
                original: mutation.original.clone(),
                replacement: mutation.replacement.clone(),
                status: MutationStatus::ConfigError(format!("Failed to read file: {}", e)),
                duration: start.elapsed(),
                line: None,
                details: None,
            };
        }
    };

    // Prepare the mutation
    let prepared = match apply_mutation_to_file(&file_path, mutation) {
        Ok(p) => p,
        Err(e) => {
            return MutationResult {
                mutation_id: mutation.id.clone(),
                file: mutation.file.clone(),
                function: mutation.function.clone(),
                original: mutation.original.clone(),
                replacement: mutation.replacement.clone(),
                status: MutationStatus::ConfigError(e.to_string()),
                duration: start.elapsed(),
                line: None,
                details: Some(e.to_string()),
            };
        }
    };

    let line = Some(prepared.site.line);

    // Write the mutated file
    if let Err(e) = std::fs::write(&file_path, &prepared.mutated_source) {
        return MutationResult {
            mutation_id: mutation.id.clone(),
            file: mutation.file.clone(),
            function: mutation.function.clone(),
            original: mutation.original.clone(),
            replacement: mutation.replacement.clone(),
            status: MutationStatus::ConfigError(format!("Failed to write mutated file: {}", e)),
            duration: start.elapsed(),
            line,
            details: None,
        };
    }

    // Run tests
    let test_result = run_cargo_test(project_dir, timeout_secs, verbose);

    // Restore original file
    if let Err(e) = std::fs::write(&file_path, &original_content) {
        eprintln!("WARNING: Failed to restore original file: {}", e);
    }

    let duration = start.elapsed();

    let (status, details) = match test_result {
        TestResult::Passed => (MutationStatus::Survived, None),
        TestResult::Failed(output) => (MutationStatus::Killed, Some(output)),
        TestResult::CompileError(output) => (MutationStatus::CompileError, Some(output)),
        TestResult::Timeout => (MutationStatus::Timeout, None),
        TestResult::Error(e) => (MutationStatus::ConfigError(e.clone()), Some(e)),
    };

    MutationResult {
        mutation_id: mutation.id.clone(),
        file: mutation.file.clone(),
        function: mutation.function.clone(),
        original: mutation.original.clone(),
        replacement: mutation.replacement.clone(),
        status,
        duration,
        line,
        details,
    }
}

enum TestResult {
    Passed,
    Failed(String),
    CompileError(String),
    Timeout,
    Error(String),
}

/// Run cargo test and return the result
fn run_cargo_test(project_dir: &Path, timeout_secs: u64, verbose: bool) -> TestResult {
    let mut cmd = Command::new("cargo");
    cmd.arg("test")
        .arg("--no-fail-fast")
        .current_dir(project_dir);

    if !verbose {
        cmd.arg("--quiet");
    }

    // TODO: Implement proper timeout handling
    let _timeout = Duration::from_secs(timeout_secs);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}\n{}", stdout, stderr);

            if output.status.success() {
                TestResult::Passed
            } else {
                // Check if it's a compile error
                if stderr.contains("error[E")
                    || stderr.contains("could not compile")
                    || stderr.contains("aborting due to")
                {
                    TestResult::CompileError(combined)
                } else {
                    TestResult::Failed(combined)
                }
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::TimedOut {
                TestResult::Timeout
            } else {
                TestResult::Error(format!("Failed to run cargo test: {}", e))
            }
        }
    }
}

/// Validate all mutations without running tests
pub fn validate_mutations(config: &Config, project_dir: &Path) -> Vec<Result<()>> {
    config
        .mutations
        .iter()
        .map(|mutation| {
            let file_path = project_dir.join(&mutation.file);

            if !file_path.exists() {
                return Err(MutationError::FileNotFound {
                    file: mutation.file.clone(),
                });
            }

            // Try to prepare the mutation (this validates everything)
            apply_mutation_to_file(&file_path, mutation).map(|_| ())
        })
        .collect()
}
