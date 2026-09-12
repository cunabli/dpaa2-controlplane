#!/bin/sh
# quality-floor.sh — the workspace quality gate, run at task acceptance.
#
# Runs the shell and cargo gates in order, stopping at the first failure with a
# FAIL line. Prints PASS only when every gate holds. Here shellcheck is a hard
# requirement: no shellcheck binary and no uvx is a FAIL, not a skip.
set -eu

here=$(dirname "$0")

run() {
  echo "== $*"
  if ! "$@"; then
    echo "FAIL: $*"
    exit 1
  fi
}

run sh "$here/shell-sanity.sh"
run sh "$here/shellcheck-suite.sh"
run cargo build --workspace
run cargo fmt --all --check
run cargo clippy --workspace -- -D warnings
run cargo clippy --workspace --tests -- -D warnings
run cargo doc --workspace --no-deps

echo "PASS"
