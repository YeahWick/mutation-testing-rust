//! Configuration file parsing for mutation testing

use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::error::MutationError;

/// Top-level configuration structure
#[derive(Debug, Deserialize)]
pub struct Config {
    pub version: String,
    #[serde(default)]
    pub settings: Settings,
    pub mutations: Vec<MutationConfig>,
}

/// Global settings for mutation testing
#[derive(Debug, Deserialize)]
pub struct Settings {
    /// Timeout in seconds for each test run
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
        }
    }
}

fn default_timeout() -> u64 {
    30
}

/// A single mutation definition
#[derive(Debug, Deserialize, Clone)]
pub struct MutationConfig {
    /// Path to the Rust source file
    pub file: PathBuf,
    /// Name of the function containing the code
    pub function: String,
    /// The code to find (parsed as AST)
    pub original: String,
    /// The code to replace it with
    pub replacement: String,
    /// Optional unique identifier (auto-generated if omitted)
    #[serde(default = "generate_id")]
    pub id: String,
}

fn generate_id() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    format!("mutation_{}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

impl MutationConfig {
    /// Create a description for this mutation
    pub fn description(&self) -> String {
        format!(
            "{} -> {} in {}::{}",
            self.original, self.replacement, self.file.display(), self.function
        )
    }
}

impl Config {
    /// Load configuration from a YAML file
    pub fn load(path: &Path) -> Result<Self, MutationError> {
        let content = std::fs::read_to_string(path).map_err(|e| MutationError::ConfigError {
            message: format!("Failed to read config file '{}': {}", path.display(), e),
        })?;

        let config: Config =
            serde_yaml::from_str(&content).map_err(|e| MutationError::ConfigError {
                message: format!("Failed to parse config file '{}': {}", path.display(), e),
            })?;

        Ok(config)
    }

    /// Validate all mutations in the configuration
    pub fn validate(&self) -> Result<(), Vec<MutationError>> {
        let mut errors = Vec::new();

        for mutation in &self.mutations {
            // Check file exists
            if !mutation.file.exists() {
                errors.push(MutationError::FileNotFound {
                    file: mutation.file.clone(),
                });
                continue;
            }

            // Check original parses as expression
            if let Err(e) = syn::parse_str::<syn::Expr>(&mutation.original) {
                errors.push(MutationError::InvalidOriginal {
                    code: mutation.original.clone(),
                    parse_error: e.to_string(),
                });
            }

            // Check replacement parses as expression
            if let Err(e) = syn::parse_str::<syn::Expr>(&mutation.replacement) {
                errors.push(MutationError::InvalidReplacement {
                    code: mutation.replacement.clone(),
                    parse_error: e.to_string(),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let yaml = r#"
version: "1.0"
settings:
  timeout: 60
mutations:
  - file: src/math.rs
    function: add
    original: a + b
    replacement: a - b
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.settings.timeout, 60);
        assert_eq!(config.mutations.len(), 1);
        assert_eq!(config.mutations[0].function, "add");
    }

    #[test]
    fn test_default_timeout() {
        let yaml = r#"
version: "1.0"
mutations: []
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.settings.timeout, 30);
    }
}
