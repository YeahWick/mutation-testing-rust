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

This specification defines an AST-based mutation testing engine for Rust using **manual mutation definitions**. Users specify mutations in a simple YAML configuration:

```yaml
- file: src/math.rs
  function: add
  original: a + b
  replacement: a - b
```

The engine uses AST parsing (not text matching) to find and replace expressions, providing:

- **Human-readable configs**: Clear "original → replacement" format
- **AST precision**: Matches syntax structure, not text patterns
- **Clear error reporting**: Detailed feedback when mutations don't match

---

## Solution Overview

### How It Works

```
┌─────────────────┐
│ mutations.yaml  │  User specifies: file, function, original, replacement
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Parse Config   │  Parse original and replacement as AST expressions
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
│  Match Original │  Find AST node matching original expression
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Apply Mutation  │  Replace with mutant, run tests
└─────────────────┘
```

### Why AST-Based Matching?

Text-based matching (`a + b` as string) is fragile:
- `a+b` and `a + b` are different strings but same code
- Comments or formatting break matches
- May match in wrong locations

AST-based matching compares syntax structure:
- `a + b` parses to `ExprBinary { left: "a", op: Add, right: "b" }`
- Whitespace and formatting are ignored
- Matches the actual code structure

---

## Configuration Format

### Human-Readable YAML

```yaml
# mutations.yaml
version: "1.0"

settings:
  timeout: 30  # seconds per test run

mutations:
  # Arithmetic mutations
  - file: src/calculator.rs
    function: add
    original: a + b
    replacement: a - b

  - file: src/calculator.rs
    function: add
    original: a + b
    replacement: a * b

  # Comparison mutations
  - file: src/validator.rs
    function: check_bounds
    original: x >= min
    replacement: x > min

  - file: src/validator.rs
    function: check_bounds
    original: x <= max
    replacement: x < max

  # Logical mutations
  - file: src/validator.rs
    function: is_valid
    original: is_active && has_permission
    replacement: is_active || has_permission

  # Return value mutations
  - file: src/auth.rs
    function: authenticate
    original: password == stored_hash
    replacement: "true"

  - file: src/auth.rs
    function: authenticate
    original: password == stored_hash
    replacement: "false"

  # Literal mutations
  - file: src/config.rs
    function: default_timeout
    original: "30"
    replacement: "0"
```

### Configuration Fields

| Field | Required | Description |
|-------|----------|-------------|
| `file` | Yes | Path to the Rust source file |
| `function` | Yes | Name of the function containing the code |
| `original` | Yes | The code to find (parsed as AST) |
| `replacement` | Yes | The code to replace it with |
| `id` | No | Optional unique identifier (auto-generated if omitted) |

### Why This Format Works

1. **Clear intent**: See exactly what changes from what
2. **Human-readable**: No need to understand AST internals
3. **Flexible**: Works for any valid Rust expression
4. **Explicit**: No ambiguity about what gets replaced

---

## Mutation Matching Algorithm

### Step 1: Parse Original and Replacement

```rust
/// Parse both original and replacement code into AST expressions
fn parse_mutation(mutation: &MutationConfig) -> Result<ParsedMutation, ParseError> {
    let original = syn::parse_str::<syn::Expr>(&mutation.original)
        .map_err(|e| ParseError::InvalidOriginal {
            code: mutation.original.clone(),
            error: e.to_string(),
        })?;

    let replacement = syn::parse_str::<syn::Expr>(&mutation.replacement)
        .map_err(|e| ParseError::InvalidReplacement {
            code: mutation.replacement.clone(),
            error: e.to_string(),
        })?;

    Ok(ParsedMutation { original, replacement })
}
```

### Step 2: Search for Original in Function

```rust
/// Find the original expression within the target function
struct ExpressionMatcher {
    target: syn::Expr,        // The AST to find
    function_name: String,
    matches: Vec<MatchedSite>,
    in_target_function: bool,
}

impl<'ast> Visit<'ast> for ExpressionMatcher {
    fn visit_item_fn(&mut self, func: &'ast syn::ItemFn) {
        if func.sig.ident == self.function_name {
            self.in_target_function = true;
            syn::visit::visit_item_fn(self, func);
            self.in_target_function = false;
        }
    }

    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if self.in_target_function && ast_equals(expr, &self.target) {
            self.matches.push(MatchedSite {
                span: get_span(expr),
                line: get_line(expr),
            });
        }
        // Continue searching in child expressions
        syn::visit::visit_expr(self, expr);
    }
}

/// Compare two AST expressions for structural equality
fn ast_equals(a: &syn::Expr, b: &syn::Expr) -> bool {
    // Compare structure, ignoring spans/whitespace
    match (a, b) {
        (syn::Expr::Binary(a), syn::Expr::Binary(b)) => {
            ast_equals(&a.left, &b.left)
                && binop_equals(&a.op, &b.op)
                && ast_equals(&a.right, &b.right)
        }
        (syn::Expr::Lit(a), syn::Expr::Lit(b)) => {
            lit_equals(&a.lit, &b.lit)
        }
        (syn::Expr::Path(a), syn::Expr::Path(b)) => {
            path_equals(&a.path, &b.path)
        }
        // ... handle other expression types
        _ => false,
    }
}
```

### Step 3: Handle Match Results

```rust
fn find_mutation_target(
    ast: &syn::File,
    mutation: &MutationConfig,
    parsed: &ParsedMutation,
) -> Result<MatchedSite, MutationError> {
    let mut matcher = ExpressionMatcher {
        target: parsed.original.clone(),
        function_name: mutation.function.clone(),
        matches: Vec::new(),
        in_target_function: false,
    };

    matcher.visit_file(ast);

    match matcher.matches.len() {
        0 => Err(MutationError::NoMatch {
            file: mutation.file.clone(),
            function: mutation.function.clone(),
            original: mutation.original.clone(),
        }),

        1 => Ok(matcher.matches.remove(0)),

        n => Err(MutationError::AmbiguousMatch {
            function: mutation.function.clone(),
            original: mutation.original.clone(),
            match_count: n,
            locations: matcher.matches,
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
    /// Original code couldn't be parsed as valid Rust
    #[error("Invalid original expression: {code}")]
    InvalidOriginal {
        code: String,
        parse_error: String,
    },

    /// Replacement code couldn't be parsed as valid Rust
    #[error("Invalid replacement expression: {code}")]
    InvalidReplacement {
        code: String,
        parse_error: String,
    },

    /// Target file doesn't exist
    #[error("File not found: {file}")]
    FileNotFound {
        file: PathBuf,
    },

    /// Target function not found in file
    #[error("Function '{function}' not found in {file}")]
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
    #[error("Found {match_count} matches for '{original}' in '{function}'")]
    AmbiguousMatch {
        function: String,
        original: String,
        match_count: usize,
        locations: Vec<MatchedSite>,
    },
}
```

### User-Friendly Error Messages

```
Error: Expression 'a + b' not found in function 'add'
  --> src/calculator.rs

  The function 'add' does not contain the expression 'a + b'.

  Check that:
    - Variable names match exactly (a, b vs x, y)
    - The expression exists in this function
    - Whitespace doesn't matter, but structure does

  Available expressions in 'add':
    - x + y  (line 5)
    - result * 2  (line 6)
```

```
Error: Found 2 matches for 'a + b' in function 'calculate'
  --> src/math.rs

  The expression 'a + b' appears multiple times:
    1. Line 10, column 12
    2. Line 15, column 8

  To fix: Make the original expression more specific,
  or split into separate mutations for each location.
```

```
Error: Function 'subtract' not found in src/calculator.rs

  Available functions in this file:
    - add
    - multiply
    - divide
```

### Validation Before Running

```rust
/// Validate all mutations before running any tests
pub fn validate_config(config: &Config) -> Result<(), Vec<MutationError>> {
    let mut errors = Vec::new();

    for mutation in &config.mutations {
        // 1. Check file exists
        if !mutation.file.exists() {
            errors.push(MutationError::FileNotFound {
                file: mutation.file.clone(),
            });
            continue;
        }

        // 2. Parse original expression
        if let Err(e) = syn::parse_str::<syn::Expr>(&mutation.original) {
            errors.push(MutationError::InvalidOriginal {
                code: mutation.original.clone(),
                parse_error: e.to_string(),
            });
            continue;
        }

        // 3. Parse replacement expression
        if let Err(e) = syn::parse_str::<syn::Expr>(&mutation.replacement) {
            errors.push(MutationError::InvalidReplacement {
                code: mutation.replacement.clone(),
                parse_error: e.to_string(),
            });
            continue;
        }

        // 4. Parse source file and find function
        let source = std::fs::read_to_string(&mutation.file)?;
        let ast = syn::parse_file(&source)?;

        let functions = collect_function_names(&ast);
        if !functions.contains(&mutation.function) {
            errors.push(MutationError::FunctionNotFound {
                file: mutation.file.clone(),
                function: mutation.function.clone(),
                available_functions: functions,
            });
            continue;
        }

        // 5. Find the original expression in the function
        let matches = find_expression(&ast, &mutation.function, &mutation.original);
        match matches.len() {
            0 => errors.push(MutationError::NoMatch {
                file: mutation.file.clone(),
                function: mutation.function.clone(),
                original: mutation.original.clone(),
            }),
            1 => {} // OK
            n => errors.push(MutationError::AmbiguousMatch {
                function: mutation.function.clone(),
                original: mutation.original.clone(),
                match_count: n,
                locations: matches,
            }),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
    pub original: String,
    pub replacement: String,
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
use syn::{Expr, visit::Visit};

/// Find all occurrences of an expression within a function
pub fn find_expression(
    ast: &syn::File,
    function_name: &str,
    original: &str,
) -> Vec<MatchedSite> {
    // Parse the original expression
    let target: syn::Expr = syn::parse_str(original)
        .expect("original should be pre-validated");

    let mut matcher = ExpressionMatcher {
        target,
        function_name: function_name.to_string(),
        matches: Vec::new(),
        in_target_function: false,
    };

    matcher.visit_file(ast);
    matcher.matches
}

struct ExpressionMatcher {
    target: syn::Expr,
    function_name: String,
    matches: Vec<MatchedSite>,
    in_target_function: bool,
}

impl<'ast> Visit<'ast> for ExpressionMatcher {
    fn visit_item_fn(&mut self, func: &'ast syn::ItemFn) {
        if func.sig.ident == self.function_name {
            self.in_target_function = true;
            syn::visit::visit_item_fn(self, func);
            self.in_target_function = false;
        }
    }

    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if self.in_target_function && ast_equals(expr, &self.target) {
            self.matches.push(MatchedSite::from_expr(expr));
        }
        syn::visit::visit_expr(self, expr);
    }
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
/// Generate source code from AST
pub fn generate_source(ast: &syn::File) -> String {
    prettyplease::unparse(ast)
}

/// Apply mutation and generate mutated source
pub fn apply_mutation(
    source: &str,
    mutation: &MutationConfig,
) -> Result<String, MutationError> {
    // Parse the source file
    let mut ast = syn::parse_file(source)?;

    // Parse original and replacement expressions
    let original_expr: syn::Expr = syn::parse_str(&mutation.original)?;
    let replacement_expr: syn::Expr = syn::parse_str(&mutation.replacement)?;

    // Find the original expression in the function
    let matches = find_expression(&ast, &mutation.function, &mutation.original);

    let target = match matches.len() {
        0 => return Err(MutationError::NoMatch { /* ... */ }),
        1 => &matches[0],
        n => return Err(MutationError::AmbiguousMatch { /* ... */ }),
    };

    // Apply the mutation
    Mutator::apply(&mut ast, target, &replacement_expr)?;

    // Generate the mutated source
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
  original: a + b
  replacement: a - b
```

**Source file (`src/math.rs`):**
```rust
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

**How it works:**
1. Parse `a + b` as AST → `ExprBinary { left: "a", op: Add, right: "b" }`
2. Parse source file into AST
3. Find function `add` in AST
4. Search for expression matching `a + b`
5. Replace with `a - b`
6. Run tests

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
  original: age >= 18
  replacement: age > 18
```

**Source:**
```rust
pub fn is_adult(age: u32) -> bool {
    age >= 18
}
```

**Result:** Tests boundary condition - does the test catch off-by-one?

### Example 3: Literal Mutation

**Configuration:**
```yaml
- file: src/config.rs
  function: default_timeout
  original: "30"
  replacement: "0"
```

**Source:**
```rust
pub fn default_timeout() -> u64 {
    30
}
```

**Result:** Tests if code handles zero timeout correctly.

### Example 4: Error Case - No Match

**Configuration:**
```yaml
- file: src/math.rs
  function: add
  original: x + y
  replacement: x - y
```

**Error output:**
```
Error: Expression 'x + y' not found in function 'add'
  --> src/math.rs

  The function 'add' does not contain 'x + y'.

  Check that variable names match exactly.
  The function uses: a, b (not x, y)
```

### Example 5: Error Case - Ambiguous Match

**Configuration:**
```yaml
- file: src/math.rs
  function: complex
  original: a + b
  replacement: a - b
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
Error: Found 2 matches for 'a + b' in function 'complex'
  --> src/math.rs

  1. Line 2, column 15
  2. Line 3, column 17

  To fix: Use a more specific expression that only matches once.
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

1. **Users specify mutations** with explicit `original` and `replacement` expressions
2. **AST-based matching** finds the original expression regardless of whitespace/formatting
3. **Clear error messages** explain exactly what went wrong and how to fix it
4. **Validation happens upfront** before any tests run

**Config format:**
```yaml
- file: src/math.rs
  function: add
  original: a + b
  replacement: a - b
```

The approach prioritizes **clarity** and **predictability** - you see exactly what will change.
