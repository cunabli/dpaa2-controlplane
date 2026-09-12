#!/bin/sh
# test-lints.sh — self-test for the source lints, runnable from the repo root.
#
# Builds a throwaway git repo and exercises the failure and pass paths of each
# lint. No framework: an assert helper and a nonzero exit on the first miss.
set -eu

checks=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() { echo "SELFTEST FAIL: $1"; exit 1; }

cd "$tmp"
git init -q
git config user.email t@example.com
git config user.name test
mkdir -p crates/x/src

# --- anchored-refs -----------------------------------------------------------
printf '// design D6 rationale\n' > crates/x/src/a.rs
git add crates/x/src/a.rs
if sh "$checks/anchored-refs.sh" --staged >/dev/null; then
  fail "anchored-refs missed a bare D6"
fi
git reset -q

printf '// design D6 anchored to ADR-0016\n' > crates/x/src/a.rs
git add crates/x/src/a.rs
if ! sh "$checks/anchored-refs.sh" --staged >/dev/null; then
  fail "anchored-refs flagged an ADR-0016-anchored D6"
fi
git reset -q
rm crates/x/src/a.rs

# .sh: a bare D6 in a # comment fails; the shebang line is exempt.
printf '#!/bin/sh\n# design D6 rationale\n' > crates/x/src/t.sh
if sh "$checks/anchored-refs.sh" crates/x/src/t.sh >/dev/null; then
  fail "anchored-refs missed a bare D6 in a shell comment"
fi
rm crates/x/src/t.sh

# .md: every line is prose, so a bare "task 1.2" fails but an anchored ref passes.
mkdir -p docs
printf 'follow task 1.2 next\n' > docs/note.md
if sh "$checks/anchored-refs.sh" docs/note.md >/dev/null; then
  fail "anchored-refs missed a bare task 1.2 in markdown"
fi
printf 'see ADR-0016 D6 for the decision\n' > docs/note.md
if ! sh "$checks/anchored-refs.sh" docs/note.md >/dev/null; then
  fail "anchored-refs flagged an ADR-0016-anchored D6 in markdown"
fi
rm docs/note.md

# scripts/checks/ is the lint's own home: its fixtures must not lint themselves.
mkdir -p scripts/checks
printf 'bare D6 here\n' > scripts/checks/fixture.md
if ! sh "$checks/anchored-refs.sh" scripts/checks >/dev/null; then
  fail "anchored-refs linted a fixture under scripts/checks/"
fi
rm scripts/checks/fixture.md

# openspec/ is a definition site: a bare D-number there is anchored by its change.
mkdir -p openspec/changes/x
printf 'D6 defined here\n' > openspec/changes/x/design.md
if ! sh "$checks/anchored-refs.sh" openspec >/dev/null; then
  fail "anchored-refs linted a definition site under openspec/"
fi
rm -rf openspec

# --- comment-density ---------------------------------------------------------
printf '// only a comment\n// and another\n' > crates/x/src/c.rs
git add crates/x/src/c.rs
if sh "$checks/comment-density.sh" --staged >/dev/null; then
  fail "comment-density passed a comment-only diff"
fi
git reset -q

printf 'fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n// one note\n' > crates/x/src/c.rs
git add crates/x/src/c.rs
if ! sh "$checks/comment-density.sh" --staged >/dev/null; then
  fail "comment-density failed a normal diff"
fi
git reset -q

# grace: 1 comment over 8 code is under the 3-line floor, so it passes.
printf 'fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\nfn f() {}\nfn g() {}\nfn h() {}\n// one note\n' > crates/x/src/c.rs
git add crates/x/src/c.rs
if ! sh "$checks/comment-density.sh" --staged >/dev/null; then
  fail "comment-density failed 1 comment over 8 code (within the 3-line grace)"
fi
git reset -q

# boundary: 4 comments over 10 code trips both >3 and >10%.
printf 'fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\nfn f() {}\nfn g() {}\nfn h() {}\nfn i() {}\nfn j() {}\n// one\n// two\n// three\n// four\n' > crates/x/src/c.rs
git add crates/x/src/c.rs
if sh "$checks/comment-density.sh" --staged >/dev/null; then
  fail "comment-density passed 4 comments over 10 code (>3 and >10%)"
fi
git reset -q

# grace: 3 comments over 40 code stays at the 3-line floor, so it passes.
{ i=0; while [ "$i" -lt 40 ]; do printf 'fn f%d() {}\n' "$i"; i=$((i + 1)); done; printf '// one\n// two\n// three\n'; } > crates/x/src/c.rs
git add crates/x/src/c.rs
if ! sh "$checks/comment-density.sh" --staged >/dev/null; then
  fail "comment-density failed 3 comments over 40 code (at the 3-line floor)"
fi
git reset -q
rm crates/x/src/c.rs

# --- string-slots ------------------------------------------------------------
printf 'struct S {\n    name: String,\n}\n' > crates/x/src/s.rs
if sh "$checks/string-slots.sh" >/dev/null; then
  fail "string-slots missed a bare name: String"
fi

printf 'crates/x/src/s.rs:name: String\n' > "$tmp/allow.txt"
if ! STRING_SLOTS_ALLOW="$tmp/allow.txt" sh "$checks/string-slots.sh" >/dev/null; then
  fail "string-slots ignored its allowlist"
fi

echo "SELFTEST PASS"
