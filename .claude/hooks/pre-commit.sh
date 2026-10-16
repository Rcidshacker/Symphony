#!/bin/bash
# PreToolUse(Bash) hook. Only acts on `git commit` commands.
# Exit 2 = block the commit. Exit 0 = allow.

# Hook input is JSON on stdin; skip anything that is not a git commit.
INPUT="$(cat)"
echo "$INPUT" | grep -q 'git commit' || exit 0

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found on PATH — skipping pre-commit checks." >&2
  exit 0
fi

# Mirrors .github/workflows/ci.yml
cargo fmt --all -- --check || { echo "cargo fmt check failed" >&2; exit 2; }
cargo clippy --all-targets --all-features -- -D warnings || { echo "clippy failed" >&2; exit 2; }
cargo test --quiet || { echo "tests failed" >&2; exit 2; }

echo "Pre-commit checks passed."
exit 0
