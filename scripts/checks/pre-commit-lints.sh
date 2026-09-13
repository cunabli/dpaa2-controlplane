#!/bin/sh
# pre-commit-lints.sh — fast staged-source lints, the pre-commit hook entry point.
#
# Runs the repo-tracked source lints over the staged change and fails if any of
# them do. Kept fast: no cargo, no whole-tree walk (see quality-floor.sh for the
# heavy acceptance gate).
set -eu

here=$(dirname "$0")
rc=0

# CHANGELOG.md is git-cliff's output, generated at release time; a session must
# never hand-edit it. --no-verify is the escape for an actual release run.
if git diff --cached --name-only | grep -q '^CHANGELOG[.]md$'; then
  echo "CHANGELOG.md is cliff-owned — generated at release, never edited in a session (--no-verify for a release run)" >&2
  rc=1
fi

sh "$here/anchored-refs.sh" --staged || rc=1
sh "$here/comment-density.sh" --staged || rc=1
sh "$here/string-slots.sh" || rc=1
sh "$here/leak-scan.sh" || rc=1
sh "$here/shell-sanity.sh" || rc=1

# Run shellcheck over the suite and hooks; a casual clone without a shellcheck
# binary or uvx is warned and skipped rather than blocked (exit 2).
sc=0
sh "$here/shellcheck-suite.sh" || sc=$?
case "$sc" in
  0) ;;
  2) echo "pre-commit: shellcheck unavailable (no PATH binary, no uvx) — skipping" >&2 ;;
  *) rc=1 ;;
esac

exit "$rc"
