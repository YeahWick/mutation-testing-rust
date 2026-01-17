//! AST mutation application
//!
//! This module applies mutations to the AST by replacing matched expressions
//! with their replacement counterparts.

use syn::visit_mut::VisitMut;

use crate::error::{MutationError, Result};
use crate::matcher::{ast_equals, MatchedSite};

/// Applies a single mutation to the AST
pub struct Mutator {
    /// The original expression to find
    target: syn::Expr,
    /// The replacement expression
    replacement: syn::Expr,
    /// The function to search in
    function_name: String,
    /// Index of the match to replace (for disambiguation)
    target_index: usize,
    /// Current match index during traversal
    current_index: usize,
    /// Whether we're currently in the target function
    in_target_function: bool,
    /// Whether the mutation was applied
    applied: bool,
}

impl VisitMut for Mutator {
    fn visit_item_fn_mut(&mut self, func: &mut syn::ItemFn) {
        if func.sig.ident == self.function_name {
            self.in_target_function = true;
            syn::visit_mut::visit_item_fn_mut(self, func);
            self.in_target_function = false;
        }
    }

    fn visit_impl_item_fn_mut(&mut self, func: &mut syn::ImplItemFn) {
        if func.sig.ident == self.function_name {
            self.in_target_function = true;
            syn::visit_mut::visit_impl_item_fn_mut(self, func);
            self.in_target_function = false;
        }
    }

    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        if self.applied {
            return; // Already applied, skip
        }

        if self.in_target_function && ast_equals(expr, &self.target) {
            if self.current_index == self.target_index {
                *expr = self.replacement.clone();
                self.applied = true;
                return; // Don't recurse into replacement
            }
            self.current_index += 1;
        }

        // Continue visiting children
        syn::visit_mut::visit_expr_mut(self, expr);
    }
}

impl Mutator {
    /// Apply a mutation to the AST
    ///
    /// # Arguments
    /// * `ast` - The AST to mutate (modified in place)
    /// * `function_name` - The function to search in
    /// * `target` - The expression to find
    /// * `replacement` - The expression to replace with
    /// * `target_site` - The specific match site to replace
    pub fn apply(
        ast: &mut syn::File,
        function_name: &str,
        target: &syn::Expr,
        replacement: &syn::Expr,
        target_site: &MatchedSite,
    ) -> Result<()> {
        let mut mutator = Mutator {
            target: target.clone(),
            replacement: replacement.clone(),
            function_name: function_name.to_string(),
            target_index: target_site.match_index,
            current_index: 0,
            in_target_function: false,
            applied: false,
        };

        mutator.visit_file_mut(ast);

        if !mutator.applied {
            return Err(MutationError::FailedToApply {
                reason: "Target expression not found during mutation".to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::find_expression_in_function;

    #[test]
    fn test_apply_mutation() {
        let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let mut ast = syn::parse_file(source).unwrap();
        let target: syn::Expr = syn::parse_str("a + b").unwrap();
        let replacement: syn::Expr = syn::parse_str("a - b").unwrap();

        let matches = find_expression_in_function(&ast, "add", &target);
        assert_eq!(matches.len(), 1);

        Mutator::apply(&mut ast, "add", &target, &replacement, &matches[0]).unwrap();

        // Verify the mutation was applied
        let mutated_source = prettyplease::unparse(&ast);
        assert!(mutated_source.contains("a - b"));
        assert!(!mutated_source.contains("a + b"));
    }

    #[test]
    fn test_apply_specific_match() {
        let source = r#"
fn calc(a: i32, b: i32) -> i32 {
    let x = a + b;
    let y = a + b;
    x * y
}
"#;
        let mut ast = syn::parse_file(source).unwrap();
        let target: syn::Expr = syn::parse_str("a + b").unwrap();
        let replacement: syn::Expr = syn::parse_str("a - b").unwrap();

        let matches = find_expression_in_function(&ast, "calc", &target);
        assert_eq!(matches.len(), 2);

        // Apply to first match only
        Mutator::apply(&mut ast, "calc", &target, &replacement, &matches[0]).unwrap();

        let mutated_source = prettyplease::unparse(&ast);
        // Should have one a - b and one a + b
        assert!(mutated_source.contains("a - b"));
        assert!(mutated_source.contains("a + b"));
    }
}
