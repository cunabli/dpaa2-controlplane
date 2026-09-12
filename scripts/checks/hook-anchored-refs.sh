#!/bin/sh
# hook-anchored-refs.sh — PostToolUse hook that runs the anchored-refs lint at
# edit time, so unanchored design/task references surface in the Claude session
# instead of waiting for a pre-commit run (ADR-0016 development-process model).
#
# Reads the hook JSON from stdin, lints the edited file, and exits 2 when it
# finds violations — exit code 2 is the code Claude Code feeds back to the
# session as stderr context. Path exclusions live in anchored-refs.sh; this
# wrapper delegates to it rather than restating them.
set -eu

# Pull the edited path out of the hook payload; absent path means nothing to do.
file_path=$(jq -r '.tool_input.file_path // empty')
[ -n "$file_path" ] || exit 0

# Only source files the lint understands, and only if they still exist.
case "$file_path" in
  *.rs|*.qnt|*.sh|*.md) ;;
  *) exit 0 ;;
esac
[ -f "$file_path" ] || exit 0

# The Claude session also writes .md files outside this repo; never lint those.
repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null) || exit 0
case "$file_path" in
  "$repo"/*) ;;
  *) exit 0 ;;
esac

findings=$("$(dirname "$0")/anchored-refs.sh" "$file_path") && exit 0

printf '%s\n' "$findings" >&2
exit 2
