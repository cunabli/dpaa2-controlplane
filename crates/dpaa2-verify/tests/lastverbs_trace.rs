//! The model records, per step, the §2 verb keys the action emitted through
//! the `/dev/dprc.N` ioctl path (`machine.qnt` `lastVerbs`). This test keeps
//! the Rust verb catalogue honest against that record: for every committed
//! ITF trace that carries `lastVerbs`, the step's set must equal
//! `ioctlpolicy::verbs_of` for the step's action — a direct string-set
//! comparison, no command-id resolution. Every committed trace is now
//! regenerated to carry `lastVerbs`, so the test fails if one lacks it. The
//! verb comparison runs only on `--mbt` board traces, whose states carry an
//! action label; the retro `quint test` freezes have no action and are held
//! to `lastVerbs` presence only.

use std::collections::BTreeSet;

use dpaa2_verify::adapter::{ModelAction, parse_mbt_trace};
use dpaa2_verify::ioctlpolicy::verbs_of;
use serde_json::Value;

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}/../..", env!("CARGO_MANIFEST_DIR")))
}

/// Every committed ITF trace: the retro freezes and the per-suite board
/// traces beside their scenario modules.
fn trace_files() -> Vec<std::path::PathBuf> {
    let root = repo_root();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join("models/traces")) {
        for e in entries.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) == Some("json") {
                out.push(e.path());
            }
        }
    }
    for suite in std::fs::read_dir(root.join("models/board"))
        .expect("models/board")
        .flatten()
    {
        if !suite.path().is_dir() {
            continue;
        }
        for e in std::fs::read_dir(suite.path())
            .expect("suite dir")
            .flatten()
        {
            let p = e.path();
            if p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".itf.json"))
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// The `lastVerbs` value of one ITF state, if the trace carries it. The var
/// is stored under its fully-qualified Quint name (`…::machine::lastVerbs`),
/// so it is found by suffix, not an exact key.
fn last_verbs(state: &Value) -> Option<&Value> {
    state
        .as_object()?
        .iter()
        .find(|(k, _)| k.ends_with("lastVerbs"))
        .map(|(_, v)| v)
}

/// The `{"#set": ["dprc assign", …]}` of an ITF `Set[str]`.
fn itf_str_set(v: &Value) -> BTreeSet<String> {
    v["#set"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e.as_str().map(str::to_owned))
        .collect()
}

fn expected(action: &ModelAction) -> BTreeSet<String> {
    verbs_of(action).into_iter().collect()
}

#[test]
fn committed_traces_lastverbs_matches_the_verb_catalogue() {
    let mut carried = 0usize;
    let mut skipped = 0usize;
    let mut saw_nonempty = false;

    for path in trace_files() {
        let json = std::fs::read_to_string(&path).expect("read trace");
        let root: Value = serde_json::from_str(&json).expect("parse trace json");
        let states = root["states"].as_array().expect("trace states");
        if !states.iter().any(|s| last_verbs(s).is_some()) {
            skipped += 1;
            continue;
        }
        carried += 1;
        // Every carried state must expose the variable; a non-empty set on any
        // step proves the cross-check below is exercised, not vacuous.
        for s in states {
            let lv = last_verbs(s).expect("state carries lastVerbs");
            saw_nonempty |= !itf_str_set(lv).is_empty();
        }
        // The verb comparison needs the action label, which only `--mbt`
        // traces carry; the retro `quint test` freezes have none, so they are
        // held to presence only.
        if !states.iter().any(|s| s.get("mbt::actionTaken").is_some()) {
            continue;
        }
        let trace = parse_mbt_trace(&json).expect("parse mbt trace with lastVerbs");
        for (k, step) in trace.steps.iter().enumerate() {
            let last = itf_str_set(last_verbs(&states[k + 1]).expect("step carries lastVerbs"));
            assert_eq!(
                last,
                expected(&step.action),
                "{}: step {k} action {:?} lastVerbs mismatch",
                path.display(),
                step.action
            );
        }
    }

    assert_eq!(
        skipped, 0,
        "every committed trace must carry lastVerbs; {skipped} lacked it"
    );
    assert!(carried > 0, "no committed trace carried lastVerbs");
    assert!(
        saw_nonempty,
        "no step carried a non-empty lastVerbs set — the cross-check was vacuous"
    );

    println!("lastVerbs cross-check: {carried} trace(s) carried lastVerbs, {skipped} skipped");
}
