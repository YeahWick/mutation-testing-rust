# mutation-testing-rust

A mutation testing framework for Rust that evaluates test quality by introducing code mutations during test execution and checking whether tests detect them.

## Overview

Mutation testing works by:
1. Reading mutation definitions from a configuration file
2. Applying code replacements (mutations) to your source code
3. Running your test suite against each mutation
4. Reporting which mutations were "killed" (detected by tests) vs "survived" (undetected)

A higher mutation score (% of killed mutations) indicates a more effective test suite.

## How It Works

### Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  mutations.yaml │────▶│  Mutation Engine │────▶│  Test Runner    │
│  (config file)  │     │  (applies edits) │     │  (cargo test)   │
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
2. **Backup Source**: Create temporary backup of original source files
3. **Apply Mutation**: Replace original code pattern with mutant code in source file
4. **Run Tests**: Execute `cargo test` against the mutated code
5. **Record Result**:
   - Tests fail → mutation "killed" (good - tests caught the bug)
   - Tests pass → mutation "survived" (bad - tests missed the bug)
6. **Restore Source**: Revert to original code
7. **Repeat**: Apply next mutation and test again
8. **Report**: Display summary of all mutations and final score

## Configuration File Format

Mutations are defined in a `mutations.yaml` file:

```yaml
version: "1.0"

settings:
  timeout: 30  # seconds per test run

targets:
  - file: "src/calculator.rs"
    mutations:
      - id: "add_to_sub"
        function: "add"
        description: "Replace addition with subtraction"
        original: "a + b"
        mutant: "a - b"

      - id: "add_to_mul"
        function: "add"
        description: "Replace addition with multiplication"
        original: "a + b"
        mutant: "a * b"

      - id: "gt_to_gte"
        function: "is_positive"
        description: "Change > to >= (off-by-one)"
        original: "x > 0"
        mutant: "x >= 0"

      - id: "boundary_check"
        function: "clamp"
        description: "Off-by-one in lower bound"
        original: "if value < min"
        mutant: "if value <= min"
```

### Configuration Fields

| Field | Description |
|-------|-------------|
| `version` | Config format version |
| `settings.timeout` | Maximum seconds for each test run |
| `targets[].file` | Path to source file to mutate |
| `targets[].mutations[].id` | Unique identifier for the mutation |
| `targets[].mutations[].function` | Target function name (for scoping) |
| `targets[].mutations[].description` | Human-readable description |
| `targets[].mutations[].original` | Code pattern to find and replace |
| `targets[].mutations[].mutant` | Replacement code (the mutation) |

## Common Mutation Types

### Arithmetic Operator Mutations
```yaml
- id: "add_to_sub"
  original: "a + b"
  mutant: "a - b"

- id: "mul_to_div"
  original: "a * b"
  mutant: "a / b"
```

### Comparison Operator Mutations
```yaml
- id: "gt_to_lt"
  original: "a > b"
  mutant: "a < b"

- id: "eq_to_neq"
  original: "a == b"
  mutant: "a != b"

- id: "gte_to_gt"
  original: "a >= b"
  mutant: "a > b"
```

### Logical Operator Mutations
```yaml
- id: "and_to_or"
  original: "a && b"
  mutant: "a || b"

- id: "negate_condition"
  original: "if valid"
  mutant: "if !valid"
```

### Return Value Mutations
```yaml
- id: "return_zero"
  original: "return result"
  mutant: "return 0"

- id: "return_true_to_false"
  original: "return true"
  mutant: "return false"
```

### Boundary Mutations (Off-by-One)
```yaml
- id: "increment_boundary"
  original: "i < len"
  mutant: "i <= len"

- id: "decrement_start"
  original: "i >= 0"
  mutant: "i > 0"
```

## Usage

### Basic Usage

```bash
# Run mutation testing with default config
cargo run -- test

# Specify a custom config file
cargo run -- test --config my-mutations.yaml

# Run with verbose output
cargo run -- test --verbose
```

### Example Output

```
Mutation Testing Report
========================

[KILLED]  add_to_sub      - Replace addition with subtraction
[KILLED]  add_to_mul      - Replace addition with multiplication
[SURVIVED] gt_to_gte      - Change > to >= (off-by-one)
[KILLED]  boundary_check  - Off-by-one in lower bound

Summary
-------
Total mutations: 4
Killed: 3
Survived: 1
Mutation Score: 75.0%

Surviving Mutations (improve your tests!):
  - gt_to_gte: Change > to >= (off-by-one)
    in function 'is_positive' at src/calculator.rs
```

## Project Structure

```
mutation-testing-rust/
├── Cargo.toml
├── mutations.yaml           # Example mutation config
├── src/
│   ├── main.rs             # CLI entry point
│   ├── lib.rs              # Library root
│   ├── config.rs           # YAML config parsing
│   ├── mutator.rs          # Source code mutation logic
│   ├── runner.rs           # Test execution and coordination
│   └── report.rs           # Result formatting and display
└── tests/
    └── integration_test.rs  # Framework self-tests
```

## Core Components

### Config Parser (`config.rs`)
- Parses `mutations.yaml` configuration file
- Validates mutation definitions
- Provides typed access to mutation data

### Mutator (`mutator.rs`)
- Reads source files
- Finds and replaces code patterns
- Creates backups before modification
- Restores original code after testing

### Runner (`runner.rs`)
- Coordinates the mutation testing process
- Executes `cargo test` for each mutation
- Handles timeouts and test failures
- Collects results for reporting

### Reporter (`report.rs`)
- Formats and displays results
- Calculates mutation score
- Highlights surviving mutations

## Mutation Score Interpretation

| Score | Interpretation |
|-------|----------------|
| 90-100% | Excellent test coverage |
| 70-89% | Good coverage, some gaps |
| 50-69% | Moderate coverage, needs improvement |
| < 50% | Poor coverage, significant gaps |

## Limitations

- **Text-based replacement**: Mutations use string pattern matching, not AST-based. Ensure patterns are unique within the target function.
- **Compilation required**: Each mutation requires recompilation, which can be slow for large projects.
- **Single file mutations**: Each mutation is applied to one file at a time.

## Future Enhancements

- [ ] AST-based mutations using `syn` crate for precise code manipulation
- [ ] Parallel mutation testing for faster execution
- [ ] Incremental testing (only re-run affected tests)
- [ ] HTML report generation
- [ ] Integration with CI/CD pipelines
- [ ] Auto-discovery of potential mutations

## License

MIT License - see [LICENSE](LICENSE) for details.
