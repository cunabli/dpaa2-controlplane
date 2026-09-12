#!/usr/bin/env sh
# Self-test for commit-msg-check.sh. Run from the repo root: sh scripts/checks/test-commit-msg.sh
set -u

root="$(git rev-parse --show-toplevel)"
check="$root/scripts/checks/commit-msg-check.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# A bd shim that always reports the bead closed, so the close-then-commit
# check passes without touching the real tracker.
mkdir "$tmp/bin"
cat > "$tmp/bin/bd" <<'EOF'
#!/usr/bin/env sh
echo '[{"id":"dpaa2-controlplane-xxx","status":"closed"}]'
EOF
chmod +x "$tmp/bin/bd"
PATH="$tmp/bin:$PATH"
export PATH

fails=0

assert_ok() {
  if sh "$check" "$1" >"$tmp/out" 2>"$tmp/err"; then
    echo "ok: $2"
  else
    echo "FAIL: $2 (expected exit 0)"; cat "$tmp/err"; fails=1
  fi
}

assert_fail() {
  if sh "$check" "$1" >"$tmp/out" 2>"$tmp/err"; then
    echo "FAIL: $2 (expected non-zero exit)"; fails=1
  elif grep -q "$3" "$tmp/err"; then
    echo "ok: $2"
  else
    echo "FAIL: $2 (stderr missing '$3')"; cat "$tmp/err"; fails=1
  fi
}

cat > "$tmp/valid" <<'EOF'
mc: bind child containers to VFIO and report sysfs state

Extends the kernel face so declared consumers converge to their
container through the product pipeline.

Change: dprc-encapsulation
BeadId: dpaa2-controlplane-xxx
Co-Authored-By: Someone <someone@example.com>
EOF
assert_ok "$tmp/valid" "valid message"

cat > "$tmp/rule-b" <<'EOF'
This title has no lowercase area prefix

Change: dprc-encapsulation
BeadId: dpaa2-controlplane-xxx
EOF
assert_fail "$tmp/rule-b" "rule b (title shape)" "title must be"

cat > "$tmp/rule-d" <<'EOF'
mc: bind child containers to VFIO and report sysfs state

This body line is deliberately far too long to fit inside the limit here.

Change: dprc-encapsulation
BeadId: dpaa2-controlplane-xxx
EOF
assert_fail "$tmp/rule-d" "rule d (body width)" "body line exceeds 72 chars"

cat > "$tmp/rule-e" <<'EOF'
mc: bind child containers to VFIO and report sysfs state

one
two
three
four
five
six
seven
eight
nine
ten
eleven
twelve
thirteen
fourteen
fifteen
sixteen

Change: dprc-encapsulation
BeadId: dpaa2-controlplane-xxx
EOF
assert_fail "$tmp/rule-e" "rule e (body length)" "essay"

cat > "$tmp/rule-f" <<'EOF'
mc: bind child containers to VFIO and report sysfs state

A perfectly reasonable body that simply forgets its trailers.
EOF
assert_fail "$tmp/rule-f" "rule f (required trailers)" "trailer"

exit $fails
