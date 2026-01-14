//! Code generation from AST
//!
//! This module handles converting mutated ASTs back to source code.

use std::path::Path;

use crate::config::MutationConfig;
use crate::error::{MutationError, Result};
use crate::matcher::{collect_function_names, find_expression_in_function, MatchedSite};
use crate::mutator::Mutator;

/// Generate source code from AST
pub fn generate_source(ast: &syn::File) -> String {
    prettyplease::unparse(ast)
}

/// Result of preparing a mutation
pub struct PreparedMutation {
    /// The mutated source code
    pub mutated_source: String,
    /// The matched site where mutation was applied
    pub site: MatchedSite,
}

/// Prepare a mutation: parse, find, apply, and generate mutated source
pub fn prepare_mutation(
    source: &str,
    mutation: &MutationConfig,
) -> Result<PreparedMutation> {
    // Parse the source file
    let mut ast = syn::parse_file(source).map_err(|e| MutationError::ParseError {
        file: mutation.file.clone(),
        error: e.to_string(),
    })?;

    // Parse original expression
    let original_expr: syn::Expr =
        syn::parse_str(&mutation.original).map_err(|e| MutationError::InvalidOriginal {
            code: mutation.original.clone(),
            parse_error: e.to_string(),
        })?;

    // Parse replacement expression
    let replacement_expr: syn::Expr =
        syn::parse_str(&mutation.replacement).map_err(|e| MutationError::InvalidReplacement {
            code: mutation.replacement.clone(),
            parse_error: e.to_string(),
        })?;

    // Check function exists
    let functions = collect_function_names(&ast);
    if !functions.contains(&mutation.function) {
        return Err(MutationError::FunctionNotFound {
            file: mutation.file.clone(),
            function: mutation.function.clone(),
            available_functions: functions,
        });
    }

    // Find the original expression in the function
    let matches = find_expression_in_function(&ast, &mutation.function, &original_expr);

    let target = match matches.len() {
        0 => {
            return Err(MutationError::NoMatch {
                file: mutation.file.clone(),
                function: mutation.function.clone(),
                original: mutation.original.clone(),
            });
        }
        1 => matches.into_iter().next().unwrap(),
        n => {
            return Err(MutationError::AmbiguousMatch {
                function: mutation.function.clone(),
                original: mutation.original.clone(),
                match_count: n,
                locations: matches.iter().map(|m| m.to_location()).collect(),
            });
        }
    };

    // Apply the mutation
    Mutator::apply(
        &mut ast,
        &mutation.function,
        &original_expr,
        &replacement_expr,
        &target,
    )?;

    // Generate the mutated source
    Ok(PreparedMutation {
        mutated_source: generate_source(&ast),
        site: target,
    })
}

/// Apply a mutation to a file and return the mutated content
pub fn apply_mutation_to_file(
    file_path: &Path,
    mutation: &MutationConfig,
) -> Result<PreparedMutation> {
    // Read source file
    let source = std::fs::read_to_string(file_path).map_err(|e| MutationError::FileReadError {
        file: file_path.to_path_buf(),
        error: e.to_string(),
    })?;

    prepare_mutation(&source, mutation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_prepare_mutation() {
        let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let mutation = MutationConfig {
            file: PathBuf::from("test.rs"),
            function: "add".to_string(),
            original: "a + b".to_string(),
            replacement: "a - b".to_string(),
            id: "test".to_string(),
        };

        let result = prepare_mutation(source, &mutation).unwrap();
        assert!(result.mutated_source.contains("a - b"));
    }

    #[test]
    fn test_function_not_found() {
        let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let mutation = MutationConfig {
            file: PathBuf::from("test.rs"),
            function: "subtract".to_string(),
            original: "a + b".to_string(),
            replacement: "a - b".to_string(),
            id: "test".to_string(),
        };

        let result = prepare_mutation(source, &mutation);
        assert!(matches!(result, Err(MutationError::FunctionNotFound { .. })));
    }

    #[test]
    fn test_no_match() {
        let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let mutation = MutationConfig {
            file: PathBuf::from("test.rs"),
            function: "add".to_string(),
            original: "x + y".to_string(),
            replacement: "x - y".to_string(),
            id: "test".to_string(),
        };

        let result = prepare_mutation(source, &mutation);
        assert!(matches!(result, Err(MutationError::NoMatch { .. })));
    }
}
