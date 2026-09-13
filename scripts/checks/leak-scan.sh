#!/bin/sh
# leak-scan.sh — public-repo leak scan over staged additions.
#
# This repo is public. Grep the lines a commit ADDS (across text sources:
# .md .rs .qnt .sh .toml .json .yaml .yml) for things that must not ship:
#   - IPv4 addresses, except 0.0.0.0, 127.0.0.1, and the RFC 5737
#     documentation ranges (192.0.2.x, 198.51.100.x, 203.0.113.x)
#   - IPv6 literals, except the RFC 3849 documentation prefix 2001:db8::
#   - MAC addresses (six colon/dash-separated hex octets)
#   - the board-vendor brand names solidrun and clearfog (case-insensitive)
#
# Prints "file:line: what leaked" and exits 1 on any hit. The escape for a
# false positive — or a deliberate release exception — is `git commit
# --no-verify`.
#
# Limitation: serial numbers and usernames have no reliable pattern and are
# NOT detected here; keep those out of commits by hand.
#
# ponytail: the address/brand matchers are heuristics with known ceilings —
# a dotted date ("2026.09.13.1") or an all-hex Rust path ("dead::beef") can
# false-positive; --no-verify is the escape. Upgrade path: a real parser if
# the noise ever bites.
set -eu

# Added lines only, as a unified-0 diff. An awk pass tracks the new-file path
# and line number from the +++/@@ headers, then scans each added line's text.
git diff --cached --unified=0 -- \
    '*.md' '*.rs' '*.qnt' '*.sh' '*.toml' '*.json' '*.yaml' '*.yml' |
  awk '
    function report(msg) { printf "%s:%s: %s\n", f, ln, msg; hit = 1 }

    # Each IPv4 dotted-quad that is not a wildcard, loopback, or RFC 5737 doc.
    function scan_ipv4(c,   t, ip) {
      t = c
      while (match(t, /[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+/)) {
        ip = substr(t, RSTART, RLENGTH)
        t = substr(t, RSTART + RLENGTH)
        if (ip == "0.0.0.0" || ip == "127.0.0.1") continue
        if (ip ~ /^192\.0\.2\.[0-9]+$/) continue
        if (ip ~ /^198\.51\.100\.[0-9]+$/) continue
        if (ip ~ /^203\.0\.113\.[0-9]+$/) continue
        report("IPv4 address " ip)
      }
    }

    # IPv6 literals: a "::" run or a full 8-group form, bounded by non-identifier
    # chars so Rust "::" paths are not mistaken for addresses.
    function scan_ipv6(c,   t, base, run, pos, before, after) {
      t = c; base = 0
      while (match(t, /[0-9A-Fa-f:]*::[0-9A-Fa-f:]*|([0-9A-Fa-f]{1,4}:){7}[0-9A-Fa-f]{1,4}/)) {
        run = substr(t, RSTART, RLENGTH)
        pos = base + RSTART
        before = (pos > 1) ? substr(c, pos - 1, 1) : ""
        after = substr(c, pos + length(run), 1)
        base = base + RSTART + RLENGTH - 1
        t = substr(t, RSTART + RLENGTH)
        if (before ~ /[0-9A-Za-z_]/ || after ~ /[0-9A-Za-z_]/) continue
        if (run ~ /^2001:0*[Dd][Bb]8:/) continue
        if (run !~ /[0-9]/ && run !~ /[0-9A-Fa-f][0-9A-Fa-f]/) continue
        report("IPv6 address " run)
      }
    }

    # MAC addresses: six 2-hex octets, bounded by non-hex so a longer hex blob
    # does not yield a spurious hit.
    function scan_mac(c,   t, base, m, pos, before, after) {
      t = c; base = 0
      while (match(t, /[0-9A-Fa-f]{2}([:-][0-9A-Fa-f]{2}){5}/)) {
        m = substr(t, RSTART, RLENGTH)
        pos = base + RSTART
        before = (pos > 1) ? substr(c, pos - 1, 1) : ""
        after = substr(c, pos + length(m), 1)
        base = base + RSTART + RLENGTH - 1
        t = substr(t, RSTART + RLENGTH)
        if (before ~ /[0-9A-Fa-f]/ || after ~ /[0-9A-Fa-f]/) continue
        report("MAC address " m)
      }
    }

    # scripts/checks/ is the lint suite and its fixtures: a test IP or brand
    # there is data, not a leak (same self-reference exclusion anchored-refs.sh
    # applies). openspec/ and CHANGELOG.md stay scanned — leaks matter there.
    /^\+\+\+ / { f = $2; sub(/^b\//, "", f); skip = (f ~ /(^|\/)scripts\/checks\//); next }
    /^@@ /     { split($3, hh, ","); ln = hh[1]; sub(/^\+/, "", ln); next }
    /^\+/ {
      if (f == "/dev/null" || skip) next
      content = substr($0, 2)
      scan_ipv4(content)
      scan_ipv6(content)
      scan_mac(content)
      if (tolower(content) ~ /solidrun/) report("brand name solidrun")
      if (tolower(content) ~ /clearfog/) report("brand name clearfog")
      ln++
    }
    END { exit hit ? 1 : 0 }
  '
