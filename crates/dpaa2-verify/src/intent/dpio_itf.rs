//! Reader for frozen dpio-lifecycle ITF traces (`models/traces/families/dpio/*.itf.json`,
//! emitted by `models/families/dpio.qnt` module `dpio_lifecycle` via `pnpm model:freeze-dpio`).
//!
//! The seat-typed twin of the pool reader ([`crate::intent::pool_itf`]): the unpooled dpio
//! machine walks TWO state vars — `s: CoreState` and `dpioSeats: ObjId -> DpioSeat` — so
//! this reader decodes both into a `DpioWorld` built from the real
//! [`dpaa2_api::families::dpio`] seat types, and `tests/dpio_replay.rs` drives the
//! pool-objects task-2.3 seat surface through the same states. The transition between two
//! states is inferred from the seat delta, mirroring `dpio_lifecycle`'s action set.
//!
//! Each seat decodes through the refined constructors: `numPriorities` feeds
//! `Priorities::new` (the `1..=8` MC gate; `dpio.qnt` `createDpioAt`), so a frozen value
//! outside the envelope has no constructor and fails the parse — a model↔core divergence
//! made loud (the freshness mechanism), and `mode`/`regime` decode to the `ChannelMode` /
//! `SeatRegime` sums (ADR-0002 §3, no tag flattening). The `probed` observation flag is
//! replayed through `DpioSeat::mark_probed`.
//!
//! The dpio→dpmcp probe draw is adapter-procedural with no api-side surface
//! (pool-objects design D4), so the `s` var reduces to a minimal custody view
//! (`CoreObj`) that lets the probe runs replay state conformance only. A `.fail()` step
//! froze no values and decodes to
//! `DpioStep::Refused`.

use std::collections::BTreeMap;

use serde_json::Value;

use dpaa2_api::core::family::Family;
use dpaa2_api::families::dpio::{ChannelMode, DpioCfg, DpioSeat, Priorities, SeatRegime};

use crate::itf::{field, num, obj_ref, opt_obj_ref, state_var, tag};

/// A `ChannelMode` sum value (`dpio.qnt` `type ChannelMode`).
fn channel_mode(v: &Value) -> Result<ChannelMode, String> {
    match tag(v)? {
        "LocalChannel" => Ok(ChannelMode::LocalChannel),
        "NoChannel" => Ok(ChannelMode::NoChannel),
        other => Err(format!("unknown ChannelMode tag `{other}`")),
    }
}

/// A `SeatRegime` sum value (`dpio.qnt` `type SeatRegime`).
fn seat_regime(v: &Value) -> Result<SeatRegime, String> {
    match tag(v)? {
        "KernelSeat" => Ok(SeatRegime::KernelSeat),
        "DpdkSeat" => Ok(SeatRegime::DpdkSeat),
        other => Err(format!("unknown SeatRegime tag `{other}`")),
    }
}

/// The reconciler-observable custody slice of one `CoreState` object — the minimal `s`
/// view the probe runs replay against (the dpmcp draw is adapter-procedural, pool-objects design D4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CoreObj {
    /// Whether the object is plugged.
    pub plugged: bool,
    /// The consumer holding it, if drawn (`allocatedBy`).
    pub allocated_by: Option<(Family, u32)>,
}

/// One frozen state: the seat register keyed by dpio ordinal and the minimal core custody
/// view (`dpio_lifecycle` vars `dpioSeats` and `s`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DpioWorld {
    /// The seats by dpio ordinal, each the real refined [`DpioSeat`].
    pub seats: BTreeMap<u32, DpioSeat>,
    /// The core objects by ref (dpmcp/dpio custody), the probe-draw state view.
    pub cores: BTreeMap<(Family, u32), CoreObj>,
}

impl DpioWorld {
    /// The seats as a flat slice, the shape the [`dpaa2_api::families::dpio`] seat arithmetic
    /// (`seat_count`/`admit_seat`) consumes.
    #[must_use]
    pub fn seat_vec(&self) -> Vec<DpioSeat> {
        self.seats.values().copied().collect()
    }
}

/// One frozen step of a directed dpio run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DpioStep {
    /// A state with both vars decoded.
    World(DpioWorld),
    /// A `.fail()` step: a disabled guard (a seat-ceiling `-ERANGE` or an exhausted probe),
    /// frozen with no values. The replayer witnesses it against the prior world.
    Refused,
}

/// One `DpioSeat` record (`dpio.qnt` `type DpioSeat`) as the refined [`DpioSeat`]. The
/// `numPriorities` decode goes through [`Priorities::new`], so a value outside `1..=8` has
/// no constructor and fails here (the envelope freshness check).
fn dpio_seat(v: &Value) -> Result<DpioSeat, String> {
    let raw = num(field(v, "numPriorities")?)?;
    let p = u8::try_from(raw).map_err(|_| format!("numPriorities {raw} exceeds a byte"))?;
    let priorities = Priorities::new(p).map_err(|e| format!("numPriorities: {e}"))?;
    let cfg = DpioCfg {
        mode: channel_mode(field(v, "mode")?)?,
        priorities,
    };
    let mut seat = DpioSeat::create(cfg, seat_regime(field(v, "regime")?)?);
    if field(v, "probed")?
        .as_bool()
        .ok_or("probed is not a bool")?
    {
        seat.mark_probed();
    }
    Ok(seat)
}

/// The seat register (`dpioSeats: ObjId -> DpioSeat`) keyed by dpio ordinal.
fn seats(v: &Value) -> Result<BTreeMap<u32, DpioSeat>, String> {
    v["#map"]
        .as_array()
        .ok_or("dpioSeats is not a #map")?
        .iter()
        .map(|entry| {
            let pair = entry.as_array().ok_or("dpioSeats entry is not a pair")?;
            let (_, n) = obj_ref(&pair[0])?;
            Ok((n, dpio_seat(&pair[1])?))
        })
        .collect()
}

/// The minimal core custody view of the `CoreState` object map.
fn cores(s: &Value) -> Result<BTreeMap<(Family, u32), CoreObj>, String> {
    field(s, "objs")?["#map"]
        .as_array()
        .ok_or("objs is not a #map")?
        .iter()
        .map(|entry| {
            let pair = entry.as_array().ok_or("objs entry is not a pair")?;
            Ok((
                obj_ref(&pair[0])?,
                CoreObj {
                    plugged: field(&pair[1], "plugged")?
                        .as_bool()
                        .ok_or("plugged is not a bool")?,
                    allocated_by: opt_obj_ref(field(&pair[1], "allocatedBy")?)?,
                },
            ))
        })
        .collect()
}

/// One frozen step as a [`DpioStep`].
fn dpio_step(state: &Value) -> Result<DpioStep, String> {
    // A `.fail()` step froze only `#meta`.
    let Ok(seats_v) = state_var(state, "dpioSeats") else {
        return Ok(DpioStep::Refused);
    };
    Ok(DpioStep::World(DpioWorld {
        seats: seats(seats_v)?,
        cores: cores(state_var(state, "s")?)?,
    }))
}

/// Parses a frozen dpio-lifecycle trace into its per-state [`DpioStep`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch, an unknown mode/regime/family tag,
/// or a `numPriorities` outside the `1..=8` MC create range (a model↔core divergence).
pub fn parse_dpio_trace(json: &str) -> Result<Vec<DpioStep>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(dpio_step)
        .collect()
}
