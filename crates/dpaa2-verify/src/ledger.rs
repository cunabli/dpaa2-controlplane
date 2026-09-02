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
    /// The committed verdict index (`models/board/VERDICTS.json`), so a
    /// cited board verdict and a ledger `**passed**` can be checked
    /// against the machine-readable fact.
    pub index: &'a crate::verdict::Index,
    /// The parsed `docs/baseline/mc-status.md` refusal register, so a
    /// register row's cited scenario and status name can be checked
    /// against the index and the MC status table (R9, R10).
    pub register: &'a [RegisterRow],
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

/// One row of the `docs/baseline/mc-status.md` refusal register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterRow {
    /// The MC status code, `None` for a `—` (non-MC refusal) cell.
    pub code: Option<u8>,
    /// The `Status` cell: a restool status name, `unknown`, or `—`.
    pub status: String,
    /// The `Evidence` cell verbatim (cites `V-<ID>[ rev N]` and/or ADRs).
    pub evidence: String,
}

/// Parses a `0x4`-style hex code cell into a `u8`; `None` for `—` or any
/// non-hex cell.
fn parse_code_cell(s: &str) -> Option<u8> {
    let hex = s
        .trim()
        .strip_prefix("0x")
        .or_else(|| s.trim().strip_prefix("0X"))?;
    u8::from_str_radix(hex, 16).ok()
}

/// Parses the refusal register — the SECOND table of
/// `docs/baseline/mc-status.md`, keyed on its header (`Code`, `Status`,
/// …, `Evidence`) so it is found by shape, not position (the first table
/// is the twelve-code reference, whose third column is `errno`).
#[must_use]
pub fn parse_register(md: &str) -> Vec<RegisterRow> {
    tables(md)
        .into_iter()
        .find(|(h, _)| h.first().is_some_and(|c| c == "Code") && h.iter().any(|c| c == "Evidence"))
        .map(|(_, rows)| rows)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            let code_cell = r.first()?;
            let status = r.get(1)?.clone();
            let evidence = r.last()?.clone();
            Some(RegisterRow {
                code: parse_code_cell(code_cell),
                status,
                evidence,
            })
        })
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

/// Parses a board cell that opens `verified YYYY-MM-DD (V-<ID>` — the
/// shape a suite writes when it is the board evidence — into `(date,
/// suite id, optional revision)`. Returns `None` for any other cell (a
/// face-qualified `... face verified`, an undated reference, prose that
/// does not lead with a suite citation), which carries no index
/// obligation.
fn parse_verdict_citation(board: &str) -> Option<(String, String, Option<u32>)> {
    if !dated_board_evidence(board) {
        return None;
    }
    let rest = board.strip_prefix("verified ")?;
    let date = rest.get(..10)?.to_owned();
    let after = rest.get(10..)?.trim_start().strip_prefix('(')?;
    let b = after.as_bytes();
    let mut j = 0;
    while j < b.len() && (b[j].is_ascii_uppercase() || b[j].is_ascii_digit() || b[j] == b'-') {
        j += 1;
    }
    while j > 0 && b[j - 1] == b'-' {
        j -= 1;
    }
    let suite = &after[..j];
    if !is_scenario_id(suite) {
        return None;
    }
    let rev = after[j..].trim_start().strip_prefix("rev ").and_then(|t| {
        let digits: String = t
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.parse::<u32>().ok()
    });
    Some((date, suite.to_owned(), rev))
}

/// R7: a `verified YYYY-MM-DD (V-<ID>[ rev N])` board cell must resolve to
/// an index entry for that suite with `pass`, the same date, and — when a
/// revision is cited — the same revision.
fn r7_verdicts(input: &LintInput<'_>, out: &mut Vec<String>) {
    for row in &input.coverage.rows {
        let Some((date, suite, rev)) = parse_verdict_citation(&row.board) else {
            continue;
        };
        let cite = match rev {
            Some(n) => format!("{suite} rev {n} @ {date}"),
            None => format!("{suite} @ {date}"),
        };
        match input.index.get(&suite) {
            None => out.push(format!(
                "R7 verdict: {} cites {cite} but the index holds no entry",
                row.id
            )),
            Some(entries) => {
                let matched = entries
                    .values()
                    .any(|s| s.pass && s.date == date && rev.is_none_or(|n| s.revision == n));
                if !matched {
                    let held = entries
                        .values()
                        .map(|s| format!("(pass={} date={} rev={})", s.pass, s.date, s.revision))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push(format!(
                        "R7 verdict: {} cites {cite} but the index holds {held}",
                        row.id
                    ));
                }
            }
        }
    }
}

/// R8: a suite-ledger `**passed**` status matches a passing index entry
/// for that suite (both ways), and every top-level index key is a
/// suite-ledger id or a `<id>-<suffix>` half of one (e.g. the
/// `V-DPRTC-3-postboot` reboot half).
fn r8_ledger(input: &LintInput<'_>, out: &mut Vec<String>) {
    for (id, status) in input.suites {
        let claims_passed = status.contains("**passed**");
        let has_pass = input
            .index
            .get(id)
            .is_some_and(|m| m.values().any(|s| s.pass));
        if claims_passed != has_pass {
            out.push(format!(
                "R8 ledger: suite {id} ledger status {} but the index {}",
                if claims_passed {
                    "says **passed**"
                } else {
                    "does not say **passed**"
                },
                if has_pass {
                    "has a passing entry"
                } else {
                    "has no passing entry"
                },
            ));
        }
    }
    for key in input.index.keys() {
        let resolves = input.suites.iter().any(|(id, _)| {
            key == id
                || key
                    .strip_prefix(id.as_str())
                    .is_some_and(|s| s.starts_with('-'))
        });
        if !resolves {
            out.push(format!(
                "R8 ledger: index key {key} is neither a suite-ledger id nor a `<id>-<suffix>` half of one"
            ));
        }
    }
}

/// The revision cited right after `id` in `text` (`V-DPCI-1 rev 2`), if
/// any.
fn cited_rev(text: &str, id: &str) -> Option<u32> {
    let pos = text.find(id)?;
    let after = text[pos + id.len()..].trim_start();
    let digits: String = after
        .strip_prefix("rev ")?
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// R9: a register row whose `Status` is a name must be a real MC status
/// name whose code the row's `Code` matches; and a row citing a scenario
/// id (optionally `rev N`) must resolve to an index entry for that suite
/// (and revision when given). A row citing only ADRs is exempt.
fn r9_register(input: &LintInput<'_>, out: &mut Vec<String>) {
    for row in input.register {
        let at = format!("row {:?} (code {:?})", row.status, row.code);
        if row.status != "unknown" && row.status != "—" {
            match crate::mcstatus::by_name(&row.status) {
                None => out.push(format!(
                    "R9 register: {at}: Status {:?} is not an MC status name",
                    row.status
                )),
                Some(st) if row.code != Some(st.code) => out.push(format!(
                    "R9 register: {at}: Status {:?} is code {:#x} but the row's Code is {:?}",
                    row.status, st.code, row.code
                )),
                Some(_) => {}
            }
        }
        for id in scenario_refs(&row.evidence) {
            match input.index.get(&id) {
                None => out.push(format!(
                    "R9 register: {at}: cites {id} but the index holds no entry for it"
                )),
                Some(entries) => {
                    if let Some(rev) = cited_rev(&row.evidence, &id)
                        && !entries.values().any(|s| s.revision == rev)
                    {
                        out.push(format!(
                            "R9 register: {at}: cites {id} rev {rev} but the index has no such revision"
                        ));
                    }
                }
            }
        }
    }
}

/// R10: every status name in any index entry's `refusals` must appear as
/// the `Status` of at least one register row.
fn r10_register(input: &LintInput<'_>, out: &mut Vec<String>) {
    let statuses: Vec<&str> = input.register.iter().map(|r| r.status.as_str()).collect();
    for (suite, entries) in input.index {
        for (label, s) in entries {
            for name in &s.refusals {
                if !statuses.contains(&name.as_str()) {
                    out.push(format!(
                        "R10 register: index entry {suite}/{label} observed refusal {name:?} but no register row has that Status"
                    ));
                }
            }
        }
    }
}

// ===================== intent-layer copies (ADR-0014 D9) =====================
// The `models/intent/` model is the source of truth (ADR-0002 §2): every
// enumeration that restates it — a witness list, a ledger narrative, an ADR
// section — is a linted copy, never a sibling (ADR-0014). ADR-0013 is the
// accepted-vocabulary record whose §5 (refusals), §6 (invariants) and §7
// (scenarios) are exactly such copies, and its own Consequences note says
// they "drift ... exactly this way" and belong under this lint. R11–R14
// cross-check those copies against the model so a drift fails in CI, the same
// design-D9 mechanism R1–R10 apply to the board ledgers. R14 extends the reach
// to the Rust domain enums (`dpaa2_api::Refusal`, `dpaa2_api::Family`), which
// restate `refuse.qnt`/`types.qnt` and so are linted copies too (ADR-0014).
// Parsing is pure over `&str`; the scenario file set arrives as two stem lists
// and the Rust variant sets as two name lists the harness reads.

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

/// R12: the plan invariants agree between `invariants.qnt` (truth) and
/// ADR-0013 §6 (copy), by id and name. `COVERAGE.md` carries no `INTENT_I`
/// rows yet — the ledger rows that tie `INTENT_I1..I9` to the baseline ids
/// land at a later task (`invariants.qnt` header) — so there is no COVERAGE
/// leg to check.
fn r12_invariants(invariants_qnt: &str, adr_md: &str, out: &mut Vec<String>) {
    let model = parse_intent_invariants(invariants_qnt);
    let adr = parse_adr_invariants(&md_section(adr_md, "### 6."));
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
    }
    for (n, aname) in &adr {
        if !model.iter().any(|(m, _)| m == n) {
            out.push(format!(
                "R12 invariants: ADR-0013 §6 lists INTENT_I{n} `{aname}`, absent from invariants.qnt"
            ));
        }
    }
}

/// R13: every scenario `.qnt` has a same-stem `.toml` and vice versa (the
/// file-level pairing; the semantic toml→plan equality is task 3.4), and the
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
/// `dpaa2_api::Refusal` variant list ([`dpaa2_api::REFUSAL_VARIANTS`]) and the
/// `dpaa2_api::Family` variant set (from [`dpaa2_api::Family::variant_name`] over
/// [`dpaa2_api::ALL_FAMILIES`]) are the copies (ADR-0014: a Rust enum that
/// restates the model is a linted copy, tied back here). Refusal names apply the
/// same `Reserved`/`Foreign` anchor alias as the ADR §5 copy (R11). Adding,
/// removing, or renaming a variant on either side — model or Rust — breaks this
/// leg; the Rust list-vs-enum tie is the crate-local exhaustive `match`
/// (`Refusal::name`, `Family::variant_name`) that will not compile until the
/// list moves with the enum.
fn r14_rust_copies(
    refuse_qnt: &str,
    types_qnt: &str,
    rust_refusals: &[&str],
    rust_families: &[&str],
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

    // Families: names identical on both sides.
    let model_families = parse_family_variants(types_qnt);
    for v in &model_families {
        if !rust_families.contains(&v.as_str()) {
            out.push(format!(
                "R14 rust: types.qnt Family {v} has no dpaa2_api::Family counterpart"
            ));
        }
    }
    for f in rust_families {
        if !model_families.iter().any(|v| v == f) {
            out.push(format!(
                "R14 rust: dpaa2_api::Family {f} is absent from types.qnt"
            ));
        }
    }
}

/// Runs the intent-layer cross-checks (R11–R14) over the `models/intent/` and
/// `models/core/` copies and returns one finding per drift; an empty vector is
/// the green verdict. Distinct from [`lint`] because it reads a different
/// document set (the model files, ADR-0013, and the `dpaa2_api` domain enums,
/// not the board ledgers).
#[must_use]
#[allow(clippy::too_many_arguments)] // one &str per copy, all read-only
pub fn intent_lint(
    refuse_qnt: &str,
    types_qnt: &str,
    alphabet_qnt: &str,
    invariants_qnt: &str,
    coverage_md: &str,
    adr_md: &str,
    scenario_qnt_stems: &[String],
    scenario_toml_stems: &[String],
    rust_refusals: &[&str],
    rust_families: &[&str],
) -> Vec<String> {
    let mut out = Vec::new();
    r11_refusals(refuse_qnt, alphabet_qnt, coverage_md, adr_md, &mut out);
    r12_invariants(invariants_qnt, adr_md, &mut out);
    r13_scenarios(scenario_qnt_stems, scenario_toml_stems, adr_md, &mut out);
    r14_rust_copies(
        refuse_qnt,
        types_qnt,
        rust_refusals,
        rust_families,
        &mut out,
    );
    out
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
    r7_verdicts(input, &mut out);
    r8_ledger(input, &mut out);
    r9_register(input, &mut out);
    r10_register(input, &mut out);
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

    /// An empty index for the rules (R1–R6) that never consult it.
    fn empty_index() -> crate::verdict::Index {
        crate::verdict::Index::new()
    }

    #[allow(clippy::too_many_arguments)]
    fn base_input<'a>(
        coverage: &'a Coverage,
        baseline: &'a [(String, String)],
        suites: &'a [(String, String)],
        roadmap: &'a [(u32, String)],
        scenarios: &'a [String],
        index: &'a crate::verdict::Index,
        dir_exists: &'a dyn Fn(&str) -> bool,
    ) -> LintInput<'a> {
        LintInput {
            coverage,
            baseline,
            suites,
            roadmap,
            scenarios,
            index,
            register: &[],
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
        let index = empty_index();
        // Baseline carries an id COVERAGE lacks, and vice versa.
        let baseline = vec![("DPRC-I2".to_owned(), "candidate".to_owned())];
        let mut out = Vec::new();
        r1_ids(
            &base_input(&cov, &baseline, &[], &[], &[], &index, &dir),
            &mut out,
        );
        assert_eq!(out.len(), 2, "{out:?}");

        let baseline_ok = vec![("DPRC-I1".to_owned(), "candidate".to_owned())];
        let mut ok = Vec::new();
        r1_ids(
            &base_input(&cov, &baseline_ok, &[], &[], &[], &index, &dir),
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
        let index = empty_index();
        let mut out = Vec::new();
        r2_tally(
            &base_input(&bad, &[], &[], &[], &[], &index, &dir),
            &mut out,
        );
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
        r2_tally(
            &base_input(&good, &[], &[], &[], &[], &index, &dir),
            &mut ok,
        );
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
        let index = empty_index();
        let mut out = Vec::new();
        r3_suites(
            &base_input(&cov, &[], suites, &[], scenarios, &index, &dir),
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
        let index = empty_index();
        let mut out = Vec::new();
        r4_changes(
            &base_input(&cov, &[], &[], &roadmap, &[], &index, &dir),
            &mut out,
        );
        assert_eq!(out.len(), 1, "{out:?}");

        let cov_ok = Coverage {
            rows: vec![cov_row("DPRC-I8", "deferred", "`pool-objects` (#6)", "—")],
            ..cov
        };
        let mut ok = Vec::new();
        r4_changes(
            &base_input(&cov_ok, &[], &[], &roadmap, &[], &index, &dir),
            &mut ok,
        );
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
        let index = empty_index();
        let mut out = Vec::new();
        r5_open(
            &base_input(&cov, &[], &[], &[], &[], &index, &dir),
            &mut out,
        );
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
        r5_open(
            &base_input(&cov_ok, &[], &[], &[], &[], &index, &dir),
            &mut ok,
        );
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
        let index = empty_index();
        let mut out = Vec::new();
        r6_levels(
            &base_input(&cov, &baseline, &[], &[], &[], &index, &dir),
            &mut out,
        );
        assert!(!out.is_empty(), "{out:?}");

        let baseline_ok = vec![("DPRC-I9".to_owned(), "verified 2026".to_owned())];
        let mut ok = Vec::new();
        r6_levels(
            &base_input(&cov, &baseline_ok, &[], &[], &[], &index, &dir),
            &mut ok,
        );
        assert!(ok.is_empty(), "{ok:?}");
    }

    /// A one-entry summary for the index-backed rules.
    fn summ(pass: bool, date: &str, revision: u32) -> crate::verdict::Summary {
        crate::verdict::Summary {
            pass,
            date: date.to_owned(),
            kind: crate::verdict::Kind::Batch,
            revision,
            steps: "1/1".to_owned(),
            source_hash: "fnv1a64:0".to_owned(),
            hook: None,
            archive: None,
            refusals: Vec::new(),
        }
    }

    fn one_entry(suite: &str, label: &str, s: crate::verdict::Summary) -> crate::verdict::Index {
        let mut idx = crate::verdict::Index::new();
        idx.entry(suite.to_owned())
            .or_default()
            .insert(label.to_owned(), s);
        idx
    }

    #[test]
    fn parse_verdict_citation_reads_date_suite_and_rev() {
        assert_eq!(
            parse_verdict_citation("verified 2026-08-23 (V-DPRC-1 rev 3, 13/13): x"),
            Some(("2026-08-23".to_owned(), "V-DPRC-1".to_owned(), Some(3)))
        );
        assert_eq!(
            parse_verdict_citation("verified 2026-08-25 (V-READBACK-1): y"),
            Some(("2026-08-25".to_owned(), "V-READBACK-1".to_owned(), None))
        );
        // A face-qualified verdict does not lead with `verified` → no
        // obligation.
        assert_eq!(
            parse_verdict_citation("positive face verified 2026-08-23 (V-DPSW-1)"),
            None
        );
        // Undated reference → no obligation.
        assert_eq!(parse_verdict_citation("verified (ADR-0007 §3)"), None);
    }

    #[test]
    fn r7_flags_a_stale_verdict_and_passes_a_matching_one() {
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
                "verified 2026-08-23 (V-DPRC-1 rev 3, 13/13): ok",
            )],
        };
        let dir = |_: &str| true;

        // The index holds the suite, but that revision did not pass.
        let bad = one_entry("V-DPRC-1", "V-DPRC-1", summ(false, "2026-08-23", 3));
        let mut out = Vec::new();
        r7_verdicts(&base_input(&cov, &[], &[], &[], &[], &bad, &dir), &mut out);
        assert_eq!(out.len(), 1, "{out:?}");

        // A passing entry with the cited date and revision clears it.
        let ok_idx = one_entry("V-DPRC-1", "V-DPRC-1", summ(true, "2026-08-23", 3));
        let mut ok = Vec::new();
        r7_verdicts(
            &base_input(&cov, &[], &[], &[], &[], &ok_idx, &dir),
            &mut ok,
        );
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn r8_flags_ledger_index_disagreement_and_orphan_keys() {
        let cov = Coverage {
            tally: Tally {
                modeled: 0,
                deferred: 0,
                board_settled: 0,
                board_pending: 0,
                candidates: 0,
            },
            rows: vec![],
        };
        let dir = |_: &str| true;
        let suites = vec![("V-DPRC-1".to_owned(), "**passed** (13/13)".to_owned())];

        // Ledger claims passed but the index has no passing entry, and a
        // stray key belongs to no ledger suite → two findings.
        let mut bad = one_entry("V-DPRC-1", "V-DPRC-1", summ(false, "2026-08-23", 1));
        bad.entry("V-ORPHAN-9".to_owned())
            .or_default()
            .insert("x".to_owned(), summ(true, "2026-08-23", 1));
        let mut out = Vec::new();
        r8_ledger(
            &base_input(&cov, &[], &suites, &[], &[], &bad, &dir),
            &mut out,
        );
        assert_eq!(out.len(), 2, "{out:?}");

        // Passing entry present, and a `-postboot` half resolves to its
        // ledger id → clean.
        let mut ok_idx = one_entry("V-DPRC-1", "V-DPRC-1", summ(true, "2026-08-23", 1));
        ok_idx
            .entry("V-DPRC-1-postboot".to_owned())
            .or_default()
            .insert("p".to_owned(), summ(true, "2026-08-23", 1));
        let mut ok = Vec::new();
        r8_ledger(
            &base_input(&cov, &[], &suites, &[], &[], &ok_idx, &dir),
            &mut ok,
        );
        assert!(ok.is_empty(), "{ok:?}");
    }

    /// An empty coverage for the register rules (R9, R10) that never read
    /// it.
    fn empty_cov() -> Coverage {
        Coverage {
            tally: Tally {
                modeled: 0,
                deferred: 0,
                board_settled: 0,
                board_pending: 0,
                candidates: 0,
            },
            rows: vec![],
        }
    }

    fn reg(code: Option<u8>, status: &str, evidence: &str) -> RegisterRow {
        RegisterRow {
            code,
            status: status.to_owned(),
            evidence: evidence.to_owned(),
        }
    }

    fn reg_input<'a>(
        cov: &'a Coverage,
        index: &'a crate::verdict::Index,
        register: &'a [RegisterRow],
        dir: &'a dyn Fn(&str) -> bool,
    ) -> LintInput<'a> {
        LintInput {
            coverage: cov,
            baseline: &[],
            suites: &[],
            roadmap: &[],
            scenarios: &[],
            index,
            register,
            dir_exists: dir,
        }
    }

    #[test]
    fn parse_register_reads_the_second_table_not_the_first() {
        let md = "\
| Code | Status | errno | restool exit |
|------|--------|-------|--------------|
| 0x4 | No privilege | EPERM | 255 |

| Code | Status | Family · verb | Condition | Raised by | Evidence |
|------|--------|---------------|-----------|-----------|----------|
| 0x4 | No privilege | dprtc create | second instance | firmware | V-DPRTC-1 rev 2 |
| — | — | dpni set | bad flag | restool | ADR-0009 |
";
        let register = parse_register(md);
        assert_eq!(register.len(), 2);
        assert_eq!(register[0].code, Some(0x4));
        assert_eq!(register[0].status, "No privilege");
        assert!(register[0].evidence.contains("V-DPRTC-1 rev 2"));
        assert_eq!(register[1].code, None);
        assert_eq!(register[1].status, "—");
    }

    #[test]
    fn r9_flags_bad_name_wrong_code_and_missing_verdict_then_passes() {
        let cov = empty_cov();
        let dir = |_: &str| true;
        let index = one_entry("V-DPCI-1", "V-DPCI-1-rev2", summ(true, "2026-08-23", 2));

        // A non-status name, a right name with the wrong code, and a
        // citation to a suite the index does not hold → three findings.
        let bad = vec![
            reg(Some(0x4), "Nope", "ADR-0009"),
            reg(Some(0x5), "No privilege", "ADR-0009"),
            reg(Some(0x4), "No privilege", "V-MISSING-9"),
        ];
        let mut out = Vec::new();
        r9_register(&reg_input(&cov, &index, &bad, &dir), &mut out);
        assert_eq!(out.len(), 3, "{out:?}");

        // Right name+code, a resolving suite+revision, and an ADR-only
        // non-MC row → clean.
        let good = vec![
            reg(Some(0x4), "No privilege", "V-DPCI-1 rev 2"),
            reg(None, "—", "ADR-0009"),
        ];
        let mut ok = Vec::new();
        r9_register(&reg_input(&cov, &index, &good, &dir), &mut ok);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn r9_flags_a_cited_revision_the_index_lacks() {
        let cov = empty_cov();
        let dir = |_: &str| true;
        let index = one_entry("V-DPCI-1", "V-DPCI-1", summ(true, "2026-08-23", 1));
        let register = vec![reg(Some(0x4), "No privilege", "V-DPCI-1 rev 2")];
        let mut out = Vec::new();
        r9_register(&reg_input(&cov, &index, &register, &dir), &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
    }

    #[test]
    fn r10_flags_an_unregistered_refusal_then_passes() {
        let cov = empty_cov();
        let dir = |_: &str| true;
        let mut s = summ(true, "2026-08-23", 1);
        s.refusals = vec!["No privilege".to_owned()];
        let index = one_entry("V-DPCI-1", "V-DPCI-1", s);

        // The register does not carry the observed status → one finding.
        let bad = vec![reg(Some(0xA), "Device is busy", "—")];
        let mut out = Vec::new();
        r10_register(&reg_input(&cov, &index, &bad, &dir), &mut out);
        assert_eq!(out.len(), 1, "{out:?}");

        // The register carries it → clean.
        let good = vec![reg(Some(0x4), "No privilege", "—")];
        let mut ok = Vec::new();
        r10_register(&reg_input(&cov, &index, &good, &dir), &mut ok);
        assert!(ok.is_empty(), "{ok:?}");
    }

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

    #[test]
    fn r12_passes_then_flags_a_renamed_invariant() {
        let mut ok = Vec::new();
        r12_invariants(INV_QNT, ADR6, &mut ok);
        assert!(ok.is_empty(), "{ok:?}");

        let adr_bad = ADR6.replace("`edgesTypedAndSingle`", "`edgesTypedAndDouble`");
        let mut out = Vec::new();
        r12_invariants(INV_QNT, &adr_bad, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].contains("INTENT_I2"), "{out:?}");
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
        let mut out = Vec::new();
        r14_rust_copies(REFUSE, TYPES, &refusals, &families, &mut out);
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn r14_flags_a_dropped_refusal_and_a_renamed_family() {
        // The deliberate-drift negative test, both directions: the Rust copies
        // drop a refusal (`Infeasible`) and rename a family (`Dpmac` → `Dpmax`),
        // all in memory.
        let refusals_bad = ["TenantAbsent", "Reserved"]; // Infeasible gone
        let families_bad = ["Dprc", "Dpni", "Dpmax"]; // renamed
        let mut out = Vec::new();
        r14_rust_copies(REFUSE, TYPES, &refusals_bad, &families_bad, &mut out);
        // Model has Infeasible, the Rust copy does not.
        assert!(
            out.iter()
                .any(|m| m
                    .contains("refuse.qnt Refusal variant Infeasible has no dpaa2_api::Refusal")),
            "{out:?}"
        );
        // Model has Dpmac, the Rust copy does not.
        assert!(
            out.iter()
                .any(|m| m.contains("types.qnt Family Dpmac has no dpaa2_api::Family")),
            "{out:?}"
        );
        // The Rust copy's Dpmax has no model family.
        assert!(
            out.iter()
                .any(|m| m.contains("dpaa2_api::Family Dpmax is absent from types.qnt")),
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
        let families: Vec<&str> = dpaa2_api::ALL_FAMILIES
            .iter()
            .map(|f| f.variant_name())
            .collect();
        let mut out = Vec::new();
        r14_rust_copies(
            &refuse,
            &types,
            &dpaa2_api::REFUSAL_VARIANTS,
            &families,
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }
}
