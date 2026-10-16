#!/bin/bash
# PostToolUse(Edit|Write) hook. Formats saved .rs files with rustfmt.
# Hook input is JSON on stdin; extract tool_input.file_path.

FILE="$(cat | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1 | sed 's#\\\\#/#g')"

case "$FILE" in
  *.rs) command -v rustfmt >/dev/null 2>&1 && rustfmt --edition 2021 "$FILE" 2>/dev/null ;;
esac

exit 0
