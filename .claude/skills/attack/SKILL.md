---
name: attack
description: "Full-cycle structured workflow for complex multi-step tasks."
disable-model-invocation: true
---

# Attack — Full-Cycle Task Execution

You are executing a structured workflow for a complex task. Follow all 5 phases in order.

## Phase 1: CLARIFY

Understand the request from `$ARGUMENTS`:
- What exactly needs to be done?
- What's the scope? Which files/modules are affected?
- Are there ambiguities that need user input?

If the task is clear, proceed. If not, ask the user focused questions before continuing.

## Phase 2: RESEARCH

Gather information needed for implementation:
- Launch Explore agents (1-3 in parallel) to understand relevant code
- Read CLAUDE.md for project conventions
- Use WebSearch if external docs/APIs are needed
- Identify dependencies, interfaces, and constraints

Summarize findings before proceeding.

## Phase 3: PLAN

Decompose into concrete tasks:
- List each task with: title, files to modify, success criteria
- Mark each as STANDARD (main session handles) or HARD (delegate to architect agent)
- Identify the execution order and any dependencies between tasks

Present the plan to the user. Proceed on approval, adjust on feedback.

## Phase 4: EXECUTE

For each task, run the evaluator-optimizer loop:

1. **IMPLEMENT** — Main session for STANDARD tasks, architect agent (opus) for HARD tasks
2. **VERIFY** — Run `/test` (verifier agent)
   - If FAIL: fix the issue, re-verify (max 3 attempts, then ask user)
3. **REVIEW** — Run `/review` (reviewer agent)
   - If critical issues: fix and re-verify
   - If warnings/suggestions: note for user, continue

## Phase 5: FINALIZE

1. Run final `/test` to confirm everything passes
2. Summarize what was done:
   - Files created/modified
   - Key decisions made
   - Any remaining TODOs or risks
3. Ask user if they want to commit the changes
