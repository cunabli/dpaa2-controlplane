#!/bin/sh
# shell-sanity.sh — meta-lint over the shell check suite and the git hooks.
#
# Three sanity rules keep these scripts small and interim-file-safe (ADR-0016
# development-process model):
#   1. length: a script past 200 lines is past one reviewable sitting and
#      should graduate to a Python tool under uv.
#   2. pipe depth: at most 3 pipe stages on a line; a deeper pipeline reads
#      better broken into named script steps.
#   3. interim-file guard: any script that calls mktemp must also arm a
#      `trap ... EXIT`, so its scratch is deleted on every exit path.
# Prints file:line findings and exits 1 on any.
set -eu

max_lines=200
max_pipes=3
rc=0

here=$(dirname "$0")
root=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null) || root=$(cd "$here/../.." && pwd)

# The suite lints itself and the hooks. An absent glob expands to a missing
# path, which the -f test drops.
for f in "$root"/scripts/checks/*.sh "$root"/.githooks/*; do
  [ -f "$f" ] || continue

  # Rule 1: length.
  n=$(wc -l < "$f")
  if [ "$n" -gt "$max_lines" ]; then
    echo "$f:$n: over $max_lines lines — graduate to a Python tool under uv"
    rc=1
  fi

  # Rule 2: pipe depth per line.
  # ponytail: crude — after dropping "||" it counts space-delimited " |"
  # tokens, so regex/sed "|" without a leading space are skipped and a real
  # pipe written without spaces would be missed. Upgrade path: an shfmt/AST
  # tokenizer if this heuristic ever misfires.
  if ! awk -v max="$max_pipes" '
    { line=$0; gsub(/\|\|/, "", line); n=gsub(/ \|/, "", line)
      if (n > max) { printf "%s:%d: %d pipe stages exceed %d — split into steps\n", FILENAME, FNR, n, max; bad=1 } }
    END { exit bad+0 }
  ' "$f"; then
    rc=1
  fi

  # Rule 3: mktemp needs a trap ... EXIT cleanup.
  if grep -q mktemp "$f" && ! grep -Eq 'trap .*EXIT' "$f"; then
    ln=$(grep -n mktemp "$f" | head -n1 | cut -d: -f1)
    echo "$f:$ln: mktemp without a 'trap ... EXIT' guard — scratch may leak"
    rc=1
  fi
done

exit "$rc"
