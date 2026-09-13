#!/bin/sh
# hook-offload-tripwire.sh — advisory PostToolUse hook (ADR-0016 development-
# process model).
#
# When a crates/*.rs or models/*.qnt source file inside this repo is edited
# directly, remind the session that such work belongs in an opus48-developer /
# rust-developer parcel (account rule; bd key offload-coding-to-opus48). Exit 2
# is the PostToolUse code that feeds the message back to Claude without blocking
# the edit. Anything else exits 0 silently.
#
# A subagent's own edits trip this too; that is by design — the message is
# self-identifying ("if this is the MAIN session"), so a parcel agent ignores
# it.
set -eu

# The hook payload is JSON on stdin; pull the edited path with the system jq.
file_path=$(/usr/bin/jq -r '.tool_input.file_path // empty')
[ -n "$file_path" ] || exit 0

# Only paths inside this repo, and only crates/*.rs or models/*.qnt sources.
repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null) || exit 0
case "$file_path" in
  "$repo"/*) ;;
  *) exit 0 ;;
esac
case "$file_path" in
  "$repo"/crates/*.rs|"$repo"/models/*.qnt) ;;
  *) exit 0 ;;
esac

echo "offload tripwire: crates/models source edited directly — if this is the MAIN session, this work belongs in an opus48-developer/rust-developer parcel (account rule; bd key offload-coding-to-opus48)" >&2
exit 2
