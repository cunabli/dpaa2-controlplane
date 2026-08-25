//! The four hand-maintained ledgers — `models/COVERAGE.md`, the
//! `docs/baseline/dp*.md` invariant tables, `models/board/README.md`'s
//! suite ledger, and `docs/ROADMAP.md` — must agree on what is modeled,
//! deferred, and board-settled, and on which suite and owning change each
//! candidate names. Drift here silently misleads a reviewer about what
//! the board has actually settled, so it fails here instead. The real
//! files are read from the repo root; `dir_exists` checks the on-disk
//! `models/board/<id>/` directory the suite ledger promises.

use std::path::{Path, PathBuf};

use dpaa2_verify::ledger::{
    Coverage, LintInput, lint, parse_baseline_table, parse_coverage, parse_register, parse_roadmap,
    parse_scenario_ids, parse_suite_ledger,
};
use dpaa2_verify::verdict::{Index, parse_index};

/// The repository root, two levels above this crate's manifest.
fn repo_root() -> PathBuf {
    PathBuf::from(format!("{}/../..", env!("CARGO_MANIFEST_DIR")))
}

fn read(root: &Path, rel: &str) -> String {
    let path = root.join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn the_four_ledgers_agree() {
    let root = repo_root();

    let coverage_md = read(&root, "models/COVERAGE.md");
    let coverage: Coverage = parse_coverage(&coverage_md).expect("parse COVERAGE.md");

    // Every family file except the non-invariant docs.
    let skip = [
        "object-model",
        "reference-environment",
        "_template",
        "traffic-inventory",
    ];
    let mut baseline: Vec<(String, String)> = Vec::new();
    let baseline_dir = root.join("docs/baseline");
    for entry in std::fs::read_dir(&baseline_dir).expect("docs/baseline") {
        let path = entry.expect("entry").path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let is_md = path.extension().and_then(|e| e.to_str()) == Some("md");
        if !is_md || skip.contains(&stem) {
            continue;
        }
        let md = std::fs::read_to_string(&path).expect("read baseline");
        baseline.extend(parse_baseline_table(&md));
    }

    let suites = parse_suite_ledger(&read(&root, "models/board/README.md"));
    let roadmap = parse_roadmap(&read(&root, "docs/ROADMAP.md"));
    let scenarios = parse_scenario_ids(&read(&root, "docs/baseline/traffic-inventory.md"));

    let board = root.join("models/board");
    let index: Index =
        parse_index(&read(&root, "models/board/VERDICTS.json")).expect("parse VERDICTS.json");
    let register = parse_register(&read(&root, "docs/baseline/mc-status.md"));
    let dir_exists = move |id: &str| board.join(id).is_dir();

    let input = LintInput {
        coverage: &coverage,
        baseline: &baseline,
        suites: &suites,
        roadmap: &roadmap,
        scenarios: &scenarios,
        index: &index,
        register: &register,
        dir_exists: &dir_exists,
    };

    let findings = lint(&input);
    assert!(
        findings.is_empty(),
        "ledger lint found {} disagreement(s):\n{}",
        findings.len(),
        findings.join("\n")
    );
}
