---
name: negotiate
description: Start a multi-round negotiation between two agents to reach consensus
---

# Agent Negotiation

## Arguments

$ARGUMENTS

## Usage

```
/negotiate <agent1> <agent2> "<topic>"
```

### Examples

```
/negotiate architect security-analyst "JWT vs server-side sessions"
/negotiate tdd-coach code-reviewer "Unit test coverage threshold"
/negotiate architect debugger "Caching strategy for API responses"
```

## Negotiation Process

### 1. Parse Arguments

Extract from `$ARGUMENTS`:

- **Agent 1**: First participant (states initial position)
- **Agent 2**: Second participant (responds to position)
- **Topic**: The technical decision to debate

If arguments are unclear, ask for clarification.

### 2. Initialize Signal File

Create coordination file:

```bash
mkdir -p .claude/agent-bus/signals
```

```json
// .claude/agent-bus/signals/neg-{timestamp}.json
{
  "id": "{timestamp}",
  "topic": "{extracted_topic}",
  "participants": ["{agent1}", "{agent2}"],
  "status": "in_progress",
  "round": 1,
  "maxRounds": 4,
  "history": []
}
```

### 3. Execute Rounds

#### Round 1: First Agent Position

Spawn **{agent1}** with prompt:

```
You are participating in a technical negotiation with {agent2}.

**Topic:** {topic}

**Your task:** State your initial position on this topic.

Consider the technical merits, tradeoffs, and long-term implications.

End your response with exactly one of:
- CONSENSUS: [agreed approach] - if the answer is obvious
- COUNTER: [your position] - your recommended approach
- QUESTION: [clarification] - if you need more info
```

#### Subsequent Rounds

Spawn alternating agents with accumulated context:

```
You are participating in a technical negotiation with {other_agent}.

**Topic:** {topic}

**Previous positions:**
Round 1 ({agent1}): {position_1}
Round 2 ({agent2}): {position_2}
...

**Your task:** Respond to the other agent's position.

- If you agree: CONSENSUS: [the agreed approach]
- If you disagree: COUNTER: [your position and reasoning]
- If unclear: QUESTION: [what you need to know]
```

### 4. Parse Responses

After each agent response:

1. Look for `CONSENSUS:`, `COUNTER:`, or `QUESTION:` in output
2. Extract the content after the keyword
3. Update signal file with new round
4. Check termination conditions

### 5. Termination Conditions

| Condition                | Action                   |
| ------------------------ | ------------------------ |
| CONSENSUS found          | End with agreement       |
| Max rounds reached       | End with deadlock        |
| Both agents ask QUESTION | Inject context, continue |
| Timeout                  | Report partial result    |

### 6. Report Outcome

```markdown
## Negotiation Result

**Topic:** {topic}
**Participants:** {agent1}, {agent2}
**Rounds:** {n}
**Outcome:** {CONSENSUS | DEADLOCK}

### Summary

{one_paragraph_summary}

### Position History

**Round 1 ({agent1}):**
{position}

**Round 2 ({agent2}):**
{response}

...

### Final Recommendation

{if_consensus: the agreed approach}
{if_deadlock: facilitator recommendation based on context}
```

## Agent Pairing Guide

| Decision Type      | Suggested Pairing            |
| ------------------ | ---------------------------- |
| Architecture       | architect + security-analyst |
| Performance        | architect + debugger         |
| Testing            | tdd-coach + code-reviewer    |
| Security tradeoffs | security-analyst + architect |
| Code design        | code-reviewer + architect    |

## Configuration

| Variable                        | Default | Description                |
| ------------------------------- | ------- | -------------------------- |
| `CLAUDE_NEGOTIATION_MAX_ROUNDS` | 4       | Max rounds before deadlock |

## Available Agents

- `architect` - System design, technology decisions
- `security-analyst` - Vulnerability assessment, secure coding
- `code-reviewer` - Code quality, bug detection
- `tdd-coach` - Testing strategy, TDD patterns
- `debugger` - Root cause analysis, performance
- `planner` - Implementation planning
