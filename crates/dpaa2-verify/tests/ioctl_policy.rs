//! The ioctl policy table (`docs/baseline/mc-ioctl-policy.md`) and the
//! code that resolves against it must agree, so a drift fails here rather
//! than surprising an operator when the kernel refuses a command the
//! table said was fine.
//!
//! Three cross-checks: the md §2 verb keys match the generated Quint
//! `VERB_CMDIDS` keys (the model owns the same catalogue the md renders);
//! every `§2` row's verdict recomputes from the `§1` whitelist (the table
//! is not trusted, it is re-derived); the `§3` probes are refused by that
//! same whitelist. A fourth asserts the operator note reaches a suite
//! whose steps need `CAP_NET_ADMIN`.

use std::collections::{BTreeMap, BTreeSet};

use dpaa2_verify::adapter::{CreateArgs, parse_mbt_trace};
use dpaa2_verify::generate::{RecoveryGuarantee, SuiteKind, SuiteSpec, generate};
use dpaa2_verify::ioctlpolicy::{Whitelist, parse_outside, parse_verbs};
use dpaa2_verify::safety::{RunClass, TrafficClass};

fn policy_md() -> String {
    let path = format!(
        "{}/../../docs/baseline/mc-ioctl-policy.md",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn md_section2_keys_match_the_quint_verb_catalogue() {
    // The md §2 verb keys and the generated Quint `VERB_CMDIDS` keys come
    // from one generator run, so they must agree: the model owns the same
    // verb→cmdid catalogue the Rust side reads out of the md.
    let md_keys: BTreeSet<String> = parse_verbs(&policy_md())
        .into_iter()
        .map(|r| r.key)
        .collect();

    let qnt_path = format!(
        "{}/../../models/core/ioctl_policy.qnt",
        env!("CARGO_MANIFEST_DIR")
    );
    let qnt = std::fs::read_to_string(&qnt_path).unwrap_or_else(|e| panic!("read {qnt_path}: {e}"));
    // The generator writes each entry as `    "<key>" -> Set(…)`; PROBES
    // entries render `"<id>" -> 0x…` and carry no `Set`, so keying on
    // ` -> Set` selects the VERB_CMDIDS block only.
    let qnt_keys: BTreeSet<String> = qnt
        .lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let rest = l.strip_prefix('"')?;
            let (key, tail) = rest.split_once('"')?;
            tail.trim_start()
                .starts_with("-> Set")
                .then(|| key.to_owned())
        })
        .collect();

    assert_eq!(
        md_keys, qnt_keys,
        "md §2 verb keys and ioctl_policy.qnt VERB_CMDIDS keys disagree"
    );
}

#[test]
fn every_table_verdict_recomputes_from_the_whitelist() {
    let md = policy_md();
    let wl = Whitelist::parse(&md);
    assert!(!wl.entries.is_empty(), "no §1 whitelist parsed");

    for row in parse_verbs(&md) {
        assert!(
            !row.verdict.contains("refused"),
            "verb {:?} resolves to a refusal: {}",
            row.key,
            row.verdict
        );
        // Rows that list no command ids (`dprc sync`, the abbreviated
        // generate-dpl walk) carry no id to recompute.
        if !row.cmdids.is_empty() {
            assert_eq!(
                wl.verdict_str(&row.cmdids),
                row.verdict,
                "verb {:?} recomputes to a different verdict than the table",
                row.key
            );
        }
    }
}

#[test]
fn outside_the_whitelist_probes_are_refused() {
    let md = policy_md();
    let wl = Whitelist::parse(&md);
    let probes = parse_outside(&md);
    assert!(!probes.is_empty(), "no §3 probes parsed");
    for (cmd, id) in probes {
        assert_eq!(
            wl.check(id),
            dpaa2_verify::ioctlpolicy::Verdict::Refused,
            "{cmd:?} (cmdid {id:#06x}) is on the whitelist, so §3 mislabels it"
        );
    }
}

#[test]
fn a_cap_net_admin_suite_carries_the_operator_note() {
    // V-READBACK-1 creates a DPNI (DPNI_CMDID_CREATE, CAP_NET_ADMIN), so
    // its regenerated header must tell the operator to run as root.
    let path = format!(
        "{}/../../models/board/V-READBACK-1/vreadback1.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let json = std::fs::read_to_string(&path).expect("read committed trace");
    let trace = parse_mbt_trace(&json).expect("parse trace");
    let spec = SuiteSpec {
        id: "V-READBACK-1".to_owned(),
        run: RunClass {
            class: TrafficClass::ObjectLifecycleOnly,
            flagged: false,
        },
        kind: SuiteKind::Standard,
        trace_file: path.clone(),
        hook: None,
        create_args: CreateArgs::default(),
        expected_refusals: BTreeMap::new(),
    };
    let suite = generate(&spec, &trace, RecoveryGuarantee::Verified).expect("generate");
    assert!(
        suite.script.contains("# operator: run as root — "),
        "{}",
        suite.script
    );
    assert!(
        suite
            .script
            .contains("gates on CAP_NET_ADMIN (docs/baseline/mc-ioctl-policy.md)"),
        "{}",
        suite.script
    );
}
