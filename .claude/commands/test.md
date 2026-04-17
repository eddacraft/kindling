---
name: test
description: Run tests and fix any failures
---

# Run Tests

## Target

$ARGUMENTS

## Instructions

### 1. Detect Test Framework

Check for:

- `jest.config.*` or `vitest.config.*` (JavaScript/TypeScript)
- `pytest.ini`, `pyproject.toml` with pytest (Python)
- `go.mod` (Go)
- `Cargo.toml` (Rust)

### 2. Run Tests

Execute the appropriate test command:

```bash
# JavaScript/TypeScript
npm test
npx jest
npx vitest run

# Python
pytest -v
python -m pytest

# Go
go test ./...

# Rust
cargo test
```

### 3. Analyze Failures

For each failing test:

- Identify the test file and test name
- Understand what the test expects
- Find the relevant source code
- Determine root cause

### 4. Fix Issues

- Fix the underlying code (not the test) unless the test is wrong
- Run tests again to verify fix
- Ensure no regressions

### 5. Report Results

```markdown
## Test Results

### Summary

- Total: X tests
- Passed: Y
- Failed: Z
- Skipped: W

### Failures Fixed

1. `test_name` in `file.test.ts`
   - Cause: Description
   - Fix: What was changed

### Remaining Issues

- Issue that couldn't be resolved
```

If no arguments provided, run all tests. If a specific file or pattern is given, run only those tests.
