//! The edit-alphabet MBT replay CI rung (task 6.7): every committed `editsSweep*` trace
//! (frozen from `models/intent/edits.qnt`) drives the model's own converge through the
//! shipped `dpaa2-config` parser and the Rust `converge_match`, and the board is diffed
//! against the model's. An edit clause the model has and Rust forgot — a rename the parser
//! drops, a config the projection loses — fails here loudly. Regenerate with
//! `pnpm model:freeze-edits` and reconcile the transcription.
//!
//! At each `converge`→`perturb` transition (one `convergeStep`): the pre-state's intent is
//! RENDERED as TOML, parsed with `dpaa2_config::parse_str`, projected into
//! [`MatchObject`]s, and `converge_match`'d against the decoded pre-state board; the result
//! must equal the decoded post-state board. At each synced (`perturb`) state the four
//! identity laws are asserted with that state's `touched`/`preBoard`, mirroring the model's
//! `identityLaws`.
//!
//! TOML mapping (transcribed as task 6.5 did — anchored = port, unanchored = link):
//! - anchored `{anchor:{N}}` → `[port.<name>]` on `dpmac.N`, a fixed valid rate, the
//!   reserved kernel tenant; `renamed = { from }` when the model stamped one. The model's
//!   config int maps to [`ConfigFacet::Anchored`] (which carries no attribute), so the port
//!   carries no config beyond its anchor — the anchored-config repair is invisible to this
//!   diff (a documented ceiling, `edits_itf.rs`).
//! - unanchored → `[link.<name>]` between the kernel and the declared far tenant
//!   `cfg0`/`cfg1` per the config int, both declared minimally in the preamble; `renamed`
//!   when stamped. The parser preserves the declared end order (`interface_a`,
//!   `interface_b`) — it does NOT canonicalize link ends (`dpaa2-config` `convert_link`) —
//!   so the projection reads ends `(kernel, cfgN)`, byte-for-byte the board decode's order.
//! - handles: the model's id → `Handle::new(id)`; a created object appears at the pre
//!   board's `max+1`, exactly as the Rust `apply` mints it. This alignment is safe ONLY
//!   because strict alternation yields ≤1 create per `convergeStep` — asserted per step
//!   below, so a future multi-create alphabet fails loudly here rather than flaking.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use dpaa2_api::{
    BoardObject, ConfigFacet, ConstructName, Family, Intent, MatchObject, MatchVerdict,
    converge_match, match_board,
};
use dpaa2_verify::edits_itf::{EditState, Phase, parse_edits_trace};

/// Every committed edit-alphabet trace under `models/intent/traces/`.
const TRACES: &[&str] = &["editsSweep1", "editsSweep2", "editsSweep3"];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/intent/traces/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

// ---- the model's cone helpers, reimplemented locally (matcher.rs test-local twins) ----

fn outside_cone(
    board: &BTreeSet<BoardObject>,
    touched: &BTreeSet<ConstructName>,
) -> BTreeSet<BoardObject> {
    board
        .iter()
        .filter(|o| o.label.as_ref().is_none_or(|l| !touched.contains(l)))
        .cloned()
        .collect()
}

fn frame_law(
    pre: &BTreeSet<BoardObject>,
    post: &BTreeSet<BoardObject>,
    touched: &BTreeSet<ConstructName>,
) -> bool {
    outside_cone(pre, touched) == outside_cone(post, touched)
}

/// renameSelfNeutralizes (`match.qnt`, decision 10): after one converge, the same intent
/// re-matches with an empty `renamed` set.
fn rename_self_neutralizes(intent: &BTreeSet<MatchObject>, board: &BTreeSet<BoardObject>) -> bool {
    match match_board(&converge_match(intent, board), intent) {
        MatchVerdict::Ok(plan) => plan.renamed.is_empty(),
        MatchVerdict::Refused(_) => false,
    }
}

// ---- the intent → TOML → parse → project round trip (the parser under test) ----

/// Renders a decoded model intent as an operator TOML document — anchored objects as
/// `[port]` tables, unanchored as `[link]` tables (the task-6.5 transcription).
fn render_toml(intent: &BTreeSet<MatchObject>) -> String {
    let mut s = String::from(
        "[intent]\nschema = 1\n\n\
         [tenant.cfg0]\ndataplane = \"userspace-poll\"\nmax_cores = 16\n\n\
         [tenant.cfg1]\ndataplane = \"userspace-poll\"\nmax_cores = 16\n",
    );
    for o in intent {
        let rename = o
            .from
            .as_ref()
            .map(|f| format!("renamed = {{ from = \"{f}\" }}\n"))
            .unwrap_or_default();
        if let Some(dpmac) = o.anchor.iter().next() {
            let _ = write!(
                s,
                "\n[port.{}]\ndpmac = \"{dpmac}\"\nrate = 10000\ntenant = \"kernel\"\n{rename}",
                o.name
            );
        } else {
            let ConfigFacet::Link { ends } = &o.config else {
                unreachable!("an unanchored edit object is a link facet (edits_itf.rs)");
            };
            let _ = write!(
                s,
                "\n[link.{}]\ninterface_a = \"{}\"\ninterface_b = \"{}\"\n{rename}",
                o.name, ends.0, ends.1
            );
        }
    }
    s
}

/// Projects a parsed [`Intent`] into the matcher's [`MatchObject`] world — the same facet
/// mapping `edits_itf.rs` decodes the board with, so the two agree byte-for-byte.
fn project(intent: &Intent) -> BTreeSet<MatchObject> {
    let ports = intent.ports.iter().map(|p| MatchObject {
        family: Family::Dpni,
        name: p.name.clone(),
        anchor: BTreeSet::from([p.dpmac]),
        config: ConfigFacet::Anchored,
        from: p.renamed.clone(),
    });
    let links = intent.links.iter().map(|l| MatchObject {
        family: Family::Dpni,
        name: l.name.clone(),
        anchor: BTreeSet::new(),
        config: ConfigFacet::Link {
            // The matcher facet keys ends by tenant NAME (task 4b.1, unchanged);
            // resolve each `TenantRef` end to the name it stands for.
            ends: (l.interface_a.resolved(), l.interface_b.resolved()),
        },
        from: l.renamed.clone(),
    });
    ports.chain(links).collect()
}

/// The decoded pre-state intent, round-tripped through the shipped parser.
fn parse_and_project(state: &EditState) -> BTreeSet<MatchObject> {
    let toml = render_toml(&state.intent);
    // A parser refusal of a model-legal TOML IS the harness firing — first suspect the
    // mapping (render_toml/project), not the parser.
    let intent = dpaa2_config::parse_str(&toml)
        .unwrap_or_else(|e| panic!("parser refused a model-legal intent: {e}\n---\n{toml}"));
    project(&intent)
}

#[test]
fn edit_traces_replay_green() {
    for file in TRACES {
        let states = parse_edits_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(states.len() >= 3, "{file}: a trace with no transitions");

        for (i, pair) in states.windows(2).enumerate() {
            let (pre, post) = (&pair[0], &pair[1]);

            // Synced-state laws (mirroring identityLaws): assert at each perturb state.
            if pre.phase == Phase::Perturb {
                assert_laws(file, i, pre);
            }

            // A convergeStep transition: converge the parsed intent against the pre board.
            if pre.phase != Phase::Converge {
                continue;
            }
            let parsed = parse_and_project(pre);
            assert_eq!(
                parsed, pre.intent,
                "{file} state {i}: the rendered+parsed intent lost or altered a construct"
            );

            // Strict alternation yields ≤1 create per convergeStep; the handle alignment
            // (created at max+1) is safe only under it — assert it so a future
            // multi-create alphabet fails here loudly.
            if let MatchVerdict::Ok(plan) = match_board(&pre.board, &parsed) {
                assert!(
                    plan.created.len() <= 1,
                    "{file} state {i}: {} creates in one convergeStep — the max+1 handle \
                     alignment is no longer safe",
                    plan.created.len()
                );
            }

            let computed = converge_match(&parsed, &pre.board);
            assert_eq!(
                computed, post.board,
                "{file} state {i}: Rust converge diverges from the model's frozen board"
            );
        }

        // The final state is synced too; the window loop stops before it.
        if let Some(last) = states.last()
            && last.phase == Phase::Perturb
        {
            assert_laws(file, states.len() - 1, last);
        }
    }
}

/// The four identity laws at a synced state (`edits.qnt` `identityLaws`).
fn assert_laws(file: &str, i: usize, s: &EditState) {
    let once = converge_match(&s.intent, &s.board);
    assert_eq!(
        converge_match(&s.intent, &once),
        once,
        "{file} state {i}: convergeIdempotent"
    );
    assert!(
        frame_law(&s.pre_board, &s.board, &s.touched),
        "{file} state {i}: frameLaw over cone {:?}",
        s.touched
    );
    assert!(
        rename_self_neutralizes(&s.intent, &s.board),
        "{file} state {i}: renameSelfNeutralizes"
    );
}

#[test]
fn replay_detects_a_diverging_board() {
    // Mutating a decoded intent under a frozen board must break the diff: emptying the
    // intent at a convergeStep whose result is non-empty removes every object, so the
    // converge no longer reproduces the frozen board — the comparator must catch it.
    let states = parse_edits_trace(&load("editsSweep1")).expect("parse");
    let step = states
        .windows(2)
        .find(|w| w[0].phase == Phase::Converge && !w[1].board.is_empty())
        .expect("a convergeStep with a non-empty result");
    let mutated = BTreeSet::new(); // the intent with every construct forgotten
    assert_ne!(
        converge_match(&mutated, &step[0].board),
        step[1].board,
        "a forgotten intent must diverge from the frozen board"
    );
}
