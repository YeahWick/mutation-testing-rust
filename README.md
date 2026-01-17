# mutation-testing-rust

An AST-based mutation testing framework for Rust that evaluates test quality by introducing code mutations and checking whether tests detect them.

## Overview

Mutation testing works by:
1. Reading mutation definitions from a YAML configuration file
2. Applying code mutations to your source code using AST-based matching
3. Running your test suite against each mutation
4. Reporting which mutations were "killed" (detected by tests) vs "survived" (undetected)

A higher mutation score (% of killed mutations) indicates a more effective test suite.

## Installation

```bash
# Clone the repository
git clone https://github.com/YeahWick/mutation-testing-rust.git
cd mutation-testing-rust

# Build
cargo build --release
```

## Quick Start

1. Create a `mutations.yaml` file in your project root:

```yaml
version: "1.0"

settings:
  timeout: 30  # seconds per test run

mutations:
  - file: src/calculator.rs
    function: add
    original: a + b
    replacement: a - b
```

2. Run mutation testing:

```bash
# From the mutation-testing-rust directory
cargo run -- test --project /path/to/your/project

# Or if installed globally
mutation-testing-rust test --project /path/to/your/project
```

## Configuration File Format

Mutations are defined in a `mutations.yaml` file:

```yaml
version: "1.0"

settings:
  timeout: 30  # seconds per test run

mutations:
  # Arithmetic operator mutation
  - file: src/calculator.rs
    function: add
    original: a + b
    replacement: a - b

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
```

### Configuration Fields

| Field | Required | Description |
|-------|----------|-------------|
| `version` | Yes | Config format version (use "1.0") |
| `settings.timeout` | No | Maximum seconds for each test run (default: 30) |
| `mutations[].file` | Yes | Path to the Rust source file |
| `mutations[].function` | Yes | Name of the function containing the code |
| `mutations[].original` | Yes | Expression to find (must be valid Rust) |
| `mutations[].replacement` | Yes | Expression to replace it with |
| `mutations[].id` | No | Optional unique identifier (auto-generated if omitted) |

## Usage

### Commands

```bash
# Run mutation tests
mutation-testing-rust test [OPTIONS]

# Validate configuration without running tests
mutation-testing-rust validate [OPTIONS]

# Show example configuration
mutation-testing-rust example
```

### Options

```
-c, --config <FILE>     Path to mutations config file [default: mutations.yaml]
-p, --project <DIR>     Project directory [default: current directory]
-v, --verbose           Enable verbose output
```

### Example Output

```
Loading configuration...
Found 4 mutation(s) in config
Validating mutations...
All mutations valid. Running tests...

Mutation Testing Report
============================================================

[KILLED]   mutation_1 - a + b -> a - b
        src/calculator.rs:5 in function 'add'
[KILLED]   mutation_2 - a + b -> a * b
        src/calculator.rs:5 in function 'add'
[SURVIVED] mutation_3 - age >= 18 -> age > 18
        src/validator.rs:12 in function 'is_adult'
[KILLED]   mutation_4 - x && y -> x || y
        src/auth.rs:8 in function 'check_access'

Summary
----------------------------------------
Total mutations:   4
Killed:            3 (good - tests caught the mutation)
Survived:          1 (bad - tests missed the mutation)

Mutation Score:    75.0%
Duration:          12.34s

Surviving Mutations (improve your tests!)
----------------------------------------
  • age >= 18 -> age > 18
    in function 'is_adult' at src/validator.rs:12
```

## How It Works

### AST-Based Matching

Unlike text-based mutation testing, this framework uses Abstract Syntax Tree (AST) parsing:

- `a + b` matches `a+b`, `a  +  b`, and `a + b` (whitespace ignored)
- Matches actual code structure, not text patterns
- Prevents false matches in comments or strings

### Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  mutations.yaml │────▶│  Mutation Engine │────▶│  Test Runner    │
│  (config file)  │     │  (AST-based)     │     │  (cargo test)   │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                                                          │
                                                          ▼
                                                 ┌─────────────────┐
                                                 │  Report         │
                                                 │  (killed/alive) │
                                                 └─────────────────┘
```

### Mutation Process

1. **Parse Config**: Load mutation definitions from `mutations.yaml`
2. **Parse Source**: Parse target .rs file into AST
3. **Find Function**: Locate target function in AST
4. **Match Original**: Find AST node matching original expression
5. **Apply Mutation**: Replace with replacement expression
6. **Run Tests**: Execute `cargo test` against the mutated code
7. **Record Result**: Tests fail → killed; Tests pass → survived
8. **Restore Source**: Revert to original code
9. **Report**: Display summary of all mutations and final score

## Common Mutation Types

### Arithmetic Operators
```yaml
- original: a + b
  replacement: a - b

- original: a * b
  replacement: a / b
```

### Comparison Operators
```yaml
- original: a > b
  replacement: a >= b

- original: a == b
  replacement: a != b
```

### Logical Operators
```yaml
- original: a && b
  replacement: a || b
```

### Literal Values
```yaml
- original: "30"
  replacement: "0"

- original: "true"
  replacement: "false"
```

## Project Structure

```
mutation-testing-rust/
├── Cargo.toml
├── mutations.yaml           # Example configuration
├── src/
│   ├── main.rs             # CLI entry point
│   ├── lib.rs              # Library root
│   ├── config.rs           # YAML config loading
│   ├── matcher.rs          # AST expression matching
│   ├── mutator.rs          # AST mutation application
│   ├── codegen.rs          # Code generation
│   ├── runner.rs           # Test execution
│   ├── report.rs           # Result reporting
│   └── error.rs            # Error types
└── docs/
    └── AST_IMPLEMENTATION_SPEC.md
```

## Mutation Score Interpretation

| Score | Interpretation |
|-------|----------------|
| 90-100% | Excellent test coverage |
| 70-89% | Good coverage, some gaps |
| 50-69% | Moderate coverage, needs improvement |
| < 50% | Poor coverage, significant gaps |

## Error Handling

The framework provides clear error messages:

```
Error: Expression 'x + y' not found in function 'add'
  --> src/calculator.rs

  The function 'add' does not contain 'x + y'.
  Check that variable names match exactly (a, b vs x, y).
```

```
Error: Found 2 matches for 'a + b' in function 'calculate'

  The expression 'a + b' appears multiple times:
    1. Line 10, column 12
    2. Line 15, column 8

  To fix: Make the original expression more specific.
```

## Limitations

- Each mutation requires recompilation (can be slow for large projects)
- Single expression mutations only (not multi-statement)
- Mutations must be unique within a function (ambiguous matches are errors)

## Future Enhancements

- [x] AST-based mutations using `syn` crate for precise code manipulation
- [ ] Parallel mutation testing for faster execution
- [ ] Incremental testing (only re-run affected tests)
- [ ] HTML report generation
- [ ] CI/CD integration examples
- [ ] Auto-discovery of potential mutations

## License

MIT License - see [LICENSE](LICENSE) for details.
