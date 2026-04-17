---
name: review
description: Comprehensive code review of recent changes or specified files
---

# Code Review

## Target

$ARGUMENTS

## Instructions

Perform a thorough code review covering:

### 1. Gather Context

- Read the files or changes to review
- Understand the purpose of the changes
- Check related tests

### 2. Review Checklist

#### Functionality

- [ ] Code does what it's supposed to do
- [ ] Edge cases are handled
- [ ] Error handling is appropriate
- [ ] No obvious bugs

#### Security

- [ ] No hardcoded secrets or credentials
- [ ] Input is validated
- [ ] No injection vulnerabilities
- [ ] Proper authentication/authorization

#### Quality

- [ ] Code is readable and maintainable
- [ ] No unnecessary duplication
- [ ] Good naming conventions
- [ ] Appropriate abstraction level

#### Performance

- [ ] No obvious performance issues
- [ ] Efficient algorithms used
- [ ] No memory leaks
- [ ] Database queries optimized

#### Testing

- [ ] Adequate test coverage
- [ ] Tests are meaningful
- [ ] Edge cases tested

### 3. Output Format

```markdown
## Code Review Summary

### Overall Assessment: [APPROVED | NEEDS_CHANGES | REJECTED]

### Critical Issues (must fix)

- Issue with file:line reference

### Major Suggestions (should fix)

- Suggestion with file:line reference

### Minor Notes (nice to fix)

- Note with file:line reference

### Positive Observations

- What was done well
```

Be specific with file:line references. Explain the "why" behind each suggestion.
