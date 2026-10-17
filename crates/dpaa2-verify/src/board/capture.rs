//! The passive ioctl-payload capture tap (dpsw-typestate D7; ADR-0004 §6/§8).
//!
//! A board sitting records restool's 64-byte MC command and response of every
//! `SEND_MC_COMMAND` ioctl with a system-wide bpftrace program filtered on
//! `comm == "restool"` and the one request value. This module is the offline
//! half — it parses that recorder's text into typed per-step records and
//! serializes them as the `mc-portal-backend` (#10) encode-fixture corpus — and
//! the live half: the `Recorder` seam the driver brackets each step with, its
//! null and bpftrace implementations.
//!
//! The tap never judges. A `PortalStep` keeps the raw command and response
//! bytes and lifts only the two header fields #10's decode is anchored on (the
//! command id and the token); the full decode is #10's own work.
//!
//! The recorder's grammar — bpftrace `%r` escaping — is each line
//! `CMD fd=<n> <64 bytes>` or `RSP ret=<n> <64 bytes>`, a CMD immediately
//! before its RSP. In the byte field a printable ASCII byte prints literally
//! including backslash printed as a single `\` (board-observed, v0.23.2, 2026-10-18); every other byte prints as `\xNN`
//! (two lowercase hex digits).

use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use dpaa2_hal::portal::SEND_MC_COMMAND;

/// The one ioctl request the tap filters on — `dpaa2-hal`'s pinned
/// `SEND_MC_COMMAND` (`_IOWR('R', 0xE0, 64)` = `0xc040_52e0`), derived from the
/// transport so the filter and the portal can never drift (dpsw-typestate D7).
// The cast is portability, not identity: `c_ulong` is u64 on the board targets.
#[allow(clippy::unnecessary_cast)]
pub const MC_IOCTL_CMD: u64 = SEND_MC_COMMAND as u64;

/// The MC command frame length restool's ioctl carries (restool
/// `mc_v10/fsl_mc_cmd.h`: `u64 header` + `u64 params[7]`).
const FRAME_LEN: usize = 64;

/// One MC command captured whole: the restool-sent command bytes and the
/// kernel-written response, plus the two header fields #10's decode anchors on
/// (restool `mc_v10` header layout).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PortalStep {
    /// The file descriptor the CMD ioctl was issued on.
    pub fd: i64,
    /// The value the RSP ioctl returned.
    pub ret: i64,
    /// MC command id — little-endian u16 at header bytes `[6,7]`.
    pub cmd_id: u16,
    /// MC token — little-endian u16 at header bytes `[4,5]`, granted by
    /// `DPRC_OPEN` and carried by every later command.
    pub token: u16,
    /// The 64 command bytes, lowercase hex.
    #[serde(with = "hexbytes")]
    pub cmd: Vec<u8>,
    /// The 64 response bytes, lowercase hex.
    #[serde(with = "hexbytes")]
    pub rsp: Vec<u8>,
}

/// A suite's captured corpus: the per-step records in capture order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Corpus {
    /// The captured MC commands, one per `CMD`/`RSP` pair.
    pub steps: Vec<PortalStep>,
}

impl Corpus {
    /// Parses the recorder's output into a corpus. Strict: a CMD must be
    /// immediately followed by its RSP, so an unpaired CMD (or an RSP with no
    /// CMD) is an error, not a skipped line. Blank lines are ignored, as is
    /// bpftrace's one `Attaching <n> probe(s)...` banner — it rides stdout into
    /// every genuine capture (board sitting 2026-10-18, v0.23.2), so tolerating
    /// exactly that line keeps the self-test parse honest; any other
    /// non-`CMD`/`RSP` line is an error.
    ///
    /// # Errors
    ///
    /// Returns the first grammar breach, naming the line: a broken pair, a byte
    /// field that is not exactly 64 bytes, or a line that is neither kind.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut steps = Vec::new();
        let mut pending: Option<(i64, Vec<u8>)> = None;
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            if line.starts_with("Attaching ")
                && (line.ends_with("probe...") || line.ends_with("probes..."))
            {
                continue;
            }
            let at = n + 1;
            if let Some(rest) = line.strip_prefix("CMD fd=") {
                if pending.is_some() {
                    return Err(format!("line {at}: a CMD with no RSP before the next CMD"));
                }
                pending = Some(split_field(rest, at)?);
            } else if let Some(rest) = line.strip_prefix("RSP ret=") {
                let (fd, cmd) = pending
                    .take()
                    .ok_or_else(|| format!("line {at}: an RSP with no preceding CMD"))?;
                let (ret, rsp) = split_field(rest, at)?;
                steps.push(PortalStep {
                    fd,
                    ret,
                    cmd_id: le16(&cmd, 6),
                    token: le16(&cmd, 4),
                    cmd,
                    rsp,
                });
            } else {
                return Err(format!(
                    "line {at}: neither a CMD nor an RSP line: {line:?}"
                ));
            }
        }
        if pending.is_some() {
            return Err("trailing CMD with no RSP".to_owned());
        }
        Ok(Self { steps })
    }

    /// The corpus as pretty JSON, the committed-beside-results fixture form.
    ///
    /// # Errors
    ///
    /// Returns the serializer error (a `Vec<u8>` that is not valid; never, in
    /// practice).
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

/// Splits a `<n> <escaped 64 bytes>` tail into the leading integer and the
/// decoded frame, erroring when the frame is not exactly 64 bytes.
fn split_field(rest: &str, at: usize) -> Result<(i64, Vec<u8>), String> {
    let (num, payload) = rest
        .split_once(' ')
        .ok_or_else(|| format!("line {at}: no byte field after the count"))?;
    let n: i64 = num
        .parse()
        .map_err(|_| format!("line {at}: bad integer {num:?}"))?;
    let bytes = unescape(payload, at)?;
    if bytes.len() != FRAME_LEN {
        return Err(format!(
            "line {at}: decoded {} bytes, expected {FRAME_LEN}",
            bytes.len()
        ));
    }
    Ok((n, bytes))
}

/// Decodes one bpftrace `%r` byte field. A printable ASCII byte is literal,
/// backslash included as a single `\`; every other byte is `\xNN`. Decode is
/// greedy: a `\` is a hex escape only when `xHH` (two hex digits) follows,
/// otherwise it is a literal 0x5c. That makes the format ambiguous — a frame
/// byte run `[0x5c, b'x', hex, hex]` renders identically to one escaped byte
/// and so decodes one byte short. Decode can only lose length, never gain it,
/// so the caller's 64-byte frame law catches the collision as a loud error,
/// never a silent mis-parse (board-observed, v0.23.2, 2026-10-18).
fn unescape(s: &str, at: usize) -> Result<Vec<u8>, String> {
    let bad = || format!("line {at}: malformed byte escape");
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(FRAME_LEN);
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' {
            out.push(b[i]);
            i += 1;
        } else if b.get(i + 1) == Some(&b'x') {
            let hi = *b.get(i + 2).ok_or_else(bad)?;
            let lo = *b.get(i + 3).ok_or_else(bad)?;
            out.push(hexpair(hi, lo).ok_or_else(bad)?);
            i += 4;
        } else {
            out.push(b'\\');
            i += 1;
        }
    }
    Ok(out)
}

/// A little-endian u16 at `frame[at..at+2]`; the frame is always 64 bytes here.
fn le16(frame: &[u8], at: usize) -> u16 {
    u16::from(frame[at]) | (u16::from(frame[at + 1]) << 8)
}

/// The byte two lowercase-or-uppercase hex digits spell, or `None`.
fn hexpair(hi: u8, lo: u8) -> Option<u8> {
    let d = |c: u8| (c as char).to_digit(16);
    u8::try_from((d(hi)? << 4) | d(lo)?).ok()
}

/// Lowercase hex of a byte slice.
fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        s.push(char::from_digit(u32::from(b & 0xf), 16).unwrap_or('0'));
    }
    s
}

/// Bytes from a lowercase hex string.
fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let b = s.as_bytes();
    if !b.len().is_multiple_of(2) {
        return Err("odd-length hex".to_owned());
    }
    (0..b.len())
        .step_by(2)
        .map(|i| hexpair(b[i], b[i + 1]).ok_or_else(|| format!("bad hex {:?}", &s[i..i + 2])))
        .collect()
}

/// Serde glue serializing the raw frames as hex strings so the committed
/// corpus is a readable byte-level fixture (ADR-0004 §6).
mod hexbytes {
    pub fn serialize<S: serde::Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::to_hex(bytes))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(d)?;
        super::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// The per-step capture seam (dpsw-typestate D7). The driver brackets each
/// probe step with a recorder; the tap is observation-only and must never
/// alter the driven step, so the recorder runs the step verbatim between its
/// start and stop.
pub trait Recorder {
    /// Proves the whole chain before any mutating step: the recorder is
    /// present, its probes attach, and a read-only self-query yields at least
    /// one CMD/RSP pair. A failure is named and loud (never a silent empty
    /// corpus — the mbt-harness dead-tap scenario).
    ///
    /// # Errors
    ///
    /// Returns the named refusal when any link of the chain is missing.
    fn preamble(&mut self) -> Result<(), String>;

    /// Records one step: starts the recorder and waits for its attach banner,
    /// runs `run` (the step's unchanged argv) only once attached, then stops
    /// the recorder and banks its payload under `tag`.
    ///
    /// # Errors
    ///
    /// Returns a named error — without running the step — when the recorder
    /// does not attach within the bound (the attach race is real).
    fn step(&mut self, tag: &str, run: &mut dyn FnMut()) -> Result<(), String>;
}

/// The null recorder: capture off. It runs the step and nothing else, so a
/// tapped and an untapped run drive byte-identical steps.
pub struct NullRecorder;

impl Recorder for NullRecorder {
    fn preamble(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn step(&mut self, _tag: &str, run: &mut dyn FnMut()) -> Result<(), String> {
        run();
        Ok(())
    }
}

/// The attach-banner wait bound: ten one-second polls of the recorder's
/// output. A timeout is a loud failure, not a shorter capture.
const ATTACH_POLLS: u32 = 10;

/// The longest sanitized tag [`sanitize_tag`] keeps in a capture filename;
/// the driver's `{i:02}-` index prefix survives this bound, so names stay
/// unique and ordered even when long labels share a prefix (ADR-0004 §6).
const TAG_MAX: usize = 60;

/// Maps a tag to its filesystem-safe filename stem: ASCII `[A-Za-z0-9._-]`
/// is kept, every other character maps to `-`, and the result is truncated
/// to [`TAG_MAX`]. The human-readable tag stays untouched in error messages;
/// only the on-disk name is sanitized, so a label carrying a `/` names a flat
/// file rather than a nonexistent subdirectory (dpsw-typestate D7).
fn sanitize_tag(tag: &str) -> String {
    tag.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .take(TAG_MAX)
        .collect()
}

/// The live recorder: a system-wide bpftrace program on the ioctl tracepoints,
/// filtered to restool's `SEND_MC_COMMAND`. One instance per sitting; it spawns
/// a fresh bpftrace per step so each step's payload is a file of its own
/// (`capture-<tag>.mc` under the results directory). The `bpftrace` path is a
/// constructor seam so a dead tap is testable with no board.
///
/// The `<tag>` is routed through `sanitize_tag` into a filesystem-safe,
/// bounded stem, and the driver index-prefixes the label (`{i:02}-<label>`),
/// so the committed fixture corpus (ADR-0004 §6) has deterministic,
/// collision-free names even when labels carry slashes or share a long prefix.
pub struct BpftraceRecorder {
    bpftrace: PathBuf,
    results: PathBuf,
}

impl BpftraceRecorder {
    /// A recorder spawning `bpftrace` and writing per-step files under
    /// `results`.
    pub fn new(bpftrace: impl Into<PathBuf>, results: impl Into<PathBuf>) -> Self {
        Self {
            bpftrace: bpftrace.into(),
            results: results.into(),
        }
    }

    /// The bpftrace program text (the recorder's grammar lives in the module
    /// docs). System-wide tracepoints, so restool children are seen with no
    /// follow-forks logic.
    fn program() -> String {
        format!(
            "tracepoint:syscalls:sys_enter_ioctl /comm == \"restool\" && args->cmd == {MC_IOCTL_CMD:#x}/ \
             {{ @p[tid] = args->arg; printf(\"CMD fd=%d %r\\n\", args->fd, buf(args->arg, 64)); }} \
             tracepoint:syscalls:sys_exit_ioctl /@p[tid] != 0/ \
             {{ printf(\"RSP ret=%d %r\\n\", args->ret, buf(@p[tid], 64)); delete(@p[tid]); }}"
        )
    }

    /// The sanitized `capture-<tag>.<suffix>` path under the results
    /// directory — the one seam both the start and attach reads build their
    /// names through (dpsw-typestate D7).
    fn capture_path(&self, tag: &str, suffix: &str) -> PathBuf {
        self.results
            .join(format!("capture-{}.{suffix}", sanitize_tag(tag)))
    }

    /// Spawns a recorder writing stdout to `<tag>.mc` and stderr to
    /// `<tag>.err`, then waits for bpftrace's `Attaching` banner.
    fn start(&self, tag: &str) -> Result<(Child, PathBuf), String> {
        let out_path = self.capture_path(tag, "mc");
        let err_path = self.capture_path(tag, "err");
        let out = File::create(&out_path).map_err(|e| format!("capture {tag}: {e}"))?;
        let err = File::create(&err_path).map_err(|e| format!("capture {tag}: {e}"))?;
        let child = Command::new(&self.bpftrace)
            .arg("-e")
            .arg(Self::program())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .map_err(|e| {
                format!(
                    "capture tap requested but bpftrace ({}) failed to spawn: {e} (dpsw-typestate D7)",
                    self.bpftrace.display()
                )
            })?;
        for _ in 0..ATTACH_POLLS {
            if self.attached(tag) {
                return Ok((child, out_path));
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        Ok((child, out_path))
    }

    /// Whether the recorder has attached. bpftrace's `Attaching` banner moved
    /// between streams across versions (board sitting 2026-10-18, v0.23.2 put
    /// it on stdout, not stderr), so either the `.mc` (stdout) or the `.err`
    /// (stderr) file carrying it counts as attached (dpsw-typestate D7).
    fn attached(&self, tag: &str) -> bool {
        let shows = |suffix: &str| {
            std::fs::read_to_string(self.capture_path(tag, suffix))
                .is_ok_and(|s| s.contains("Attaching"))
        };
        shows("mc") || shows("err")
    }

    /// SIGINT the recorder so bpftrace flushes, then reap it.
    fn stop(child: &mut Child) {
        let _ = Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status();
        let _ = child.wait();
    }
}

/// Classifies a self-test capture (dpsw-typestate D7): `Ok` once it parsed to
/// at least one `CMD`/`RSP` pair; otherwise a named refusal that tells the two
/// failures apart — a grammar breach surfaces the parse error verbatim, while a
/// cleanly-parsed but empty corpus is the dead-tap refusal.
fn self_test_verdict(text: &str) -> Result<(), String> {
    match Corpus::parse(text) {
        Ok(c) if !c.steps.is_empty() => Ok(()),
        Ok(_) => Err(
            "capture self-test yielded no CMD/RSP pair — a dead tap (dpsw-typestate D7)".to_owned(),
        ),
        Err(e) => Err(format!(
            "capture self-test did not parse: {e} (dpsw-typestate D7)"
        )),
    }
}

impl Recorder for BpftraceRecorder {
    fn preamble(&mut self) -> Result<(), String> {
        Command::new(&self.bpftrace).arg("--version").output().map_err(|e| {
            format!(
                "capture tap requested but bpftrace ({}) is not runnable: {e} (dpsw-typestate D7)",
                self.bpftrace.display()
            )
        })?;
        let (mut child, out_path) = self.start("selftest")?;
        if !self.attached("selftest") {
            Self::stop(&mut child);
            return Err(
                "capture recorder did not attach within the bound (dpsw-typestate D7)".to_owned(),
            );
        }
        let _ = Command::new("restool").arg("dprc").arg("list").output();
        std::thread::sleep(std::time::Duration::from_secs(1));
        Self::stop(&mut child);
        let text = std::fs::read_to_string(&out_path).unwrap_or_default();
        self_test_verdict(&text)
    }

    fn step(&mut self, tag: &str, run: &mut dyn FnMut()) -> Result<(), String> {
        let (mut child, _) = self.start(tag)?;
        if !self.attached(tag) {
            Self::stop(&mut child);
            return Err(format!(
                "capture recorder did not attach for step {tag} within the bound (dpsw-typestate D7)"
            ));
        }
        run();
        Self::stop(&mut child);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64-byte MC frame: `prefix` then zeros (restool `mc_v10` layout).
    fn frame(prefix: &[u8]) -> [u8; FRAME_LEN] {
        let mut f = [0u8; FRAME_LEN];
        f[..prefix.len()].copy_from_slice(prefix);
        f
    }

    /// Genuine board capture (ADR-0004 §6; board capture 2026-10-17,
    /// `restool dprc list` — the 6.6.52 reference kernel with the tracing
    /// fragment, bpftrace v0.23.2):
    /// `GET_CONTAINER_ID`, `DPRC_OPEN` granting token 0x033e, the object walk,
    /// `DPRC_CLOSE`. The frames hold the genuine bytes; the recorder's `%r`
    /// text is reproduced through the reference escaper, so the fixture is
    /// exact (literal printable bytes and all) and cannot drift by miscount.
    fn board_fixture() -> String {
        let line = |tag: &str, f: &[u8; FRAME_LEN]| {
            let esc: String = f.iter().map(|&b| escape_byte(b)).collect();
            format!("{tag} {esc}\n")
        };
        [
            line(
                "CMD fd=3",
                &frame(&[0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x11, 0x83]),
            ),
            line(
                "RSP ret=0",
                &frame(&[
                    0xd1, 0, 0, 0, 0, 0, 0x11, 0x83, 0, 0, 0, 0, 0x0a, 0, 0, 0, 0x27,
                ]),
            ),
            line(
                "CMD fd=3",
                &frame(&[0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x51, 0x80, 0x01]),
            ),
            line(
                "RSP ret=0",
                &frame(&[0xd1, 0, 0, 0, 0x3e, 0x03, 0x51, 0x80, 0x01]),
            ),
            line(
                "CMD fd=3",
                &frame(&[0x00, 0x00, 0x01, 0x00, 0x3e, 0x03, 0x91, 0x15]),
            ),
            line(
                "RSP ret=0",
                &frame(&[0xd1, 0, 0, 0, 0x3e, 0x03, 0x91, 0x15, 0, 0, 0, 0, 0x61]),
            ),
            line(
                "CMD fd=3",
                &frame(&[0x00, 0x00, 0x01, 0x00, 0x3e, 0x03, 0xa1, 0x15]),
            ),
            line(
                "RSP ret=0",
                &frame(&[
                    0xd1, 0, 0, 0, 0x3e, 0x03, 0xa1, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0x57, 0x19,
                    0x01, 0x00, 0x07, 0, 0, 0, 0x08, 0, 0x05, 0, 0, 0, 0, 0, b'd', b'p', b'n',
                    b'i',
                ]),
            ),
            line(
                "CMD fd=3",
                &frame(&[0x00, 0x00, 0x01, 0x00, 0x3e, 0x03, 0x01, 0x80]),
            ),
            line(
                "RSP ret=0",
                &frame(&[0xd1, 0, 0, 0, 0x3e, 0x03, 0x01, 0x80]),
            ),
        ]
        .concat()
    }

    #[test]
    fn the_board_fixture_parses_to_the_lifted_header_fields() {
        let corpus = Corpus::parse(&board_fixture()).expect("fixture parses");
        assert_eq!(corpus.steps.len(), 5);
        for step in &corpus.steps {
            assert_eq!(step.cmd.len(), 64);
            assert_eq!(step.rsp.len(), 64);
        }
        // GET_CONTAINER_ID with no token yet; then DPRC_OPEN; then the walk
        // under the granted token 0x033e.
        assert_eq!(corpus.steps[0].cmd_id, 0x8311);
        assert_eq!(corpus.steps[0].token, 0x0000);
        assert_eq!(corpus.steps[1].cmd_id, 0x8051);
        assert_eq!(corpus.steps[2].cmd_id, 0x1591);
        assert_eq!(corpus.steps[2].token, 0x033e);
        assert_eq!(corpus.steps[3].cmd_id, 0x15a1);
        assert_eq!(corpus.steps[4].cmd_id, 0x8001);
        // GET_OBJ_COUNT answers 0x61 = 97 at the first param byte (offset 12).
        assert_eq!(corpus.steps[2].rsp[12], 0x61);
        // GET_OBJ RSP carries the family name "dpni" at the string offset.
        assert_eq!(&corpus.steps[3].rsp[32..36], b"dpni");
    }

    /// The committed verbatim recorder output — genuine bpftrace stdout, hand
    /// banked, not generated — is the ground truth for the escaping grammar
    /// (ADR-0004 §6; board capture 2026-10-17, `restool dprc list`). Parsing it
    /// must lift the same header fields, and the reference escaper applied to
    /// the parsed frames must reproduce the file byte-for-byte — so an escaping
    /// bug shared by the escaper and the parser cannot hide behind round-trip.
    #[test]
    fn the_committed_board_capture_parses_and_the_escaper_reproduces_it() {
        use std::fmt::Write as _;
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../models/board/V-DPSW-2/capture-dprc-list.mc"
        ))
        .expect("read committed capture fixture");
        let corpus = Corpus::parse(&text).expect("fixture parses");

        let ids: Vec<u16> = corpus.steps.iter().map(|s| s.cmd_id).collect();
        assert_eq!(ids, [0x8311, 0x8051, 0x1591, 0x15a1, 0x8001]);
        for step in &corpus.steps[2..] {
            assert_eq!(step.token, 0x033e, "token carried from the OPEN onward");
        }
        assert_eq!(corpus.steps[2].rsp[12], 0x61);
        assert_eq!(&corpus.steps[3].rsp[32..36], b"dpni");

        let mut rebuilt = String::new();
        for step in &corpus.steps {
            let esc = |f: &[u8]| -> String { f.iter().map(|&b| escape_byte(b)).collect() };
            let _ = writeln!(rebuilt, "CMD fd={} {}", step.fd, esc(&step.cmd));
            let _ = writeln!(rebuilt, "RSP ret={} {}", step.ret, esc(&step.rsp));
        }
        assert_eq!(
            rebuilt, text,
            "the reference escaper must reproduce bpftrace's bytes"
        );
    }

    #[test]
    fn a_literal_byte_decodes_to_itself() {
        // The RSP of GET_OBJ_COUNT holds a literal `a` (0x61) and `'` (0x27)
        // appears in the first RSP — both printable, printed verbatim by %r.
        let corpus = Corpus::parse(&board_fixture()).unwrap();
        assert_eq!(corpus.steps[0].rsp[16], 0x27);
    }

    #[test]
    fn the_corpus_round_trips_through_json() {
        let corpus = Corpus::parse(&board_fixture()).unwrap();
        let json = corpus.to_json().unwrap();
        let back: Corpus = serde_json::from_str(&json).unwrap();
        assert_eq!(corpus, back);
    }

    #[test]
    fn an_unpaired_cmd_is_an_error_not_a_skip() {
        let fx = board_fixture();
        let only_cmd = fx.lines().next().unwrap();
        assert!(Corpus::parse(only_cmd).unwrap_err().contains("CMD"));

        let two_cmds = format!("{only_cmd}\n{only_cmd}");
        assert!(Corpus::parse(&two_cmds).unwrap_err().contains("no RSP"));

        let orphan_rsp = fx.lines().nth(1).unwrap();
        assert!(
            Corpus::parse(orphan_rsp)
                .unwrap_err()
                .contains("no preceding CMD")
        );
    }

    #[test]
    fn a_short_frame_is_an_error() {
        assert!(
            Corpus::parse("CMD fd=3 \\x00\\x01\nRSP ret=0 \\x00")
                .unwrap_err()
                .contains("expected 64")
        );
    }

    #[test]
    fn the_attach_banner_is_tolerated_like_a_blank_line() {
        let esc = |f: &[u8; FRAME_LEN]| -> String { f.iter().map(|&b| escape_byte(b)).collect() };
        let cmd = frame(&[0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x11, 0x83]);
        let rsp = frame(&[0xd1, 0, 0, 0, 0, 0, 0x11, 0x83]);
        let text = format!(
            "Attaching 2 probes...\nCMD fd=3 {}\nRSP ret=0 {}\n",
            esc(&cmd),
            esc(&rsp)
        );
        let corpus = Corpus::parse(&text).expect("banner tolerated like a blank");
        assert_eq!(corpus.steps.len(), 1);
        assert_eq!(corpus.steps[0].cmd_id, 0x8311);
    }

    #[test]
    fn an_arbitrary_non_cmd_line_is_still_an_error() {
        assert!(
            Corpus::parse("totally bogus line")
                .unwrap_err()
                .contains("neither a CMD nor an RSP")
        );
        assert!(
            Corpus::parse("Attaching probes now")
                .unwrap_err()
                .contains("neither a CMD nor an RSP")
        );
    }

    #[test]
    fn the_filter_constant_is_the_transport_request() {
        assert_eq!(MC_IOCTL_CMD, 0xc040_52e0);
    }

    /// The escape reference the parser must invert exactly. Backslash (0x5c) is
    /// a printable, so bpftrace prints it as a single literal `\`.
    fn escape_byte(b: u8) -> String {
        match b {
            0x20..=0x7e => (b as char).to_string(),
            _ => format!("\\x{b:02x}"),
        }
    }

    proptest::proptest! {
        /// Full-domain inversion, minus the format's own lossy collision: a
        /// 0x5c immediately followed by 0x78 renders as `\x…`, the same text as
        /// one `\xHH` escape, so that pair is irreversibly ambiguous by the
        /// grammar's design and is filtered out (the 64-byte frame law is its
        /// runtime backstop). Every other byte vector round-trips exactly.
        #[test]
        fn unescape_inverts_the_reference_escape(bytes: Vec<u8>) {
            proptest::prop_assume!(!bytes.windows(2).any(|w| w == [0x5c, 0x78]));
            let escaped: String = bytes.iter().map(|&b| escape_byte(b)).collect();
            proptest::prop_assert_eq!(unescape(&escaped, 1).unwrap(), bytes);
        }
    }

    #[test]
    fn a_dead_tap_refuses_at_the_preamble() {
        let tmp = std::env::temp_dir();
        let mut rec = BpftraceRecorder::new("/nonexistent/bpftrace-xyz", tmp);
        let err = rec.preamble().unwrap_err();
        assert!(err.contains("bpftrace"), "{err}");
        assert!(err.contains("D7"), "{err}");
    }

    /// Regression (board sitting 2026-10-18, bpftrace v0.23.2): the attach
    /// banner lands on stdout, with stderr empty. A stub bpftrace reproducing
    /// that must still attach and parse — the old stderr-only detection and
    /// strict parser both loud-failed a healthy board here.
    #[test]
    fn the_preamble_accepts_the_banner_on_stdout_only() {
        use std::io::Write as _;
        use std::os::unix::fs::PermissionsExt as _;

        let dir = std::env::temp_dir().join(format!("capture-stub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let esc = |f: &[u8; FRAME_LEN]| -> String { f.iter().map(|&b| escape_byte(b)).collect() };
        let cmd = frame(&[0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x11, 0x83]);
        let rsp = frame(&[0xd1, 0, 0, 0, 0, 0, 0x11, 0x83]);
        let data = dir.join("capture.txt");
        std::fs::write(
            &data,
            format!(
                "Attaching 2 probes...\nCMD fd=3 {}\nRSP ret=0 {}\n",
                esc(&cmd),
                esc(&rsp)
            ),
        )
        .unwrap();

        // `--version` must answer fast: the preamble's precheck blocks on `output()`.
        let script = dir.join("bpftrace-stub.sh");
        let mut f = File::create(&script).unwrap();
        writeln!(
            f,
            "#!/bin/sh\ncase \"$1\" in --version) echo stub; exit 0;; esac\ncat {}\nsleep 30",
            data.display()
        )
        .unwrap();
        drop(f);
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut rec = BpftraceRecorder::new(&script, &dir);
        rec.preamble()
            .expect("banner on stdout only must attach and parse");
    }

    /// A backslash byte (0x5c → a single `\`) and a literal space (0x20) at the
    /// very frame edge survive escape and parse byte-for-byte — the edge space
    /// pins the `\r`-only line trim that must not eat a legal payload byte.
    #[test]
    fn a_backslash_and_an_edge_space_round_trip() {
        let esc = |f: &[u8; FRAME_LEN]| -> String { f.iter().map(|&b| escape_byte(b)).collect() };
        let cmd = frame(&[0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x11, 0x83, 0x5c]);
        let mut rsp = frame(&[0xd1, 0, 0, 0, 0, 0, 0x11, 0x83]);
        rsp[FRAME_LEN - 1] = 0x20;
        let text = format!("CMD fd=3 {}\nRSP ret=0 {}\n", esc(&cmd), esc(&rsp));
        let corpus = Corpus::parse(&text).expect("backslash and edge space parse");
        assert_eq!(corpus.steps.len(), 1);
        assert_eq!(corpus.steps[0].cmd, cmd.to_vec());
        assert_eq!(corpus.steps[0].rsp, rsp.to_vec());
    }

    /// The exact board line (live capture-selftest.mc, bpftrace v0.23.2, the
    /// walk step whose object-index byte is 0x5c). bpftrace printed 0x5c as one
    /// literal `\`, so the text carries a single backslash before the next
    /// `\x00`, not three. Pinned VERBATIM as a Rust literal — not rebuilt with
    /// `escape_byte` — so it proves board truth, not escaper self-consistency.
    #[test]
    fn the_board_backslash_line_parses() {
        let esc = |f: &[u8; FRAME_LEN]| -> String { f.iter().map(|&b| escape_byte(b)).collect() };
        let cmd_line = format!(
            "CMD fd=3 \\x00\\x00\\x01\\x00t\\x02\\xa1\\x15\\{}",
            "\\x00".repeat(55)
        );
        let rsp = frame(&[0xd1, 0, 0, 0, 0x3e, 0x03, 0xa1, 0x15]);
        let text = format!("{cmd_line}\nRSP ret=0 {}\n", esc(&rsp));
        let corpus = Corpus::parse(&text).expect("board backslash line parses");
        assert_eq!(corpus.steps.len(), 1);
        assert_eq!(
            corpus.steps[0].cmd,
            frame(&[0x00, 0x00, 0x01, 0x00, 0x74, 0x02, 0xa1, 0x15, 0x5c]).to_vec()
        );
    }

    /// The format's inherent collision is caught loudly, never silent: a frame
    /// whose bytes are `[0x5c, b'x', b'0', b'0']` escapes to the same text as a
    /// single `\x00`, so it decodes one byte short and the 64-byte frame law
    /// rejects it by name (board grammar ambiguity, v0.23.2).
    #[test]
    fn an_ambiguous_backslash_x_run_fails_loudly_short() {
        let esc = |f: &[u8; FRAME_LEN]| -> String { f.iter().map(|&b| escape_byte(b)).collect() };
        let cmd = frame(&[0x5c, 0x78, 0x30, 0x30]);
        let err = Corpus::parse(&format!("CMD fd=3 {}", esc(&cmd))).unwrap_err();
        assert!(err.contains("expected 64"), "{err}");
    }

    /// The self-test verdict tells the two refusals apart: a grammar breach
    /// carries the parse error text; a clean but empty corpus is the dead tap.
    #[test]
    fn the_self_test_verdict_splits_parse_error_from_dead_tap() {
        let parse_err = self_test_verdict("totally bogus line").unwrap_err();
        assert!(parse_err.contains("did not parse"), "{parse_err}");
        assert!(
            parse_err.contains("neither a CMD nor an RSP"),
            "{parse_err}"
        );
        assert!(parse_err.contains("D7"), "{parse_err}");
        let dead = self_test_verdict("Attaching 2 probes...\n").unwrap_err();
        assert!(dead.contains("a dead tap"), "{dead}");
        assert!(dead.contains("D7"), "{dead}");
    }

    /// The board label that ENOENT'd step 2 (live sitting 2026-10-18,
    /// V-DPSW-2) sanitizes to a slash-free, bounded stem — the `/` that made
    /// `File::create` target a nonexistent subdirectory is gone. Two long
    /// tags differing only past the bound still collide here, which is WHY
    /// the driver index-prefixes the label: the `{i:02}-` survives truncation
    /// and keeps the committed corpus names unique (ADR-0004 §6).
    #[test]
    fn the_sanitizer_is_slash_free_bounded_and_collides_past_the_bound() {
        let board = "DPSW-I3/I9 defer face: create the bindable reference dpsw + its dpmcp at root, WITHOUT a dpbp";
        let safe = sanitize_tag(board);
        assert!(!safe.contains('/'), "the slash is gone: {safe}");
        assert!(safe.len() <= TAG_MAX, "bounded: {}", safe.len());

        let a = sanitize_tag(&format!("{}-aaa", "x".repeat(TAG_MAX)));
        let b = sanitize_tag(&format!("{}-bbb", "x".repeat(TAG_MAX)));
        assert_eq!(a, b, "tags differing only past the bound collide");
    }

    /// A slashed tag must not ENOENT: `step` sanitizes it to a flat filename,
    /// so the recorder spawns, attaches, runs the step, and banks the capture
    /// file (live sitting 2026-10-18 — the raw `/` label failed `File::create`
    /// before the step ran). Reuses the stdout-banner stub pattern.
    #[test]
    fn a_slashed_step_tag_banks_a_sanitized_file() {
        use std::io::Write as _;
        use std::os::unix::fs::PermissionsExt as _;

        let dir = std::env::temp_dir().join(format!("capture-slash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let script = dir.join("bpftrace-stub.sh");
        let mut f = File::create(&script).unwrap();
        writeln!(
            f,
            "#!/bin/sh\ncase \"$1\" in --version) echo stub; exit 0;; esac\necho 'Attaching 2 probes...'\nsleep 30"
        )
        .unwrap();
        drop(f);
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut rec = BpftraceRecorder::new(&script, &dir);
        let tag = "00-DPSW-I3/I9 defer face: create the bindable reference dpsw";
        let mut ran = false;
        rec.step(tag, &mut || ran = true)
            .expect("a slashed tag must not ENOENT");
        assert!(ran, "the step ran");
        assert!(
            dir.join(format!("capture-{}.mc", sanitize_tag(tag)))
                .exists(),
            "the sanitized capture file is on disk"
        );
    }
}
