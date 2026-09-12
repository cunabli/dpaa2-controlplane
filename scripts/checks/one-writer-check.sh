#!/usr/bin/env sh
# One writer per crate per task: a single commit touches at most one crate.
# Root Cargo.toml, Cargo.lock, and anything outside crates/ are exempt.
set -u

crates="$(git diff --cached --name-only \
  | sed -n 's#^\(crates/[^/]*\)/.*#\1#p' \
  | sort -u)"

count="$(printf '%s' "$crates" | grep -c .)"
if [ "$count" -gt 1 ]; then
  echo >&2 "commit-msg: one writer per crate per task — this commit touches:"
  printf '%s\n' "$crates" | sed 's/^/  /' >&2
  exit 1
fi
exit 0
