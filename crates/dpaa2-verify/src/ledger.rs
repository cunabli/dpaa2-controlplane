//! Ledger lint: four hand-maintained documents describe the same
//! invariant/scenario program from different angles, and they drift.
//! `models/COVERAGE.md` is the disposition ledger, the
//! `docs/baseline/dp*.md` family tables are the invariant source of
//! truth, `models/board/README.md` is the suite ledger, `docs/ROADMAP.md`
//! is the change series, and `docs/baseline/traffic-inventory.md` is the
//! canonical scenario list. When any two disagree — a candidate the
//! ledger claims board-settled while its baseline row still reads
//! `board-pending`, an owning-change citation that names the wrong
//! roadmap row, a suite with no directory — the reader is misled and the
//! honesty mechanism (design D9) is broken. This module parses each
//! document into plain rows and applies six cross-checks (R1–R6) so a
//! disagreement fails in CI rather than in review. Parsing is pure over
//! `&str`; the only filesystem touch is a `dir_exists` closure the caller
//! injects for the `models/board/<id>/` check.

/// The four dispositions the tally line counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tally {
    /// Rows dispositioned `modeled`.
    pub modeled: u32,
    /// Rows dispositioned `deferred`.
    pub deferred: u32,
    /// Rows dispositioned `board-settled`.
    pub board_settled: u32,
    /// Rows dispositioned `board-pending`.
    pub board_pending: u32,
    /// The stated total number of candidates.
    pub candidates: u32,
}

/// One row of the coverage ledger table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageRow {
    /// The invariant candidate id (`DPRC-I1`).
    pub id: String,
    /// The disposition keyword (`modeled` / `deferred` / …).
    pub disposition: String,
    /// The "Location / owning change / settling scenario" cell.
    pub location: String,
    /// The "CI rung" cell.
    pub rung: String,
    /// The "Board status" cell.
    pub board: String,
}

/// The parsed coverage ledger: its tally line and its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    /// The tally line's stated counts.
    pub tally: Tally,
    /// One entry per candidate row.
    pub rows: Vec<CoverageRow>,
}

/// Everything [`lint`] cross-checks, already parsed from its document.
pub struct LintInput<'a> {
    /// The parsed `models/COVERAGE.md`.
    pub coverage: &'a Coverage,
    /// `(id, status)` from every `docs/baseline/dp*.md` invariant table.
    pub baseline: &'a [(String, String)],
    /// `(suite id, status cell)` from the `models/board/README.md` ledger.
    pub suites: &'a [(String, String)],
    /// `(row number, change name)` from the roadmap series table.
    pub roadmap: &'a [(u32, String)],
    /// Canonical scenario ids from `traffic-inventory.md`.
    pub scenarios: &'a [String],
    /// Reports whether `models/board/<id>/` exists.
    pub dir_exists: &'a dyn Fn(&str) -> bool,
}

/// Splits one markdown table line into trimmed cells, dropping the outer
/// pipes.
fn split_row(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_owned()).collect()
}

/// A markdown separator row is `|---|:--:|…` — every cell dashes/colons.
fn is_separator(line: &str) -> bool {
    let cells = split_row(line);
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

/// Every pipe-table in `md`, as `(header cells, data rows)`. A table is a
/// run of pipe-lines whose second line is a separator.
fn tables(md: &str) -> Vec<(Vec<String>, Vec<Vec<String>>)> {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let is_pipe = lines[i].trim_start().starts_with('|');
        if is_pipe && i + 1 < lines.len() && is_separator(lines[i + 1]) {
            let header = split_row(lines[i]);
            let mut rows = Vec::new();
            let mut j = i + 2;
            while j < lines.len() && lines[j].trim_start().starts_with('|') {
                rows.push(split_row(lines[j]));
                j += 1;
            }
            out.push((header, rows));
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Whether `s` is exactly an invariant id: `DP`, uppercase letters, `-I`,
/// digits (`DPRTC-I5`).
fn is_invariant_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("DP") else {
        return false;
    };
    let Some((fam, num)) = rest.split_once("-I") else {
        return false;
    };
    !fam.is_empty()
        && fam.chars().all(|c| c.is_ascii_uppercase())
        && !num.is_empty()
        && num.chars().all(|c| c.is_ascii_digit())
}

/// Whether `s` is exactly a scenario/suite id: `V-`, hyphen-separated
/// uppercase-or-digit segments, a final all-digit segment
/// (`V-LIFE-DPNI-1`).
fn is_scenario_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("V-") else {
        return false;
    };
    let segs: Vec<&str> = rest.split('-').collect();
    if segs.len() < 2 {
        return false;
    }
    let (last, front) = segs.split_last().expect("len >= 2");
    !last.is_empty()
        && last.chars().all(|c| c.is_ascii_digit())
        && front.iter().all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
}

/// Parses `models/COVERAGE.md` into its tally line and candidate rows.
///
/// # Errors
///
/// Returns a message if the tally line is missing or malformed, or the
/// candidate table is absent.
pub fn parse_coverage(md: &str) -> Result<Coverage, String> {
    let tally_line = md
        .lines()
        .find(|l| l.trim_start().starts_with("Tally:"))
        .ok_or("no `Tally:` line in COVERAGE.md")?;
    let nums: Vec<u32> = tally_line
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let [modeled, deferred, board_settled, board_pending, candidates] = nums[..] else {
        return Err(format!(
            "tally line does not carry exactly five counts: {tally_line:?}"
        ));
    };
    let tally = Tally {
        modeled,
        deferred,
        board_settled,
        board_pending,
        candidates,
    };

    let rows: Vec<CoverageRow> = tables(md)
        .into_iter()
        .find(|(h, _)| h.first().is_some_and(|c| c == "Candidate"))
        .ok_or("no `Candidate` table in COVERAGE.md")?
        .1
        .into_iter()
        .filter_map(|r| match &r[..] {
            [id, disposition, location, rung, board] if is_invariant_id(id) => Some(CoverageRow {
                id: id.clone(),
                disposition: disposition.clone(),
                location: location.clone(),
                rung: rung.clone(),
                board: board.clone(),
            }),
            _ => None,
        })
        .collect();
    if rows.is_empty() {
        return Err("no candidate rows parsed from COVERAGE.md".to_owned());
    }
    Ok(Coverage { tally, rows })
}

/// Parses the `| Id | Proposition | Observables | Status |` invariant
/// table of a baseline family file into `(id, status)` pairs.
#[must_use]
pub fn parse_baseline_table(md: &str) -> Vec<(String, String)> {
    tables(md)
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .filter_map(|r| match &r[..] {
            [id, .., status] if is_invariant_id(id) => Some((id.clone(), status.clone())),
            _ => None,
        })
        .collect()
}

/// Parses the `| Suite | Module | Status |` ledger into `(suite id,
/// status cell)` pairs.
#[must_use]
pub fn parse_suite_ledger(md: &str) -> Vec<(String, String)> {
    tables(md)
        .into_iter()
        .find(|(h, _)| h.first().is_some_and(|c| c == "Suite"))
        .map(|(_, rows)| rows)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| match &r[..] {
            [id, .., status] if is_scenario_id(id) => Some((id.clone(), status.clone())),
            _ => None,
        })
        .collect()
}

/// Parses the roadmap "series" table into `(row number, change name)`
/// pairs, stripping the backticks around the name.
#[must_use]
pub fn parse_roadmap(md: &str) -> Vec<(u32, String)> {
    tables(md)
        .into_iter()
        .find(|(h, _)| h.first().is_some_and(|c| c == "#"))
        .map(|(_, rows)| rows)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            let n: u32 = r.first()?.parse().ok()?;
            let name = r.get(1)?.trim_matches('`').to_owned();
            Some((n, name))
        })
        .collect()
}

/// Parses the scenario ids from the traffic-inventory tables (the rows
/// whose first cell is a scenario id).
#[must_use]
pub fn parse_scenario_ids(md: &str) -> Vec<String> {
    tables(md)
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .filter_map(|r| r.into_iter().next())
        .filter(|c| is_scenario_id(c))
        .collect()
}

/// Every scenario/suite id token in `text` (`V-DPRC-1`, `V-LIFE-DPNI-1`),
/// deduplicated. A range like `V-POOL-1..3` yields only the token present
/// (`V-POOL-1`), since the `.` ends the token.
#[must_use]
pub fn scenario_refs(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'V' && b[i + 1] == b'-' {
            let mut j = i + 1;
            while j < b.len()
                && (b[j].is_ascii_uppercase() || b[j].is_ascii_digit() || b[j] == b'-')
            {
                j += 1;
            }
            // Trim trailing hyphens so `V-POOL-` never survives.
            let mut end = j;
            while end > i && b[end - 1] == b'-' {
                end -= 1;
            }
            let tok = &text[i..end];
            if is_scenario_id(tok) && !out.iter().any(|t| t == tok) {
                out.push(tok.to_owned());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Every owning-change reference in `text`: a `` `name` (#N) `` pair as
/// `(Some(name), N)`, and every other `#N` as `(None, N)`.
#[must_use]
pub fn change_refs(text: &str) -> Vec<(Option<String>, u32)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'#' {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 {
                if let Ok(num) = text[i + 1..j].parse::<u32>() {
                    let name = backtick_name_before(b, text, i);
                    out.push((name, num));
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// When the `#` at `hash` is wrapped as `` `name` (#N)``, returns `name`.
fn backtick_name_before(b: &[u8], text: &str, hash: usize) -> Option<String> {
    if hash == 0 || b[hash - 1] != b'(' {
        return None;
    }
    let paren = hash - 1;
    if paren == 0 {
        return None;
    }
    let mut q = paren - 1;
    while q > 0 && b[q] == b' ' {
        q -= 1;
    }
    if b[q] != b'`' {
        return None;
    }
    let close = q;
    let mut r = close;
    while r > 0 {
        r -= 1;
        if b[r] == b'`' {
            return Some(text[r + 1..close].to_owned());
        }
    }
    None
}

/// Truncates `s` to roughly `max` chars for a readable finding.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}

/// R1: every baseline invariant id appears in COVERAGE, and every
/// COVERAGE candidate appears in some baseline table.
fn r1_ids(input: &LintInput<'_>, out: &mut Vec<String>) {
    let cov: Vec<&str> = input.coverage.rows.iter().map(|r| r.id.as_str()).collect();
    let base: Vec<&str> = input.baseline.iter().map(|(id, _)| id.as_str()).collect();
    for (id, _) in input.baseline {
        if !cov.contains(&id.as_str()) {
            out.push(format!(
                "R1 ids: baseline invariant {id} is absent from COVERAGE.md"
            ));
        }
    }
    for row in &input.coverage.rows {
        if !base.contains(&row.id.as_str()) {
            out.push(format!(
                "R1 ids: COVERAGE candidate {} appears in no baseline table",
                row.id
            ));
        }
    }
}

/// R2: the tally counts and total equal a recount of the Disposition
/// column.
fn r2_tally(input: &LintInput<'_>, out: &mut Vec<String>) {
    let t = &input.coverage.tally;
    let mut got = Tally {
        modeled: 0,
        deferred: 0,
        board_settled: 0,
        board_pending: 0,
        candidates: 0,
    };
    for row in &input.coverage.rows {
        match row.disposition.as_str() {
            "modeled" => got.modeled += 1,
            "deferred" => got.deferred += 1,
            "board-settled" => got.board_settled += 1,
            "board-pending" => got.board_pending += 1,
            other => out.push(format!(
                "R2 tally: {} carries unknown disposition {other:?}",
                row.id
            )),
        }
    }
    got.candidates = u32::try_from(input.coverage.rows.len()).unwrap_or(u32::MAX);
    if &got != t {
        out.push(format!("R2 tally: stated {t:?} but recount is {got:?}"));
    }
}

/// Whether a cell's disposition/board marks the row board-verified.
fn is_board_verified(row: &CoverageRow) -> bool {
    row.disposition == "board-settled" || row.board.starts_with("verified")
}

/// Whether a board cell opens with `verified ` and a `YYYY-MM-DD` date —
/// the shape a suite writes when it is the evidence. An undated
/// `verified (…)` cell is a prior-work reference and carries no
/// suite-citation obligation.
fn dated_board_evidence(board: &str) -> bool {
    let Some(rest) = board.strip_prefix("verified ") else {
        return false;
    };
    let b = rest.as_bytes();
    b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
}

/// R3: scenario ids cited in a COVERAGE row must resolve to a suite (with
/// a directory) or an inventory scenario, and every suite must own its
/// directory. Rows that claim the board actually settled them — a
/// `board-settled` disposition, or a board cell opening with `verified
/// YYYY-MM-DD` (a dated suite verdict is board evidence) — must anchor
/// that claim on a cited suite with a directory. An undated `verified
/// (…)` cell is a prior-work reference (e.g. an ADR), not a suite
/// verdict, so it carries no such obligation whatever else it cites.
fn r3_suites(input: &LintInput<'_>, out: &mut Vec<String>) {
    let suite_ids: Vec<&str> = input.suites.iter().map(|(id, _)| id.as_str()).collect();
    let is_suite = |id: &str| suite_ids.contains(&id);
    let is_scenario = |id: &str| input.scenarios.iter().any(|s| s == id);

    for row in &input.coverage.rows {
        let cited = {
            let mut v = scenario_refs(&row.location);
            for id in scenario_refs(&row.board) {
                if !v.contains(&id) {
                    v.push(id);
                }
            }
            v
        };
        for id in &cited {
            if is_suite(id) {
                if !(input.dir_exists)(id) {
                    out.push(format!(
                        "R3 suites: {} cites suite {id} but models/board/{id}/ is missing",
                        row.id
                    ));
                }
            } else if !is_scenario(id) {
                out.push(format!(
                    "R3 suites: {} cites {id}, which is neither a suite ledger row nor an inventory scenario",
                    row.id
                ));
            }
        }
        // Rows that claim the board settled them (board-settled, or a
        // dated `verified YYYY-MM-DD` verdict) must anchor on a cited
        // suite with a directory. Undated `verified (…)` is prior work
        // and exempt.
        let obligated = row.disposition == "board-settled" || dated_board_evidence(&row.board);
        if obligated {
            let anchored = cited
                .iter()
                .any(|id| is_suite(id) && (input.dir_exists)(id));
            if !anchored {
                out.push(format!(
                    "R3 suites: {} claims board evidence (disposition {:?}) but cites no suite with a directory (board: {:?})",
                    row.id,
                    row.disposition,
                    clip(&row.board, 80)
                ));
            }
        }
    }

    for (id, _) in input.suites {
        if !(input.dir_exists)(id) {
            out.push(format!(
                "R3 suites: suite ledger row {id} has no models/board/{id}/ directory"
            ));
        }
    }
}

/// R4: every `` `name` (#N)`` pair matches roadmap row N's name, and
/// every `#N` is a roadmap row number.
fn r4_changes(input: &LintInput<'_>, out: &mut Vec<String>) {
    let by_num: Vec<(u32, &str)> = input
        .roadmap
        .iter()
        .map(|(n, name)| (*n, name.as_str()))
        .collect();
    let lookup = |n: u32| by_num.iter().find(|(m, _)| *m == n).map(|(_, name)| *name);

    let mut seen: Vec<(Option<String>, u32)> = Vec::new();
    for row in &input.coverage.rows {
        for cell in [&row.location, &row.board] {
            for (name, n) in change_refs(cell) {
                let key = (name.clone(), n);
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                match lookup(n) {
                    None => out.push(format!(
                        "R4 change: {} cites #{n}, which is not a roadmap row",
                        row.id
                    )),
                    Some(actual) => {
                        if let Some(name) = name
                            && name != actual
                        {
                            out.push(format!(
                                "R4 change: {} cites `{name}` (#{n}) but roadmap row {n} is `{actual}`",
                                row.id
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// R5: a board cell that starts with `open:` must cite the owning change
/// as a `#N`.
fn r5_open(input: &LintInput<'_>, out: &mut Vec<String>) {
    for row in &input.coverage.rows {
        if row.board.starts_with("open:") && change_refs(&row.board).is_empty() {
            out.push(format!(
                "R5 open: {} has an `open:` board cell with no owning-change #N (board: {:?})",
                row.id,
                clip(&row.board, 80)
            ));
        }
    }
}

/// R6: the baseline status and the ledger's board level must agree.
fn r6_levels(input: &LintInput<'_>, out: &mut Vec<String>) {
    for row in &input.coverage.rows {
        let Some((_, b)) = input.baseline.iter().find(|(id, _)| *id == row.id) else {
            continue;
        };
        let b = b.trim();
        let verified = is_board_verified(row);
        let note = |out: &mut Vec<String>, why: &str| {
            out.push(format!(
                "R6 level ({why}): {} — baseline {:?} vs ledger disposition {:?} board {:?}",
                row.id,
                clip(b, 80),
                row.disposition,
                clip(&row.board, 80)
            ));
        };
        if b.starts_with("board-pending")
            && (row.disposition == "board-settled" || row.board.starts_with("verified"))
        {
            note(out, "a: board-pending baseline, board-verified ledger");
        }
        if b.starts_with("verified")
            && !(row.disposition == "board-settled" || row.board.contains("verified"))
        {
            note(out, "b: verified baseline, ledger carries no verification");
        }
        if verified && !b.starts_with("verified") {
            note(out, "c: board-verified ledger, baseline not verified");
        }
    }
}

/// Runs every rule and returns one human-readable finding per violation;
/// an empty vector is the green verdict.
#[must_use]
pub fn lint(input: &LintInput<'_>) -> Vec<String> {
    let mut out = Vec::new();
    r1_ids(input, &mut out);
    r2_tally(input, &mut out);
    r3_suites(input, &mut out);
    r4_changes(input, &mut out);
    r5_open(input, &mut out);
    r6_levels(input, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cov_row(id: &str, disp: &str, loc: &str, board: &str) -> CoverageRow {
        CoverageRow {
            id: id.to_owned(),
            disposition: disp.to_owned(),
            location: loc.to_owned(),
            rung: "—".to_owned(),
            board: board.to_owned(),
        }
    }

    fn base_input<'a>(
        coverage: &'a Coverage,
        baseline: &'a [(String, String)],
        suites: &'a [(String, String)],
        roadmap: &'a [(u32, String)],
        scenarios: &'a [String],
        dir_exists: &'a dyn Fn(&str) -> bool,
    ) -> LintInput<'a> {
        LintInput {
            coverage,
            baseline,
            suites,
            roadmap,
            scenarios,
            dir_exists,
        }
    }

    #[test]
    fn parses_a_minimal_coverage_ledger() {
        let md = "\
Tally: 1 modeled, 1 deferred, 0 board-settled, 0 board-pending — 2 candidates.

| Candidate | Disposition | Location / owning change / settling scenario | CI rung | Board status |
|-----------|-------------|--------|---------|--------------|
| DPRC-I1 | modeled | `core` | apalache | — |
| DPRC-I2 | deferred | `pool-objects` (#6) | — | open: V-POOL-1 → #6 |
";
        let cov = parse_coverage(md).expect("parse");
        assert_eq!(cov.tally.candidates, 2);
        assert_eq!(cov.rows.len(), 2);
        assert_eq!(cov.rows[1].board, "open: V-POOL-1 → #6");
    }

    #[test]
    fn scenario_refs_finds_multi_segment_and_range() {
        assert_eq!(scenario_refs("V-DPRC-1's rev"), vec!["V-DPRC-1"]);
        assert_eq!(scenario_refs("bind (V-LIFE-DPNI-1)"), vec!["V-LIFE-DPNI-1"]);
        assert_eq!(scenario_refs("V-POOL-1..3 exhaustion"), vec!["V-POOL-1"]);
        assert!(scenario_refs("no ids here").is_empty());
    }

    #[test]
    fn change_refs_attaches_the_backtick_name() {
        assert_eq!(
            change_refs("owned by `pool-objects` (#6)"),
            vec![(Some("pool-objects".to_owned()), 6)]
        );
        assert_eq!(change_refs("→ #4 only"), vec![(None, 4)]);
    }

    #[test]
    fn is_scenario_id_shapes() {
        assert!(is_scenario_id("V-DPRC-1"));
        assert!(is_scenario_id("V-LIFE-DPNI-1"));
        assert!(!is_scenario_id("V-POOL"));
        assert!(!is_scenario_id("DPRC-I1"));
    }

    // --- one violating + one passing snippet per rule ---

    #[test]
    fn r1_flags_and_passes() {
        let cov = Coverage {
            tally: Tally {
                modeled: 1,
                deferred: 0,
                board_settled: 0,
                board_pending: 0,
                candidates: 1,
            },
            rows: vec![cov_row("DPRC-I1", "modeled", "`core`", "—")],
        };
        let dir = |_: &str| true;
        // Baseline carries an id COVERAGE lacks, and vice versa.
        let baseline = vec![("DPRC-I2".to_owned(), "candidate".to_owned())];
        let mut out = Vec::new();
        r1_ids(&base_input(&cov, &baseline, &[], &[], &[], &dir), &mut out);
        assert_eq!(out.len(), 2, "{out:?}");

        let baseline_ok = vec![("DPRC-I1".to_owned(), "candidate".to_owned())];
        let mut ok = Vec::new();
        r1_ids(
            &base_input(&cov, &baseline_ok, &[], &[], &[], &dir),
            &mut ok,
        );
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn r2_flags_and_passes() {
        let rows = vec![cov_row("DPRC-I1", "modeled", "`core`", "—")];
        let bad = Coverage {
            tally: Tally {
                modeled: 2,
                deferred: 0,
                board_settled: 0,
                board_pending: 0,
                candidates: 1,
            },
            rows: rows.clone(),
        };
        let dir = |_: &str| true;
        let mut out = Vec::new();
        r2_tally(&base_input(&bad, &[], &[], &[], &[], &dir), &mut out);
        assert_eq!(out.len(), 1, "{out:?}");

        let good = Coverage {
            tally: Tally {
                modeled: 1,
                deferred: 0,
                board_settled: 0,
                board_pending: 0,
                candidates: 1,
            },
            rows,
        };
        let mut ok = Vec::new();
        r2_tally(&base_input(&good, &[], &[], &[], &[], &dir), &mut ok);
        assert!(ok.is_empty(), "{ok:?}");
    }

    fn r3_one(row: CoverageRow, suites: &[(String, String)], scenarios: &[String]) -> Vec<String> {
        let cov = Coverage {
            tally: Tally {
                modeled: 0,
                deferred: 0,
                board_settled: 0,
                board_pending: 0,
                candidates: 1,
            },
            rows: vec![row],
        };
        let dir = |_: &str| true;
        let mut out = Vec::new();
        r3_suites(
            &base_input(&cov, &[], suites, &[], scenarios, &dir),
            &mut out,
        );
        out
    }

    #[test]
    fn r3_dated_verified_needs_a_suite_with_a_directory() {
        // Dated verdict citing an inventory-only id (no suite, no dir):
        // the citation resolves, but the board claim is unanchored.
        let scenarios = vec!["V-X-1".to_owned()];
        let out = r3_one(
            cov_row(
                "DPX-I1",
                "modeled",
                "`main`",
                "verified 2026-08-23 (V-X-1 …)",
            ),
            &[],
            &scenarios,
        );
        assert_eq!(out.len(), 1, "{out:?}");
    }

    #[test]
    fn r3_undated_verified_prior_work_passes() {
        // Undated `verified (…)` naming an inventory-only deferred face:
        // prior work, no suite obligation.
        let scenarios = vec!["V-Y-2".to_owned()];
        let out = r3_one(
            cov_row(
                "DPX-I2",
                "modeled",
                "`main`",
                "verified (ADR-0001 §3); face V-Y-2 → `dpmac-typestate` (#7)",
            ),
            &[],
            &scenarios,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn r3_board_settled_with_inventory_only_fails() {
        let scenarios = vec!["V-Y-1".to_owned()];
        let out = r3_one(
            cov_row("DPX-I3", "board-settled", "settled by V-Y-1", "—"),
            &[],
            &scenarios,
        );
        assert_eq!(out.len(), 1, "{out:?}");
    }

    #[test]
    fn r3_suite_with_directory_anchors_a_board_settled_row() {
        let suites = vec![("V-DPRC-1".to_owned(), "passed".to_owned())];
        let out = r3_one(
            cov_row(
                "DPX-I4",
                "board-settled",
                "V-DPRC-1",
                "verified 2026-08-23 (V-DPRC-1)",
            ),
            &suites,
            &[],
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn r4_flags_wrong_name_and_passes() {
        let cov = Coverage {
            tally: Tally {
                modeled: 0,
                deferred: 1,
                board_settled: 0,
                board_pending: 0,
                candidates: 1,
            },
            rows: vec![cov_row("DPRC-I8", "deferred", "`wrong-name` (#6)", "—")],
        };
        let roadmap = vec![(6, "pool-objects".to_owned())];
        let dir = |_: &str| true;
        let mut out = Vec::new();
        r4_changes(&base_input(&cov, &[], &[], &roadmap, &[], &dir), &mut out);
        assert_eq!(out.len(), 1, "{out:?}");

        let cov_ok = Coverage {
            rows: vec![cov_row("DPRC-I8", "deferred", "`pool-objects` (#6)", "—")],
            ..cov
        };
        let mut ok = Vec::new();
        r4_changes(&base_input(&cov_ok, &[], &[], &roadmap, &[], &dir), &mut ok);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn r5_flags_open_without_change_and_passes() {
        let cov = Coverage {
            tally: Tally {
                modeled: 1,
                deferred: 0,
                board_settled: 0,
                board_pending: 0,
                candidates: 1,
            },
            rows: vec![cov_row(
                "DPRC-I3",
                "modeled",
                "`main`",
                "open: V-DPRC-5 someday",
            )],
        };
        let dir = |_: &str| true;
        let mut out = Vec::new();
        r5_open(&base_input(&cov, &[], &[], &[], &[], &dir), &mut out);
        assert_eq!(out.len(), 1, "{out:?}");

        let cov_ok = Coverage {
            rows: vec![cov_row(
                "DPRC-I3",
                "modeled",
                "`main`",
                "open: V-DPRC-5 → #4",
            )],
            ..cov
        };
        let mut ok = Vec::new();
        r5_open(&base_input(&cov_ok, &[], &[], &[], &[], &dir), &mut ok);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn r6_flags_level_mismatch_and_passes() {
        // Ledger says board-settled; baseline still board-pending → (a) and (c).
        let cov = Coverage {
            tally: Tally {
                modeled: 0,
                deferred: 0,
                board_settled: 1,
                board_pending: 0,
                candidates: 1,
            },
            rows: vec![cov_row(
                "DPRC-I9",
                "board-settled",
                "V-DPRC-1",
                "verified 2026",
            )],
        };
        let baseline = vec![("DPRC-I9".to_owned(), "board-pending (unknown 1)".to_owned())];
        let dir = |_: &str| true;
        let mut out = Vec::new();
        r6_levels(&base_input(&cov, &baseline, &[], &[], &[], &dir), &mut out);
        assert!(!out.is_empty(), "{out:?}");

        let baseline_ok = vec![("DPRC-I9".to_owned(), "verified 2026".to_owned())];
        let mut ok = Vec::new();
        r6_levels(
            &base_input(&cov, &baseline_ok, &[], &[], &[], &dir),
            &mut ok,
        );
        assert!(ok.is_empty(), "{ok:?}");
    }
}
