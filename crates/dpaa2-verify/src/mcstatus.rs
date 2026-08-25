//! The Management-Complex command status table as code (openspec task 6.4).
//!
//! Every MC command header carries a STATUS byte the firmware sets and
//! restool prints; a refusal is only pinned once its exact status name is
//! quoted (DPNI-I6, DPMAC-I8). This table is the single machine-readable
//! copy of that enum so probe plans, generated suites, verdicts, and the
//! `docs/baseline/mc-status.md` register all name refusals from the same
//! source.
//!
//! Sources: the code values come from restool's `mc_v10/fsl_mc_cmd.h`
//! (`enum mc_cmd_status`); the printed names from restool's `restool.c`
//! `mc_status_to_string`; the errno mapping from `restool.c`
//! `flib_error_to_mc_status`; and the STATUS byte itself is the command
//! header field documented in the DPAA2 manual §5.5, Table 2.

/// One MC command status: the firmware code, the name restool prints, and
/// the errno restool maps it to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McStatus {
    /// The STATUS-byte value the firmware sets (`enum mc_cmd_status`).
    pub code: u8,
    /// The string restool prints for it (`mc_status_to_string`).
    pub name: &'static str,
    /// The errno restool maps it to (`flib_error_to_mc_status`); 0 for
    /// the two non-error codes.
    pub errno: i32,
}

/// The twelve MC command statuses restool knows, in code order.
pub const MC_STATUSES: [McStatus; 12] = [
    McStatus {
        code: 0x0,
        name: "Command completed successfully",
        errno: 0,
    },
    McStatus {
        code: 0x1,
        name: "Command ready to be processed",
        errno: 0,
    },
    McStatus {
        code: 0x3,
        name: "Authentication error",
        errno: 13,
    }, // EACCES
    McStatus {
        code: 0x4,
        name: "No privilege",
        errno: 1,
    }, // EPERM
    McStatus {
        code: 0x5,
        name: "DMA or I/O error",
        errno: 5,
    }, // EIO
    McStatus {
        code: 0x6,
        name: "Configuration error",
        errno: 6,
    }, // ENXIO
    McStatus {
        code: 0x7,
        name: "Operation timed out",
        errno: 110,
    }, // ETIMEDOUT
    McStatus {
        code: 0x8,
        name: "No resources",
        errno: 119,
    }, // ENAVAIL
    McStatus {
        code: 0x9,
        name: "No memory available",
        errno: 12,
    }, // ENOMEM
    McStatus {
        code: 0xA,
        name: "Device is busy",
        errno: 16,
    }, // EBUSY
    McStatus {
        code: 0xB,
        name: "Unsupported operation",
        errno: 524,
    }, // ENOTSUPP
    McStatus {
        code: 0xC,
        name: "Invalid state",
        errno: 19,
    }, // ENODEV
];

/// The status restool prints as `name`, if any.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static McStatus> {
    MC_STATUSES.iter().find(|s| s.name == name)
}

/// The status the firmware sets as `code`, if any.
#[must_use]
pub fn by_code(code: u8) -> Option<&'static McStatus> {
    MC_STATUSES.iter().find(|s| s.code == code)
}

/// The process exit code restool reports for a command that failed with
/// `errno`: the 8-bit two's complement of `-errno` (255 for EPERM, 250
/// for ENXIO, 137 for ENAVAIL, 240 for EBUSY).
#[must_use]
pub fn exit_code(errno: i32) -> i32 {
    (-errno).rem_euclid(256)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_name_and_by_code_round_trip_every_row() {
        for s in &MC_STATUSES {
            assert_eq!(by_name(s.name), Some(s), "name {:?}", s.name);
            assert_eq!(by_code(s.code), Some(s), "code {:#x}", s.code);
        }
        assert!(by_name("not a status").is_none());
        assert!(by_code(0xFF).is_none());
    }

    #[test]
    fn exit_code_is_the_twos_complement_restool_reports() {
        assert_eq!(exit_code(1), 255); // EPERM
        assert_eq!(exit_code(119), 137); // ENAVAIL
        assert_eq!(exit_code(6), 250); // ENXIO
        assert_eq!(exit_code(16), 240); // EBUSY
        assert_eq!(exit_code(0), 0);
    }
}
