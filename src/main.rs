//! CLI for mutation testing framework

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use colored::Colorize;

use mutation_testing_rust::{
    run_mutation_tests, validate_mutations, Config, MutationReport,
};

#[derive(Parser)]
#[command(name = "mutation-testing-rust")]
#[command(author, version, about = "AST-based mutation testing for Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run mutation tests
    Test {
        /// Path to the mutations config file
        #[arg(short, long, default_value = "mutations.yaml")]
        config: PathBuf,

        /// Project directory (defaults to current directory)
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Validate mutation configuration without running tests
    Validate {
        /// Path to the mutations config file
        #[arg(short, long, default_value = "mutations.yaml")]
        config: PathBuf,

        /// Project directory (defaults to current directory)
        #[arg(short, long)]
        project: Option<PathBuf>,
    },

    /// Show example configuration
    Example,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Test {
            config,
            project,
            verbose,
        } => run_tests(&config, project, verbose),

        Commands::Validate { config, project } => validate_config(&config, project),

        Commands::Example => {
            print_example();
            ExitCode::SUCCESS
        }
    }
}

fn run_tests(config_path: &PathBuf, project: Option<PathBuf>, verbose: bool) -> ExitCode {
    let project_dir = project.unwrap_or_else(|| PathBuf::from("."));

    // Load configuration
    println!("{}", "Loading configuration...".dimmed());
    let config = match Config::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: {}", "Error".red().bold(), e);
            return ExitCode::FAILURE;
        }
    };

    println!(
        "Found {} mutation(s) in config",
        config.mutations.len()
    );

    // Validate configuration first
    println!("{}", "Validating mutations...".dimmed());
    let validation_results = validate_mutations(&config, &project_dir);
    let errors: Vec<_> = validation_results
        .into_iter()
        .filter_map(|r| r.err())
        .collect();

    if !errors.is_empty() {
        eprintln!("{}", "Configuration errors found:".red().bold());
        for error in &errors {
            eprintln!("  • {}", error);
        }
        return ExitCode::FAILURE;
    }

    println!("{}", "All mutations valid. Running tests...".green());
    println!();

    // Run mutation tests
    let results = run_mutation_tests(&config, &project_dir, verbose);

    // Generate and print report
    let report = MutationReport::new(results);
    report.print();

    // Return appropriate exit code
    if report.survived() > 0 {
        ExitCode::from(1) // Some mutations survived
    } else if report.config_errors() > 0 {
        ExitCode::from(2) // Configuration errors
    } else {
        ExitCode::SUCCESS
    }
}

fn validate_config(config_path: &PathBuf, project: Option<PathBuf>) -> ExitCode {
    let project_dir = project.unwrap_or_else(|| PathBuf::from("."));

    // Load configuration
    println!("{}", "Loading configuration...".dimmed());
    let config = match Config::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: {}", "Error".red().bold(), e);
            return ExitCode::FAILURE;
        }
    };

    println!(
        "Found {} mutation(s) in config",
        config.mutations.len()
    );

    // Validate all mutations
    println!("{}", "Validating mutations...".dimmed());
    println!();

    let validation_results = validate_mutations(&config, &project_dir);
    let mut all_valid = true;

    for (i, result) in validation_results.iter().enumerate() {
        let mutation = &config.mutations[i];
        match result {
            Ok(()) => {
                println!(
                    "{} {} -> {} in {}::{}",
                    "✓".green(),
                    mutation.original,
                    mutation.replacement,
                    mutation.file.display(),
                    mutation.function
                );
            }
            Err(e) => {
                all_valid = false;
                println!(
                    "{} {} -> {} in {}::{}",
                    "✗".red(),
                    mutation.original,
                    mutation.replacement,
                    mutation.file.display(),
                    mutation.function
                );
                println!("  {}: {}", "Error".red(), e);
            }
        }
    }

    println!();
    if all_valid {
        println!(
            "{} All {} mutations are valid!",
            "✓".green().bold(),
            config.mutations.len()
        );
        ExitCode::SUCCESS
    } else {
        let error_count = validation_results.iter().filter(|r| r.is_err()).count();
        println!(
            "{} {} of {} mutations have errors",
            "✗".red().bold(),
            error_count,
            config.mutations.len()
        );
        ExitCode::FAILURE
    }
}

fn print_example() {
    let example = r#"# Example mutations.yaml configuration file
version: "1.0"

settings:
  timeout: 30  # seconds per test run

mutations:
  # Arithmetic operator mutation
  - file: src/calculator.rs
    function: add
    original: a + b
    replacement: a - b

  # Another arithmetic mutation for the same expression
  - file: src/calculator.rs
    function: add
    original: a + b
    replacement: a * b

  # Comparison operator mutation (boundary testing)
  - file: src/validator.rs
    function: is_adult
    original: age >= 18
    replacement: age > 18

  # Logical operator mutation
  - file: src/auth.rs
    function: check_access
    original: is_admin && is_active
    replacement: is_admin || is_active

  # Return value mutation
  - file: src/auth.rs
    function: authenticate
    original: password == stored
    replacement: "true"

  # Literal value mutation
  - file: src/config.rs
    function: default_timeout
    original: "30"
    replacement: "0"
"#;

    println!("{}", example);
}
