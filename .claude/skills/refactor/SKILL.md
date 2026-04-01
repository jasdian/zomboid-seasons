---
name: refactor
description: "Focused refactoring: research existing code, implement changes, verify."
disable-model-invocation: true
---

# Refactor — Research, Implement, Verify

Focused refactoring workflow for `$ARGUMENTS`.

## Steps

### 1. Research
Launch Explore agent(s) to understand the target code:
- What does the current code do?
- What are its callers/dependents?
- What invariants must be preserved?

### 2. Plan
Identify:
- What to change
- What to preserve (public API, behavior, tests)
- What might break
- Whether this is STANDARD (main session) or HARD (delegate to architect)

### 3. Implement
Make the changes. For STANDARD refactors, implement directly. For HARD refactors (cross-module, complex type changes), delegate to the architect agent.

### 4. Verify
Run `/test` to ensure nothing broke.

### 5. Review
Run `/review` to check quality of the refactored code.

### 6. Report
- What changed and why
- Before/after comparison (key structural changes)
- Any risks or follow-up work needed
