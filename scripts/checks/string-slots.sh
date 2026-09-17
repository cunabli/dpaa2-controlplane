#!/bin/sh
# string-slots.sh — name slots must be newtypes, never bare String.
#
# Greps struct-field-like lines under crates/*/src for name/label/alias fields
# typed as String, Option<String>, or &str. Known current hits live in the
# inline allowlist below (one `file:line-fragment` per line) as the backlog;
# anything new fails the check. The self-test points STRING_SLOTS_ALLOW at a
# fixture allowlist to exercise the mechanism.
#
# Limitation: skips paths containing /tests/, but cannot see #[cfg(test)] or
# `mod tests` blocks inside a source file — those hits still fire.
set -eu

# The backlog of accepted String slots. Default is the inline heredoc; a test
# overrides the source with STRING_SLOTS_ALLOW=<file>.
if [ -n "${STRING_SLOTS_ALLOW:-}" ]; then
  allow=$(cat "$STRING_SLOTS_ALLOW")
else
  allow=$(cat <<'ALLOW'
crates/dpaa2-api/src/core/inventory.rs:fn judge_label(label: &str, declared: &BTreeSet<ConstructName>) -> Availability {
crates/dpaa2-api/src/matcher.rs:fn tenant_compiled(name_: &str, config: ConfigFacet) -> MatchObject {
crates/dpaa2-api/src/matcher.rs:name_: &str,
crates/dpaa2-api/src/refuse.rs:fn knl(name: &str) -> Tenant {
crates/dpaa2-api/src/refuse.rs:fn link(name: &str, a: &str, b: &str) -> Link {
crates/dpaa2-api/src/refuse.rs:fn poll(name: &str) -> Tenant {
crates/dpaa2-api/src/refuse.rs:fn port(name: &str, dpmac: u32, rate: i64, tenant: &str) -> Port {
crates/dpaa2-api/src/refuse.rs:fn tenant(name: &str, dp: Dataplane, cores: i64, iso: Isolation) -> Tenant {
crates/dpaa2-api/src/refuse.rs:let mk = |name: &str, dpmac: u32, from: Option<&str>| Port {
crates/dpaa2-api/src/core/types.rs:fn err(name: &str) -> NameError {
crates/dpaa2-config/src/parse.rs:fn parse_family(tenant: &TenantName, name: &str) -> Result<Family, Error> {
crates/dpaa2-mc/src/parse.rs:label: String::new(),
crates/dpaa2-mc/src/parse.rs:pub label: String,
crates/dpaa2-tools/src/link.rs:pub fn link_path(dir: &Path, name: &str) -> PathBuf {
crates/dpaa2-tools/src/link.rs:pub fn render_link(mac: MacAddr, name: &str) -> String {
crates/dpaa2-verify/src/board/adapter.rs:fn pick<'a>(picks: &'a Value, name: &str) -> Result<&'a Value, String> {
crates/dpaa2-verify/src/board/adapter.rs:let ep = |name: &str| endpoint_ref(pick(picks, name)?);
crates/dpaa2-verify/src/board/adapter.rs:let obj = |name: &str| obj_ref(pick(picks, name)?);
crates/dpaa2-verify/src/board/adapter.rs:object_name: &str,
crates/dpaa2-verify/src/board/driver.rs:pub label: String,
crates/dpaa2-verify/src/board/fitcheck.rs:pub label: String,
crates/dpaa2-verify/src/board/generate.rs:let clean = |name: &str| -> Option<String> {
crates/dpaa2-verify/src/board/generate.rs:let common = |name: &str| -> Option<String> {
crates/dpaa2-verify/src/board/generate.rs:let lying = |name: &str| -> Option<String> {
crates/dpaa2-verify/src/board/generate.rs:let other = |name: &str| -> Option<String> {
crates/dpaa2-verify/src/board/generate.rs:let pass = |name: &str| -> Option<String> {
crates/dpaa2-verify/src/board/generate.rs:let results = |name: &str| -> Option<String> {
crates/dpaa2-verify/src/board/generate.rs:name: &str,
crates/dpaa2-verify/src/itf.rs:pub(crate) fn field<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> {
crates/dpaa2-verify/src/board/ioctlpolicy.rs:pub name: String,
crates/dpaa2-verify/src/intent/lint.rs:fn model_spelling(adr_name: &str) -> &str {
crates/dpaa2-verify/src/board/ledger.rs:label: &str,
crates/dpaa2-verify/src/intent/lint.rs:let name: String = rest
crates/dpaa2-verify/src/intent/lint.rs:let name: String = rest.chars().take_while(|&c| c != '*' && c != ' ').collect();
crates/dpaa2-verify/src/intent/lint.rs:let name: String = rest.chars().take_while(|&c| c != '`').collect();
crates/dpaa2-verify/src/intent/lint.rs:let name: String = seg
crates/dpaa2-verify/src/intent/lint.rs:let name: String = tail
crates/dpaa2-verify/src/main.rs:fn maybe_upsert(args: &DiffArgs, v: &Verdict, label: &str) -> Result<String, String> {
crates/dpaa2-verify/src/main.rs:label: Option<String>,
crates/dpaa2-verify/src/board/mcstatus.rs:pub fn by_name(name: &str) -> Option<&'static McStatus> {
crates/dpaa2-verify/src/board/snapshot.rs:fn diff_object(name: &str, old: &Object, new: &Object, out: &mut Vec<String>) {
crates/dpaa2-verify/src/board/snapshot.rs:label: String::new(),
crates/dpaa2-verify/src/board/snapshot.rs:let read_req = |name: &str| -> Result<String, String> {
crates/dpaa2-verify/src/board/snapshot.rs:move |name: &str| std::fs::read_to_string(dir.join(name)).ok()
crates/dpaa2-verify/src/board/snapshot.rs:pub label: String,
crates/dpaa2-verify/src/board/verdict.rs:fn is_hook_scannable(name: &str) -> bool {
crates/dpaa2-verify/src/board/verdict.rs:let read = |name: &str| match name {
crates/dpaa2-verify/src/board/verdict.rs:let read = |name: &str| (name == "step-0-exit.txt").then(|| "3\n".to_owned());
crates/dpaa2-verify/src/board/verdict.rs:move |name: &str| match name {
crates/dpaa2-verify/src/board/verdict.rs:name: &str,
crates/dpaa2-verify/src/board/verdict.rs:pub fn revision_of(name: &str) -> u32 {
crates/dpaa2-verify/src/board/verdict.rs:pub fn upsert(index: &mut Index, suite: &str, label: &str, v: &Verdict, archive: Option<String>) {
ALLOW
)
fi

grep -rnE '(name|label|alias)[a-z_]*[[:space:]]*:[[:space:]]*(String|Option<String>|&str)' \
     crates/*/src --include='*.rs' 2>/dev/null |
  grep -v '/tests/' |
  awk -v allow="$allow" '
    BEGIN {
      na = split(allow, lines, "\n")
      for (idx = 1; idx <= na; idx++) {
        a = lines[idx]
        if (a == "") continue
        i = index(a, ":"); nk++; af[nk] = substr(a, 1, i-1); ag[nk] = substr(a, i+1)
      }
    }
    {
      i = index($0, ":"); file = substr($0, 1, i-1); rest = substr($0, i+1)
      j = index(rest, ":"); content = substr(rest, j+1)
      for (m = 1; m <= nk; m++)
        if (af[m] == file && index(content, ag[m]) > 0) { next }
      print; n++
    }
    END { exit (n > 0) ? 1 : 0 }
  '
