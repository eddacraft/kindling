---
name: debug
description: Systematically debug an issue using scientific method
---

# Debug Issue

## Problem Description

$ARGUMENTS

## Debugging Methodology

### 1. Reproduce the Issue

- Document exact steps to reproduce
- Note environment details
- Identify consistency (always/sometimes)

### 2. Gather Information

- Check logs for errors
- Review stack traces
- Note when the issue started
- Identify recent changes

### 3. Form Hypotheses

Based on evidence, list possible causes:

1. Hypothesis A
2. Hypothesis B
3. Hypothesis C

### 4. Test Hypotheses

For each hypothesis:

- Design a test to confirm/refute
- Execute the test
- Record results
- Update understanding

### 5. Binary Search

If the cause is unclear:

- Identify the boundaries of the problem
- Test the middle point
- Narrow down systematically

### 6. Fix and Verify

- Implement the fix
- Verify the original issue is resolved
- Check for regressions
- Add test to prevent recurrence

## Output Format

```markdown
## Debug Report

### Issue

What was observed

### Root Cause

What was actually wrong

### Evidence

How we determined the cause

### Fix Applied

What was changed

### Verification

How we confirmed the fix

### Prevention

How to avoid this in the future
```

Never guess. Always gather evidence before concluding.
