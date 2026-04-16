---
name: commit
description: Stage changes and create a well-formatted git commit
---

# Git Commit

## Instructions

Create a git commit for the current changes following these steps:

### 1. Review Changes

```bash
git status
git diff --staged
git diff
```

### 2. Stage Changes

- Stage all relevant files
- Don't stage files with secrets (.env, credentials, etc.)
- Don't stage generated files unless intentional

### 3. Commit Message Format

Use conventional commits format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

#### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Formatting, missing semicolons, etc.
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `perf`: Performance improvement
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

#### Guidelines

- Subject line: 50 chars max, imperative mood
- Body: Wrap at 72 chars, explain what and why (not how)
- Reference issues: "Fixes #123" or "Relates to #456"

### 4. Commit

```bash
git add <files>
git commit -m "$(cat <<'EOF'
<commit message>
EOF
)"
```

$ARGUMENTS
