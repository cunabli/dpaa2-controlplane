#!/usr/bin/env sh
# One writer per crate per task: a single commit touches at most one crate.
# Root Cargo.toml, Cargo.lock, and anything outside crates/ are exempt.
#
# Exemption: an atomic API change that must ripple into consumer crates in the
# same commit (one task, one writer, several crates) carries a
# "One-Writer-Exempt: <reason>" trailer in the commit message ($1) and passes.
set -u

msgfile="${1:-}"

crates="$(git diff --cached --name-only \
  | sed -n 's#^\(crates/[^/]*\)/.*#\1#p' \
  | sort -u)"

count="$(printf '%s' "$crates" | grep -c .)"
if [ "$count" -gt 1 ]; then
  if [ -n "$msgfile" ] && grep -Eq '^One-Writer-Exempt: .+' "$msgfile"; then
    reason="$(grep -E '^One-Writer-Exempt: .+' "$msgfile" | head -n1 | sed 's/^One-Writer-Exempt: //')"
    echo >&2 "commit-msg: one-writer exemption honoured ($reason) for crates:"
    printf '%s\n' "$crates" | sed 's/^/  /' >&2
    exit 0
  fi
  echo >&2 "commit-msg: one writer per crate per task — this commit touches:"
  printf '%s\n' "$crates" | sed 's/^/  /' >&2
  exit 1
fi
exit 0
