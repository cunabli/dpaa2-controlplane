#!/bin/sh
# anchored-refs.sh — flag unanchored design/task references in source comments.
#
# A comment that cites a design decision ("D6") or a task ("task 3.2") must
# anchor it to something durable: an ADR number, an openspec change slug, or a
# bead id. Bare "D6" rots — the reader cannot find what it points at.
#
# Scope by extension: comment lines in .rs/.qnt (// /// //! or a block comment),
# # comment lines in .sh (shebang excepted), and every line in .md prose.
#   --staged      (default) lint only lines added in `git diff --cached`
#   --tree        lint crates/, models/, docs/, and the repo-root *.md files
#   <path>        lint the .rs/.qnt/.sh/.md files under that tree
set -eu

mode=${1:---staged}

# Paths that must never be linted: the lint and its fixtures (scripts/checks/),
# openspec definition sites (a bare D-number there IS anchored by its change),
# and CHANGELOG.md (git-cliff generates it from commit titles). Matched against
# both repo-relative and absolute paths, so the leading (^|/) either anchors or
# follows a directory separator.
excl='(^|/)(scripts/checks|openspec)/|(^|/)CHANGELOG[.]md$'

# Anchors that qualify a reference: ADR number, bead id, or a live change slug.
# The slug list is the directory names under openspec/changes[/archive]; if that
# layout is absent, fall back to the parents of any openspec/**/tasks.md.
slugs=""
if [ -d openspec/changes ]; then
  for d in openspec/changes/*/ openspec/changes/archive/*/; do
    [ -d "$d" ] || continue
    b=$(basename "$d")
    [ "$b" = "archive" ] && continue
    # Strip the YYYY-MM-DD- prefix so the durable, undated spec name is the
    # recognized anchor; dated references still match it as a substring.
    b=$(printf '%s' "$b" | sed -E 's/^[0-9]{4}-[0-9]{2}-[0-9]{2}-//')
    slugs="$slugs|$b"
  done
else
  # Pipe through a subshell and capture its output, so a fragile for-over-find
  # is avoided while the accumulated slugs still survive the loop.
  slugs="$slugs$(find openspec -name tasks.md 2>/dev/null | while IFS= read -r t; do
    printf '|%s' "$(basename "$(dirname "$t")")"
  done)"
fi
anchor="ADR-[0-9][0-9][0-9][0-9]|bead dpaa2-controlplane-$slugs"

# scan_files prints "file:lineno:content" for every line of the linted files
# under the given roots, dropping excluded paths.
scan_files() {
  find "$@" -type f \
    \( -name '*.rs' -o -name '*.qnt' -o -name '*.sh' -o -name '*.md' \) 2>/dev/null |
    grep -Ev "$excl" |
    while IFS= read -r f; do
      grep -n '^' "$f" | sed "s|^|$f:|"
    done
}

# emit_stream prints "file:lineno:content" for the lines in scope.
emit_stream() {
  case "$mode" in
    --staged)
      # Added lines only; --unified=0 leaves just +/- lines so new line numbers
      # follow the hunk header directly. Excluded files are dropped whole.
      git diff --cached --unified=0 -- '*.rs' '*.qnt' '*.sh' '*.md' | awk -v excl="$excl" '
        /^\+\+\+ / { f=$2; sub(/^b\//,"",f); skip=(f ~ excl); next }
        /^@@ / { split($3,a,","); ln=a[1]; sub(/^\+/,"",ln); next }
        /^\+/ && f!="/dev/null" && !skip { print f ":" ln ":" substr($0,2); ln++ }
      '
      ;;
    --tree)
      { scan_files crates models docs
        find . -maxdepth 1 -type f -name '*.md' 2>/dev/null | grep -Ev "$excl" |
          while IFS= read -r f; do grep -n '^' "$f" | sed "s|^|$f:|"; done
      }
      ;;
    *)
      scan_files "$mode"
      ;;
  esac
}

emit_stream | awk -v anchor="$anchor" '
  {
    # split off file and lineno (paths carry no colon in this tree)
    i = index($0, ":"); file = substr($0, 1, i-1); rest = substr($0, i+1)
    j = index(rest, ":"); lineno = substr(rest, 1, j-1); content = substr(rest, j+1)

    # comment scope depends on the file kind
    ext = file; sub(/.*\./, "", ext)
    if (ext == "rs" || ext == "qnt") {
      if (content !~ /\/\// && content !~ /\/\*/ && content !~ /^[[:space:]]*\*/) next
    } else if (ext == "sh") {
      if (content ~ /^#!/) next        # shebang is not a comment reference
      if (content !~ /#/) next         # # comment lines only
    } else if (ext == "md") {
      # every line of markdown prose is in scope
    } else next

    s = " " content " "        # pad so boundary classes have something to match
    found = ""
    if (match(s, /[^A-Za-z0-9_]D[0-9]+[^A-Za-z0-9_]/))
      found = substr(s, RSTART+1, RLENGTH-2)
    else if (match(s, /[^A-Za-z0-9_][Tt][Aa][Ss][Kk] [0-9]+\.[0-9]+[^A-Za-z0-9_]/))
      found = substr(s, RSTART+1, RLENGTH-2)
    if (found == "") next

    if (content ~ anchor) next   # same-line anchor qualifies the reference

    printf "%s:%s: unanchored reference \"%s\" — qualify with ADR/change slug/bead id\n", file, lineno, found
    n++
  }
  END { exit (n > 0) ? 1 : 0 }
'
