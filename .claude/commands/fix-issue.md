---
name: fix-issue
argument-hint: [issue-number]
---

Fix GitHub issue #$ARGUMENTS:
1. `gh issue view $ARGUMENTS` — read the full issue
2. Identify the relevant source files
3. Implement the minimal fix — no extra changes
4. Write a regression test
5. Run `cargo test` — all green
6. Commit: "fix: <description> (closes #$ARGUMENTS)"
