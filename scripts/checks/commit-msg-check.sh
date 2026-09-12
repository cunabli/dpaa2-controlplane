#!/usr/bin/env sh
# Enforces commit-message mechanics and the repo commit rules (ADR-0016).
# Escape hatch for everything here: git commit --no-verify.
set -u

msgfile="$1"
gitdir="$(git rev-parse --git-dir)"
here="$(dirname "$0")"

# A trailer line: "Word: value", e.g. Change:, BeadId:, Co-Authored-By:.
trailer_re='^[A-Za-z][A-Za-z-]*: '

fail() {
  echo >&2 "commit-msg: $1"
  exit 1
}

title="$(sed -n '1p' "$msgfile")"

# (a) Nothing to check for merges, fixup/squash, or reverts.
[ -f "$gitdir/MERGE_HEAD" ] && exit 0
case "$title" in
  fixup!*|squash!*|"Revert "*) exit 0 ;;
esac

# (b) Title shape and length.
if [ "${#title}" -gt 72 ]; then
  fail "title exceeds 72 chars: $title"
fi
if ! printf '%s\n' "$title" | grep -Eq '^[a-z0-9-]+(\([a-z0-9-]+\))?!?: .+'; then
  fail "title must be '<area>: <summary>' with a lowercase area prefix — got: $title"
fi

# (c) Blank line between title and body.
line2="$(sed -n '2p' "$msgfile")"
if [ -n "$line2" ]; then
  fail "leave a blank line between the title and the body: $line2"
fi

body="$(sed -n '3,$p' "$msgfile")"

# (d) Body width, exempting URLs and trailer lines.
over="$(printf '%s\n' "$body" | awk -v re="$trailer_re" '
  /^#/ { next }
  /http:\/\// || /https:\/\// { next }
  $0 ~ re { next }
  length($0) > 72 { print; exit }
')"
if [ -n "$over" ]; then
  fail "body line exceeds 72 chars: $over"
fi

# (e) Body length, ignoring blanks, comments, and the trailer block.
lines="$(printf '%s\n' "$body" | awk -v re="$trailer_re" '
  /^[[:space:]]*$/ { next }
  /^#/ { next }
  $0 ~ re { next }
  { n++ }
  END { print n+0 }
')"
if [ "$lines" -gt 15 ]; then
  fail "body reads like an essay — move detail to the ADR/spec/bead and link it"
fi

# (f) Exactly one Change: and one BeadId: trailer, both well-formed.
change_n="$(grep -Ec '^Change: [a-z0-9-]+$' "$msgfile" || true)"
if [ "$change_n" -ne 1 ]; then
  fail "need exactly one 'Change: <slug>' trailer (lowercase slug); use --no-verify to bypass"
fi
bead_n="$(grep -Ec '^BeadId: dpaa2-controlplane-[a-z0-9.]+$' "$msgfile" || true)"
if [ "$bead_n" -ne 1 ]; then
  fail "need exactly one 'BeadId: dpaa2-controlplane-<id>' trailer; use --no-verify to bypass"
fi
beadid="$(grep -E '^BeadId: dpaa2-controlplane-[a-z0-9.]+$' "$msgfile" | sed 's/^BeadId: //')"

# (g) Close-then-commit: the bead must already be closed. Without bd, warn only.
if command -v bd >/dev/null 2>&1; then
  if json="$(bd show "$beadid" --json 2>/dev/null)"; then
    status="$(printf '%s' "$json" | grep -oE '"status"[[:space:]]*:[[:space:]]*"[a-z]+"' | head -n1 | sed -E 's/.*"([a-z]+)"$/\1/')"
    if [ -z "$status" ]; then
      echo >&2 "commit-msg: warning — could not read status for $beadid; skipping close-then-commit check"
    elif [ "$status" != "closed" ]; then
      fail "close the bead first (close-then-commit): $beadid is '$status'"
    fi
  else
    echo >&2 "commit-msg: warning — could not query $beadid; skipping close-then-commit check"
  fi
else
  echo >&2 "commit-msg: warning — bd not on PATH; skipping close-then-commit check"
fi

# (h) The bead export edited by the close must be part of this commit.
if git status --porcelain -- .beads/issues.jsonl | grep -Eq '^.M'; then
  fail "stage .beads/issues.jsonl — the close you just did belongs in this commit"
fi

# One writer per crate per task.
"$here/one-writer-check.sh"
