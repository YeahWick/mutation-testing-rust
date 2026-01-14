# AST-Based Mutation Testing Implementation Specification

## Document Information

| Field | Value |
|-------|-------|
| Version | 2.0 |
| Status | Draft |
| Author | Claude |
| Date | 2026-01-14 |

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Solution Overview](#solution-overview)
3. [Configuration Format](#configuration-format)
4. [Mutation Matching Algorithm](#mutation-matching-algorithm)
5. [Error Handling](#error-handling)
6. [Technical Architecture](#technical-architecture)
7. [Core Components](#core-components)
8. [Data Structures](#data-structures)
9. [Implementation Phases](#implementation-phases)
10. [Examples](#examples)
11. [Dependencies](#dependencies)

---

## Executive Summary

This specification defines an AST-based mutation testing engine for Rust using **manual mutation definitions**. Users specify mutations in a human-readable YAML configuration file by providing:

- The target file and function
- A description of the mutation
- The **replacement code** (what the code should become)

The engine parses the replacement code as an AST expression and searches for matching original expressions within the target function. This approach provides:

- **Human-readable configs**: Just write what you want the code to become
- **AST precision**: Mutations match syntax structure, not text patterns
- **Clear error reporting**: Detailed feedback when mutations don't match

---

## Solution Overview

### How It Works

```
┌─────────────────┐
│ mutations.yaml  │  User specifies: file, function, replacement code
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Parse Config   │  Parse replacement code as AST expression
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Parse Source   │  Parse target .rs file into AST
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Find Function  │  Locate target function in AST
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Match Original  │  Find expression that replacement would replace
└────────┬────────┘  (infer original from replacement structure)
         │
         ▼
┌─────────────────┐
│ Apply Mutation  │  Replace original with mutant, run tests
└─────────────────┘
```

### Key Insight: Inferring Original from Replacement

When a user specifies `a - b` as a replacement:
1. Parse `a - b` as an AST: `ExprBinary { left: a, op: Sub, right: b }`
2. The **structure** tells us what to look for: a binary expression with left=`a`, right=`b`
3. Search the function for any binary expression matching that structure
4. The original will have the same operands but a different operator

This means the user only needs to specify what they want, not what they're replacing.

---

## Configuration Format

### Human-Readable YAML

```yaml
# mutations.yaml
version: "1.0"

settings:
  timeout: 30  # seconds per test run

mutations:
  # Simple, readable format
  - file: src/calculator.rs
    function: add
    description: Replace addition with subtraction
    replace_with: a - b

  - file: src/calculator.rs
    function: add
    description: Replace addition with multiplication
    replace_with: a * b

  - file: src/calculator.rs
    function: divide
    description: Return zero instead of computing
    replace_with: "0.0"

  - file: src/validator.rs
    function: is_valid
    description: Change AND to OR in validation
    replace_with: is_active || has_permission

  - file: src/validator.rs
    function: check_bounds
    description: Off-by-one error in lower bound
    replace_with: x > min   # was x >= min

  - file: src/validator.rs
    function: check_bounds
    description: Off-by-one error in upper bound
    replace_with: x < max   # was x <= max

  - file: src/auth.rs
    function: authenticate
    description: Always return authentication failure
    replace_with: "false"

  - file: src/auth.rs
    function: authenticate
    description: Skip password check
    replace_with: "true"
```

### Configuration Fields

| Field | Required | Description |
|-------|----------|-------------|
| `file` | Yes | Path to the Rust source file |
| `function` | Yes | Name of the function to mutate |
| `description` | Yes | Human-readable description of the mutation |
| `replace_with` | Yes | The replacement code (what it should become) |
| `id` | No | Optional unique identifier (auto-generated if omitted) |

### Why This Format Works

1. **Intuitive**: Write what you want the code to become
2. **No duplication**: Don't need to specify both original and replacement
3. **Self-documenting**: The description explains the intent
4. **Flexible**: Works for any expression type

---

## Mutation Matching Algorithm

### Step 1: Parse the Replacement

```rust
/// Parse the replacement code into an AST expression
fn parse_replacement(code: &str) -> Result<syn::Expr, ParseError> {
    syn::parse_str::<syn::Expr>(code)
        .map_err(|e| ParseError::InvalidReplacement {
            code: code.to_string(),
            error: e.to_string(),
        })
}
```

### Step 2: Determine What to Search For

The replacement expression tells us what structure to match:

```rust
/// Determine what original expression pattern to search for
fn infer_search_pattern(replacement: &syn::Expr) -> SearchPattern {
    match replacement {
        // Binary expression: look for same operands, any operator
        syn::Expr::Binary(bin) => SearchPattern::BinaryExpr {
            left: extract_pattern(&bin.left),
            right: extract_pattern(&bin.right),
            exclude_op: Some(bin.op.clone()),  // Don't match if already this op
        },

        // Literal: look for same type of literal
        syn::Expr::Lit(lit) => SearchPattern::Literal {
            kind: literal_kind(&lit.lit),
            exclude_value: Some(lit.lit.clone()),
        },

        // Unary expression: look for same operand
        syn::Expr::Unary(unary) => SearchPattern::UnaryExpr {
            operand: extract_pattern(&unary.expr),
            exclude_op: Some(unary.op.clone()),
        },

        // Path/identifier: look for same identifier with different value
        syn::Expr::Path(path) => SearchPattern::Identifier {
            name: path_to_string(path),
        },

        // Method call: look for same receiver and method
        syn::Expr::MethodCall(call) => SearchPattern::MethodCall {
            receiver: extract_pattern(&call.receiver),
            method: call.method.to_string(),
        },

        // Default: exact structural match excluding the replacement itself
        _ => SearchPattern::Structural {
            pattern: replacement.clone(),
        },
    }
}
```

### Step 3: Search Within Function

```rust
/// Find matching expressions within a function
struct ExpressionFinder {
    pattern: SearchPattern,
    function_name: String,
    matches: Vec<MatchedSite>,
}

impl<'ast> Visit<'ast> for ExpressionFinder {
    fn visit_item_fn(&mut self, func: &'ast syn::ItemFn) {
        if func.sig.ident.to_string() == self.function_name {
            // Only search within this function
            syn::visit::visit_item_fn(self, func);
        }
        // Don't recurse into other functions
    }

    fn visit_expr_binary(&mut self, expr: &'ast syn::ExprBinary) {
        if let SearchPattern::BinaryExpr { left, right, exclude_op } = &self.pattern {
            if matches_pattern(&expr.left, left)
               && matches_pattern(&expr.right, right)
               && !matches_op(&expr.op, exclude_op)
            {
                self.matches.push(MatchedSite {
                    original_expr: syn::Expr::Binary(expr.clone()),
                    span: expr.span(),
                });
            }
        }
        // Continue visiting children
        syn::visit::visit_expr_binary(self, expr);
    }

    fn visit_expr_lit(&mut self, expr: &'ast syn::ExprLit) {
        if let SearchPattern::Literal { kind, exclude_value } = &self.pattern {
            if matches_literal_kind(&expr.lit, kind)
               && !matches_literal_value(&expr.lit, exclude_value)
            {
                self.matches.push(MatchedSite {
                    original_expr: syn::Expr::Lit(expr.clone()),
                    span: expr.span(),
                });
            }
        }
        syn::visit::visit_expr_lit(self, expr);
    }

    // ... similar for other expression types
}
```

### Step 4: Handle Match Results

```rust
/// Process the search results
fn process_matches(
    mutation: &MutationConfig,
    matches: Vec<MatchedSite>,
) -> Result<MutationTarget, MutationError> {
    match matches.len() {
        0 => Err(MutationError::NoMatch {
            mutation_id: mutation.id.clone(),
            function: mutation.function.clone(),
            file: mutation.file.clone(),
            replacement: mutation.replace_with.clone(),
            hint: suggest_fix(&mutation),
        }),

        1 => Ok(MutationTarget {
            site: matches.into_iter().next().unwrap(),
            replacement: mutation.parsed_replacement.clone(),
        }),

        n => Err(MutationError::AmbiguousMatch {
            mutation_id: mutation.id.clone(),
            function: mutation.function.clone(),
            match_count: n,
            locations: matches.iter().map(|m| m.span).collect(),
            hint: "Add more context to the replacement to disambiguate".to_string(),
        }),
    }
}
```

---

## Error Handling

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum MutationError {
    /// Replacement code couldn't be parsed as valid Rust
    #[error("Invalid replacement code")]
    InvalidReplacement {
        mutation_id: String,
        code: String,
        parse_error: String,
    },

    /// Target file doesn't exist or can't be read
    #[error("Cannot read source file")]
    FileNotFound {
        file: PathBuf,
        io_error: String,
    },

    /// Target file contains invalid Rust syntax
    #[error("Cannot parse source file")]
    InvalidSourceFile {
        file: PathBuf,
        parse_error: String,
    },

    /// Target function not found in file
    #[error("Function '{function}' not found in {file}")]
    FunctionNotFound {
        mutation_id: String,
        file: PathBuf,
        function: String,
        available_functions: Vec<String>,
    },

    /// No matching expression found in function
    #[error("No matching expression found")]
    NoMatch {
        mutation_id: String,
        file: PathBuf,
        function: String,
        replacement: String,
        hint: String,
    },

    /// Multiple matching expressions found (ambiguous)
    #[error("Ambiguous match: found {match_count} possible locations")]
    AmbiguousMatch {
        mutation_id: String,
        function: String,
        match_count: usize,
        locations: Vec<Span>,
        hint: String,
    },

    /// Mutation would create invalid syntax
    #[error("Mutation would create invalid code")]
    InvalidMutation {
        mutation_id: String,
        reason: String,
    },
}
```

### User-Friendly Error Messages

```rust
impl MutationError {
    pub fn display_error(&self) -> String {
        match self {
            MutationError::FunctionNotFound {
                mutation_id, file, function, available_functions
            } => {
                let mut msg = format!(
                    "Error in mutation '{}':\n\
                     Function '{}' not found in {}\n\n",
                    mutation_id, function, file.display()
                );

                if !available_functions.is_empty() {
                    msg.push_str("Available functions in this file:\n");
                    for f in available_functions {
                        msg.push_str(&format!("  - {}\n", f));
                    }
                }
                msg
            }

            MutationError::NoMatch {
                mutation_id, file, function, replacement, hint
            } => {
                format!(
                    "Error in mutation '{}':\n\
                     No matching expression found for '{}'\n\
                     in function '{}' at {}\n\n\
                     Hint: {}\n",
                    mutation_id, replacement, function, file.display(), hint
                )
            }

            MutationError::AmbiguousMatch {
                mutation_id, function, match_count, locations, hint
            } => {
                let mut msg = format!(
                    "Error in mutation '{}':\n\
                     Found {} matching expressions in function '{}':\n\n",
                    mutation_id, match_count, function
                );

                for (i, loc) in locations.iter().enumerate() {
                    msg.push_str(&format!(
                        "  {}. Line {}, column {}\n",
                        i + 1, loc.start().line, loc.start().column
                    ));
                }

                msg.push_str(&format!("\nHint: {}\n", hint));
                msg
            }

            // ... other error types
            _ => format!("{}", self),
        }
    }
}
```

### Helpful Hints

```rust
/// Generate helpful hints when a mutation doesn't match
fn suggest_fix(mutation: &MutationConfig) -> String {
    let replacement = &mutation.replace_with;

    // Check if it looks like a binary expression
    if let Ok(expr) = syn::parse_str::<syn::Expr>(replacement) {
        match expr {
            syn::Expr::Binary(bin) => {
                let left = quote::quote!(#bin.left).to_string();
                let right = quote::quote!(#bin.right).to_string();
                format!(
                    "Looking for a binary expression with '{}' and '{}'.\n\
                     Check that these variable names match exactly in the function.\n\
                     Try: grep for '{}' and '{}' in the function body.",
                    left, right, left, right
                )
            }
            syn::Expr::Lit(_) => {
                "Looking for a literal value of the same type.\n\
                 Check that the function contains a literal that could be replaced."
                    .to_string()
            }
            _ => {
                "Check that the expression structure exists in the function.\n\
                 Variable names must match exactly."
                    .to_string()
            }
        }
    } else {
        format!(
            "The replacement '{}' may not be valid Rust syntax.\n\
             Try parsing it separately to check for errors.",
            replacement
        )
    }
}
```

### Validation at Load Time

```rust
/// Validate all mutations before running tests
pub fn validate_config(config: &Config) -> ValidationReport {
    let mut report = ValidationReport::new();

    for mutation in &config.mutations {
        // 1. Check file exists
        if !mutation.file.exists() {
            report.add_error(ValidationError::FileNotFound {
                mutation_id: mutation.id.clone(),
                file: mutation.file.clone(),
            });
            continue;
        }

        // 2. Parse replacement code
        let parsed_replacement = match syn::parse_str::<syn::Expr>(&mutation.replace_with) {
            Ok(expr) => expr,
            Err(e) => {
                report.add_error(ValidationError::InvalidReplacement {
                    mutation_id: mutation.id.clone(),
                    code: mutation.replace_with.clone(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        // 3. Parse source file
        let source = match std::fs::read_to_string(&mutation.file) {
            Ok(s) => s,
            Err(e) => {
                report.add_error(ValidationError::FileReadError {
                    mutation_id: mutation.id.clone(),
                    file: mutation.file.clone(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        let ast = match syn::parse_file(&source) {
            Ok(f) => f,
            Err(e) => {
                report.add_error(ValidationError::ParseError {
                    mutation_id: mutation.id.clone(),
                    file: mutation.file.clone(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        // 4. Find function
        let functions = collect_function_names(&ast);
        if !functions.contains(&mutation.function) {
            report.add_error(ValidationError::FunctionNotFound {
                mutation_id: mutation.id.clone(),
                file: mutation.file.clone(),
                function: mutation.function.clone(),
                available: functions,
            });
            continue;
        }

        // 5. Find matching expression
        let pattern = infer_search_pattern(&parsed_replacement);
        let matches = find_matches(&ast, &mutation.function, &pattern);

        match matches.len() {
            0 => report.add_error(ValidationError::NoMatch {
                mutation_id: mutation.id.clone(),
                function: mutation.function.clone(),
                replacement: mutation.replace_with.clone(),
            }),
            1 => report.add_valid(mutation.id.clone()),
            n => report.add_warning(ValidationWarning::AmbiguousMatch {
                mutation_id: mutation.id.clone(),
                match_count: n,
            }),
        }
    }

    report
}
```

---

## Technical Architecture

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    Mutation Testing Engine                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │    Config    │  │  Validator   │  │   Matcher    │          │
│  │    Loader    │  │              │  │              │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                   │
│         └────────────┬────┴────────────────┘                   │
│                      ▼                                          │
│              ┌──────────────┐                                   │
│              │ Orchestrator │                                   │
│              └──────┬───────┘                                   │
│                     │                                           │
│         ┌───────────┼───────────┐                              │
│         ▼           ▼           ▼                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐                       │
│  │  Mutator │ │  Runner  │ │ Reporter │                       │
│  └──────────┘ └──────────┘ └──────────┘                       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Workflow

```
1. Load Config        → Parse mutations.yaml
2. Validate           → Check all mutations are valid before running
3. For each mutation:
   a. Parse source    → Load target file as AST
   b. Find function   → Locate function in AST
   c. Match pattern   → Find original expression
   d. Apply mutation  → Replace original with mutant
   e. Write file      → Save mutated source
   f. Run tests       → Execute cargo test
   g. Record result   → Killed/Survived/Error
   h. Restore file    → Revert to original
4. Generate Report    → Display results
```

---

## Core Components

### Config Loader (`src/config.rs`)

```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub version: String,
    pub settings: Settings,
    pub mutations: Vec<MutationConfig>,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Deserialize)]
pub struct MutationConfig {
    pub file: PathBuf,
    pub function: String,
    pub description: String,
    pub replace_with: String,
    #[serde(default = "generate_id")]
    pub id: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
```

### Matcher (`src/matcher.rs`)

```rust
use syn::{Expr, ExprBinary, visit::Visit};

/// Pattern describing what to search for
pub enum SearchPattern {
    /// Binary expression with specific operands
    BinaryExpr {
        left: OperandPattern,
        right: OperandPattern,
        exclude_op: Option<syn::BinOp>,
    },
    /// Literal of specific type
    Literal {
        kind: LiteralKind,
        exclude_value: Option<syn::Lit>,
    },
    /// Unary expression
    UnaryExpr {
        operand: OperandPattern,
        exclude_op: Option<syn::UnOp>,
    },
    /// Any structural match
    Structural {
        pattern: Expr,
    },
}

/// Pattern for matching operands
pub enum OperandPattern {
    /// Exact identifier match
    Ident(String),
    /// Any expression
    Any,
    /// Nested pattern
    Nested(Box<SearchPattern>),
}

/// Find all matching expressions in a function
pub fn find_matches(
    ast: &syn::File,
    function_name: &str,
    pattern: &SearchPattern,
) -> Vec<MatchedSite> {
    let mut finder = ExpressionFinder {
        pattern: pattern.clone(),
        function_name: function_name.to_string(),
        matches: Vec::new(),
        in_target_function: false,
    };

    finder.visit_file(ast);
    finder.matches
}
```

### Mutator (`src/mutator.rs`)

```rust
use syn::visit_mut::VisitMut;

/// Applies a single mutation to the AST
pub struct Mutator {
    target_span: proc_macro2::Span,
    replacement: syn::Expr,
    applied: bool,
}

impl VisitMut for Mutator {
    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        // Check if this is our target expression
        if !self.applied && spans_match(expr, &self.target_span) {
            *expr = self.replacement.clone();
            self.applied = true;
            return;  // Don't recurse into replacement
        }

        // Continue visiting children
        syn::visit_mut::visit_expr_mut(self, expr);
    }
}

impl Mutator {
    pub fn apply(
        ast: &mut syn::File,
        target: &MatchedSite,
        replacement: &syn::Expr,
    ) -> Result<(), MutationError> {
        let mut mutator = Mutator {
            target_span: target.span,
            replacement: replacement.clone(),
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
```

### Code Generator (`src/codegen.rs`)

```rust
use quote::ToTokens;

/// Generate source code from AST
pub fn generate_source(ast: &syn::File) -> String {
    prettyplease::unparse(ast)
}

/// Apply mutation and generate mutated source
pub fn generate_mutant(
    original_source: &str,
    mutation: &MutationConfig,
) -> Result<String, MutationError> {
    // Parse original
    let mut ast = syn::parse_file(original_source)?;

    // Parse replacement
    let replacement = syn::parse_str::<syn::Expr>(&mutation.replace_with)?;

    // Find match
    let pattern = infer_search_pattern(&replacement);
    let matches = find_matches(&ast, &mutation.function, &pattern);

    let target = match matches.len() {
        0 => return Err(MutationError::NoMatch { /* ... */ }),
        1 => &matches[0],
        n => return Err(MutationError::AmbiguousMatch { /* ... */ }),
    };

    // Apply mutation
    Mutator::apply(&mut ast, target, &replacement)?;

    // Generate source
    Ok(generate_source(&ast))
}
```

---

## Data Structures

```rust
/// A location where a mutation can be applied
#[derive(Debug, Clone)]
pub struct MatchedSite {
    pub original_expr: syn::Expr,
    pub span: proc_macro2::Span,
    pub line: usize,
    pub column: usize,
}

/// Result of running a mutation test
#[derive(Debug)]
pub struct MutationResult {
    pub mutation_id: String,
    pub description: String,
    pub file: PathBuf,
    pub function: String,
    pub status: MutationStatus,
    pub duration: std::time::Duration,
    pub details: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum MutationStatus {
    /// Tests failed - mutation was detected
    Killed,
    /// Tests passed - mutation was NOT detected
    Survived,
    /// Tests timed out
    Timeout,
    /// Mutated code failed to compile
    CompileError,
    /// Configuration error (couldn't apply mutation)
    ConfigError,
}

/// Final report of mutation testing run
#[derive(Debug)]
pub struct MutationReport {
    pub mutations: Vec<MutationResult>,
    pub total: usize,
    pub killed: usize,
    pub survived: usize,
    pub errors: usize,
    pub duration: std::time::Duration,
}

impl MutationReport {
    pub fn score(&self) -> f64 {
        let testable = self.killed + self.survived;
        if testable == 0 {
            return 100.0;
        }
        (self.killed as f64 / testable as f64) * 100.0
    }
}
```

---

## Implementation Phases

### Phase 1: Core Infrastructure

**Deliverables:**
- Project setup with `Cargo.toml`
- Config parsing with serde
- Basic CLI with clap
- File reading and AST parsing

**Success Criteria:**
- Can load and parse `mutations.yaml`
- Can parse Rust source files

### Phase 2: Matching Engine

**Deliverables:**
- Pattern inference from replacement code
- AST visitor for finding matches
- Function-scoped searching
- Match result handling (0, 1, or many)

**Success Criteria:**
- Can find matching expressions in functions
- Proper error handling for no match / ambiguous match

### Phase 3: Mutation Application

**Deliverables:**
- AST mutation via `VisitMut`
- Code generation with `prettyplease`
- File backup and restore

**Success Criteria:**
- Can apply mutations and generate valid Rust code
- Original files are never corrupted

### Phase 4: Test Runner

**Deliverables:**
- `cargo test` execution
- Timeout handling
- Result collection

**Success Criteria:**
- End-to-end mutation testing works
- Accurate killed/survived detection

### Phase 5: Reporting

**Deliverables:**
- Text report output
- Mutation score calculation
- Surviving mutation details

**Success Criteria:**
- Clear, actionable reports
- Easy to identify test gaps

---

## Examples

### Example 1: Arithmetic Mutation

**Configuration:**
```yaml
- file: src/math.rs
  function: add
  description: Replace addition with subtraction
  replace_with: a - b
```

**Source file (`src/math.rs`):**
```rust
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

**How it works:**
1. Parse `a - b` → `ExprBinary { left: "a", op: Sub, right: "b" }`
2. Infer pattern: binary expr with left="a", right="b", op != Sub
3. Search `add` function → find `a + b`
4. Replace with `a - b`
5. Run tests

**Generated mutant:**
```rust
pub fn add(a: i32, b: i32) -> i32 {
    a - b
}
```

### Example 2: Comparison Mutation

**Configuration:**
```yaml
- file: src/validator.rs
  function: is_adult
  description: Change >= to > (off-by-one)
  replace_with: age > 18
```

**Source:**
```rust
pub fn is_adult(age: u32) -> bool {
    age >= 18
}
```

**Matching:**
- Pattern: binary expr, left="age", right="18", op != Gt
- Finds: `age >= 18`
- Replaces with: `age > 18`

### Example 3: Literal Mutation

**Configuration:**
```yaml
- file: src/config.rs
  function: default_timeout
  description: Return zero instead of default
  replace_with: "0"
```

**Source:**
```rust
pub fn default_timeout() -> u64 {
    30
}
```

**Matching:**
- Pattern: integer literal, value != 0
- Finds: `30`
- Replaces with: `0`

### Example 4: Error Case - No Match

**Configuration:**
```yaml
- file: src/math.rs
  function: add
  description: This won't match
  replace_with: x - y
```

**Error output:**
```
Error in mutation 'add_1':
No matching expression found for 'x - y'
in function 'add' at src/math.rs

Hint: Looking for a binary expression with 'x' and 'y'.
Check that these variable names match exactly in the function.
The function uses variables 'a' and 'b', not 'x' and 'y'.
```

### Example 5: Error Case - Ambiguous Match

**Configuration:**
```yaml
- file: src/math.rs
  function: complex
  description: Change addition
  replace_with: a - b
```

**Source:**
```rust
pub fn complex(a: i32, b: i32) -> i32 {
    let sum = a + b;      // Match 1
    let other = a + b;    // Match 2
    sum * other
}
```

**Error output:**
```
Error in mutation 'complex_1':
Found 2 matching expressions in function 'complex':

  1. Line 2, column 15
  2. Line 3, column 17

Hint: Add more context to the replacement to disambiguate,
or split into separate mutations targeting each location.
```

---

## Dependencies

### Cargo.toml

```toml
[package]
name = "mutation-testing-rust"
version = "0.1.0"
edition = "2021"

[dependencies]
# AST parsing
syn = { version = "2.0", features = ["full", "visit", "visit-mut", "parsing"] }
quote = "1.0"
proc-macro2 = "1.0"

# Code formatting
prettyplease = "0.2"

# CLI
clap = { version = "4.0", features = ["derive"] }

# Configuration
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Utilities
tempfile = "3.0"
colored = "2.0"

[dev-dependencies]
pretty_assertions = "1.0"
```

---

## Project Structure

```
mutation-testing-rust/
├── Cargo.toml
├── mutations.yaml           # Example configuration
├── src/
│   ├── main.rs             # CLI entry point
│   ├── lib.rs              # Library root
│   ├── config.rs           # YAML config loading
│   ├── matcher.rs          # Expression matching
│   ├── mutator.rs          # AST mutation
│   ├── codegen.rs          # Code generation
│   ├── runner.rs           # Test execution
│   ├── report.rs           # Result reporting
│   └── error.rs            # Error types
└── tests/
    ├── matching_tests.rs   # Matcher unit tests
    └── integration.rs      # End-to-end tests
```

---

## Summary

This specification defines a **manual mutation testing framework** where:

1. **Users specify mutations** in a simple YAML format with just the replacement code
2. **The engine infers** what original expression to find based on the replacement structure
3. **AST-based matching** provides precise, whitespace-insensitive matching
4. **Clear error messages** help users fix configuration issues
5. **Validation happens upfront** before any tests run

The approach prioritizes **human readability** and **clear error handling** over automatic discovery, making it easier to maintain and debug mutation configurations.
