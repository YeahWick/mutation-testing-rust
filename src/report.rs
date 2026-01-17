//! Report generation for mutation testing results
//!
//! This module formats and displays mutation testing results.

use colored::Colorize;
use std::time::Duration;

use crate::runner::{MutationResult, MutationStatus};

/// Summary report of mutation testing
#[derive(Debug)]
pub struct MutationReport {
    pub results: Vec<MutationResult>,
    pub total_duration: Duration,
}

impl MutationReport {
    /// Create a new report from results
    pub fn new(results: Vec<MutationResult>) -> Self {
        let total_duration = results.iter().map(|r| r.duration).sum();
        Self {
            results,
            total_duration,
        }
    }

    /// Count of mutations that were killed (detected by tests)
    pub fn killed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == MutationStatus::Killed)
            .count()
    }

    /// Count of mutations that survived (not detected by tests)
    pub fn survived(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == MutationStatus::Survived)
            .count()
    }

    /// Count of mutations that timed out
    pub fn timeouts(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == MutationStatus::Timeout)
            .count()
    }

    /// Count of mutations with compile errors
    pub fn compile_errors(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == MutationStatus::CompileError)
            .count()
    }

    /// Count of mutations with configuration errors
    pub fn config_errors(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.status, MutationStatus::ConfigError(_)))
            .count()
    }

    /// Total number of mutations
    pub fn total(&self) -> usize {
        self.results.len()
    }

    /// Calculate mutation score (percentage of killed mutations)
    /// Only considers killed and survived (excludes errors/timeouts)
    pub fn score(&self) -> f64 {
        let testable = self.killed() + self.survived();
        if testable == 0 {
            return 100.0;
        }
        (self.killed() as f64 / testable as f64) * 100.0
    }

    /// Get surviving mutations (test gaps)
    pub fn surviving_mutations(&self) -> Vec<&MutationResult> {
        self.results
            .iter()
            .filter(|r| r.status == MutationStatus::Survived)
            .collect()
    }

    /// Print the report to stdout
    pub fn print(&self) {
        println!();
        println!("{}", "Mutation Testing Report".bold());
        println!("{}", "=".repeat(60));
        println!();

        // Print each result
        for result in &self.results {
            let status_str = match &result.status {
                MutationStatus::Killed => "[KILLED]".green().bold(),
                MutationStatus::Survived => "[SURVIVED]".red().bold(),
                MutationStatus::Timeout => "[TIMEOUT]".yellow().bold(),
                MutationStatus::CompileError => "[COMPILE ERROR]".yellow().bold(),
                MutationStatus::ConfigError(_) => "[CONFIG ERROR]".yellow().bold(),
            };

            let location = if let Some(line) = result.line {
                format!("{}:{}", result.file.display(), line)
            } else {
                result.file.display().to_string()
            };

            println!(
                "{} {} - {} -> {}",
                status_str,
                result.mutation_id.dimmed(),
                result.original,
                result.replacement
            );
            println!(
                "        {} in function '{}'",
                location.dimmed(),
                result.function
            );
        }

        // Print summary
        println!();
        println!("{}", "Summary".bold());
        println!("{}", "-".repeat(40));
        println!("Total mutations:   {}", self.total());
        println!(
            "Killed:            {} {}",
            self.killed(),
            "(good - tests caught the mutation)".dimmed()
        );
        println!(
            "Survived:          {} {}",
            self.survived(),
            "(bad - tests missed the mutation)".dimmed()
        );

        if self.timeouts() > 0 {
            println!("Timeouts:          {}", self.timeouts());
        }
        if self.compile_errors() > 0 {
            println!("Compile errors:    {}", self.compile_errors());
        }
        if self.config_errors() > 0 {
            println!("Config errors:     {}", self.config_errors());
        }

        println!();
        let score = self.score();
        let score_str = format!("{:.1}%", score);
        let score_colored = if score >= 90.0 {
            score_str.green().bold()
        } else if score >= 70.0 {
            score_str.yellow().bold()
        } else {
            score_str.red().bold()
        };
        println!("Mutation Score:    {}", score_colored);
        println!(
            "Duration:          {:.2}s",
            self.total_duration.as_secs_f64()
        );

        // Print surviving mutations if any
        let survivors = self.surviving_mutations();
        if !survivors.is_empty() {
            println!();
            println!(
                "{}",
                "Surviving Mutations (improve your tests!)".red().bold()
            );
            println!("{}", "-".repeat(40));
            for mutation in survivors {
                let location = if let Some(line) = mutation.line {
                    format!("{}:{}", mutation.file.display(), line)
                } else {
                    mutation.file.display().to_string()
                };

                println!(
                    "  • {} -> {}",
                    mutation.original.yellow(),
                    mutation.replacement.yellow()
                );
                println!(
                    "    in function '{}' at {}",
                    mutation.function, location
                );
            }
        }

        // Score interpretation
        println!();
        println!("{}", "Score Interpretation".dimmed());
        println!("{}", "-".repeat(40).dimmed());
        println!("{}", "90-100%: Excellent test coverage".dimmed());
        println!("{}", "70-89%:  Good coverage, some gaps".dimmed());
        println!("{}", "50-69%:  Moderate, needs improvement".dimmed());
        println!("{}", "<50%:    Poor, significant gaps".dimmed());
    }
}

/// Format duration in a human-readable way
#[allow(dead_code)]
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        let mins = (secs / 60.0).floor();
        let remaining_secs = secs % 60.0;
        format!("{}m {:.0}s", mins, remaining_secs)
    }
}
