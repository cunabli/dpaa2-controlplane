#!/bin/sh
# Runs shellcheck over the shell check suite and the git hooks.
# (script: shellcheck-suite.sh)
#
# Resolves a shellcheck: a PATH binary first, else uvx running a pinned release
# of shellcheck-py. Warnings fail (-S warning). The exit code lets each caller
# set its own policy for a missing tool (ADR-0016 development-process model):
#   0  clean
#   1  shellcheck reported findings
#   2  no shellcheck available (neither a PATH binary nor uvx)
set -eu

here=$(dirname "$0")
root=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null) || root=$(cd "$here/../.." && pwd)

if command -v shellcheck >/dev/null 2>&1; then
  set -- shellcheck
elif command -v uvx >/dev/null 2>&1; then
  # Pin the shellcheck-py release so a casual clone and CI resolve the same
  # tool; bump this deliberately.
  set -- uvx --from shellcheck-py==0.11.0.1 shellcheck
else
  exit 2
fi

echo "shellcheck-suite: using $*" >&2
"$@" -S warning "$root"/scripts/checks/*.sh "$root"/.githooks/*
