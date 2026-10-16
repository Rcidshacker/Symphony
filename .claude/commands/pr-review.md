---
name: pr-review
argument-hint: [pr-number]
---

Review PR #$ARGUMENTS:
1. `gh pr view $ARGUMENTS --json title,body,files` — read the PR
2. `gh pr diff $ARGUMENTS` — read the diff
3. Run code-reviewer agent on changed files
4. Show the findings to the user and wait for approval before posting.
5. Post review: `gh pr review $ARGUMENTS --comment --body "<findings>"`
   Use APPROVE only if zero CRITICAL/WARNING findings.
