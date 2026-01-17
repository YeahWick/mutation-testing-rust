//! AST-based expression matching for mutation testing
//!
//! This module finds expressions in Rust source code by comparing AST structures,
//! ignoring whitespace and formatting differences.

use syn::visit::Visit;
use syn::{BinOp, Expr, Lit, UnOp};

use crate::error::MatchLocation;

/// A site where a matching expression was found
#[derive(Debug, Clone)]
pub struct MatchedSite {
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// The index of this match (for disambiguation when applying mutations)
    pub match_index: usize,
}

impl MatchedSite {
    pub fn to_location(&self) -> MatchLocation {
        MatchLocation {
            line: self.line,
            column: self.column,
        }
    }
}

/// Find all occurrences of an expression within a specific function
pub fn find_expression_in_function(
    ast: &syn::File,
    function_name: &str,
    target_expr: &syn::Expr,
) -> Vec<MatchedSite> {
    let mut matcher = ExpressionMatcher {
        target: target_expr.clone(),
        function_name: function_name.to_string(),
        matches: Vec::new(),
        in_target_function: false,
        current_match_index: 0,
    };

    matcher.visit_file(ast);
    matcher.matches
}

/// Collect all function names in a file
pub fn collect_function_names(ast: &syn::File) -> Vec<String> {
    let mut collector = FunctionCollector { functions: Vec::new() };
    collector.visit_file(ast);
    collector.functions
}

struct FunctionCollector {
    functions: Vec<String>,
}

impl<'ast> Visit<'ast> for FunctionCollector {
    fn visit_item_fn(&mut self, func: &'ast syn::ItemFn) {
        self.functions.push(func.sig.ident.to_string());
        syn::visit::visit_item_fn(self, func);
    }

    fn visit_impl_item_fn(&mut self, func: &'ast syn::ImplItemFn) {
        self.functions.push(func.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, func);
    }
}

struct ExpressionMatcher {
    target: syn::Expr,
    function_name: String,
    matches: Vec<MatchedSite>,
    in_target_function: bool,
    current_match_index: usize,
}

impl<'ast> Visit<'ast> for ExpressionMatcher {
    fn visit_item_fn(&mut self, func: &'ast syn::ItemFn) {
        if func.sig.ident == self.function_name {
            self.in_target_function = true;
            syn::visit::visit_item_fn(self, func);
            self.in_target_function = false;
        }
    }

    fn visit_impl_item_fn(&mut self, func: &'ast syn::ImplItemFn) {
        if func.sig.ident == self.function_name {
            self.in_target_function = true;
            syn::visit::visit_impl_item_fn(self, func);
            self.in_target_function = false;
        }
    }

    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if self.in_target_function && ast_equals(expr, &self.target) {
            let span = get_span(expr);
            self.matches.push(MatchedSite {
                line: span.start().line,
                column: span.start().column + 1, // 1-indexed
                match_index: self.current_match_index,
            });
            self.current_match_index += 1;
        }
        // Continue searching in child expressions
        syn::visit::visit_expr(self, expr);
    }
}

/// Get the span of an expression
fn get_span(expr: &syn::Expr) -> proc_macro2::Span {
    use quote::ToTokens;
    expr.to_token_stream()
        .into_iter()
        .next()
        .map(|t| t.span())
        .unwrap_or_else(proc_macro2::Span::call_site)
}

/// Compare two AST expressions for structural equality (ignoring spans/whitespace)
pub fn ast_equals(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        // Binary expressions (a + b, a * b, etc.)
        (Expr::Binary(a), Expr::Binary(b)) => {
            ast_equals(&a.left, &b.left)
                && binop_equals(&a.op, &b.op)
                && ast_equals(&a.right, &b.right)
        }

        // Unary expressions (!a, -a, etc.)
        (Expr::Unary(a), Expr::Unary(b)) => unop_equals(&a.op, &b.op) && ast_equals(&a.expr, &b.expr),

        // Literals (42, "hello", true, etc.)
        (Expr::Lit(a), Expr::Lit(b)) => lit_equals(&a.lit, &b.lit),

        // Paths/identifiers (a, foo::bar, etc.)
        (Expr::Path(a), Expr::Path(b)) => path_equals(&a.path, &b.path),

        // Parenthesized expressions
        (Expr::Paren(a), Expr::Paren(b)) => ast_equals(&a.expr, &b.expr),
        // Unwrap parentheses when comparing
        (Expr::Paren(a), b) => ast_equals(&a.expr, b),
        (a, Expr::Paren(b)) => ast_equals(a, &b.expr),

        // Function calls
        (Expr::Call(a), Expr::Call(b)) => {
            ast_equals(&a.func, &b.func)
                && a.args.len() == b.args.len()
                && a.args.iter().zip(b.args.iter()).all(|(a, b)| ast_equals(a, b))
        }

        // Method calls
        (Expr::MethodCall(a), Expr::MethodCall(b)) => {
            ast_equals(&a.receiver, &b.receiver)
                && a.method == b.method
                && a.args.len() == b.args.len()
                && a.args.iter().zip(b.args.iter()).all(|(a, b)| ast_equals(a, b))
        }

        // Field access (a.field)
        (Expr::Field(a), Expr::Field(b)) => {
            ast_equals(&a.base, &b.base) && member_equals(&a.member, &b.member)
        }

        // Index expressions (a[i])
        (Expr::Index(a), Expr::Index(b)) => {
            ast_equals(&a.expr, &b.expr) && ast_equals(&a.index, &b.index)
        }

        // Cast expressions (a as T)
        (Expr::Cast(a), Expr::Cast(b)) => {
            ast_equals(&a.expr, &b.expr) && type_equals(&a.ty, &b.ty)
        }

        // Reference expressions (&a, &mut a)
        (Expr::Reference(a), Expr::Reference(b)) => {
            a.mutability.is_some() == b.mutability.is_some() && ast_equals(&a.expr, &b.expr)
        }

        // Tuple expressions (a, b, c)
        (Expr::Tuple(a), Expr::Tuple(b)) => {
            a.elems.len() == b.elems.len()
                && a.elems.iter().zip(b.elems.iter()).all(|(a, b)| ast_equals(a, b))
        }

        // Array expressions [a, b, c]
        (Expr::Array(a), Expr::Array(b)) => {
            a.elems.len() == b.elems.len()
                && a.elems.iter().zip(b.elems.iter()).all(|(a, b)| ast_equals(a, b))
        }

        // If expressions
        (Expr::If(a), Expr::If(b)) => {
            ast_equals(&a.cond, &b.cond)
            // We don't compare the blocks for simpler matching
        }

        // Block expressions
        (Expr::Block(a), Expr::Block(b)) => {
            a.block.stmts.len() == b.block.stmts.len()
            // Simplified: don't do deep comparison of blocks
        }

        // Return expressions
        (Expr::Return(a), Expr::Return(b)) => match (&a.expr, &b.expr) {
            (Some(a), Some(b)) => ast_equals(a, b),
            (None, None) => true,
            _ => false,
        },

        // Range expressions (a..b, a..=b, etc.)
        (Expr::Range(a), Expr::Range(b)) => {
            match (&a.start, &b.start) {
                (Some(a), Some(b)) => {
                    if !ast_equals(a, b) {
                        return false;
                    }
                }
                (None, None) => {}
                _ => return false,
            }
            match (&a.end, &b.end) {
                (Some(a), Some(b)) => {
                    if !ast_equals(a, b) {
                        return false;
                    }
                }
                (None, None) => {}
                _ => return false,
            }
            // Compare range limits type
            std::mem::discriminant(&a.limits) == std::mem::discriminant(&b.limits)
        }

        // Different expression types don't match
        _ => false,
    }
}

fn binop_equals(a: &BinOp, b: &BinOp) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn unop_equals(a: &UnOp, b: &UnOp) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn lit_equals(a: &Lit, b: &Lit) -> bool {
    match (a, b) {
        (Lit::Str(a), Lit::Str(b)) => a.value() == b.value(),
        (Lit::ByteStr(a), Lit::ByteStr(b)) => a.value() == b.value(),
        (Lit::CStr(a), Lit::CStr(b)) => a.value() == b.value(),
        (Lit::Byte(a), Lit::Byte(b)) => a.value() == b.value(),
        (Lit::Char(a), Lit::Char(b)) => a.value() == b.value(),
        (Lit::Int(a), Lit::Int(b)) => a.base10_digits() == b.base10_digits(),
        (Lit::Float(a), Lit::Float(b)) => a.base10_digits() == b.base10_digits(),
        (Lit::Bool(a), Lit::Bool(b)) => a.value == b.value,
        _ => false,
    }
}

fn path_equals(a: &syn::Path, b: &syn::Path) -> bool {
    if a.segments.len() != b.segments.len() {
        return false;
    }
    a.segments
        .iter()
        .zip(b.segments.iter())
        .all(|(a, b)| a.ident == b.ident)
}

fn member_equals(a: &syn::Member, b: &syn::Member) -> bool {
    match (a, b) {
        (syn::Member::Named(a), syn::Member::Named(b)) => a == b,
        (syn::Member::Unnamed(a), syn::Member::Unnamed(b)) => a.index == b.index,
        _ => false,
    }
}

fn type_equals(a: &syn::Type, b: &syn::Type) -> bool {
    // Simplified type comparison - just check the string representation
    use quote::ToTokens;
    a.to_token_stream().to_string() == b.to_token_stream().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_expr(s: &str) -> syn::Expr {
        syn::parse_str(s).unwrap()
    }

    #[test]
    fn test_binary_expr_equals() {
        assert!(ast_equals(&parse_expr("a + b"), &parse_expr("a + b")));
        assert!(ast_equals(&parse_expr("a+b"), &parse_expr("a + b"))); // whitespace ignored
        assert!(!ast_equals(&parse_expr("a + b"), &parse_expr("a - b")));
        assert!(!ast_equals(&parse_expr("a + b"), &parse_expr("x + y")));
    }

    #[test]
    fn test_literal_equals() {
        assert!(ast_equals(&parse_expr("42"), &parse_expr("42")));
        assert!(!ast_equals(&parse_expr("42"), &parse_expr("43")));
        assert!(ast_equals(&parse_expr("true"), &parse_expr("true")));
        assert!(!ast_equals(&parse_expr("true"), &parse_expr("false")));
    }

    #[test]
    fn test_comparison_equals() {
        assert!(ast_equals(&parse_expr("a >= b"), &parse_expr("a >= b")));
        assert!(!ast_equals(&parse_expr("a >= b"), &parse_expr("a > b")));
    }

    #[test]
    fn test_find_expression() {
        let source = r#"
            fn add(a: i32, b: i32) -> i32 {
                a + b
            }

            fn other() {
                let x = 1 + 2;
            }
        "#;

        let ast = syn::parse_file(source).unwrap();
        let target = parse_expr("a + b");

        let matches = find_expression_in_function(&ast, "add", &target);
        assert_eq!(matches.len(), 1);

        // Should not find in other function
        let matches = find_expression_in_function(&ast, "other", &target);
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_collect_functions() {
        let source = r#"
            fn foo() {}
            fn bar() {}

            impl Thing {
                fn baz(&self) {}
            }
        "#;

        let ast = syn::parse_file(source).unwrap();
        let functions = collect_function_names(&ast);

        assert!(functions.contains(&"foo".to_string()));
        assert!(functions.contains(&"bar".to_string()));
        assert!(functions.contains(&"baz".to_string()));
    }
}
