---
name: delegate
description: Delegate task to specialized expert via Codex MCP
---

# Delegate to Expert

## Task

$ARGUMENTS

## Available Experts

Select the appropriate expert based on the task:

### 1. Architect

**Use for**: System design, complex debugging, technology decisions

### 2. Plan Reviewer

**Use for**: Pre-implementation validation, plan critique

### 3. Scope Analyst

**Use for**: Ambiguity detection, requirements clarification

### 4. Code Reviewer

**Use for**: Quality analysis, bug detection, PR review

### 5. Security Analyst

**Use for**: Vulnerability assessment, security audit

## Delegation Process

### 1. Analyze Request

- Determine which expert is most appropriate
- Gather relevant code and documentation context

### 2. Delegate via Codex MCP

Use the Codex MCP server to delegate. The server exposes these tools:

**`mcp__codex__codex`** - Start a new Codex session:

```json
{
  "prompt": "As a [expert type], analyze: [task description]\n\nContext:\n[relevant code/docs]",
  "approval-policy": "on-request",
  "sandbox": "read-only"
}
```

**`mcp__codex__codex_reply`** - Continue an existing session:

```json
{
  "conversationId": "[id from previous response]",
  "prompt": "Follow-up question or clarification"
}
```

### 3. Expert Prompt Templates

**Architect**:

```
As a software architect, analyze the following system design question:
[task]

Consider: scalability, maintainability, performance, and trade-offs.
```

**Code Reviewer**:

```
As a senior code reviewer, review the following code for:
- Bugs and logic errors
- Code quality and best practices
- Performance issues
- Maintainability concerns

[code]
```

**Security Analyst**:

```
As a security analyst, assess the following for vulnerabilities:
- Input validation issues
- Authentication/authorization flaws
- Data exposure risks
- Injection vulnerabilities

[code or system description]
```

### 4. Synthesize Response

- Never pass through raw responses
- Integrate expert insights with your own analysis
- Provide actionable recommendations

## Automatic Expert Selection

If no expert is specified, select based on keywords:

- "design", "architecture", "scale" → Architect
- "review plan", "validate approach" → Plan Reviewer
- "unclear", "ambiguous", "requirements" → Scope Analyst
- "review code", "PR", "bugs" → Code Reviewer
- "security", "vulnerability", "audit" → Security Analyst
