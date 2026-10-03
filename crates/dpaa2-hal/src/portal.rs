//! The MC-portal ioctl transport: the `/dev/dprc.N` MC-command primitive
//! (ADR-0021).
//!
//! This is the workspace's single unsafe module (ADR-0018). It encodes a
//! 64-byte MC command, sends it through the fsl-mc-uapi `RESTOOL_SEND_MC_COMMAND`
//! ioctl, and decodes the typed response — nothing more. The command vocabulary
//! is a closed sum ([`DpseciRead`]) over exactly the whitelisted dpseci reads
//! (`docs/baseline/mc-ioctl-policy.md`): a write command id is unrepresentable
//! by construction, never forbidden by review.
//!
//! Policy stays out (ADR-0018): the primitive types three outcomes and judges
//! none of them. A completed read is [`Outcome::Response`]; an MC refusal is the
//! header status [`Outcome::Status`]; a transport refusal — the kernel whitelist
//! `-EACCES`, or `ENOENT`/permission on the device node — is the `io::Error` of
//! the `Result`. Retry, tolerance, and error mapping belong to `dpaa2-mc`.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;

/// The `RESTOOL_SEND_MC_COMMAND` ioctl request (`restool/common/fsl_mc_ioctl.h`:47,
/// the request restool sends to a `/dev/dprc.N` node — the legacy `0x4` goes only
/// to `/dev/mc_restool`, `restool/common/fsl_mc_sys.c`:81). `_IOWR('R', 0xE0,
/// struct mc_command)` over the 64-byte command.
const SEND_MC_COMMAND: libc::c_ulong = ioc(IOC_READ | IOC_WRITE, b'R', 0xE0, FRAME_LEN);

const IOC_READ: libc::c_ulong = 2;
const IOC_WRITE: libc::c_ulong = 1;

/// The asm-generic `_IOC(dir, type, nr, size)` encoding, matching `linux/ioctl.h`
/// (size field 14 bits at shift 16, dir 2 bits at shift 30).
const fn ioc(dir: libc::c_ulong, ty: u8, nr: u8, size: usize) -> libc::c_ulong {
    (dir << 30)
        | ((size as libc::c_ulong) << 16)
        | ((ty as libc::c_ulong) << 8)
        | (nr as libc::c_ulong)
}

/// `struct mc_command` size: `u64 header` + `u64 params[7]` (`restool/mc_v10/fsl_mc_cmd.h`:76).
const FRAME_LEN: usize = 64;

// mc_cmd_header byte offsets within the 8-byte header (fsl_mc_cmd.h `struct
// mc_cmd_header`): status is byte 2, token bytes 4..6, cmd_id bytes 6..8, all
// little-endian on the board.
const HDR_STATUS: usize = 2;
const HDR_TOKEN: usize = 4;
const HDR_CMDID: usize = 6;
/// First byte of `params[]` — the header is bytes 0..8.
const PARAMS: usize = 8;

/// `MC_CMD_STATUS_READY`, the status the encoder stamps on an outgoing command
/// (`fsl_mc_cmd.h` `mc_encode_cmd_header`).
const STATUS_READY: u8 = 0x1;
/// `MC_CMD_STATUS_OK`: the firmware's completed-successfully status.
const STATUS_OK: u8 = 0x0;

// dpseci command ids = `DPSECI_CMD_V1(id)` = `(id << 4) | 1` (fsl_dpseci_cmd.h),
// each matched against its `fsl_mc_accepted_cmds[]` row in
// `docs/baseline/mc-ioctl-policy.md`.
/// `DPSECI_CMDID_OPEN` — whitelist row 42 (OPEN).
const CMDID_OPEN: u16 = 0x8091;
/// `DPSECI_CMDID_GET_ATTR` — whitelist row 38 (`GET_ATTR`).
const CMDID_GET_ATTR: u16 = 0x0041;
/// `DPSECI_CMDID_GET_API_VERSION` — whitelist row 43 (`GET_API_VERSION`).
const CMDID_GET_API_VERSION: u16 = 0xa091;
/// `DPSECI_CMDID_GET_TX_QUEUE` — whitelist row 24 (`DPSECI_GET_TX_QUEUE`).
const CMDID_GET_TX_QUEUE: u16 = 0x1971;
/// `DPSECI_CMDID_CLOSE` — whitelist row 41 (CLOSE).
const CMDID_CLOSE: u16 = 0x8001;

/// An MC authentication token, returned by [`DpseciRead::Open`] and passed to the
/// token-scoped reads. Distinct from a raw `u16` so a token is never confused
/// with an object id or a queue index; it is only ever minted from an `Open`
/// response, never fabricated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token(u16);

impl Token {
    /// The raw 16-bit token value, for carrying it back into a later command.
    #[must_use]
    pub fn value(self) -> u16 {
        self.0
    }
}

/// The closed command vocabulary: exactly the whitelisted dpseci reads this
/// primitive encodes (`docs/baseline/mc-ioctl-policy.md`, dpseci info row). No
/// variant carries a create/destroy/set id, so a write is unrepresentable —
/// ADR-0021's fence is the type, not a review rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpseciRead {
    /// Open a control session on a dpseci object by id; the response carries the
    /// [`Token`] every later command needs.
    Open {
        /// The dpseci object id to open.
        dpseci_id: u32,
    },
    /// Read the object attributes (id, queue counts, options mask).
    GetAttributes {
        /// The session token from [`DpseciRead::Open`].
        token: Token,
    },
    /// Read the dpseci API version (no token — a module-global query).
    GetApiVersion,
    /// Read one Tx queue's attributes (fqid, priority).
    GetTxQueue {
        /// The session token from [`DpseciRead::Open`].
        token: Token,
        /// The queue index, relative to the object's configured count.
        queue: u8,
    },
    /// Close the control session.
    Close {
        /// The session token from [`DpseciRead::Open`].
        token: Token,
    },
}

/// dpseci attributes decoded from a `GET_ATTR` response (`struct
/// dpseci_rsp_get_attr`, `fsl_dpseci_cmd.h`). `options` is the raw mask; typing it
/// into a flag vocabulary is `dpaa2-api`'s job, not the policy-free HAL's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpseciAttributes {
    /// The dpseci object id.
    pub id: u32,
    /// Number of queues towards the SEC.
    pub num_tx_queues: u8,
    /// Number of queues back from the SEC.
    pub num_rx_queues: u8,
    /// The raw options mask (e.g. `DPSECI_OPT_HAS_CG`).
    pub options: u32,
}

/// The dpseci API version from a `GET_API_VERSION` response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiVersion {
    /// Major version.
    pub major: u16,
    /// Minor version.
    pub minor: u16,
}

/// One Tx queue's attributes from a `GET_TX_QUEUE` response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxQueue {
    /// Virtual FQID for frames sent to the SEC.
    pub fqid: u32,
    /// SEC processing priority for the queue.
    pub priority: u8,
}

/// A decoded response, one variant per [`DpseciRead`] command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadResponse {
    /// An [`DpseciRead::Open`] session token.
    Opened(Token),
    /// [`DpseciRead::GetAttributes`] attributes.
    Attributes(DpseciAttributes),
    /// [`DpseciRead::GetApiVersion`] version.
    ApiVersion(ApiVersion),
    /// [`DpseciRead::GetTxQueue`] queue attributes.
    TxQueue(TxQueue),
    /// [`DpseciRead::Close`] completed (no payload).
    Closed,
}

/// The MC command status, mirroring `enum mc_cmd_status` (`fsl_mc_cmd.h`) and the
/// register in `docs/baseline/mc-status.md`. An unlisted firmware code is carried
/// verbatim as [`McStatus::Unknown`] so no refusal is silently dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McStatus {
    /// `0x1` ready to be processed.
    Ready,
    /// `0x3` authentication error.
    AuthError,
    /// `0x4` no privilege.
    NoPrivilege,
    /// `0x5` DMA or I/O error.
    DmaError,
    /// `0x6` configuration error.
    ConfigError,
    /// `0x7` operation timed out.
    Timeout,
    /// `0x8` no resources.
    NoResource,
    /// `0x9` no memory available.
    NoMemory,
    /// `0xA` device is busy.
    Busy,
    /// `0xB` unsupported operation.
    UnsupportedOp,
    /// `0xC` invalid state.
    InvalidState,
    /// A status code outside the known vocabulary, carried verbatim.
    Unknown(u8),
}

impl McStatus {
    fn from_code(code: u8) -> Self {
        match code {
            0x1 => Self::Ready,
            0x3 => Self::AuthError,
            0x4 => Self::NoPrivilege,
            0x5 => Self::DmaError,
            0x6 => Self::ConfigError,
            0x7 => Self::Timeout,
            0x8 => Self::NoResource,
            0x9 => Self::NoMemory,
            0xA => Self::Busy,
            0xB => Self::UnsupportedOp,
            0xC => Self::InvalidState,
            other => Self::Unknown(other),
        }
    }
}

/// The two outcomes the ioctl can return once it succeeds as a transport. The
/// third outcome — a transport refusal — is the `Err` of the `io::Result`
/// [`McPortal::execute`] returns, never a value here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The MC answered `OK`: a decoded response.
    Response(ReadResponse),
    /// The MC answered with a non-OK status (a refusal it judged itself).
    Status(McStatus),
}

impl DpseciRead {
    /// Encode this command into its 64-byte `struct mc_command` frame, mirroring
    /// the flib encoders in `restool/mc_v10/dpseci.c` (command flags zero, as
    /// restool sends for these reads).
    fn encode(self) -> [u8; FRAME_LEN] {
        let (cmdid, token) = match self {
            Self::Open { .. } => (CMDID_OPEN, 0),
            Self::GetAttributes { token } => (CMDID_GET_ATTR, token.0),
            Self::GetApiVersion => (CMDID_GET_API_VERSION, 0),
            Self::GetTxQueue { token, .. } => (CMDID_GET_TX_QUEUE, token.0),
            Self::Close { token } => (CMDID_CLOSE, token.0),
        };
        let mut frame = [0u8; FRAME_LEN];
        frame[HDR_STATUS] = STATUS_READY;
        frame[HDR_TOKEN..HDR_TOKEN + 2].copy_from_slice(&token.to_le_bytes());
        frame[HDR_CMDID..HDR_CMDID + 2].copy_from_slice(&cmdid.to_le_bytes());
        match self {
            // struct dpseci_cmd_open: dpseci_id at params byte 0.
            Self::Open { dpseci_id } => {
                frame[PARAMS..PARAMS + 4].copy_from_slice(&dpseci_id.to_le_bytes());
            }
            // struct dpseci_cmd_get_queue: pad[5] then queue at params byte 5.
            Self::GetTxQueue { queue, .. } => frame[PARAMS + 5] = queue,
            Self::GetAttributes { .. } | Self::GetApiVersion | Self::Close { .. } => {}
        }
        frame
    }

    /// Decode the response frame for this command (response layouts in
    /// `restool/mc_v10/fsl_dpseci_cmd.h`). The caller invokes this only after the
    /// header status read as OK.
    fn decode_response(self, frame: &[u8; FRAME_LEN]) -> ReadResponse {
        match self {
            Self::Open { .. } => ReadResponse::Opened(Token(le16(frame, HDR_TOKEN))),
            // struct dpseci_rsp_get_attr: id@0, num_tx@8, num_rx@9, options@16.
            Self::GetAttributes { .. } => ReadResponse::Attributes(DpseciAttributes {
                id: le32(frame, PARAMS),
                num_tx_queues: frame[PARAMS + 8],
                num_rx_queues: frame[PARAMS + 9],
                options: le32(frame, PARAMS + 16),
            }),
            // struct dpseci_rsp_get_api_version: major@0, minor@2.
            Self::GetApiVersion => ReadResponse::ApiVersion(ApiVersion {
                major: le16(frame, PARAMS),
                minor: le16(frame, PARAMS + 2),
            }),
            // struct dpseci_rsp_get_tx_queue: pad@0, fqid@4, priority@8.
            Self::GetTxQueue { .. } => ReadResponse::TxQueue(TxQueue {
                fqid: le32(frame, PARAMS + 4),
                priority: frame[PARAMS + 8],
            }),
            Self::Close { .. } => ReadResponse::Closed,
        }
    }
}

fn le16(frame: &[u8; FRAME_LEN], at: usize) -> u16 {
    u16::from_le_bytes([frame[at], frame[at + 1]])
}

fn le32(frame: &[u8; FRAME_LEN], at: usize) -> u32 {
    u32::from_le_bytes([frame[at], frame[at + 1], frame[at + 2], frame[at + 3]])
}

/// Split a completed-ioctl frame into its [`Outcome`]: the header status decides
/// between a decoded response and an MC refusal. Pure, so every outcome is
/// fixture-testable without a device.
fn interpret(command: DpseciRead, frame: &[u8; FRAME_LEN]) -> Outcome {
    let status = frame[HDR_STATUS];
    if status == STATUS_OK {
        Outcome::Response(command.decode_response(frame))
    } else {
        Outcome::Status(McStatus::from_code(status))
    }
}

/// A handle on one `/dev/dprc.N` MC portal.
#[derive(Debug)]
pub struct McPortal {
    device: File,
}

impl McPortal {
    /// Open the portal device node at `path` (`/dev/dprc.N`) for MC commands.
    ///
    /// # Errors
    ///
    /// Propagates the open error verbatim — `NotFound` for a missing node,
    /// `PermissionDenied` for an unprivileged caller. Both are transport
    /// refusals; what they mean is `dpaa2-mc`'s policy.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let device = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self { device })
    }

    /// Send one read command and return its [`Outcome`].
    ///
    /// # Errors
    ///
    /// Returns the transport refusal as an `io::Error` — the kernel whitelist
    /// `-EACCES`, or any other ioctl failure. An MC-judged refusal is not an
    /// error here: it rides back as [`Outcome::Status`].
    pub fn execute(&self, command: DpseciRead) -> io::Result<Outcome> {
        let mut frame = command.encode();
        self.ioctl(&mut frame)?;
        Ok(interpret(command, &frame))
    }

    #[allow(unsafe_code)]
    fn ioctl(&self, frame: &mut [u8; FRAME_LEN]) -> io::Result<()> {
        // SAFETY: `frame` is exactly the FRAME_LEN bytes the request code encodes
        // as the command size, and `_IOWR` has the kernel read then write only
        // those bytes for the call's duration; `device` is an open /dev/dprc.N
        // portal fd, live for the whole call. This is the one unsafe site the
        // workspace permits (ADR-0021).
        let rc =
            unsafe { libc::ioctl(self.device.as_raw_fd(), SEND_MC_COMMAND, frame.as_mut_ptr()) };
        if rc == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_ids_are_the_v1_macro_and_match_the_whitelist() {
        // DPSECI_CMD_V1(id) = (id << 4) | 1, and each id is its whitelist row.
        let v1 = |id: u16| (id << 4) | 1;
        assert_eq!(CMDID_OPEN, v1(0x809));
        assert_eq!(CMDID_GET_ATTR, v1(0x004));
        assert_eq!(CMDID_GET_API_VERSION, v1(0xa09));
        assert_eq!(CMDID_GET_TX_QUEUE, v1(0x197));
        assert_eq!(CMDID_CLOSE, v1(0x800));
        // Whitelist match rule `cmdid & mask == value` (mc-ioctl-policy.md).
        assert_eq!(CMDID_OPEN & 0xfc00, 0x8000); // row 42 OPEN
        assert_eq!(CMDID_GET_ATTR & 0xfff0, 0x0040); // row 38 GET_ATTR
        assert_eq!(CMDID_GET_API_VERSION & 0xfc00, 0xa000); // row 43 GET_API_VERSION
        assert_eq!(CMDID_GET_TX_QUEUE & 0xfff0, 0x1970); // row 24 DPSECI_GET_TX_QUEUE
        assert_eq!(CMDID_CLOSE & 0xfff0, 0x8000); // row 41 CLOSE
    }

    #[test]
    fn send_command_request_code_matches_fsl_mc_ioctl_h() {
        // _IOWR('R', 0xE0, struct mc_command), size 64.
        assert_eq!(SEND_MC_COMMAND, 0xc040_52e0);
    }

    fn header(frame: &[u8; FRAME_LEN]) -> (u8, u16, u16) {
        (
            frame[HDR_STATUS],
            le16(frame, HDR_TOKEN),
            le16(frame, HDR_CMDID),
        )
    }

    #[test]
    fn open_encodes_the_golden_frame() {
        let frame = DpseciRead::Open {
            dpseci_id: 0x0102_0304,
        }
        .encode();
        assert_eq!(header(&frame), (STATUS_READY, 0, CMDID_OPEN));
        assert_eq!(&frame[PARAMS..PARAMS + 4], &[0x04, 0x03, 0x02, 0x01]);
        assert!(frame[PARAMS + 4..].iter().all(|&b| b == 0));
    }

    #[test]
    fn get_attributes_encodes_the_golden_frame() {
        let frame = DpseciRead::GetAttributes {
            token: Token(0xABCD),
        }
        .encode();
        assert_eq!(header(&frame), (STATUS_READY, 0xABCD, CMDID_GET_ATTR));
        assert!(frame[PARAMS..].iter().all(|&b| b == 0));
    }

    #[test]
    fn get_api_version_encodes_the_golden_frame() {
        let frame = DpseciRead::GetApiVersion.encode();
        assert_eq!(header(&frame), (STATUS_READY, 0, CMDID_GET_API_VERSION));
        assert!(frame[PARAMS..].iter().all(|&b| b == 0));
    }

    #[test]
    fn get_tx_queue_encodes_the_golden_frame() {
        let frame = DpseciRead::GetTxQueue {
            token: Token(0x11),
            queue: 7,
        }
        .encode();
        assert_eq!(header(&frame), (STATUS_READY, 0x11, CMDID_GET_TX_QUEUE));
        // Queue sits at params byte 5 (pad[5] then queue).
        assert_eq!(frame[PARAMS + 5], 7);
        assert_eq!(frame[PARAMS], 0);
    }

    #[test]
    fn close_encodes_the_golden_frame() {
        let frame = DpseciRead::Close { token: Token(0x22) }.encode();
        assert_eq!(header(&frame), (STATUS_READY, 0x22, CMDID_CLOSE));
        assert!(frame[PARAMS..].iter().all(|&b| b == 0));
    }

    // Build a response frame: status byte plus a params payload from byte PARAMS.
    fn response(status: u8, params: &[(usize, &[u8])]) -> [u8; FRAME_LEN] {
        let mut frame = [0u8; FRAME_LEN];
        frame[HDR_STATUS] = status;
        for (at, bytes) in params {
            frame[PARAMS + at..PARAMS + at + bytes.len()].copy_from_slice(bytes);
        }
        frame
    }

    #[test]
    fn outcome_one_decodes_an_ok_attributes_response() {
        // id=5, num_tx=16, num_rx=16, options=DPSECI_OPT_HAS_CG (0x20).
        let frame = response(
            STATUS_OK,
            &[
                (0, &5u32.to_le_bytes()),
                (8, &[16]),
                (9, &[16]),
                (16, &0x20u32.to_le_bytes()),
            ],
        );
        let outcome = interpret(DpseciRead::GetAttributes { token: Token(1) }, &frame);
        assert_eq!(
            outcome,
            Outcome::Response(ReadResponse::Attributes(DpseciAttributes {
                id: 5,
                num_tx_queues: 16,
                num_rx_queues: 16,
                options: 0x20,
            }))
        );
    }

    #[test]
    fn outcome_one_decodes_api_version_and_tx_queue_and_token() {
        let ver = response(
            STATUS_OK,
            &[(0, &5u16.to_le_bytes()), (2, &4u16.to_le_bytes())],
        );
        assert_eq!(
            interpret(DpseciRead::GetApiVersion, &ver),
            Outcome::Response(ReadResponse::ApiVersion(ApiVersion { major: 5, minor: 4 }))
        );

        let q = response(STATUS_OK, &[(4, &0xdead_beefu32.to_le_bytes()), (8, &[2])]);
        assert_eq!(
            interpret(
                DpseciRead::GetTxQueue {
                    token: Token(1),
                    queue: 0
                },
                &q
            ),
            Outcome::Response(ReadResponse::TxQueue(TxQueue {
                fqid: 0xdead_beef,
                priority: 2
            }))
        );

        // Open reads its token from the response header, not params.
        let mut opened = [0u8; FRAME_LEN];
        opened[HDR_TOKEN..HDR_TOKEN + 2].copy_from_slice(&0x0777u16.to_le_bytes());
        assert_eq!(
            interpret(DpseciRead::Open { dpseci_id: 1 }, &opened),
            Outcome::Response(ReadResponse::Opened(Token(0x0777)))
        );
        assert_eq!(Token(0x0777).value(), 0x0777);
    }

    #[test]
    fn outcome_two_surfaces_a_non_ok_status() {
        // 0x4 No privilege (mc-status.md) — an MC refusal, not a transport error.
        let frame = response(0x4, &[]);
        assert_eq!(
            interpret(DpseciRead::GetAttributes { token: Token(1) }, &frame),
            Outcome::Status(McStatus::NoPrivilege)
        );
        // An unlisted code rides back verbatim.
        assert_eq!(
            interpret(DpseciRead::GetApiVersion, &response(0x2, &[])),
            Outcome::Status(McStatus::Unknown(0x2))
        );
    }

    #[test]
    fn outcome_three_is_the_transport_error_on_a_missing_node() {
        // The whitelist -EACCES and a permission/ENOENT on the node share this
        // path; a missing node exercises it with no board (the live face is
        // task 5.2, dpseci-typestate).
        let err = McPortal::open("/dev/dprc.nonexistent-fixture").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
