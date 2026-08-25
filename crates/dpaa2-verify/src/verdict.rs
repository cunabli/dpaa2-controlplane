//! Machine-readable verdicts (openspec task 6.2).
//!
//! A run — a batch suite's result directory, or an online transcript —
//! produces a human report on stdout and, from the same inputs, a
//! `Verdict` value: a self-describing JSON fact about how the run went
//! against the pinned reference pair. Verdicts roll up into a per-suite
//! `Index` map so a reader can see, at a glance, which suite revisions
//! pass on the reference environment and which have open findings.
//!
//! Everything here is pure over its inputs; the only filesystem contact
//! is through the `read`/`list` closures the caller injects, exactly like
//! [`crate::ledger`] and [`crate::snapshot`].

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::adapter::Observed;
use crate::driver::{ProbeRecord, StepRecord};
use crate::generate::{StepReport, SuitePlan};

/// The pinned reference pair every verdict is a fact about
/// (`docs/baseline/reference-environment.md`).
pub const REFERENCE_PAIR: ReferencePair = ReferencePair {
    mc_firmware: Cow::Borrowed("10.39.0"),
    kernel: Cow::Borrowed("6.6.52"),
    restool: Cow::Borrowed("v2.4"),
};

/// The firmware / kernel / restool triple a verdict is asserted against.
///
/// Fields are `Cow<'static, str>` so [`REFERENCE_PAIR`] stays a `const`
/// while the struct still derives `Deserialize` (a bare `&'static str`
/// field cannot).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferencePair {
    /// Management-Complex firmware version.
    pub mc_firmware: Cow<'static, str>,
    /// Kernel version.
    pub kernel: Cow<'static, str>,
    /// restool version.
    pub restool: Cow<'static, str>,
}

/// Which kind of run a verdict summarizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A generated batch suite's result directory, diffed against its plan.
    Batch,
    /// A hand-authored probe transcript.
    Probes,
    /// A model-trace drive transcript.
    Trace,
}

/// One step's outcome, uniform across batch, probe, and trace runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepOutcome {
    /// Step index in its run.
    pub index: usize,
    /// Step title (batch/trace) or probe label.
    pub title: String,
    /// Conformance: `None` when there was nothing to observe or the step
    /// was skipped, else whether every observation matched.
    pub conform: Option<bool>,
    /// Exit codes recorded for the step: every line of `step-N-exit.txt`
    /// (batch) or the single `exit_code` (transcripts), when present.
    pub exit_codes: Vec<i32>,
    /// What the read-back observed, when the step observed anything.
    pub observed: Option<Observed>,
    /// Field- or verdict-level mismatch details; empty when conforming.
    pub mismatches: Vec<String>,
    /// The MC status text of a refusal, when the step recorded one.
    pub refusal: Option<String>,
    /// The MC status name the step declared it should be refused with, if
    /// any (a batch plan's `refusal`, a probe step's `refusal`).
    #[serde(default)]
    pub expected_refusal: Option<String>,
    /// The operator skipped the step (probe runs only).
    pub skipped: bool,
}

/// One `PASS `/`FAIL ` line emitted by a suite's hook script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookLine {
    /// Whether the line begins `PASS ` (rather than `FAIL `).
    pub pass: bool,
    /// The line verbatim.
    pub line: String,
}

/// The full, self-describing verdict for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// Suite id the run belongs to.
    pub suite: String,
    /// Suite revision (trailing `-rev<N>`, else 1).
    pub revision: u32,
    /// Which kind of run this was.
    pub kind: Kind,
    /// The plan's `trace_file` (batch) or the transcript file name.
    pub source: String,
    /// `fnv1a64:<16 hex>` of the plan text (batch) or the transcript text.
    pub source_hash: String,
    /// The reference pair this verdict is a fact about.
    pub reference: ReferencePair,
    /// The run's date, `YYYY-MM-DD`.
    pub date: String,
    /// Whether the run passed: every judged step conforms and no hook
    /// `FAIL ` line.
    pub pass: bool,
    /// Steps with a conformance verdict (`conform == Some(_)`).
    pub judged: usize,
    /// Steps that conformed (`conform == Some(true)`).
    pub passed: usize,
    /// Every step's outcome, in order.
    pub steps: Vec<StepOutcome>,
    /// `(model id, board name)` pairs the run created.
    pub created: Vec<(String, String)>,
    /// The hook script's `PASS `/`FAIL ` lines (batch only).
    pub hook: Vec<HookLine>,
}

/// The compact per-run row the [`Index`] keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// Whether the run passed.
    pub pass: bool,
    /// The run's date.
    pub date: String,
    /// Which kind of run.
    pub kind: Kind,
    /// The suite revision, so a reader can still pick "rev 3".
    pub revision: u32,
    /// Conforming steps over judged steps, e.g. `13/13`.
    pub steps: String,
    /// The source hash.
    pub source_hash: String,
    /// Passing hook lines over total, e.g. `6/8`; `None` when the run had
    /// no hook lines (so a step-clean run with hook `FAIL`s is visible).
    pub hook: Option<String>,
    /// The evidence archive path, when one was recorded.
    pub archive: Option<String>,
    /// The MC status names any step was refused with (the observed
    /// refusal text, expected or not), sorted and deduplicated. Empty for
    /// a run that hit no refusal. `#[serde(default)]` so an index written
    /// before this field parses.
    #[serde(default)]
    pub refusals: Vec<String>,
}

/// Suite id → run label (e.g. `V-READBACK-1`, `V-DPDBG-1/probes-rev2`) →
/// [`Summary`]. `BTreeMap` at both levels so the file is deterministic.
pub type Index = BTreeMap<String, BTreeMap<String, Summary>>;

/// FNV-1a 64-bit over the UTF-8 bytes, rendered `fnv1a64:<16 hex>`.
#[must_use]
pub fn fnv1a64(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// The first `MC error: <name> (status 0x<hex>)` in `text`, returned as
/// `<name> (status 0x<hex>)`; `None` when absent.
#[must_use]
pub fn mc_status(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some((_, after)) = line.split_once("MC error: ") else {
            continue;
        };
        let Some(status) = after.find("(status 0x") else {
            continue;
        };
        if let Some(close) = after[status..].find(')') {
            return Some(after[..=status + close].trim().to_owned());
        }
    }
    None
}

/// The revision a dir or file stem names: trailing `-rev<N>` → N, else 1.
#[must_use]
pub fn revision_of(name: &str) -> u32 {
    name.rsplit_once("-rev")
        .and_then(|(_, n)| n.parse::<u32>().ok())
        .unwrap_or(1)
}

/// `YYYY-MM-DD` (UTC) for a Unix timestamp, by Howard Hinnant's
/// days-to-civil algorithm.
#[must_use]
pub fn civil_date(secs_since_epoch: u64) -> String {
    let days = i64::try_from(secs_since_epoch / 86_400).unwrap_or(i64::MAX);
    // Shift the epoch to 0000-03-01 so leap days fall at era boundaries.
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153; // [0, 11], March-based
    let day = day_of_year - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

/// The `PASS `/`FAIL ` lines of a hook script's output.
#[must_use]
pub fn hook_lines(text: &str) -> Vec<HookLine> {
    text.lines()
        .filter_map(|l| {
            if l.starts_with("PASS ") {
                Some(HookLine {
                    pass: true,
                    line: l.to_owned(),
                })
            } else if l.starts_with("FAIL ") {
                Some(HookLine {
                    pass: false,
                    line: l.to_owned(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Whether a results-directory file name is one the hook scan skips
/// (step outputs, bookkeeping, and JSON transcripts).
fn is_hook_scannable(name: &str) -> bool {
    let json = std::path::Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json") || e.eq_ignore_ascii_case("jsonl"));
    !name.starts_with("step-")
        && !matches!(name, "created.txt" | "teardown.log" | "dmesg.txt")
        && !json
}

/// Whether `s` is a model id in either spelling: `<fam>_<n>` — the
/// underscore form scripts record (`dpni_0`) — or `<fam>.<n>`, the dotted
/// form the older recovery-suite emitter wrote to `created.txt`
/// (`dprc.2`). Both are on disk, so `parse_created` accepts either and
/// returns the id exactly as written; [`crate::generate::diff`]'s
/// `replacen('_', ".", 1)` then normalises the separator before binding.
fn is_model_id(s: &str) -> bool {
    is_fam_sep(s, '_') || is_fam_sep(s, '.')
}

/// Whether `s` is `<fam>.<n>` — ascii letters, `.`, ascii digits — a board
/// object name (`dpni.1`).
fn is_board_name(s: &str) -> bool {
    is_fam_sep(s, '.')
}

/// Whether `s` splits once on `sep` into a non-empty all-letters family
/// and a non-empty all-digits number.
fn is_fam_sep(s: &str, sep: char) -> bool {
    let Some((fam, num)) = s.split_once(sep) else {
        return false;
    };
    !fam.is_empty()
        && fam.chars().all(|c| c.is_ascii_alphabetic())
        && !num.is_empty()
        && num.chars().all(|c| c.is_ascii_digit())
}

/// The `(model id, board name)` bindings a run recorded in `created.txt`,
/// one per line.
///
/// A refused create leaves its refusal text in the file — an empty name
/// (`dpdcei_0 ` with nothing after it), or restool's whole `Usage:` dump
/// and its option lines — so a line that is not a binding is evidence for
/// the read-back, not a parse error. Only a line whose first
/// whitespace-separated token has the model-id shape `<fam>_<n>` and whose
/// single remaining token has the board-name shape `<fam>.<n>` is kept;
/// every other line is skipped.
#[must_use]
pub fn parse_created(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let (model, board) = (it.next()?, it.next()?);
            if it.next().is_some() {
                return None; // more than a bare (model, board) pair
            }
            (is_model_id(model) && is_board_name(board))
                .then(|| (model.to_owned(), board.to_owned()))
        })
        .collect()
}

/// Builds a verdict from a batch suite's plan and its result files.
///
/// `read` maps a result file name to its content; `list` returns every
/// file name in the results directory. A `created.txt` line that is not a
/// binding (a refused create's refusal text) is skipped, not an error
/// ([`parse_created`]).
pub fn from_batch(
    plan: &SuitePlan,
    plan_text: &str,
    reports: &[StepReport],
    read: impl Fn(&str) -> Option<String>,
    list: impl Fn() -> Vec<String>,
    revision: u32,
    date: String,
) -> Verdict {
    let steps: Vec<StepOutcome> = reports
        .iter()
        .map(|r| {
            let exit_codes = read(&format!("step-{}-exit.txt", r.index))
                .map(|t| {
                    t.lines()
                        .filter_map(|l| l.trim().parse::<i32>().ok())
                        .collect()
                })
                .unwrap_or_default();
            let refusal = read(&format!("step-{}-err.txt", r.index))
                .as_deref()
                .and_then(mc_status);
            let expected_refusal = plan
                .steps
                .iter()
                .find(|s| s.index == r.index)
                .and_then(|s| s.refusal.clone());
            StepOutcome {
                index: r.index,
                title: r.title.clone(),
                conform: r.verdict.as_ref().map(|v| v.pass),
                exit_codes,
                observed: r.observed.clone(),
                mismatches: r
                    .verdict
                    .as_ref()
                    .map_or_else(Vec::new, |v| v.mismatches.clone()),
                refusal,
                expected_refusal,
                skipped: false,
            }
        })
        .collect();

    let created = read("created.txt")
        .map(|t| parse_created(&t))
        .unwrap_or_default();

    let mut files = list();
    files.sort();
    let hook: Vec<HookLine> = files
        .iter()
        .filter(|n| is_hook_scannable(n))
        .filter_map(|n| read(n))
        .flat_map(|t| hook_lines(&t))
        .collect();

    let judged = steps.iter().filter(|s| s.conform.is_some()).count();
    let passed = steps.iter().filter(|s| s.conform == Some(true)).count();
    let pass = steps.iter().all(|s| s.conform != Some(false)) && hook.iter().all(|h| h.pass);

    Verdict {
        suite: plan.id.clone(),
        revision,
        kind: Kind::Batch,
        source: plan.trace_file.clone(),
        source_hash: fnv1a64(plan_text),
        reference: REFERENCE_PAIR,
        date,
        pass,
        judged,
        passed,
        steps,
        created,
        hook,
    }
}

/// One probe record's outcome.
fn probe_outcome(pr: &ProbeRecord) -> StepOutcome {
    let has_verdict =
        pr.exit_verdict.is_some() || pr.readback_verdict.is_some() || pr.refusal_verdict.is_some();
    let conform = if pr.skipped || !has_verdict {
        None
    } else {
        Some(
            pr.exit_verdict.as_ref().is_none_or(|v| v.pass)
                && pr.readback_verdict.as_ref().is_none_or(|v| v.pass)
                && pr.refusal_verdict.as_ref().is_none_or(|v| v.pass),
        )
    };
    let mismatches = [
        pr.exit_verdict.as_ref(),
        pr.readback_verdict.as_ref(),
        pr.refusal_verdict.as_ref(),
    ]
    .into_iter()
    .flatten()
    .filter(|v| !v.pass)
    .map(|v| v.detail.clone())
    .collect();
    StepOutcome {
        index: pr.index,
        title: pr.label.clone(),
        conform,
        exit_codes: pr.exit_code.into_iter().collect(),
        observed: pr.observed.clone(),
        mismatches,
        refusal: pr.output.as_deref().and_then(mc_status),
        expected_refusal: pr.refusal.clone(),
        skipped: pr.skipped,
    }
}

/// One trace record's outcome.
fn trace_outcome(sr: &StepRecord) -> StepOutcome {
    StepOutcome {
        index: sr.index,
        title: sr.title.clone(),
        conform: sr.verdict.as_ref().map(|v| v.pass),
        exit_codes: Vec::new(),
        observed: sr.observed.clone(),
        mismatches: sr
            .verdict
            .as_ref()
            .map_or_else(Vec::new, |v| v.mismatches.clone()),
        refusal: mc_status(&sr.stderr),
        expected_refusal: None,
        skipped: false,
    }
}

/// Builds a verdict from an online transcript (probe and/or trace lines).
///
/// A line carrying `"kind":"probe"` is a [`ProbeRecord`], otherwise a
/// [`StepRecord`]; mixed files are allowed and the kind is `Probes` when
/// any probe line is present. The suite is `suite_override`, else the
/// suite of the first probe line.
///
/// # Errors
///
/// Fails on a malformed JSON line, or when no suite can be determined.
pub fn from_transcript(
    suite_override: Option<&str>,
    name: &str,
    text: &str,
    revision: u32,
    date: String,
) -> Result<Verdict, String> {
    let mut steps = Vec::new();
    let mut created = Vec::new();
    let mut any_probe = false;
    let mut first_probe_suite = None;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if value.get("kind").and_then(serde_json::Value::as_str) == Some("probe") {
            any_probe = true;
            let pr: ProbeRecord = serde_json::from_value(value).map_err(|e| e.to_string())?;
            if first_probe_suite.is_none() {
                first_probe_suite = Some(pr.suite.clone());
            }
            steps.push(probe_outcome(&pr));
        } else {
            let sr: StepRecord = serde_json::from_value(value).map_err(|e| e.to_string())?;
            if let Some(board) = &sr.created {
                created.push((format!("step-{}", sr.index), board.clone()));
            }
            steps.push(trace_outcome(&sr));
        }
    }

    let suite = suite_override
        .map(ToOwned::to_owned)
        .or(first_probe_suite)
        .ok_or("transcript names no suite; pass --id")?;
    let judged = steps.iter().filter(|s| s.conform.is_some()).count();
    let passed = steps.iter().filter(|s| s.conform == Some(true)).count();
    let pass = steps.iter().all(|s| s.conform != Some(false));

    Ok(Verdict {
        suite,
        revision,
        kind: if any_probe { Kind::Probes } else { Kind::Trace },
        source: name.to_owned(),
        source_hash: fnv1a64(text),
        reference: REFERENCE_PAIR,
        date,
        pass,
        judged,
        passed,
        steps,
        created,
        hook: Vec::new(),
    })
}

/// The compact [`Summary`] of a verdict, tagged with an evidence archive.
#[must_use]
pub fn summary(v: &Verdict, archive: Option<String>) -> Summary {
    let hook = (!v.hook.is_empty()).then(|| {
        let passing = v.hook.iter().filter(|h| h.pass).count();
        format!("{passing}/{}", v.hook.len())
    });
    let mut refusals: Vec<String> = v
        .steps
        .iter()
        .filter_map(|s| s.refusal.as_deref())
        .map(|r| crate::driver::status_name(r).to_owned())
        .collect();
    refusals.sort();
    refusals.dedup();
    Summary {
        pass: v.pass,
        date: v.date.clone(),
        kind: v.kind.clone(),
        revision: v.revision,
        steps: format!("{}/{}", v.passed, v.judged),
        source_hash: v.source_hash.clone(),
        hook,
        archive,
        refusals,
    }
}

/// Inserts or replaces `index[suite][label]` with `v`'s summary.
pub fn upsert(index: &mut Index, suite: &str, label: &str, v: &Verdict, archive: Option<String>) {
    index
        .entry(suite.to_owned())
        .or_default()
        .insert(label.to_owned(), summary(v, archive));
}

/// Parses an index; an empty or whitespace-only string is the empty index.
///
/// # Errors
///
/// Fails on malformed JSON.
pub fn parse_index(json: &str) -> Result<Index, String> {
    if json.trim().is_empty() {
        return Ok(Index::new());
    }
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Renders an index as pretty JSON with a trailing newline.
#[must_use]
pub fn render_index(index: &Index) -> String {
    let mut s = serde_json::to_string_pretty(index).unwrap_or_default();
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{ExitEvidence, StepVerdict};

    #[test]
    fn fnv1a64_matches_known_vectors() {
        assert_eq!(fnv1a64(""), "fnv1a64:cbf29ce484222325");
        assert_eq!(fnv1a64("a"), "fnv1a64:af63dc4c8601ec8c");
    }

    #[test]
    fn mc_status_extracts_name_and_status() {
        assert_eq!(
            mc_status("restool: MC error: No privilege (status 0x4)\n").as_deref(),
            Some("No privilege (status 0x4)")
        );
        assert_eq!(mc_status("all fine here"), None);
    }

    #[test]
    fn revision_of_reads_the_trailing_rev() {
        assert_eq!(revision_of("V-DPRC-1-rev3"), 3);
        assert_eq!(revision_of("probes-rev2"), 2);
        assert_eq!(revision_of("V-DPCI-1"), 1);
        assert_eq!(revision_of("probes"), 1);
    }

    #[test]
    fn civil_date_matches_the_reference_points() {
        assert_eq!(civil_date(0), "1970-01-01");
        // 1_756_080_000 is 2025-08-25 UTC (verified against `date -u`); the
        // brief's "2026-08-25" for this epoch was off by a year.
        assert_eq!(civil_date(1_756_080_000), "2025-08-25");
    }

    #[test]
    fn hook_lines_keeps_only_pass_and_fail() {
        let text = "PASS a ok\nRECORD b\nFAIL c bad\nnoise\n";
        let lines = hook_lines(text);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].pass && lines[0].line == "PASS a ok");
        assert!(!lines[1].pass && lines[1].line == "FAIL c bad");
    }

    fn report(index: usize, pass: bool) -> StepReport {
        StepReport {
            index,
            title: format!("step {index}"),
            verdict: Some(StepVerdict {
                pass,
                mismatches: if pass {
                    vec![]
                } else {
                    vec!["boom".to_owned()]
                },
                exit: ExitEvidence { ok: true },
            }),
            observed: Some(Observed::default()),
        }
    }

    fn plan() -> SuitePlan {
        SuitePlan {
            id: "V-T-1".to_owned(),
            class: "object-lifecycle-only".to_owned(),
            flagged: false,
            trace_file: "t.itf.json".to_owned(),
            steps: vec![],
            hook: None,
            create_args: crate::adapter::CreateArgs::default(),
        }
    }

    #[test]
    fn from_batch_fails_on_a_hook_fail_and_passes_without_one() {
        let reports = [report(0, true), report(1, true)];
        let files = |extra: &str| {
            let extra = extra.to_owned();
            move || {
                vec![
                    "created.txt".to_owned(),
                    "readback.txt".to_owned(),
                    extra.clone(),
                ]
            }
        };
        let read = |with_fail: bool| {
            move |name: &str| match name {
                "created.txt" => Some("dpni_0 dpni.1\n".to_owned()),
                "readback.txt" => Some(if with_fail {
                    "PASS x ok\nFAIL y bad\n".to_owned()
                } else {
                    "PASS x ok\n".to_owned()
                }),
                "step-0-exit.txt" | "step-1-exit.txt" => Some("0\n".to_owned()),
                _ => None,
            }
        };

        let bad = from_batch(
            &plan(),
            "plan text",
            &reports,
            read(true),
            files("readback.txt"),
            1,
            "2026-08-25".to_owned(),
        );
        assert!(!bad.pass, "a hook FAIL line fails the run");
        assert_eq!(bad.judged, 2);
        assert_eq!(bad.passed, 2);
        assert_eq!(
            bad.created,
            vec![("dpni_0".to_owned(), "dpni.1".to_owned())]
        );
        assert_eq!(bad.steps[0].exit_codes, vec![0]);

        let good = from_batch(
            &plan(),
            "plan text",
            &reports,
            read(false),
            files("readback.txt"),
            1,
            "2026-08-25".to_owned(),
        );
        assert!(good.pass, "no hook FAIL, every step conforms");
        // Same plan text hashes the same regardless of results.
        assert_eq!(good.source_hash, bad.source_hash);
    }

    #[test]
    fn from_transcript_reads_probe_and_trace_lines() {
        let skipped = r#"{"kind":"probe","suite":"V-P-1","index":0,"label":"skipped one","expect":"x","cmd":["restool","x"],"instruction":null,"skipped":true,"output":null,"exit_code":null,"exit_verdict":null,"observed":null,"readback_verdict":null}"#;
        let failing = r#"{"kind":"probe","suite":"V-P-1","index":1,"label":"bad readback","expect":"x","cmd":["restool","x"],"instruction":null,"skipped":false,"output":"MC error: No privilege (status 0x4)\n","exit_code":0,"exit_verdict":{"pass":true,"detail":"expected zero exit, got exit 0"},"observed":null,"readback_verdict":{"pass":false,"detail":"expected dpni.1 present, read back absent"}}"#;
        let trace = r#"{"index":2,"title":"CreateObject","commands":["restool x"],"awaited":null,"instruction":null,"created":"dpni.1","stderr":"","expected":null,"observed":null,"verdict":{"pass":true,"mismatches":[],"exit":{"ok":true}}}"#;
        let text = format!("{skipped}\n{failing}\n{trace}\n");

        let v = from_transcript(None, "probes.jsonl", &text, 1, "2026-08-25".to_owned()).unwrap();
        assert_eq!(v.suite, "V-P-1");
        assert_eq!(v.kind, Kind::Probes);
        assert_eq!(v.steps.len(), 3);
        assert_eq!(v.steps[0].conform, None, "skipped judges nothing");
        assert_eq!(v.steps[1].conform, Some(false), "a failing readback fails");
        assert_eq!(
            v.steps[1].refusal.as_deref(),
            Some("No privilege (status 0x4)")
        );
        assert_eq!(v.steps[2].conform, Some(true));
        assert_eq!(v.created, vec![("step-2".to_owned(), "dpni.1".to_owned())]);
        assert!(!v.pass);
        assert_eq!(v.judged, 2);
        assert_eq!(v.passed, 1);
    }

    #[test]
    fn from_transcript_needs_a_suite_for_trace_only_lines() {
        let trace = r#"{"index":0,"title":"t","commands":[],"awaited":null,"instruction":null,"created":null,"stderr":"","expected":null,"observed":null,"verdict":null}"#;
        assert!(from_transcript(None, "t.jsonl", trace, 1, "d".to_owned()).is_err());
        let v = from_transcript(Some("V-X-1"), "t.jsonl", trace, 1, "d".to_owned()).unwrap();
        assert_eq!(v.suite, "V-X-1");
        assert_eq!(v.kind, Kind::Trace);
    }

    #[test]
    fn index_round_trips_and_upsert_replaces_by_label() {
        let mut index = Index::new();
        let reports = [report(0, true)];
        let v = from_batch(
            &plan(),
            "p",
            &reports,
            |_| None,
            Vec::new,
            2,
            "2026-08-25".to_owned(),
        );
        upsert(
            &mut index,
            &v.suite,
            "V-T-1-rev2",
            &v,
            Some("a.tar".to_owned()),
        );
        let rendered = render_index(&index);
        assert!(rendered.ends_with('\n'));
        let back = parse_index(&rendered).unwrap();
        assert_eq!(back, index);
        assert_eq!(back["V-T-1"]["V-T-1-rev2"].revision, 2);
        assert_eq!(back["V-T-1"]["V-T-1-rev2"].steps, "1/1");
        // Empty text is the empty index.
        assert!(parse_index("  ").unwrap().is_empty());
    }

    #[test]
    fn parse_created_keeps_bindings_and_skips_refusal_text() {
        // A good line, an empty-name refusal, and restool's Usage dump
        // (header + an option line) all in one file.
        let text = "\
dprc_2 dprc.2
dpdcei_0
Usage: restool dpdcei create --engine=<engine> --priority=<number> [OPTIONS]
--engine=<engine>
   compression or decompression engine to be selected.
dpni_0 dpni.1
dpbp.0 dpbp.1
";
        // The dotted `dpbp.0` is the older recovery emitter's model-id
        // spelling and must bind too, returned exactly as written.
        assert_eq!(
            parse_created(text),
            vec![
                ("dprc_2".to_owned(), "dprc.2".to_owned()),
                ("dpni_0".to_owned(), "dpni.1".to_owned()),
                ("dpbp.0".to_owned(), "dpbp.1".to_owned()),
            ]
        );
    }
}
