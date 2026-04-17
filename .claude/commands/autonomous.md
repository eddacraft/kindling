---
name: autonomous
description: Execute a long-running task autonomously with checkpoints
---

# Autonomous Task Execution

## Task

$ARGUMENTS

## Execution Protocol

### 1. Task Planning

Before starting:

- Break the task into discrete, verifiable steps
- Identify dependencies between steps
- Create rollback points
- Define success criteria

### 2. Execution Loop

For each step:

```
1. [START] Log step beginning
2. [EXECUTE] Perform the action
3. [VERIFY] Confirm step success
4. [CHECKPOINT] Save progress
5. [CONTINUE] or [ESCALATE]
```

### 3. Progress Reporting

Report status regularly:

```
[PROGRESS] Step X/Y: Description
[SUCCESS] Completed: What was done
[WARNING] Issue: What happened (continuing)
[ERROR] Failed: What happened (action taken)
[BLOCKED] Need input: Question for user
```

### 4. Error Handling

- Retry transient failures (3x with exponential backoff)
- Log all errors with full context
- Rollback on critical failures
- Escalate when blocked

### 5. Checkpointing

After significant progress:

- Commit changes (if code)
- Save state to `.claude/autonomous-state.json`
- Log checkpoint to `.claude/logs/autonomous.log`

### 6. Completion

When finished:

- Verify all success criteria met
- Run tests if applicable
- Summarize what was accomplished
- Clean up temporary files

## Safety Guardrails

- Never delete without creating backup first
- Verify destructive operations before executing
- Respect file permissions
- Stop on repeated failures (>3 for same step)
- Maximum runtime: 10 minutes per step

## Use TodoWrite

Track all subtasks with the TodoWrite tool to maintain visibility.
