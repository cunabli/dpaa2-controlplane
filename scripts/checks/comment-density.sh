#!/bin/sh
# comment-density.sh --staged — reject comment-heavy staged Rust diffs.
#
# Over the added lines of `git diff --cached` for *.rs, counts non-doc line
# comments against lines of code. Fails when a diff is comment-only, or when
# comments exceed max(3, 10% of the code) they accompany.
set -eu

[ "${1:---staged}" = "--staged" ] || { echo "usage: comment-density.sh --staged" >&2; exit 2; }

git diff --cached --unified=0 -- '*.rs' | awk '
  /^\+\+\+ / { next }
  /^\+/ {
    line = substr($0, 2)
    t = line; sub(/^[[:space:]]+/, "", t)
    if (t == "") next                                   # blank
    if (t ~ /^\/\/\// || t ~ /^\/\/!/) next             # doc comment: neither
    if (t ~ /^\/\//) { comments++; next }               # non-doc comment
    code++                                              # everything else is code
  }
  END {
    # ponytail: ceiling is 10% with a 3-line grace. Tree density measures 6.2%
    # non-doc comments; a why-note is 1-3 lines per 15-30-line function (5-10%);
    # narration starts at ~25%, so 10% is the ceiling and 3 lines the small-diff grace.
    if (code == 0 && comments > 0) {
      printf "FAIL comment-density: %d comment lines, 0 code lines (comment-only diff; ceiling max(3, 10%%))\n", comments
      exit 1
    }
    if (comments > 3 && comments * 10 > code) {
      printf "FAIL comment-density: %d comment lines vs %d code lines exceeds the max(3, 10%%) ceiling\n", comments, code
      exit 1
    }
    exit 0
  }
'
