//! Offline pre-sitting pin for kernel-facing literals in the board suites.
//! Every driver basename, kernel symbol and `/sys/bus/fsl-mc` path shape a
//! suite authors must match `models/board/board-facts.json`, the record of
//! what the board was actually observed to expose. This is the preventive
//! half of the stop-after-two-kills rule (bead dpaa2-controlplane-28o;
//! plan-rollback review nomination 2): V-ROLLBACK-1 rev4e lost two sittings
//! to a writer matching `dpaa2-eth` while the board driver is `fsl_dpaa2_eth`.
//!
//! Accepted ceiling: underscore-named literals (`fsl_*`) are open-classed,
//! but a hyphen-named literal is scanned only once its exact word is a
//! record key, so a NEW hyphen-named driver not yet recorded escapes the
//! token rule; recording it is its entry into the scan set. Comment-only
//! lines skip the token and pairing rules. The sysfs rule still bounds
//! path shapes on every line.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const RECORD: &str = "models/board/board-facts.json";
const SYSFS_ROOT: &str = "/sys/bus/fsl-mc";

#[derive(Deserialize)]
struct Fallback {
    partner: String,
}

#[derive(Deserialize)]
struct BoardFacts {
    driver_basenames: BTreeMap<String, String>,
    tolerated_fallbacks: BTreeMap<String, Fallback>,
    kernel_symbols: BTreeMap<String, String>,
    sysfs_prefixes: Vec<String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(format!("{}/../..", env!("CARGO_MANIFEST_DIR")))
}

fn load_facts() -> BoardFacts {
    let text = std::fs::read_to_string(repo_root().join(RECORD)).expect("read board-facts.json");
    serde_json::from_str(&text).expect("parse board-facts.json")
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The longest end past `body_start` that consumes only `class` bytes and
/// lands on a word boundary, mirroring a greedy `\b...+\b` regex match.
fn token_end(line: &[u8], body_start: usize, class: fn(u8) -> bool) -> Option<usize> {
    let mut end = body_start;
    while end < line.len() && class(line[end]) {
        end += 1;
    }
    while end > body_start {
        let next_word = line.get(end).is_some_and(|&b| is_word(b));
        if is_word(line[end - 1]) != next_word {
            return Some(end);
        }
        end -= 1;
    }
    None
}

/// Every kernel-facing token on a line: matches of the open class
/// `\bfsl_[a-z0-9_]+\b`, plus exact-word matches of each hyphen-named
/// record key, where a word also spans hyphens (so `dpaa2-eth-bound` and
/// `vfio-census` are other words).
fn kernel_tokens<'a>(facts: &BoardFacts, line: &'a str) -> Vec<&'a str> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let at_boundary = i == 0 || !is_word(bytes[i - 1]);
        let end = (at_boundary && bytes[i..].starts_with(b"fsl_"))
            .then(|| {
                token_end(bytes, i + 4, |b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'
                })
            })
            .flatten();
        match end {
            Some(end) => {
                out.push(&line[i..end]);
                i = end;
            }
            None => i += 1,
        }
    }
    let hyphen_word = |b: &u8| is_word(*b) || *b == b'-';
    let hyphen_keys = facts
        .driver_basenames
        .keys()
        .chain(facts.tolerated_fallbacks.keys())
        .filter(|k| k.contains('-'));
    for key in hyphen_keys {
        for (start, word) in line.match_indices(key.as_str()) {
            let end = start + word.len();
            let before = start.checked_sub(1).and_then(|j| bytes.get(j));
            if !before.is_some_and(hyphen_word) && !bytes.get(end).is_some_and(hyphen_word) {
                out.push(word);
            }
        }
    }
    out
}

/// Every `/sys/bus/fsl-mc...` path literal, extended over path-ish bytes.
fn sysfs_paths(line: &str) -> Vec<&str> {
    line.match_indices(SYSFS_ROOT)
        .map(|(start, _)| {
            let len = line[start..]
                .bytes()
                .take_while(|&b| b.is_ascii_alphanumeric() || b"_./$-".contains(&b))
                .count();
            &line[start..start + len]
        })
        .collect()
}

/// The pin's verdict on one suite line: one operator-facing message per violation.
fn check_line(facts: &BoardFacts, file: &str, line_no: usize, line: &str) -> Vec<String> {
    let mut violations = Vec::new();
    // Comment-only lines carry prose, not match patterns; the sysfs rule still reads them.
    let tokens = if line.trim_start().starts_with('#') {
        Vec::new()
    } else {
        kernel_tokens(facts, line)
    };
    for token in &tokens {
        if let Some(fallback) = facts.tolerated_fallbacks.get(*token) {
            if !tokens.contains(&fallback.partner.as_str()) {
                violations.push(format!(
                    "{file}:{line_no}: fallback-pairing rule: `{token}` appears without its observed \
                     partner `{}` (the V-ROLLBACK-1 rev4e shape); match `{}` too, see \
                     {RECORD} tolerated_fallbacks.{token}",
                    fallback.partner, fallback.partner
                ));
            }
        } else if !facts.driver_basenames.contains_key(*token)
            && !facts.kernel_symbols.contains_key(*token)
        {
            violations.push(format!(
                "{file}:{line_no}: token rule: `{token}` is not a board-observed literal; record it \
                 with its provenance under {RECORD} driver_basenames or kernel_symbols, or fix the spelling"
            ));
        }
    }
    for path in sysfs_paths(line) {
        if !facts
            .sysfs_prefixes
            .iter()
            .any(|p| path.starts_with(p.as_str()))
        {
            violations.push(format!(
                "{file}:{line_no}: sysfs-shape rule: `{path}` matches no recorded path shape; record \
                 the observed shape under {RECORD} sysfs_prefixes, or fix the path"
            ));
        }
    }
    violations
}

fn suite_scripts(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("board dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            suite_scripts(&path, found);
        } else if path.extension().and_then(|e| e.to_str()) == Some("sh") {
            found.push(path);
        }
    }
}

#[test]
fn every_suite_literal_matches_the_board_facts_record() {
    let facts = load_facts();
    let root = repo_root();
    let mut scripts = Vec::new();
    suite_scripts(&root.join("models/board"), &mut scripts);
    scripts.sort();
    // Guards against an accidentally-empty walk passing vacuously.
    assert!(
        scripts.len() >= 90,
        "expected the committed suite scripts, found {}",
        scripts.len()
    );
    let mut violations = Vec::new();
    for path in &scripts {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        let text = std::fs::read_to_string(path).expect("read suite script");
        for (n, line) in text.lines().enumerate() {
            violations.extend(check_line(&facts, &rel, n + 1, line));
        }
    }
    assert!(
        violations.is_empty(),
        "board-facts pin violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn rev4e_unpaired_fallback_violates_the_pairing_rule() {
    let v = check_line(
        &load_facts(),
        "seed.sh",
        1,
        r#"case "${drv##*/}" in dpaa2-eth) c=$((c+1));; esac"#,
    );
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(
        v[0].contains("fallback-pairing rule") && v[0].contains("`dpaa2-eth`"),
        "{v:?}"
    );
}

#[test]
fn paired_fallback_line_is_clean() {
    let v = check_line(
        &load_facts(),
        "seed.sh",
        1,
        r#"case "${drv##*/}" in fsl_dpaa2_eth|dpaa2-eth) c=$((c + 1)) ;; esac"#,
    );
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn unrecorded_driver_token_violates_the_token_rule() {
    let v = check_line(
        &load_facts(),
        "seed.sh",
        1,
        "ls /sys/bus/fsl-mc/drivers/fsl_dpaa2_imagined",
    );
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(
        v[0].contains("token rule") && v[0].contains("`fsl_dpaa2_imagined`"),
        "{v:?}"
    );
}

#[test]
fn path_outside_the_recorded_shapes_violates_the_sysfs_rule() {
    let v = check_line(
        &load_facts(),
        "seed.sh",
        7,
        "cat /sys/bus/fsl-mc/driver_overrides",
    );
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(
        v[0].starts_with("seed.sh:7: sysfs-shape rule")
            && v[0].contains("`/sys/bus/fsl-mc/driver_overrides`"),
        "{v:?}"
    );
}

#[test]
fn hyphenated_log_name_on_an_executable_line_is_clean() {
    let v = check_line(
        &load_facts(),
        "seed.sh",
        1,
        r#"echo x 2>>"$RESULTS/vfio-census.log""#,
    );
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn bare_fallback_in_a_comment_line_is_clean() {
    let v = check_line(
        &load_facts(),
        "seed.sh",
        1,
        "  # confirm the dpaa2-eth bind",
    );
    assert!(v.is_empty(), "{v:?}");
}
