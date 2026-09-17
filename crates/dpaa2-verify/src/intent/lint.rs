//! Intent-layer copy lint (R11–R16): the `models/intent/` model is the source
//! of truth (ADR-0002 §2), and every ADR/COVERAGE enumeration that restates it
//! — a witness list, a ledger narrative, an ADR section, a Rust domain enum —
//! is a linted copy (ADR-0014), cross-checked here so a drift fails in CI. Split
//! from the board-suite ledger lint (`crate::board::ledger`), whose shared table
//! helper `split_row` this module reuses.

use crate::board::ledger::split_row;

// ===================== intent-layer copies (ADR-0014 D9) =====================
// The `models/intent/` model is the source of truth (ADR-0002 §2): every
// enumeration that restates it — a witness list, a ledger narrative, an ADR
// section — is a linted copy, never a sibling (ADR-0014). ADR-0013 is the
// accepted-vocabulary record whose §5 (refusals), §6 (invariants) and §7
// (scenarios) are exactly such copies, and its own Consequences note says
// they "drift ... exactly this way" and belong under this lint. R11–R16
// cross-check those copies against the model so a drift fails in CI, the same
// design-D9 mechanism R1–R10 apply to the board ledgers. R14 extends the reach
// to the Rust domain enums (`dpaa2_api::Refusal`, `dpaa2_api::core::family::Family`), which
// restate `refuse.qnt`/`types.qnt` and so are linted copies too (ADR-0014); it
// also ties `dpaa2_api::Warning` (WARNING_VARIANTS) to refuse.qnt's
// `type Warning =` and the lowercase `Family::as_str` names to intent_raw.qnt's
// FAMILY_NAMES mapping. R15 ties `match.qnt`'s four identity-across-time laws to
// the COVERAGE identity-laws table (task 6.4), so a renamed or dropped law fails
// in CI, not review; R16 ties intent_raw.qnt's three raw-surface laws to the
// COVERAGE raw-surface-laws table the same way. Parsing is pure over `&str`; the
// scenario file set arrives as two stem lists and the Rust variant sets as name
// lists the harness reads.

/// The body of the markdown/Quint section whose heading line first starts with
/// `heading` (the heading line excluded), up to the next `## ` / `### `
/// heading or end of document.
fn md_section(text: &str, heading: &str) -> String {
    text.lines()
        .skip_while(|l| !l.trim_start().starts_with(heading))
        .skip(1)
        .take_while(|l| !(l.starts_with("## ") || l.starts_with("### ")))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The ADR spells the anchor refusals `Reserved` / `Foreign`; the model spells
/// them `ReservedAnchor` / `ForeignAnchor` (refuse.qnt DEVIATION; ADR-0013
/// §11). Canonicalise an ADR §5 name to the model spelling before comparing.
fn model_spelling(adr_name: &str) -> &str {
    match adr_name {
        "Reserved" => "ReservedAnchor",
        "Foreign" => "ForeignAnchor",
        other => other,
    }
}

/// The constructor names of `refuse.qnt`'s `type Refusal =` sum type, in
/// declaration order — the source of truth for the refusal vocabulary.
fn parse_refusal_variants(refuse_qnt: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;
    for line in refuse_qnt.lines() {
        if line.trim_start().starts_with("type Refusal =") {
            in_block = true;
            continue;
        }
        if in_block {
            let Some(rest) = line.trim().strip_prefix("| ") else {
                break; // the first non-`|` line closes the sum type
            };
            let name: String = rest
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// The constructor names of `types.qnt`'s `type Family =` sum type. Unlike the
/// one-per-line refusals, families are several to a line (`| Dprc | Dpni | …`),
/// so this splits every `|`-led line of the block and takes each segment's
/// leading identifier — the source of truth for the family vocabulary.
fn parse_family_variants(types_qnt: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;
    for line in types_qnt.lines() {
        if line.trim_start().starts_with("type Family =") {
            in_block = true;
            continue;
        }
        if in_block {
            let t = line.trim();
            if !t.starts_with('|') {
                break; // the first non-`|` line closes the sum type
            }
            for seg in t.split('|') {
                let name: String = seg
                    .trim()
                    .chars()
                    .take_while(char::is_ascii_alphanumeric)
                    .collect();
                if !name.is_empty() {
                    out.push(name);
                }
            }
        }
    }
    out
}

/// The constructor names of `refuse.qnt`'s `type Warning =` sum type, in
/// declaration order — the source of truth for the warning vocabulary.
fn parse_warning_variants(refuse_qnt: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;
    for line in refuse_qnt.lines() {
        if line.trim_start().starts_with("type Warning =") {
            in_block = true;
            continue;
        }
        if in_block {
            let Some(rest) = line.trim().strip_prefix("| ") else {
                break; // the first non-`|` line closes the sum type
            };
            let name: String = rest
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// The lowercase restool names of `intent_raw.qnt`'s `FAMILY_NAMES` mapping —
/// the model's hand-maintained copy of [`dpaa2_api::core::family::Family::as_str`]'s
/// vocabulary (parse.rs `parse_family`). Scans from the `pure val FAMILY_NAMES`
/// line to the line closing the list (`]`) and returns every double-quoted
/// string in order — the `("dprc", Dprc)`-style tuples' left elements.
fn parse_raw_family_names(intent_raw_qnt: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;
    // The declaration line itself carries `]` (in the `List[(str, Family)]`
    // annotation), so the list literal's close is found by bracket balance, not
    // the first `]`: the block ends when the running `[`-minus-`]` depth (which
    // the `= [` opener raises above zero) returns to zero.
    let mut depth: i32 = 0;
    for line in intent_raw_qnt.lines() {
        if !in_block {
            if line.trim_start().starts_with("pure val FAMILY_NAMES") {
                in_block = true;
            } else {
                continue;
            }
        }
        let mut rest = line;
        while let Some(open) = rest.find('"') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('"') else {
                break;
            };
            out.push(after[..close].to_owned());
            rest = &after[close + 1..];
        }
        depth += i32::try_from(line.matches('[').count()).unwrap_or(0);
        depth -= i32::try_from(line.matches(']').count()).unwrap_or(0);
        if depth <= 0 {
            break; // the list literal's closing bracket ends the mapping
        }
    }
    out
}

/// The refusal witnesses of `alphabet.qnt` as `(def name, matched variant)`:
/// every `val w<X> = hasRefusal(r => match r { | <Y>(_) => …`. The `wRefused`
/// catch-all (no `match`) and the non-refusal witnesses (`wAccepted`, the
/// warning and structure ones) do not match this shape and are skipped.
fn parse_refusal_witnesses(alphabet_qnt: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in alphabet_qnt.lines() {
        let Some(rest) = line.trim_start().strip_prefix("val w") else {
            continue;
        };
        let Some((tail, body)) = rest.split_once('=') else {
            continue;
        };
        let def = format!("w{}", tail.trim());
        let Some(after) = body.trim_start().strip_prefix("hasRefusal(") else {
            continue;
        };
        let Some(arm) = after
            .find("match r {")
            .map(|i| &after[i + "match r {".len()..])
        else {
            continue;
        };
        let Some(cons) = arm.trim_start().strip_prefix("| ") else {
            continue;
        };
        let variant: String = cons
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if !variant.is_empty() {
            out.push((def, variant));
        }
    }
    out
}

/// The `` - `Name` — … `` bullets of an ADR section — the shape §5 lists a
/// refusal variant in. Only uppercase-led all-alphanumeric names qualify, so
/// prose back-ticks (`` `pool` ``, `` `userspace-event` ``) are ignored.
fn parse_adr_backtick_bullets(section: &str) -> Vec<String> {
    section
        .lines()
        .filter_map(|l| {
            let rest = l.trim_start().strip_prefix("- `")?;
            let name: String = rest.chars().take_while(|&c| c != '`').collect();
            let ok = name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && name.chars().all(|c| c.is_ascii_alphanumeric());
            ok.then_some(name)
        })
        .collect()
}

/// The `` - **INTENT_I<n> `name`** — … `` bullets of ADR §6, as `(n, name)`.
fn parse_adr_invariants(section: &str) -> Vec<(u32, String)> {
    section
        .lines()
        .filter_map(|l| {
            let rest = l.trim_start().strip_prefix("- **INTENT_I")?;
            let (num, tail) = rest.split_once(' ')?;
            let n: u32 = num.parse().ok()?;
            let name: String = tail
                .trim_start_matches('`')
                .chars()
                .take_while(|&c| c != '`')
                .collect();
            Some((n, name))
        })
        .collect()
}

/// The `// ---- <name> (INTENT_I<n>): …` section headers of `invariants.qnt`,
/// as `(n, name)` — the source of truth for the plan invariants. The
/// non-invariant `---- helpers ----` / `---- the two rungs ----` headers carry
/// no `(INTENT_I` and are skipped.
fn parse_intent_invariants(invariants_qnt: &str) -> Vec<(u32, String)> {
    invariants_qnt
        .lines()
        .filter_map(|l| {
            let rest = l.trim_start().strip_prefix("// ---- ")?;
            let (name, after) = rest.split_once(" (INTENT_I")?;
            let num: String = after.chars().take_while(char::is_ascii_digit).collect();
            Some((num.parse().ok()?, name.to_owned()))
        })
        .collect()
}

/// The `| INTENT_I<n> | <name> | … |` table rows of COVERAGE.md's
/// intent-invariants section, as `(n, name)` — the ledger copy R12 checks
/// against the model (task 5.1). Non-row lines (the preamble, the header, the
/// separator) carry no `INTENT_I<n>` first cell and are skipped.
fn parse_coverage_invariants(section: &str) -> Vec<(u32, String)> {
    section
        .lines()
        .filter_map(|l| {
            let cells = split_row(l);
            let n: u32 = cells.first()?.strip_prefix("INTENT_I")?.parse().ok()?;
            Some((n, cells.get(1)?.clone()))
        })
        .collect()
}

/// The `- **<name>**` scenario bullets of ADR §7.
fn parse_adr_scenarios(section: &str) -> Vec<String> {
    section
        .lines()
        .filter_map(|l| {
            let rest = l.trim_start().strip_prefix("- **")?;
            let name: String = rest.chars().take_while(|&c| c != '*' && c != ' ').collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// R11: the refusal vocabulary agrees across its three copies and the model.
/// `refuse.qnt`'s `type Refusal =` is the truth; `alphabet.qnt`'s witnesses
/// (bijective with the variants — every variant witnessed, no witness naming a
/// phantom), ADR-0013 §5 (with the `Reserved`/`Foreign` spelling alias), and
/// COVERAGE.md's intent-coverage section (which must name every variant) are
/// the copies.
fn r11_refusals(
    refuse_qnt: &str,
    alphabet_qnt: &str,
    coverage_md: &str,
    adr_md: &str,
    out: &mut Vec<String>,
) {
    let variants = parse_refusal_variants(refuse_qnt);
    let is_variant = |v: &str| variants.iter().any(|x| x == v);

    // (a1) witnesses ⟺ variants, bijectively.
    let witnesses = parse_refusal_witnesses(alphabet_qnt);
    for (def, variant) in &witnesses {
        if !is_variant(variant) {
            out.push(format!(
                "R11 refusals: alphabet.qnt witness {def} names {variant}, not a refuse.qnt Refusal variant"
            ));
        } else if def != &format!("w{variant}") {
            out.push(format!(
                "R11 refusals: alphabet.qnt witness {def} matches variant {variant} — the def name should be w{variant}"
            ));
        }
    }
    for v in &variants {
        if !witnesses.iter().any(|(_, w)| w == v) {
            out.push(format!(
                "R11 refusals: Refusal variant {v} has no w{v} witness in alphabet.qnt"
            ));
        }
    }

    // (a2) ADR §5 ⟺ variants, applying the anchor-name alias.
    let adr: Vec<String> = parse_adr_backtick_bullets(&md_section(adr_md, "### 5."));
    for v in &variants {
        if !adr.iter().any(|a| model_spelling(a) == v) {
            out.push(format!(
                "R11 refusals: Refusal variant {v} is absent from ADR-0013 §5"
            ));
        }
    }
    for a in &adr {
        if !is_variant(model_spelling(a)) {
            out.push(format!(
                "R11 refusals: ADR-0013 §5 lists `{a}`, not a refuse.qnt Refusal variant"
            ));
        }
    }

    // (a3) COVERAGE.md's intent section names every variant (model spelling).
    let section = md_section(coverage_md, "## Intent alphabet coverage");
    let tokens: Vec<&str> = section
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    for v in &variants {
        if !tokens.contains(&v.as_str()) {
            out.push(format!(
                "R11 refusals: Refusal variant {v} is not named in COVERAGE.md's intent-coverage section"
            ));
        }
    }
}

/// R12: the plan invariants agree across `invariants.qnt` (truth) and its two
/// copies — ADR-0013 §6 and `COVERAGE.md`'s intent-invariants section — by id
/// and name, each checked both ways. Since intent-layer task 5.1 the ledger carries one row
/// per invariant tying it to its baseline anchors (`invariants.qnt`
/// header); this leg keeps that copy from drifting from the model the same way
/// the ADR §6 leg does.
fn r12_invariants(invariants_qnt: &str, coverage_md: &str, adr_md: &str, out: &mut Vec<String>) {
    let model = parse_intent_invariants(invariants_qnt);
    let adr = parse_adr_invariants(&md_section(adr_md, "### 6."));
    let cov = parse_coverage_invariants(&md_section(coverage_md, "## Intent invariants"));
    for (n, name) in &model {
        match adr.iter().find(|(m, _)| m == n) {
            None => out.push(format!(
                "R12 invariants: INTENT_I{n} `{name}` (invariants.qnt) is absent from ADR-0013 §6"
            )),
            Some((_, aname)) if aname != name => out.push(format!(
                "R12 invariants: INTENT_I{n} is `{name}` in invariants.qnt but `{aname}` in ADR-0013 §6"
            )),
            Some(_) => {}
        }
        match cov.iter().find(|(m, _)| m == n) {
            None => out.push(format!(
                "R12 invariants: INTENT_I{n} `{name}` (invariants.qnt) has no row in COVERAGE.md's intent-invariants section"
            )),
            Some((_, cname)) if cname != name => out.push(format!(
                "R12 invariants: INTENT_I{n} is `{name}` in invariants.qnt but `{cname}` in COVERAGE.md's intent-invariants section"
            )),
            Some(_) => {}
        }
    }
    for (n, aname) in &adr {
        if !model.iter().any(|(m, _)| m == n) {
            out.push(format!(
                "R12 invariants: ADR-0013 §6 lists INTENT_I{n} `{aname}`, absent from invariants.qnt"
            ));
        }
    }
    for (n, cname) in &cov {
        if !model.iter().any(|(m, _)| m == n) {
            out.push(format!(
                "R12 invariants: COVERAGE.md's intent-invariants section lists INTENT_I{n} `{cname}`, absent from invariants.qnt"
            ));
        }
    }
}

/// R13: every scenario `.qnt` has a same-stem `.toml` and vice versa (the
/// file-level pairing; the semantic toml→plan equality lives in `tests/intent_pairing.rs`, task 3.4), and the
/// scenario set equals ADR-0013 §7's five worked witnesses.
fn r13_scenarios(qnt_stems: &[String], toml_stems: &[String], adr_md: &str, out: &mut Vec<String>) {
    for s in qnt_stems {
        if !toml_stems.contains(s) {
            out.push(format!(
                "R13 scenarios: scenarios/{s}.qnt has no same-stem scenarios/{s}.toml"
            ));
        }
    }
    for s in toml_stems {
        if !qnt_stems.contains(s) {
            out.push(format!(
                "R13 scenarios: scenarios/{s}.toml has no same-stem scenarios/{s}.qnt"
            ));
        }
    }
    let adr = parse_adr_scenarios(&md_section(adr_md, "### 7."));
    for s in qnt_stems {
        if !adr.contains(s) {
            out.push(format!(
                "R13 scenarios: scenario {s} is absent from ADR-0013 §7"
            ));
        }
    }
    for s in &adr {
        if !qnt_stems.contains(s) {
            out.push(format!(
                "R13 scenarios: ADR-0013 §7 lists {s}, which has no scenarios/{s}.qnt"
            ));
        }
    }
}

/// R14: the Rust domain copies agree with the model. `refuse.qnt`'s
/// `type Refusal =` and `types.qnt`'s `type Family =` are the truth; the
/// `dpaa2_api::Refusal` variant list ([`dpaa2_api::intent::refuse::REFUSAL_VARIANTS`]) and the
/// `dpaa2_api::core::family::Family` variant set (from [`dpaa2_api::core::family::Family::variant_name`] over
/// [`dpaa2_api::core::family::ALL_FAMILIES`]) are the copies (ADR-0014: a Rust enum that
/// restates the model is a linted copy, tied back here). Refusal names apply the
/// same `Reserved`/`Foreign` anchor alias as the ADR §5 copy (R11). Adding,
/// removing, or renaming a variant on either side — model or Rust — breaks this
/// leg; the Rust list-vs-enum tie is the crate-local exhaustive `match`
/// (`Refusal::name`, `Family::variant_name`) that will not compile until the
/// list moves with the enum.
///
/// The warning list ([`dpaa2_api::intent::refuse::WARNING_VARIANTS`], tied to the enum by
/// `Warning::name`) is checked the same way against `refuse.qnt`'s
/// `type Warning =`, and the lowercase restool family names (`Family::as_str`
/// over `ALL_FAMILIES`) against `intent_raw.qnt`'s `FAMILY_NAMES` mapping —
/// both copies kept deliberately, the mapping layer and the model each owning
/// one (ADR-0014 rule 9).
#[allow(clippy::too_many_arguments)] // one name list per linted copy, all read-only
fn r14_rust_copies(
    refuse_qnt: &str,
    types_qnt: &str,
    intent_raw_qnt: &str,
    rust_refusals: &[&str],
    rust_families: &[&str],
    rust_warnings: &[&str],
    rust_family_strs: &[&str],
    out: &mut Vec<String>,
) {
    // Refusals: model spelling (ReservedAnchor/ForeignAnchor) is canonical; map
    // each Rust name through the anchor alias before comparing.
    let model_refusals = parse_refusal_variants(refuse_qnt);
    for v in &model_refusals {
        if !rust_refusals.iter().any(|r| model_spelling(r) == v) {
            out.push(format!(
                "R14 rust: refuse.qnt Refusal variant {v} has no dpaa2_api::Refusal counterpart"
            ));
        }
    }
    for r in rust_refusals {
        if !model_refusals.iter().any(|v| v == model_spelling(r)) {
            out.push(format!(
                "R14 rust: dpaa2_api::Refusal variant {r} is absent from refuse.qnt"
            ));
        }
    }

    // Warnings: names identical on both sides.
    let model_warnings = parse_warning_variants(refuse_qnt);
    for v in &model_warnings {
        if !rust_warnings.contains(&v.as_str()) {
            out.push(format!(
                "R14 rust: refuse.qnt Warning variant {v} has no dpaa2_api::Warning counterpart"
            ));
        }
    }
    for w in rust_warnings {
        if !model_warnings.iter().any(|v| v == w) {
            out.push(format!(
                "R14 rust: dpaa2_api::Warning variant {w} is absent from refuse.qnt"
            ));
        }
    }

    // Families: names identical on both sides.
    let model_families = parse_family_variants(types_qnt);
    for v in &model_families {
        if !rust_families.contains(&v.as_str()) {
            out.push(format!(
                "R14 rust: types.qnt Family {v} has no dpaa2_api::core::family::Family counterpart"
            ));
        }
    }
    for f in rust_families {
        if !model_families.iter().any(|v| v == f) {
            out.push(format!(
                "R14 rust: dpaa2_api::core::family::Family {f} is absent from types.qnt"
            ));
        }
    }

    // Lowercase family names: Family::as_str vs intent_raw.qnt FAMILY_NAMES.
    let model_family_names = parse_raw_family_names(intent_raw_qnt);
    for v in &model_family_names {
        if !rust_family_strs.contains(&v.as_str()) {
            out.push(format!(
                "R14 rust: intent_raw.qnt FAMILY_NAMES entry {v} has no Family::as_str counterpart"
            ));
        }
    }
    for f in rust_family_strs {
        if !model_family_names.iter().any(|v| v == f) {
            out.push(format!(
                "R14 rust: Family::as_str name {f} is absent from intent_raw.qnt FAMILY_NAMES"
            ));
        }
    }
}

/// The four identity-across-time law names from `match.qnt`'s "Named invariants"
/// header block: each `//   <name> (decision …)` line between the `Named invariants`
/// marker and the `Apalache-marked` footer. Parsing the header block, not the `pure
/// def … : bool` shape, is deliberate: that shape catches the `isPaired` and
/// `ambiguousFamily` helper predicates too, and — more to the point — `swapCorrect`
/// is not a `pure def` at all but a directed `run` (`swapCorrectTest`), since the
/// swap counterexample rides the sweep as the swap action's reachable shape
/// (`edits.qnt` header). The header block is the one place all four are named as
/// identifiers, the same comment-header source of truth [`parse_intent_invariants`]
/// reads for the plan invariants.
fn parse_match_laws(match_qnt: &str) -> Vec<String> {
    match_qnt
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("// Named invariants"))
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with("// Apalache-marked"))
        .filter_map(|l| {
            let rest = l.trim_start().strip_prefix("//")?.trim_start();
            let (name, tail) = rest.split_once(' ')?;
            let named = tail.trim_start().starts_with("(decision")
                && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && name.chars().all(|c| c.is_ascii_alphanumeric());
            named.then(|| name.to_owned())
        })
        .collect()
}

/// The second cell (the `match.qnt` def name) of each data row of the COVERAGE
/// identity-laws table: a lowercase-led ascii-alphanumeric identifier, so the
/// header row (`Name`) and the separator (`------`) drop out.
fn parse_coverage_laws(section: &str) -> Vec<String> {
    section
        .lines()
        .filter_map(|l| {
            let name = split_row(l).into_iter().nth(1)?;
            let ok = name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && name.chars().all(|c| c.is_ascii_alphanumeric());
            ok.then_some(name)
        })
        .collect()
}

/// R15: the four identity-across-time laws agree across `match.qnt` (truth) and
/// its COVERAGE copy — the "Identity-across-time laws" table — by name, checked
/// both ways. A law the model names and the table forgets, or a table name the
/// model does not name, fails here; the same design-D9 mechanism R12 applies to
/// the plan invariants (intent-layer task 6.4).
fn r15_identity_laws(match_qnt: &str, coverage_md: &str, out: &mut Vec<String>) {
    let model = parse_match_laws(match_qnt);
    let table = parse_coverage_laws(&md_section(coverage_md, "## Identity-across-time laws"));
    for name in &model {
        if !table.iter().any(|t| t == name) {
            out.push(format!(
                "R15 identity laws: match.qnt law `{name}` has no row in COVERAGE.md's identity-laws table"
            ));
        }
    }
    for name in &table {
        if !model.iter().any(|m| m == name) {
            out.push(format!(
                "R15 identity laws: COVERAGE.md's identity-laws table lists `{name}`, absent from match.qnt's named laws"
            ));
        }
    }
}

/// The raw-surface law names from `intent_raw.qnt`'s "Named invariants" header
/// block — the source of truth for the config-surface laws. The block spans the
/// `// Named invariants` line (its tail included) up to but excluding the
/// `// Apalache-marked` footer; the lines are stripped of their `//` and joined
/// with spaces (a law's name and its `(descriptor)` can straddle a line break),
/// then every maximal ascii-alphanumeric token that is lowercase-led and
/// immediately followed (after optional spaces) by `(` is a law name — the same
/// comment-header source of truth [`parse_match_laws`] reads for the identity
/// laws. Against the real file this yields `parseOkWellFormed`,
/// `acceptedSurvives`, `nearMissRefusedByName`.
fn parse_raw_laws(intent_raw_qnt: &str) -> Vec<String> {
    let joined = intent_raw_qnt
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("// Named invariants"))
        .take_while(|l| !l.trim_start().starts_with("// Apalache-marked"))
        .map(|l| l.trim_start().trim_start_matches('/').trim_start())
        .collect::<Vec<_>>()
        .join(" ");
    let b = joined.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphanumeric() {
            let start = i;
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            let tok = &joined[start..i];
            if j < b.len()
                && b[j] == b'('
                && tok.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            {
                out.push(tok.to_owned());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// R16: the three raw-surface laws agree across `intent_raw.qnt` (truth) and its
/// COVERAGE copy — the "Raw surface laws" table — by name, checked both ways,
/// the same design-D9 mechanism R15 applies to the identity laws. The table's
/// `raw_conformance` row names the Rust MBT harness, not a model law; its
/// underscore keeps it outside [`parse_coverage_laws`]'s lowercase-led
/// all-alphanumeric identifier filter, so it is deliberately outside this leg.
fn r16_raw_laws(intent_raw_qnt: &str, coverage_md: &str, out: &mut Vec<String>) {
    let model = parse_raw_laws(intent_raw_qnt);
    let table = parse_coverage_laws(&md_section(coverage_md, "## Raw surface laws"));
    for name in &model {
        if !table.iter().any(|t| t == name) {
            out.push(format!(
                "R16 raw laws: intent_raw.qnt law `{name}` has no row in COVERAGE.md's raw-surface-laws table"
            ));
        }
    }
    for name in &table {
        if !model.iter().any(|m| m == name) {
            out.push(format!(
                "R16 raw laws: COVERAGE.md's raw-surface-laws table lists `{name}`, absent from intent_raw.qnt's named laws"
            ));
        }
    }
}

/// Runs the intent-layer cross-checks (R11–R16) over the `models/intent/` and
/// `models/core/` copies and returns one finding per drift; an empty vector is
/// the green verdict. Distinct from [`crate::board::ledger::lint`] because it reads a different
/// document set (the model files, ADR-0013, and the `dpaa2_api` domain enums,
/// not the board ledgers).
#[must_use]
#[allow(clippy::too_many_arguments)] // one &str per copy, all read-only
pub fn intent_lint(
    refuse_qnt: &str,
    types_qnt: &str,
    intent_raw_qnt: &str,
    alphabet_qnt: &str,
    invariants_qnt: &str,
    match_qnt: &str,
    coverage_md: &str,
    adr_md: &str,
    scenario_qnt_stems: &[String],
    scenario_toml_stems: &[String],
    rust_refusals: &[&str],
    rust_families: &[&str],
    rust_warnings: &[&str],
    rust_family_strs: &[&str],
) -> Vec<String> {
    let mut out = Vec::new();
    r11_refusals(refuse_qnt, alphabet_qnt, coverage_md, adr_md, &mut out);
    r12_invariants(invariants_qnt, coverage_md, adr_md, &mut out);
    r13_scenarios(scenario_qnt_stems, scenario_toml_stems, adr_md, &mut out);
    r14_rust_copies(
        refuse_qnt,
        types_qnt,
        intent_raw_qnt,
        rust_refusals,
        rust_families,
        rust_warnings,
        rust_family_strs,
        &mut out,
    );
    r15_identity_laws(match_qnt, coverage_md, &mut out);
    r16_raw_laws(intent_raw_qnt, coverage_md, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- intent-layer copies (R11–R13) ---

    /// A three-variant refuse.qnt slice: one plain, one aliased anchor, and a
    /// Warning block after it the parser must not fold in.
    const REFUSE: &str = "\
module intent_refuse {
  type Refusal =
    | TenantAbsent({ construct: str, tenant: str })   // rule 1
    | ReservedAnchor({ port: str, dpmac: int })       // spec name Reserved
    | Infeasible({ family: Family, needed: int })     // feasibility

  type Warning =
    | UnknownCeiling({ family: Family, needed: int })
}";
    const ALPHABET: &str = "\
  val wAccepted = match compile(intent, REF_INVENTORY) { | Ok(_) => true | Refused(_) => false }
  val wRefused = hasRefusal(_ => true)
  val wTenantAbsent = hasRefusal(r => match r { | TenantAbsent(_) => true | _ => false })
  val wReservedAnchor = hasRefusal(r => match r { | ReservedAnchor(_) => true | _ => false })
  val wInfeasible = hasRefusal(r => match r { | Infeasible(_) => true | _ => false })
  val wUnknownCeiling = hasWarning(w => match w { | UnknownCeiling(_) => true | _ => false })
  val wThreeTenants = intent.tenants.length() == 3";
    const ADR5: &str = "\
### 5. The refusal vocabulary

All 3 variants of `refuse.qnt`, grouped by rule.

- `TenantAbsent` — a construct names a tenant not declared → declare it.
- `Reserved` — the dpmac is Reserved by the safety matrix → pick another.
- `Infeasible` — the summed count exceeds a ceiling.

Two warnings attach: `UnknownCeiling` (…).

### 6. The invariants
";
    const COV_INTENT: &str = "\
## Intent alphabet coverage (task 2.4)

- **Reached** (traces of 3000): TenantAbsent 235, ReservedAnchor 1488.
- **Unreachable, covered elsewhere** (0 traces): `Infeasible`.
";

    #[test]
    fn parses_refusal_variants_and_witnesses() {
        assert_eq!(
            parse_refusal_variants(REFUSE),
            vec!["TenantAbsent", "ReservedAnchor", "Infeasible"]
        );
        // The catch-all, wAccepted and the warning/structure witnesses drop out.
        assert_eq!(
            parse_refusal_witnesses(ALPHABET),
            vec![
                ("wTenantAbsent".to_owned(), "TenantAbsent".to_owned()),
                ("wReservedAnchor".to_owned(), "ReservedAnchor".to_owned()),
                ("wInfeasible".to_owned(), "Infeasible".to_owned()),
            ]
        );
    }

    #[test]
    fn r11_passes_when_every_copy_agrees() {
        let mut out = Vec::new();
        r11_refusals(REFUSE, ALPHABET, COV_INTENT, ADR5, &mut out);
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn r11_flags_a_renamed_adr_variant_and_a_deleted_coverage_variant() {
        // The deliberate-drift negative test: ADR §5 renames `Infeasible`, and
        // COVERAGE.md drops `ReservedAnchor` — both mutations are in memory.
        let adr_bad = ADR5.replace("`Infeasible`", "`Infeasable`");
        let cov_bad = COV_INTENT.replace("ReservedAnchor 1488", "");
        let mut out = Vec::new();
        r11_refusals(REFUSE, ALPHABET, &cov_bad, &adr_bad, &mut out);
        // Infeasible: missing from ADR + the phantom `Infeasable` ADR lists.
        assert!(
            out.iter()
                .any(|m| m.contains("Infeasible is absent from ADR-0013 §5")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("ADR-0013 §5 lists `Infeasable`")),
            "{out:?}"
        );
        // ReservedAnchor: gone from COVERAGE.
        assert!(
            out.iter()
                .any(|m| m.contains("ReservedAnchor is not named in COVERAGE.md")),
            "{out:?}"
        );
    }

    #[test]
    fn r11_flags_a_witness_without_a_variant_and_a_variant_without_a_witness() {
        // Drop the wInfeasible witness and add one naming a phantom variant.
        let alpha_bad = ALPHABET
            .replace(
                "  val wInfeasible = hasRefusal(r => match r { | Infeasible(_) => true | _ => false })\n",
                "  val wPhantom = hasRefusal(r => match r { | Phantom(_) => true | _ => false })\n",
            );
        let mut out = Vec::new();
        r11_refusals(REFUSE, &alpha_bad, COV_INTENT, ADR5, &mut out);
        assert!(
            out.iter()
                .any(|m| m.contains("witness wPhantom names Phantom, not a refuse.qnt")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("Infeasible has no wInfeasible witness")),
            "{out:?}"
        );
    }

    const INV_QNT: &str = "\
module intent_invariants {
  // ---- helpers ----
  // ---- containmentByTenant (INTENT_I1): every object sits in a container ----
  // ---- edgesTypedAndSingle (INTENT_I2): typed connect ends ----
  // ---- the two rungs ----
}";
    const ADR6: &str = "\
### 6. The invariants the plan type makes unrepresentable

- **INTENT_I1 `containmentByTenant`** — every object sits in a real container.
- **INTENT_I2 `edgesTypedAndSingle`** — typed connect ends, no double connect.

### 7. The scenarios
";
    /// The COVERAGE.md intent-invariants section (task 5.1): a preamble line
    /// the parser must skip, then one row per invariant, matching `INV_QNT`/`ADR6`.
    const COV_INVARIANTS: &str = "\
## Intent invariants (task 5.1)

Plan invariants of `invariants.qnt`, ids INTENT_I1–I9, linted by R12.

| Invariant | Name | CI rung | Anchors / baseline ties |
|-----------|------|---------|-------------------------|
| INTENT_I1 | containmentByTenant | simulate | object-model.md §1 |
| INTENT_I2 | edgesTypedAndSingle | simulate | object-model.md §2 |
";

    #[test]
    fn r12_passes_then_flags_a_renamed_adr_invariant() {
        // The intent-invariants section bounded by the alphabet heading that
        // follows it, as in the real COVERAGE.md; all three copies agree.
        let cov = format!("{COV_INVARIANTS}{COV_INTENT}");
        let mut ok = Vec::new();
        r12_invariants(INV_QNT, &cov, ADR6, &mut ok);
        assert!(ok.is_empty(), "{ok:?}");

        let adr_bad = ADR6.replace("`edgesTypedAndSingle`", "`edgesTypedAndDouble`");
        let mut out = Vec::new();
        r12_invariants(INV_QNT, &cov, &adr_bad, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].contains("INTENT_I2"), "{out:?}");
        assert!(out[0].contains("ADR-0013 §6"), "{out:?}");
    }

    #[test]
    fn r12_flags_a_deleted_coverage_row() {
        // COVERAGE drops the INTENT_I2 row → exactly the missing-row finding.
        let cov_bad = format!(
            "{}{COV_INTENT}",
            COV_INVARIANTS.replace(
                "| INTENT_I2 | edgesTypedAndSingle | simulate | object-model.md §2 |\n",
                ""
            )
        );
        let mut out = Vec::new();
        r12_invariants(INV_QNT, &cov_bad, ADR6, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(
            out[0].contains(
                "INTENT_I2 `edgesTypedAndSingle` (invariants.qnt) has no row in COVERAGE.md's intent-invariants section"
            ),
            "{out:?}"
        );
    }

    #[test]
    fn r12_flags_a_renamed_coverage_name() {
        // COVERAGE renames the INTENT_I2 name cell → the mismatch finding.
        let cov_bad = format!(
            "{}{COV_INTENT}",
            COV_INVARIANTS.replace("edgesTypedAndSingle", "edgesTypedAndDouble")
        );
        let mut out = Vec::new();
        r12_invariants(INV_QNT, &cov_bad, ADR6, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].contains("INTENT_I2"), "{out:?}");
        assert!(
            out[0].contains("COVERAGE.md's intent-invariants section"),
            "{out:?}"
        );
    }

    #[test]
    fn r13_pairs_scenarios_and_matches_the_adr() {
        let adr7 = "\
### 7. The scenarios as worked witnesses

- **fabric** (`scenarios/fabric.*`) — a hardware fabric.
- **vwire** (`scenarios/vwire.*`) — pseudo-wires.

## Consequences
";
        let qnt = vec!["fabric".to_owned(), "vwire".to_owned()];
        let toml = vec!["fabric".to_owned(), "vwire".to_owned()];
        let mut ok = Vec::new();
        r13_scenarios(&qnt, &toml, adr7, &mut ok);
        assert!(ok.is_empty(), "{ok:?}");

        // A .qnt with no .toml, and an ADR scenario with no file.
        let qnt_bad = vec!["fabric".to_owned(), "vwire".to_owned(), "orphan".to_owned()];
        let adr7_bad = adr7.replace("**vwire**", "**ghost**");
        let mut out = Vec::new();
        r13_scenarios(&qnt_bad, &toml, &adr7_bad, &mut out);
        assert!(
            out.iter().any(|m| m.contains("orphan.qnt has no")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("orphan is absent from ADR-0013 §7")),
            "{out:?}"
        );
        assert!(out.iter().any(|m| m.contains("lists ghost")), "{out:?}");
    }

    /// A `types.qnt` slice: the several-to-a-line `type Family =` block and a
    /// following declaration the parser must not fold in.
    const TYPES: &str = "\
module core_types {
  type Family =
    | Dprc | Dpni
    | Dpmac

  type ObjId = { fam: Family, num: int }
}";

    #[test]
    fn parses_family_variants_across_lines() {
        assert_eq!(parse_family_variants(TYPES), vec!["Dprc", "Dpni", "Dpmac"]);
    }

    #[test]
    fn r14_passes_when_the_rust_copies_match_the_model() {
        // REFUSE names the anchor `ReservedAnchor`; the Rust copy carries the
        // accepted `Reserved` spelling, so the alias must bridge them.
        let refusals = ["TenantAbsent", "Reserved", "Infeasible"];
        let families = ["Dprc", "Dpni", "Dpmac"];
        let warnings = ["UnknownCeiling"]; // REFUSE's lone Warning variant
        let family_strs: Vec<&str> = dpaa2_api::core::family::ALL_FAMILIES
            .iter()
            .map(|f| f.as_str())
            .collect();
        let mut out = Vec::new();
        r14_rust_copies(
            REFUSE,
            TYPES,
            RAW_QNT,
            &refusals,
            &families,
            &warnings,
            &family_strs,
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn r14_flags_a_dropped_refusal_and_a_renamed_family() {
        // The deliberate-drift negative test, both directions: the Rust copies
        // drop a refusal (`Infeasible`) and rename a family (`Dpmac` → `Dpmax`),
        // all in memory.
        let refusals_bad = ["TenantAbsent", "Reserved"]; // Infeasible gone
        let families_bad = ["Dprc", "Dpni", "Dpmax"]; // renamed
        let warnings = ["UnknownCeiling"]; // matches REFUSE, no warning drift here
        let family_strs: Vec<&str> = dpaa2_api::core::family::ALL_FAMILIES
            .iter()
            .map(|f| f.as_str())
            .collect();
        let mut out = Vec::new();
        r14_rust_copies(
            REFUSE,
            TYPES,
            RAW_QNT,
            &refusals_bad,
            &families_bad,
            &warnings,
            &family_strs,
            &mut out,
        );
        // Model has Infeasible, the Rust copy does not.
        assert!(
            out.iter()
                .any(|m| m
                    .contains("refuse.qnt Refusal variant Infeasible has no dpaa2_api::Refusal")),
            "{out:?}"
        );
        // Model has Dpmac, the Rust copy does not.
        assert!(
            out.iter().any(
                |m| m.contains("types.qnt Family Dpmac has no dpaa2_api::core::family::Family")
            ),
            "{out:?}"
        );
        // The Rust copy's Dpmax has no model family.
        assert!(
            out.iter()
                .any(|m| m
                    .contains("dpaa2_api::core::family::Family Dpmax is absent from types.qnt")),
            "{out:?}"
        );
    }

    /// The real crate enums agree with the real model files — the same check the
    /// integration test runs, kept here so a `dpaa2_api` edit fails the unit
    /// suite too (ADR-0014).
    #[test]
    fn r14_ties_the_live_dpaa2_api_enums_to_the_model() {
        let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
        let refuse = std::fs::read_to_string(format!("{root}/models/intent/refuse.qnt"))
            .expect("read refuse.qnt");
        let types = std::fs::read_to_string(format!("{root}/models/core/types.qnt"))
            .expect("read types.qnt");
        let families: Vec<&str> = dpaa2_api::core::family::ALL_FAMILIES
            .iter()
            .map(|f| f.variant_name())
            .collect();
        let family_strs: Vec<&str> = dpaa2_api::core::family::ALL_FAMILIES
            .iter()
            .map(|f| f.as_str())
            .collect();
        let mut out = Vec::new();
        r14_rust_copies(
            &refuse,
            &types,
            RAW_QNT,
            &dpaa2_api::intent::refuse::REFUSAL_VARIANTS,
            &families,
            &dpaa2_api::intent::refuse::WARNING_VARIANTS,
            &family_strs,
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    // --- intent-layer identity laws (R15) ---

    /// A `match.qnt` slice: the "Named invariants" header block (the four laws,
    /// three as `pure def … : bool`, swapCorrect as a `run`), plus helper bool
    /// defs and prose `(decision N)` mentions the parser must not fold in.
    const MATCH: &str = "\
module intent_match {
  // flaw (decision 10) was found by hand — prose, not a law.
  //
  // Named invariants (the four families, each citing its decision):
  //   frameLaw (decision 12) — an edit perturbs no object outside the cone.
  //     cone, ordinal renumbering notwithstanding (decision 8).
  //   renameSelfNeutralizes (decision 10) — the `from` clause is inert.
  //   convergeIdempotent (decisions 9-11) — the board is a fixpoint.
  //   swapCorrect (decision 10) — the wan0<->e0 swap cross-binds.
  // Apalache-marked subset: none.
  pure def isPaired(pairs: Set[MatchPair], c: CompiledObject): bool = false
  pure def frameLaw(pre: Board, post: Board, touched: Set[str]): bool = true
  run swapCorrectTest = all { true }
}";

    #[test]
    fn parse_match_laws_reads_the_four_named_laws() {
        assert_eq!(
            parse_match_laws(MATCH),
            vec![
                "frameLaw",
                "renameSelfNeutralizes",
                "convergeIdempotent",
                "swapCorrect",
            ]
        );
    }

    /// The COVERAGE.md identity-laws section (task 6.4): a preamble line the
    /// parser skips, then one row per law matching `MATCH`'s named set.
    const COV_LAWS: &str = "\
## Identity-across-time laws (task 6.4)

Identity laws of `match.qnt`, linted by R15.

| Law | Name | CI rung | Anchors / ties |
|-----|------|---------|----------------|
| Frame | frameLaw | simulate + property | ADR-0015 decision 12 |
| Rename-inert | renameSelfNeutralizes | simulate + property | ADR-0015 decision 10 |
| Fixpoint | convergeIdempotent | simulate + property | ADR-0015 decisions 9-11 |
| Swap | swapCorrect | simulate + property | ADR-0015 decision 10 |
";

    #[test]
    fn r15_passes_when_the_table_matches_the_model() {
        let mut ok = Vec::new();
        r15_identity_laws(MATCH, COV_LAWS, &mut ok);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn r15_flags_a_dropped_law_and_a_renamed_table_name() {
        // COVERAGE drops the frameLaw row and renames swapCorrect → the
        // missing-law finding plus the phantom-table-name finding.
        let cov_bad = COV_LAWS
            .replace(
                "| Frame | frameLaw | simulate + property | ADR-0015 decision 12 |\n",
                "",
            )
            .replace("swapCorrect", "swapWrong");
        let mut out = Vec::new();
        r15_identity_laws(MATCH, &cov_bad, &mut out);
        assert!(
            out.iter()
                .any(|m| m.contains("match.qnt law `frameLaw` has no row")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("identity-laws table lists `swapWrong`")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("match.qnt law `swapCorrect` has no row")),
            "{out:?}"
        );
    }

    // --- intent-layer raw-surface laws + Rust warning/family-name copies (R16, R14) ---

    /// An `intent_raw.qnt` slice: the real three-line `// Named invariants:`
    /// header and its `// Apalache-marked` footer (verbatim, lines 49-53), plus
    /// the real 16-pair `FAMILY_NAMES` mapping (lines 153-157). The header block
    /// feeds `parse_raw_laws`; the mapping feeds `parse_raw_family_names`.
    const RAW_QNT: &str = "\
// Named invariants: parseOkWellFormed (surface-refusal law), acceptedSurvives
//   (no-surprise law), nearMissRefusedByName (each near-miss refuses by name) —
//   collected as intent/raw_alphabet.qnt `rawInvariants`, simulator-only.
// Apalache-marked subset: none (the quantifier shape is measured in the parcel
//   summary; the seal decision is upstream, per the compileLaws precedent).
module intent_raw {
  pure val FAMILY_NAMES: List[(str, Family)] = [
    (\"dprc\", Dprc), (\"dpni\", Dpni), (\"dpmac\", Dpmac), (\"dpbp\", Dpbp),
    (\"dpio\", Dpio), (\"dpcon\", Dpcon), (\"dpmcp\", Dpmcp), (\"dpseci\", Dpseci),
    (\"dpsw\", Dpsw), (\"dpdmux\", Dpdmux), (\"dpaiop\", Dpaiop), (\"dpci\", Dpci),
    (\"dpdcei\", Dpdcei), (\"dpdmai\", Dpdmai), (\"dprtc\", Dprtc), (\"dpdbg\", Dpdbg)]
}";

    /// The COVERAGE.md raw-surface-laws section (task 3.3e): a preamble line the
    /// parser skips, then the real 4-row table. The `raw_conformance` row names
    /// the Rust harness, and its underscore keeps it outside the leg.
    const COV_RAW: &str = "\
## Raw surface laws (task 3.3e)

The `intent-layer` change's config-surface laws.

| Law | Name | CI rung | Anchors / ties |
|-----|------|---------|----------------|
| Surface-refusal | parseOkWellFormed | simulate | ADR-0013 §2, §5; parse.rs convert |
| No-surprise | acceptedSurvives | simulate | ADR-0013 §2; design D5 |
| Near-miss-by-name | nearMissRefusedByName | simulate | ADR-0013 §5; parse.rs |
| MBT-conformance | raw_conformance | itf-replay | ADR-0013 §2/§5; parse.rs |
";

    #[test]
    fn parse_raw_laws_reads_the_three_named_laws() {
        assert_eq!(
            parse_raw_laws(RAW_QNT),
            vec![
                "parseOkWellFormed",
                "acceptedSurvives",
                "nearMissRefusedByName",
            ]
        );
    }

    #[test]
    fn r16_passes_when_the_table_matches_the_model() {
        let mut ok = Vec::new();
        r16_raw_laws(RAW_QNT, COV_RAW, &mut ok);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn r16_flags_a_dropped_law_and_a_renamed_table_name() {
        // COVERAGE drops the parseOkWellFormed row and renames
        // nearMissRefusedByName → the missing-law findings plus the phantom
        // table-name finding.
        let cov_bad = COV_RAW
            .replace(
                "| Surface-refusal | parseOkWellFormed | simulate | ADR-0013 §2, §5; parse.rs convert |\n",
                "",
            )
            .replace("nearMissRefusedByName", "nearMissRenamed");
        let mut out = Vec::new();
        r16_raw_laws(RAW_QNT, &cov_bad, &mut out);
        assert!(
            out.iter()
                .any(|m| m.contains("intent_raw.qnt law `parseOkWellFormed` has no row")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("raw-surface-laws table lists `nearMissRenamed`")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("intent_raw.qnt law `nearMissRefusedByName` has no row")),
            "{out:?}"
        );
    }

    #[test]
    fn r14_flags_warning_and_family_name_drift() {
        // A refuse.qnt fixture whose Warning block adds a seeded third variant
        // (the two real ones plus FutureWarning), and a Rust family-name list
        // that drops `dpdbg` and adds `dpfoo` — both drifts, in memory.
        let refuse_bad = REFUSE.replace(
            "  type Warning =\n    | UnknownCeiling({ family: Family, needed: int })\n",
            "  type Warning =\n    | UnknownCeiling({ family: Family, needed: int })\n    | UnmeasuredCombination({ tenant: str, rates: Set[int] })\n    | FutureWarning({ tenant: str })\n",
        );
        let refusals = ["TenantAbsent", "Reserved", "Infeasible"];
        let families = ["Dprc", "Dpni", "Dpmac"];
        let warnings = ["UnknownCeiling", "UnmeasuredCombination"]; // FutureWarning absent
        let family_strs: Vec<&str> = dpaa2_api::core::family::ALL_FAMILIES
            .iter()
            .map(|f| f.as_str())
            .filter(|s| *s != "dpdbg") // drop dpdbg
            .chain(std::iter::once("dpfoo")) // add a phantom
            .collect();
        let mut out = Vec::new();
        r14_rust_copies(
            &refuse_bad,
            TYPES,
            RAW_QNT,
            &refusals,
            &families,
            &warnings,
            &family_strs,
            &mut out,
        );
        assert!(
            out.iter()
                .any(|m| m.contains("Warning variant FutureWarning has no")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("as_str name dpfoo is absent")),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|m| m.contains("FAMILY_NAMES entry dpdbg has no")),
            "{out:?}"
        );
    }
}
