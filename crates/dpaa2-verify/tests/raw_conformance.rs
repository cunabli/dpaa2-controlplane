//! The raw-surface MBT-conformance CI rung (task 3.3e, bead gqf.48): every frozen
//! raw trace serializes to a TOML document on the real `dpaa2-config` surface, feeds
//! through `parse_str`, and the Rust verdict is asserted to agree with the model's
//! own frozen `parse` — the mechanism that catches a clause the model has and
//! `parse.rs` forgot, or vice versa (the gqf.39/gqf.40 bug class). The model is the
//! oracle; a regenerated trace whose model verdict no longer matches what `parse_str`
//! derives fails here loudly. Regenerate with `pnpm model:freeze-raw`.
//!
//! Coverage is asserted over the corpus, not by hand: the union of near-miss kinds
//! the frozen model verdicts carry must cover every dimension the dirty alphabet
//! reaches. A missing kind — an unpaired near-miss — is the gqf.39 hole shape and
//! fails the harness.

use std::collections::BTreeSet;

use dpaa2_config::parse_str;
use dpaa2_verify::raw_itf::{Kind, ModelVerdict, kinds, normalize, parse_raw_case, to_toml};

/// Every committed raw trace under `models/intent/traces/` (freeze names, sans suffix).
const TRACES: &[&str] = &[
    "rawAcceptedTrace",
    "rawReservedKernelTrace",
    "rawDuplicateNameTrace",
    "rawSelfLoopTrace",
    "rawUnresolvedMemberTrace",
    "rawUnknownFamilyTrace",
    "rawDanglingTenantTrace",
    "rawPoolContradictionTrace",
    "rawDirtyMixTrace",
];

/// Every near-miss dimension the corpus must exercise (the task 3.3e list): reserved
/// kernel, duplicate name, self-loop link, unresolved member, unknown family, dangling
/// tenant reference (`TenantAbsent`), and both isolation/pool contradictions.
const REQUIRED_KINDS: &[Kind] = &[
    Kind::ReservedKernel,
    Kind::DuplicateName,
    Kind::LinkSelfLoop,
    Kind::RawMemberUnresolved,
    Kind::UnknownExtraFamily,
    Kind::TenantAbsent,
    Kind::PoolWithoutRestricted,
    Kind::RestrictedWithoutPool,
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/intent/traces/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn raw_traces_conform_to_the_config_parser() {
    let mut seen_accepted = false;
    let mut seen_kinds: BTreeSet<Kind> = BTreeSet::new();

    for file in TRACES {
        let case = parse_raw_case(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        let toml = to_toml(&case.raw);
        let rust = parse_str(&toml);

        match &case.verdict {
            ModelVerdict::Ok(model_intent) => {
                seen_accepted = true;
                let mut rust_intent = rust.unwrap_or_else(|e| {
                    panic!("{file}: model accepts but parse.rs refuses: {e}\n--- TOML ---\n{toml}")
                });
                let mut model_intent = (**model_intent).clone();
                normalize(&mut rust_intent);
                normalize(&mut model_intent);
                assert_eq!(
                    rust_intent, model_intent,
                    "{file}: the parsed Intent diverges from the model's\n--- TOML ---\n{toml}"
                );
            }
            ModelVerdict::Refused(set) => {
                seen_kinds.extend(kinds(&case.verdict));
                let err = rust
                    .err()
                    .unwrap_or_else(|| {
                        panic!("{file}: model refuses but parse.rs accepts\n--- TOML ---\n{toml}")
                    })
                    .to_string();
                // parse.rs short-circuits with ONE error; the model carries the whole
                // set (the intent_raw.qnt DEVIATION). The Rust error must correspond to
                // at least one refusal in the model set.
                assert!(
                    set.iter().any(|r| r.matches_error(&err)),
                    "{file}: parse.rs error `{err}` matches no model refusal in {set:?}\n\
                     --- TOML ---\n{toml}"
                );
            }
        }
    }

    assert!(seen_accepted, "the corpus visited no accepted state");
    for kind in REQUIRED_KINDS {
        assert!(
            seen_kinds.contains(kind),
            "near-miss kind {kind:?} is absent from the trace corpus — an unpaired \
             near-miss is the gqf.39 hole shape; add a trace that reaches it"
        );
    }
}

#[test]
fn conformance_detects_a_diverging_intent() {
    // Mutating the raw under a frozen ParseOk must break the diff: bumping the
    // accepted crypto's flows (a leaf with no cross-references, so it still parses)
    // yields an Intent the model's frozen one no longer equals — the comparator must
    // catch it, so the accepted arm is not vacuous.
    let mut case = parse_raw_case(&load("rawAcceptedTrace")).expect("parse");
    let ModelVerdict::Ok(model_intent) = &case.verdict else {
        panic!("rawAcceptedTrace is not accepted");
    };
    case.raw.crypto[0].flows += 1;
    let mut rust_intent = parse_str(&to_toml(&case.raw)).expect("still parses");
    let mut model_intent = (**model_intent).clone();
    normalize(&mut rust_intent);
    normalize(&mut model_intent);
    assert_ne!(
        rust_intent, model_intent,
        "a bumped crypto flows must diverge from the frozen accepted Intent"
    );
}
