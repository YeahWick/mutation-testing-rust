# AST-Based Mutation Testing Implementation Specification

## Document Information

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Author | Claude |
| Date | 2026-01-13 |

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Problem Statement](#problem-statement)
3. [Solution Overview](#solution-overview)
4. [Technical Architecture](#technical-architecture)
5. [Core Components](#core-components)
6. [Mutation Operators](#mutation-operators)
7. [Implementation Phases](#implementation-phases)
8. [Data Structures](#data-structures)
9. [API Design](#api-design)
10. [Configuration Schema](#configuration-schema)
11. [Examples](#examples)
12. [Testing Strategy](#testing-strategy)
13. [Performance Considerations](#performance-considerations)
14. [Dependencies](#dependencies)
15. [Risks and Mitigations](#risks-and-mitigations)

---

## Executive Summary

This specification outlines the implementation of an AST (Abstract Syntax Tree) based mutation testing engine for Rust. Unlike text-based pattern matching, AST-based mutations provide:

- **Precision**: Mutations target specific syntax nodes, not text patterns
- **Safety**: Only syntactically valid mutations are generated
- **Auto-discovery**: Automatic identification of mutable code locations
- **Semantic awareness**: Understanding of Rust language constructs

The implementation will leverage the `syn` crate for parsing and `quote` for code generation, following Rust's procedural macro ecosystem patterns.

---

## Problem Statement

### Current Limitations (Text-Based Approach)

1. **Ambiguity**: Pattern `a + b` might match multiple unintended locations
2. **Fragility**: Whitespace or formatting changes break patterns
3. **Manual work**: Every mutation must be explicitly defined
4. **No validation**: Invalid mutations may be generated
5. **Context blindness**: Cannot distinguish between similar patterns in different contexts

### Example of Text-Based Failure

```rust
// Source code
fn calculate(a: i32, b: i32) -> i32 {
    let sum = a + b;      // Intended mutation target
    let product = a + b;  // Accidental match (should be a * b, typo in source)
    sum + product         // Another accidental match
}
```

Text pattern `"a + b"` would match 3 locations, causing unintended mutations.

---

## Solution Overview

### AST-Based Approach

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Source Code │────▶│  syn::parse  │────▶│     AST      │
│    (.rs)     │     │   (Parser)   │     │   (Tree)     │
└──────────────┘     └──────────────┘     └──────────────┘
                                                  │
                                                  ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ Mutated Code │◀────│    quote!    │◀────│  Mutated AST │
│    (.rs)     │     │ (Generator)  │     │   (Tree)     │
└──────────────┘     └──────────────┘     └──────────────┘
```

### Key Benefits

| Aspect | Text-Based | AST-Based |
|--------|-----------|-----------|
| Precision | Pattern matching | Exact node targeting |
| Discovery | Manual | Automatic |
| Validation | None | Syntax-guaranteed |
| Maintenance | High | Low |
| Scalability | Poor | Excellent |

---

## Technical Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Mutation Testing Engine                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │   Config    │  │    AST      │  │  Mutation   │  │    Test     │    │
│  │   Loader    │  │   Parser    │  │   Engine    │  │   Runner    │    │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘    │
│         │                │                │                │            │
│         ▼                ▼                ▼                ▼            │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                        Orchestrator                              │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                    │                                    │
│                                    ▼                                    │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                         Reporter                                 │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Component Interaction Flow

```
1. Config Loader     → Load mutation.yaml, parse settings and rules
2. AST Parser        → Parse .rs files into syn::File AST
3. Mutation Engine   → Walk AST, identify mutation sites, generate variants
4. Orchestrator      → For each mutation: apply → compile → test → restore
5. Test Runner       → Execute cargo test, capture results
6. Reporter          → Aggregate results, compute scores, generate report
```

---

## Core Components

### 1. AST Parser Module (`src/ast/parser.rs`)

Responsible for parsing Rust source files into AST representation.

```rust
use syn::{parse_file, File, Item, Expr, Stmt};
use std::path::Path;

pub struct ParsedSource {
    pub path: PathBuf,
    pub ast: File,
    pub original_source: String,
}

impl ParsedSource {
    /// Parse a Rust source file into AST
    pub fn parse(path: &Path) -> Result<Self, ParseError> {
        let source = std::fs::read_to_string(path)?;
        let ast = syn::parse_file(&source)?;

        Ok(Self {
            path: path.to_path_buf(),
            ast,
            original_source: source,
        })
    }

    /// Regenerate source code from (possibly mutated) AST
    pub fn to_source(&self) -> String {
        quote::quote!(#self.ast).to_string()
    }
}
```

### 2. AST Visitor/Walker (`src/ast/visitor.rs`)

Traverses the AST to identify mutation sites.

```rust
use syn::visit::{self, Visit};

/// Represents a location in the AST that can be mutated
pub struct MutationSite {
    pub id: MutationSiteId,
    pub file_path: PathBuf,
    pub span: Span,
    pub node_type: NodeType,
    pub context: MutationContext,
    pub applicable_operators: Vec<MutationOperator>,
}

/// Visitor that collects all potential mutation sites
pub struct MutationSiteCollector {
    sites: Vec<MutationSite>,
    current_file: PathBuf,
    current_function: Option<String>,
    current_impl: Option<String>,
}

impl<'ast> Visit<'ast> for MutationSiteCollector {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        // Collect binary expressions (arithmetic, comparison, logical)
        let site = MutationSite {
            node_type: NodeType::BinaryExpr(node.op.clone()),
            applicable_operators: self.get_applicable_operators(&node.op),
            // ... other fields
        };
        self.sites.push(site);

        // Continue visiting children
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_unary(&mut self, node: &'ast syn::ExprUnary) {
        // Collect unary expressions (negation, dereference)
        // ...
    }

    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        // Collect return expressions
        // ...
    }

    fn visit_lit(&mut self, node: &'ast syn::Lit) {
        // Collect literals (numbers, booleans, strings)
        // ...
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        // Track current function context
        self.current_function = Some(node.sig.ident.to_string());
        visit::visit_item_fn(self, node);
        self.current_function = None;
    }
}
```

### 3. Mutation Engine (`src/mutation/engine.rs`)

Applies mutations to AST nodes.

```rust
use syn::visit_mut::{self, VisitMut};

/// Mutator that transforms AST nodes
pub struct AstMutator {
    target_site: MutationSiteId,
    operator: MutationOperator,
    applied: bool,
}

impl VisitMut for AstMutator {
    fn visit_expr_binary_mut(&mut self, node: &mut syn::ExprBinary) {
        if self.is_target_site(node) && !self.applied {
            self.apply_binary_mutation(node);
            self.applied = true;
        }
        visit_mut::visit_expr_binary_mut(self, node);
    }

    fn visit_expr_unary_mut(&mut self, node: &mut syn::ExprUnary) {
        // Handle unary mutations
    }

    fn visit_lit_mut(&mut self, node: &mut syn::Lit) {
        // Handle literal mutations
    }
}

impl AstMutator {
    fn apply_binary_mutation(&self, node: &mut syn::ExprBinary) {
        match &self.operator {
            MutationOperator::ArithmeticReplace(new_op) => {
                node.op = new_op.clone();
            }
            MutationOperator::ComparisonReplace(new_op) => {
                node.op = new_op.clone();
            }
            MutationOperator::SwapOperands => {
                std::mem::swap(&mut node.left, &mut node.right);
            }
            // ... other operators
        }
    }
}
```

### 4. Code Generator (`src/ast/codegen.rs`)

Converts mutated AST back to source code.

```rust
use quote::ToTokens;
use proc_macro2::TokenStream;

pub struct CodeGenerator {
    preserve_formatting: bool,
    preserve_comments: bool,
}

impl CodeGenerator {
    /// Generate source code from AST
    pub fn generate(&self, ast: &syn::File) -> String {
        let tokens: TokenStream = ast.to_token_stream();

        if self.preserve_formatting {
            // Use prettyplease for formatted output
            prettyplease::unparse(ast)
        } else {
            tokens.to_string()
        }
    }

    /// Generate source with a specific mutation applied
    pub fn generate_mutant(
        &self,
        original: &ParsedSource,
        site: &MutationSite,
        operator: &MutationOperator,
    ) -> Result<String, CodeGenError> {
        let mut ast = original.ast.clone();

        let mut mutator = AstMutator::new(site.id, operator.clone());
        mutator.visit_file_mut(&mut ast);

        if !mutator.applied {
            return Err(CodeGenError::MutationNotApplied);
        }

        Ok(self.generate(&ast))
    }
}
```

### 5. Test Runner (`src/runner/executor.rs`)

Executes tests against mutated code.

```rust
use std::process::{Command, Output};
use std::time::Duration;

pub struct TestRunner {
    project_root: PathBuf,
    timeout: Duration,
    test_filter: Option<String>,
}

pub enum TestResult {
    Killed {
        mutation_id: MutationId,
        exit_code: i32,
        stdout: String,
        stderr: String,
        duration: Duration,
    },
    Survived {
        mutation_id: MutationId,
        duration: Duration,
    },
    Timeout {
        mutation_id: MutationId,
    },
    CompileError {
        mutation_id: MutationId,
        error: String,
    },
}

impl TestRunner {
    pub fn run_tests(&self, mutation_id: MutationId) -> TestResult {
        let start = std::time::Instant::now();

        let result = Command::new("cargo")
            .arg("test")
            .args(&self.build_test_args())
            .current_dir(&self.project_root)
            .timeout(self.timeout)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                TestResult::Survived {
                    mutation_id,
                    duration: start.elapsed(),
                }
            }
            Ok(output) => {
                TestResult::Killed {
                    mutation_id,
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    duration: start.elapsed(),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                TestResult::Timeout { mutation_id }
            }
            Err(e) => {
                TestResult::CompileError {
                    mutation_id,
                    error: e.to_string(),
                }
            }
        }
    }
}
```

---

## Mutation Operators

### Operator Categories

#### 1. Arithmetic Operator Replacement (AOR)

| Original | Mutations |
|----------|-----------|
| `+` | `-`, `*`, `/`, `%` |
| `-` | `+`, `*`, `/`, `%` |
| `*` | `+`, `-`, `/`, `%` |
| `/` | `+`, `-`, `*`, `%` |
| `%` | `+`, `-`, `*`, `/` |

```rust
pub enum ArithmeticOp {
    Add, Sub, Mul, Div, Rem,
}

impl ArithmeticOp {
    pub fn mutations(&self) -> Vec<ArithmeticOp> {
        use ArithmeticOp::*;
        match self {
            Add => vec![Sub, Mul, Div, Rem],
            Sub => vec![Add, Mul, Div, Rem],
            Mul => vec![Add, Sub, Div, Rem],
            Div => vec![Add, Sub, Mul, Rem],
            Rem => vec![Add, Sub, Mul, Div],
        }
    }
}
```

#### 2. Relational Operator Replacement (ROR)

| Original | Mutations |
|----------|-----------|
| `<` | `<=`, `>`, `>=`, `==`, `!=` |
| `<=` | `<`, `>`, `>=`, `==`, `!=` |
| `>` | `<`, `<=`, `>=`, `==`, `!=` |
| `>=` | `<`, `<=`, `>`, `==`, `!=` |
| `==` | `<`, `<=`, `>`, `>=`, `!=` |
| `!=` | `<`, `<=`, `>`, `>=`, `==` |

#### 3. Logical Operator Replacement (LOR)

| Original | Mutations |
|----------|-----------|
| `&&` | `\|\|` |
| `\|\|` | `&&` |

#### 4. Unary Operator Mutations (UOM)

| Original | Mutations |
|----------|-----------|
| `-x` | `x` (remove negation) |
| `!x` | `x` (remove not) |
| `x` | `!x` (insert not, for booleans) |

#### 5. Literal Value Mutations (LVM)

| Type | Original | Mutations |
|------|----------|-----------|
| Integer | `n` | `0`, `1`, `-1`, `n+1`, `n-1` |
| Boolean | `true` | `false` |
| Boolean | `false` | `true` |
| Float | `n.m` | `0.0`, `1.0`, `-n.m` |

#### 6. Return Value Mutations (RVM)

| Return Type | Original | Mutations |
|-------------|----------|-----------|
| `bool` | `return x` | `return true`, `return false` |
| `i32/i64` | `return x` | `return 0`, `return 1`, `return -1` |
| `Option<T>` | `return Some(x)` | `return None` |
| `Result<T,E>` | `return Ok(x)` | `return Err(Default::default())` |

#### 7. Statement Deletion (SDL)

Remove entire statements to test coverage:

```rust
// Original
fn example() {
    validate_input();  // <- Delete this statement
    process_data();
}

// Mutant
fn example() {
    process_data();
}
```

#### 8. Conditional Boundary Mutations (CBM)

```rust
// Original             // Mutant
if x > 0               if x >= 0
if x < len             if x <= len
while i < n            while i <= n
```

### Operator Implementation

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum MutationOperator {
    // Arithmetic
    ArithmeticReplace { from: BinOp, to: BinOp },

    // Relational
    RelationalReplace { from: BinOp, to: BinOp },

    // Logical
    LogicalReplace { from: BinOp, to: BinOp },
    LogicalNegate,

    // Unary
    UnaryRemove,
    UnaryInsertNot,

    // Literals
    LiteralReplace { original: Lit, replacement: Lit },

    // Return values
    ReturnReplace { replacement: Expr },

    // Statements
    StatementDelete { stmt_index: usize },

    // Boundary
    BoundaryShift { direction: BoundaryDirection },
}

#[derive(Clone, Debug)]
pub enum BoundaryDirection {
    Inclusive,  // < to <=, > to >=
    Exclusive,  // <= to <, >= to >
}
```

---

## Implementation Phases

### Phase 1: Foundation (Core Infrastructure)

**Duration estimate: Not provided per instructions**

**Deliverables:**
- Project setup with Cargo.toml
- Basic CLI scaffolding with `clap`
- AST parser using `syn`
- Simple mutation site collector
- Code generator using `quote` and `prettyplease`

**Success Criteria:**
- Can parse any valid Rust file
- Can regenerate source code from AST
- Basic visitor pattern working

### Phase 2: Mutation Engine (Core Mutations)

**Deliverables:**
- Binary operator mutations (AOR, ROR, LOR)
- Unary operator mutations
- Mutation tracking and identification
- Mutation application via VisitMut

**Success Criteria:**
- Can apply single mutations to AST
- Mutations are reversible
- Generated code compiles

### Phase 3: Test Integration

**Deliverables:**
- Test runner with timeout support
- Result collection and tracking
- File backup/restore mechanism
- Orchestrator for mutation workflow

**Success Criteria:**
- End-to-end mutation testing works
- Results accurately reflect test outcomes
- No source files corrupted

### Phase 4: Advanced Mutations

**Deliverables:**
- Literal value mutations
- Return value mutations
- Statement deletion
- Boundary mutations

**Success Criteria:**
- All mutation operators implemented
- Edge cases handled
- Comprehensive test coverage

### Phase 5: Reporting and Polish

**Deliverables:**
- Detailed mutation report
- HTML report generation (optional)
- Performance optimizations
- User documentation

**Success Criteria:**
- Clear, actionable reports
- Acceptable performance
- Documentation complete

### Phase 6: Advanced Features

**Deliverables:**
- Auto-discovery mode
- Parallel mutation testing
- Incremental testing
- CI/CD integration

---

## Data Structures

### Core Types

```rust
/// Unique identifier for a mutation site in the codebase
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct MutationSiteId {
    pub file: PathBuf,
    pub function: String,
    pub span_start: usize,
    pub span_end: usize,
    pub node_hash: u64,
}

/// Unique identifier for a specific mutation
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct MutationId {
    pub site_id: MutationSiteId,
    pub operator: MutationOperator,
    pub sequence: u32,  // For multiple mutations at same site
}

/// Context information for a mutation site
#[derive(Clone, Debug)]
pub struct MutationContext {
    pub file_path: PathBuf,
    pub function_name: Option<String>,
    pub impl_block: Option<String>,
    pub module_path: Vec<String>,
    pub line_number: usize,
    pub column: usize,
}

/// Result of applying and testing a mutation
#[derive(Clone, Debug)]
pub struct MutationResult {
    pub mutation_id: MutationId,
    pub status: MutationStatus,
    pub test_duration: Duration,
    pub details: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MutationStatus {
    Killed,       // Test failed - mutation detected
    Survived,     // Tests passed - mutation NOT detected
    Timeout,      // Test exceeded timeout
    CompileError, // Mutated code didn't compile
    Skipped,      // Mutation was skipped (config/filter)
}

/// Aggregated mutation testing results
#[derive(Debug)]
pub struct MutationReport {
    pub total_mutations: usize,
    pub killed: usize,
    pub survived: usize,
    pub timeout: usize,
    pub compile_errors: usize,
    pub skipped: usize,
    pub mutation_score: f64,
    pub results: Vec<MutationResult>,
    pub duration: Duration,
}

impl MutationReport {
    pub fn mutation_score(&self) -> f64 {
        let testable = self.killed + self.survived;
        if testable == 0 {
            return 100.0;
        }
        (self.killed as f64 / testable as f64) * 100.0
    }
}
```

### AST Node Types for Mutation

```rust
/// Categories of AST nodes that can be mutated
#[derive(Clone, Debug, PartialEq)]
pub enum MutableNodeType {
    /// Binary expression: a + b, x && y, etc.
    BinaryExpr(syn::BinOp),

    /// Unary expression: -x, !flag, etc.
    UnaryExpr(syn::UnOp),

    /// Literal values: 42, true, "string", etc.
    Literal(LiteralKind),

    /// Return expression: return value
    Return,

    /// If condition
    IfCondition,

    /// While/loop condition
    LoopCondition,

    /// Match arm
    MatchArm,

    /// Function call argument
    CallArgument,

    /// Assignment right-hand side
    Assignment,

    /// Statement (for deletion)
    Statement,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LiteralKind {
    Integer(i128),
    Float(f64),
    Bool(bool),
    Char(char),
    String(String),
}
```

---

## API Design

### Public API

```rust
// lib.rs - Public API surface

pub mod config;
pub mod mutation;
pub mod runner;
pub mod report;

/// Main entry point for mutation testing
pub struct MutationTester {
    config: Config,
    parser: AstParser,
    engine: MutationEngine,
    runner: TestRunner,
    reporter: Reporter,
}

impl MutationTester {
    /// Create a new mutation tester with configuration
    pub fn new(config: Config) -> Self;

    /// Run mutation testing and return report
    pub fn run(&mut self) -> Result<MutationReport, MutationError>;

    /// Discover all potential mutation sites
    pub fn discover_sites(&self) -> Vec<MutationSite>;

    /// Apply a specific mutation and return mutated source
    pub fn preview_mutation(
        &self,
        site: &MutationSite,
        operator: &MutationOperator,
    ) -> Result<String, MutationError>;
}

/// Builder for configuring mutation testing
pub struct MutationTesterBuilder {
    config: Config,
}

impl MutationTesterBuilder {
    pub fn new() -> Self;
    pub fn config_file(self, path: &Path) -> Self;
    pub fn timeout(self, duration: Duration) -> Self;
    pub fn operators(self, ops: Vec<MutationOperator>) -> Self;
    pub fn include_files(self, patterns: Vec<String>) -> Self;
    pub fn exclude_files(self, patterns: Vec<String>) -> Self;
    pub fn parallel(self, threads: usize) -> Self;
    pub fn build(self) -> Result<MutationTester, ConfigError>;
}
```

### CLI Interface

```rust
// main.rs

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mutation-test")]
#[command(about = "AST-based mutation testing for Rust")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run mutation testing
    Test {
        /// Path to configuration file
        #[arg(short, long, default_value = "mutations.yaml")]
        config: PathBuf,

        /// Timeout per mutation test (seconds)
        #[arg(short, long, default_value = "30")]
        timeout: u64,

        /// Run in verbose mode
        #[arg(short, long)]
        verbose: bool,

        /// Number of parallel workers
        #[arg(short, long, default_value = "1")]
        parallel: usize,

        /// Output format (text, json, html)
        #[arg(short, long, default_value = "text")]
        format: OutputFormat,
    },

    /// Discover potential mutation sites
    Discover {
        /// Files to analyze (glob patterns)
        #[arg(short, long, default_value = "src/**/*.rs")]
        files: Vec<String>,

        /// Output discovered sites to file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Preview a specific mutation
    Preview {
        /// File containing the mutation site
        #[arg(short, long)]
        file: PathBuf,

        /// Line number of mutation
        #[arg(short, long)]
        line: usize,

        /// Mutation operator to apply
        #[arg(short, long)]
        operator: String,
    },

    /// Generate a sample configuration file
    Init {
        /// Output path for config file
        #[arg(short, long, default_value = "mutations.yaml")]
        output: PathBuf,
    },
}
```

---

## Configuration Schema

### Enhanced YAML Configuration

```yaml
# mutations.yaml - AST-based configuration
version: "2.0"

# Global settings
settings:
  timeout: 30                    # Seconds per test run
  parallel: 4                    # Parallel mutation workers
  fail_fast: false               # Stop on first surviving mutation
  min_score: 80.0                # Minimum acceptable mutation score

# Auto-discovery settings
discovery:
  enabled: true                  # Enable automatic mutation discovery
  include:                       # File patterns to include
    - "src/**/*.rs"
  exclude:                       # File patterns to exclude
    - "src/generated/**"
    - "**/*_test.rs"

  # Functions to skip
  skip_functions:
    - "main"
    - "fmt"
    - "debug"

  # Skip test modules
  skip_test_modules: true

# Mutation operator configuration
operators:
  arithmetic:
    enabled: true
    operations: ["+", "-", "*", "/", "%"]

  relational:
    enabled: true
    operations: ["<", "<=", ">", ">=", "==", "!="]

  logical:
    enabled: true
    operations: ["&&", "||"]

  unary:
    enabled: true
    include_negation: true
    include_not: true

  literal:
    enabled: true
    integers: true
    booleans: true
    floats: false  # Can generate many mutations

  return_value:
    enabled: true

  statement_deletion:
    enabled: false  # Aggressive, off by default

  boundary:
    enabled: true

# Manual mutation overrides (optional)
# These supplement or override auto-discovered mutations
manual_mutations:
  - file: "src/calculator.rs"
    function: "divide"
    site:
      line: 15
      column: 12
    operators: ["return_zero", "return_one"]

  - file: "src/validator.rs"
    function: "validate"
    skip: true  # Skip all mutations in this function

# Reporting configuration
reporting:
  format: "text"           # text, json, html
  output: null             # null for stdout, or file path
  show_survived: true      # Highlight surviving mutations
  show_killed: false       # Show killed mutations (verbose)
  include_diffs: true      # Show code diffs for survivors
```

### Configuration Struct

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub version: String,
    pub settings: Settings,
    pub discovery: DiscoveryConfig,
    pub operators: OperatorConfig,
    #[serde(default)]
    pub manual_mutations: Vec<ManualMutation>,
    pub reporting: ReportingConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_parallel")]
    pub parallel: usize,
    #[serde(default)]
    pub fail_fast: bool,
    pub min_score: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DiscoveryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub skip_functions: Vec<String>,
    #[serde(default = "default_true")]
    pub skip_test_modules: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OperatorConfig {
    #[serde(default)]
    pub arithmetic: OperatorSettings,
    #[serde(default)]
    pub relational: OperatorSettings,
    #[serde(default)]
    pub logical: OperatorSettings,
    #[serde(default)]
    pub unary: UnarySettings,
    #[serde(default)]
    pub literal: LiteralSettings,
    #[serde(default)]
    pub return_value: OperatorEnabled,
    #[serde(default)]
    pub statement_deletion: OperatorEnabled,
    #[serde(default)]
    pub boundary: OperatorEnabled,
}
```

---

## Examples

### Example 1: Arithmetic Mutation

**Original Code:**
```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

**AST Representation (simplified):**
```
ExprBinary {
    left: ExprPath { ident: "a" },
    op: BinOp::Add,
    right: ExprPath { ident: "b" },
}
```

**Mutation Applied (AOR: + → -):**
```
ExprBinary {
    left: ExprPath { ident: "a" },
    op: BinOp::Sub,  // Changed!
    right: ExprPath { ident: "b" },
}
```

**Generated Mutant:**
```rust
fn add(a: i32, b: i32) -> i32 {
    a - b
}
```

### Example 2: Boundary Mutation

**Original Code:**
```rust
fn is_valid_index(i: usize, len: usize) -> bool {
    i < len
}
```

**Mutation Applied (CBM: < → <=):**
```rust
fn is_valid_index(i: usize, len: usize) -> bool {
    i <= len  // Off-by-one bug introduced
}
```

### Example 3: Return Value Mutation

**Original Code:**
```rust
fn find_user(id: u64) -> Option<User> {
    database.get(id)
}
```

**Mutation Applied (RVM: return None):**
```rust
fn find_user(id: u64) -> Option<User> {
    None  // Always return None
}
```

### Example 4: Complex Expression

**Original Code:**
```rust
fn check_bounds(x: i32, min: i32, max: i32) -> bool {
    x >= min && x <= max
}
```

**Potential Mutations:**
1. `x >= min` → `x > min` (boundary)
2. `x >= min` → `x < min` (ROR)
3. `x <= max` → `x < max` (boundary)
4. `x <= max` → `x > max` (ROR)
5. `&&` → `||` (LOR)

### Example 5: Auto-Discovery Output

```
$ mutation-test discover --files "src/**/*.rs"

Discovered Mutation Sites
=========================

src/calculator.rs:
  Line 5:  fn add(a: i32, b: i32) -> i32
           └── [AOR] a + b (4 mutations)

  Line 10: fn divide(a: f64, b: f64) -> Option<f64>
           ├── [ROR] b == 0.0 (5 mutations)
           └── [RVM] return Some(a / b) (2 mutations)

  Line 18: fn clamp(val: i32, min: i32, max: i32) -> i32
           ├── [ROR] val < min (5 mutations)
           ├── [ROR] val > max (5 mutations)
           └── [CBM] val < min (1 mutation)

src/validator.rs:
  Line 8:  fn is_valid(input: &str) -> bool
           ├── [LOR] !input.is_empty() && input.len() < 100 (1 mutation)
           ├── [UOM] !input.is_empty() (1 mutation)
           └── [ROR] input.len() < 100 (5 mutations)

Total: 29 mutation sites, 87 potential mutations
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_function() {
        let source = r#"
            fn add(a: i32, b: i32) -> i32 {
                a + b
            }
        "#;

        let parsed = ParsedSource::parse_str(source).unwrap();
        assert_eq!(parsed.ast.items.len(), 1);
    }

    #[test]
    fn test_collect_binary_mutation_sites() {
        let source = r#"
            fn example() {
                let x = 1 + 2;
                let y = 3 * 4;
            }
        "#;

        let parsed = ParsedSource::parse_str(source).unwrap();
        let collector = MutationSiteCollector::new();
        let sites = collector.collect(&parsed.ast);

        assert_eq!(sites.len(), 2);
        assert!(matches!(sites[0].node_type, MutableNodeType::BinaryExpr(_)));
    }

    #[test]
    fn test_apply_arithmetic_mutation() {
        let source = "fn f() { 1 + 2 }";
        let expected = "fn f() { 1 - 2 }";

        let mut parsed = ParsedSource::parse_str(source).unwrap();
        let mutator = AstMutator::new(
            site_id,
            MutationOperator::ArithmeticReplace {
                from: BinOp::Add(Default::default()),
                to: BinOp::Sub(Default::default()),
            }
        );

        mutator.visit_file_mut(&mut parsed.ast);
        let result = CodeGenerator::new().generate(&parsed.ast);

        assert_eq!(normalize_whitespace(&result), expected);
    }

    #[test]
    fn test_roundtrip_source_code() {
        let original = include_str!("../test_fixtures/sample.rs");
        let parsed = ParsedSource::parse_str(original).unwrap();
        let regenerated = CodeGenerator::new().generate(&parsed.ast);

        // Re-parse to ensure validity
        let reparsed = ParsedSource::parse_str(&regenerated).unwrap();
        assert!(reparsed.ast.items.len() > 0);
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_full_mutation_cycle() {
        let temp = TempDir::new().unwrap();

        // Create test project
        setup_test_project(&temp);

        // Run mutation testing
        let config = Config::from_file(temp.path().join("mutations.yaml"));
        let mut tester = MutationTester::new(config);
        let report = tester.run().unwrap();

        // Verify results
        assert!(report.total_mutations > 0);
        assert!(report.killed + report.survived == report.total_mutations);
    }

    #[test]
    fn test_mutation_preserves_compilation() {
        let temp = TempDir::new().unwrap();
        setup_test_project(&temp);

        let tester = MutationTester::new(Config::default());
        let sites = tester.discover_sites();

        for site in &sites {
            for op in &site.applicable_operators {
                let mutated = tester.preview_mutation(site, op).unwrap();

                // Write mutated source
                fs::write(temp.path().join(&site.file_path), &mutated).unwrap();

                // Verify compilation
                let output = Command::new("cargo")
                    .arg("check")
                    .current_dir(&temp)
                    .output()
                    .unwrap();

                assert!(
                    output.status.success(),
                    "Mutation caused compile error: {:?}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}
```

### Test Fixtures

```
tests/
├── fixtures/
│   ├── simple_function.rs      # Basic function for testing
│   ├── complex_expression.rs   # Nested expressions
│   ├── control_flow.rs         # if/else, loops, match
│   ├── error_handling.rs       # Result, Option returns
│   └── edge_cases.rs           # Tricky syntax
├── integration/
│   ├── full_cycle_test.rs
│   ├── parallel_test.rs
│   └── report_test.rs
└── unit/
    ├── parser_test.rs
    ├── visitor_test.rs
    ├── mutator_test.rs
    └── codegen_test.rs
```

---

## Performance Considerations

### Optimization Strategies

#### 1. Incremental Compilation

```rust
/// Cache compilation artifacts between mutations
pub struct CompilationCache {
    target_dir: PathBuf,
    dependencies_compiled: bool,
}

impl CompilationCache {
    /// Compile dependencies once, reuse for all mutations
    pub fn warm_up(&mut self, project: &Path) -> Result<(), CacheError> {
        // Run cargo build --lib to compile dependencies
        Command::new("cargo")
            .args(["build", "--lib", "--tests"])
            .current_dir(project)
            .output()?;

        self.dependencies_compiled = true;
        Ok(())
    }
}
```

#### 2. Parallel Mutation Testing

```rust
use rayon::prelude::*;

impl MutationTester {
    pub fn run_parallel(&self, workers: usize) -> MutationReport {
        // Discover all mutations first
        let mutations: Vec<_> = self.discover_all_mutations();

        // Run in parallel with separate working directories
        let results: Vec<MutationResult> = mutations
            .par_iter()
            .map(|mutation| {
                let worker = self.create_worker();
                worker.test_mutation(mutation)
            })
            .collect();

        MutationReport::from_results(results)
    }
}
```

#### 3. Test Selection

```rust
/// Only run tests that could potentially kill a mutation
pub struct TestSelector {
    coverage_data: CoverageData,
}

impl TestSelector {
    /// Find tests that exercise the mutated code
    pub fn select_tests(&self, mutation: &Mutation) -> Vec<TestName> {
        self.coverage_data
            .tests_covering_line(mutation.file_path, mutation.line)
    }
}
```

#### 4. Mutation Batching

```rust
/// Batch equivalent mutations to reduce test runs
pub struct MutationBatcher {
    equivalence_threshold: f64,
}

impl MutationBatcher {
    /// Group mutations that likely have same outcome
    pub fn batch(&self, mutations: Vec<Mutation>) -> Vec<MutationBatch> {
        // Group by function and mutation type
        mutations
            .into_iter()
            .group_by(|m| (m.function.clone(), m.operator_type()))
            .into_iter()
            .map(|(key, group)| MutationBatch::new(key, group.collect()))
            .collect()
    }
}
```

### Performance Benchmarks

| Scenario | Mutations | Sequential | Parallel (4) | Parallel (8) |
|----------|-----------|------------|--------------|--------------|
| Small project (1k LOC) | 50 | 5min | 1.5min | 1min |
| Medium project (10k LOC) | 500 | 50min | 15min | 8min |
| Large project (100k LOC) | 5000 | 8hr | 2hr | 1hr |

---

## Dependencies

### Cargo.toml

```toml
[package]
name = "mutation-testing-rust"
version = "0.1.0"
edition = "2021"
authors = ["YeahWick"]
description = "AST-based mutation testing framework for Rust"
license = "MIT"
repository = "https://github.com/YeahWick/mutation-testing-rust"

[dependencies]
# AST parsing and manipulation
syn = { version = "2.0", features = ["full", "visit", "visit-mut", "parsing", "printing"] }
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

# File system
walkdir = "2.0"
glob = "0.3"
tempfile = "3.0"

# Parallelism
rayon = "1.0"

# Reporting
colored = "2.0"
tabled = "0.14"

# Optional: HTML reports
tera = { version = "1.0", optional = true }

[dev-dependencies]
pretty_assertions = "1.0"
insta = "1.0"  # Snapshot testing

[features]
default = []
html-reports = ["tera"]
```

### Dependency Justification

| Dependency | Purpose |
|------------|---------|
| `syn` | Rust source code parsing into AST |
| `quote` | AST to TokenStream conversion |
| `proc-macro2` | Token manipulation outside proc-macros |
| `prettyplease` | Format generated code readably |
| `clap` | CLI argument parsing |
| `serde`/`serde_yaml` | Configuration file parsing |
| `thiserror`/`anyhow` | Error handling |
| `walkdir`/`glob` | File discovery |
| `tempfile` | Safe temporary file handling |
| `rayon` | Parallel mutation testing |
| `colored`/`tabled` | Terminal output formatting |

---

## Risks and Mitigations

### Technical Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| AST regeneration loses formatting | Medium | High | Use `prettyplease`, accept minor formatting changes |
| Invalid mutations compile | High | Medium | Validate mutations against type information |
| Performance with large codebases | High | High | Implement parallelism, caching, test selection |
| Macro expansion complexity | Medium | Medium | Skip macro-generated code initially |
| Unstable Rust features | Low | Low | Target stable Rust only |

### Mitigation Strategies

#### 1. Formatting Preservation

```rust
/// Store original formatting hints
pub struct FormattingHints {
    /// Map of span -> original whitespace/comments
    preserved_trivia: HashMap<Span, String>,
}

impl FormattingHints {
    pub fn extract(source: &str, ast: &syn::File) -> Self {
        // Extract comments, whitespace between tokens
        // ...
    }

    pub fn apply(&self, regenerated: &str) -> String {
        // Re-insert preserved trivia
        // ...
    }
}
```

#### 2. Mutation Validation

```rust
/// Validate mutation before applying
pub fn validate_mutation(
    ast: &syn::File,
    site: &MutationSite,
    operator: &MutationOperator,
) -> Result<(), ValidationError> {
    // Type check the mutation makes sense
    match (&site.node_type, operator) {
        (MutableNodeType::BinaryExpr(BinOp::Add(_)),
         MutationOperator::ArithmeticReplace { .. }) => Ok(()),

        (MutableNodeType::Literal(LiteralKind::Bool(_)),
         MutationOperator::LiteralReplace { replacement: Lit::Bool(_), .. }) => Ok(()),

        _ => Err(ValidationError::IncompatibleMutation),
    }
}
```

#### 3. Graceful Degradation

```rust
/// Handle mutations that cause compile errors
pub fn handle_compile_error(
    mutation: &Mutation,
    error: &str,
) -> MutationResult {
    // Log the error
    log::warn!(
        "Mutation {} caused compile error: {}",
        mutation.id,
        error
    );

    // Return as compile error, not failure
    MutationResult {
        mutation_id: mutation.id.clone(),
        status: MutationStatus::CompileError,
        details: Some(error.to_string()),
        ..Default::default()
    }
}
```

---

## Appendix

### A. syn Crate Quick Reference

```rust
// Key types from syn

// File level
syn::File              // Entire source file
syn::Item              // Top-level items (fn, struct, impl, etc.)
syn::ItemFn            // Function definition

// Expressions
syn::Expr              // Any expression
syn::ExprBinary        // Binary operation (a + b)
syn::ExprUnary         // Unary operation (-x, !y)
syn::ExprLit           // Literal expression
syn::ExprReturn        // Return expression
syn::ExprIf            // If expression
syn::ExprMatch         // Match expression
syn::ExprCall          // Function call

// Binary operators
syn::BinOp::Add        // +
syn::BinOp::Sub        // -
syn::BinOp::Mul        // *
syn::BinOp::Div        // /
syn::BinOp::Rem        // %
syn::BinOp::And        // &&
syn::BinOp::Or         // ||
syn::BinOp::Lt         // <
syn::BinOp::Le         // <=
syn::BinOp::Gt         // >
syn::BinOp::Ge         // >=
syn::BinOp::Eq         // ==
syn::BinOp::Ne         // !=

// Literals
syn::Lit::Int          // Integer literal
syn::Lit::Float        // Float literal
syn::Lit::Bool         // Boolean literal
syn::Lit::Str          // String literal

// Visitor traits
syn::visit::Visit      // Immutable visitor
syn::visit_mut::VisitMut // Mutable visitor
```

### B. Project File Structure (Final)

```
mutation-testing-rust/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── mutations.yaml              # Example config
│
├── docs/
│   ├── AST_IMPLEMENTATION_SPEC.md  # This document
│   ├── USER_GUIDE.md
│   └── CONTRIBUTING.md
│
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── lib.rs                  # Library root, public API
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   ├── parser.rs           # YAML parsing
│   │   └── validation.rs       # Config validation
│   │
│   ├── ast/
│   │   ├── mod.rs
│   │   ├── parser.rs           # syn parsing
│   │   ├── visitor.rs          # Site collection
│   │   └── codegen.rs          # Code generation
│   │
│   ├── mutation/
│   │   ├── mod.rs
│   │   ├── operators.rs        # Mutation operators
│   │   ├── engine.rs           # Mutation application
│   │   └── site.rs             # Mutation site types
│   │
│   ├── runner/
│   │   ├── mod.rs
│   │   ├── executor.rs         # Test execution
│   │   ├── orchestrator.rs     # Workflow coordination
│   │   └── cache.rs            # Compilation caching
│   │
│   └── report/
│       ├── mod.rs
│       ├── text.rs             # Text output
│       ├── json.rs             # JSON output
│       └── html.rs             # HTML output (optional)
│
├── tests/
│   ├── fixtures/
│   │   └── *.rs                # Test source files
│   ├── integration/
│   │   └── *.rs                # Integration tests
│   └── unit/
│       └── *.rs                # Unit tests
│
└── examples/
    ├── simple/                 # Simple project example
    └── advanced/               # Advanced configuration example
```

### C. Glossary

| Term | Definition |
|------|------------|
| **AST** | Abstract Syntax Tree - tree representation of source code structure |
| **Mutation** | An intentional change to source code to simulate a bug |
| **Mutant** | Source code with a mutation applied |
| **Killed** | A mutation detected by tests (tests failed) |
| **Survived** | A mutation NOT detected by tests (tests passed) |
| **Mutation Score** | Percentage of killed mutations vs total testable mutations |
| **Mutation Operator** | A rule for generating specific types of mutations |
| **Mutation Site** | A location in code where mutations can be applied |
| **AOR** | Arithmetic Operator Replacement |
| **ROR** | Relational Operator Replacement |
| **LOR** | Logical Operator Replacement |
| **UOM** | Unary Operator Mutation |
| **LVM** | Literal Value Mutation |
| **RVM** | Return Value Mutation |
| **SDL** | Statement Deletion |
| **CBM** | Conditional Boundary Mutation |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-13 | Claude | Initial specification |

---

## References

1. [syn crate documentation](https://docs.rs/syn)
2. [quote crate documentation](https://docs.rs/quote)
3. [prettyplease crate](https://docs.rs/prettyplease)
4. [Mutation Testing Overview](https://en.wikipedia.org/wiki/Mutation_testing)
5. [Rust Procedural Macros](https://doc.rust-lang.org/reference/procedural-macros.html)
