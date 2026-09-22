//! Reader for frozen pool-lifecycle ITF traces (`models/traces/families/dpbp/*.itf.json`,
//! emitted by `models/families/dpbp.qnt` module `dpbp_lifecycle` via `pnpm model:freeze-pool`).
//!
//! The allocator-custody twin of the dpni/dprc readers ([`crate::intent::dpni_itf`]): the
//! P3 substrate `models/families/pool_lifecycle.qnt` walks TWO state vars — `s: CoreState`
//! and `conv: Conv` — so this reader decodes both into a `PoolWorld` built from the real
//! [`dpaa2_api::families::pool_lifecycle`] count vocabulary, and `tests/pool_replay.rs`
//! drives the pool-objects task-2.1/2.2 pure functions through the same states. The
//! transition between two states is not named in the ITF; the replayer infers it from the
//! count delta, mirroring `pool_lifecycle`'s action set. dpbp is the representative trio
//! twin — dpmcp/dpcon are byte-identical `FAMILY`/`CEILING = 3` instantiations.
//!
//! The `s` var carries the full `CoreState`; this reader reduces it to the model's count
//! vocabulary (`poolPopulation` and the custody/label split), which is the count↔individual
//! boundary the Rust surface reasons at (pool-objects design D2). A frozen pool population
//! over the family ceiling has no MC-legal model state, so it surfaces here as a parse error
//! — the DPBP-I7 envelope made loud (ADR-0011; the freshness mechanism, mirroring dpni's
//! out-of-envelope create rejection).
//!
//! A `.fail()` directed step froze no variable values (only `#meta`); it decodes to
//! `PoolStep::Refused`, the disabled-guard sentinel the replayer witnesses against the
//! prior world.
//!
//! Mapping (the only place the two encodings are reconciled): every `Family` tag ⇒ the
//! corpus `Family` of the same name (`itf::obj_ref`, strict — ADR-0002 §3, no tag
//! flattening); the `CoreState` custody edges (`parent`/`allocatedBy` as `Option[ObjId]`)
//! and the `Conv` record (`derivedReq`/`managed`/`refusal`) ⇒ the count fields of
//! `PoolCensus` plus the ghost-set size.

use std::collections::BTreeSet;

use serde_json::Value;

use dpaa2_api::core::family::Family;
use dpaa2_api::families::pool_lifecycle::PoolCensus;

use crate::itf::{field, int64, obj_ref, opt_obj_ref, set_items, state_var};

/// dpbp is the representative pool twin; the pool container is the Linux root `dprc.1`, the
/// DPL-born boot object is ordinal 0 (`pool_lifecycle` `born`), and the ceiling is the
/// board's free count (`dpbp.qnt` `CEILING = 3`; DPBP-I7).
const POOL_FAMILY: Family = Family::Dpbp;
const POOL_CONTAINER: (Family, u32) = (Family::Dprc, 1);
const BORN_ORDINAL: u32 = 0;
const CEILING: i64 = 3;

/// One frozen state as the pool-objects count vocabulary: the observed [`PoolCensus`], the
/// derived requirement, the reconciler ghost-set size, the drawn-foreign count, and the
/// observable refusal flag (`pool_lifecycle` vars `s` and `conv`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PoolWorld {
    /// The census the ceiling/convergence predicates judge (the model's `poolPopulation`
    /// split by custody and label).
    pub census: PoolCensus,
    /// The compiled per-family target (`conv.derivedReq`).
    pub derived_req: i64,
    /// The reconciler-grown ghost-set size (`conv.managed.size()`, the model's `managedCount`).
    pub managed_count: i64,
    /// Drawn objects that are neither DPL-born nor reconciler-managed — count-indistinguishable
    /// from a drawn managed one, they fold into [`PoolCensus::managed`] (the conservative bias,
    /// pool-objects design D2). Carried so the replay can state the exact isomorphism.
    pub foreign_drawn: i64,
    /// The observable `ShrinkBelowDraw` refusal flag (`conv.refusal`).
    pub refusal: bool,
}

/// One frozen step of a directed pool run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PoolStep {
    /// A state with both vars decoded.
    World(PoolWorld),
    /// A `.fail()` step: the guard was disabled, so quint froze no variable values. The
    /// replayer witnesses the refused action against the prior world (a census-ceiling or
    /// drawn-victim refusal).
    Refused,
}

/// The reconciler ghost set `conv.managed` as decoded object refs.
fn managed_set(conv: &Value) -> Result<BTreeSet<(Family, u32)>, String> {
    set_items(field(conv, "managed")?)?
        .iter()
        .map(obj_ref)
        .collect()
}

/// Reduces the `CoreState` object map to the model's pool count vocabulary: `poolPopulation`
/// (every `FAMILY` object parented to the pool container) split by custody (`allocatedBy`)
/// and, for the free pool, by label (the DPL-born ordinal, the reconciler ghost set, or an
/// undeclared foreign).
fn census_of(s: &Value, managed: &BTreeSet<(Family, u32)>) -> Result<(PoolCensus, i64), String> {
    let entries = field(s, "objs")?["#map"]
        .as_array()
        .ok_or("objs is not a #map")?;
    let (
        mut population,
        mut free,
        mut drawn,
        mut born,
        mut foreign_free,
        mut foreign_drawn,
        mut born_drawn,
    ) = (0i64, 0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
    for entry in entries {
        let pair = entry.as_array().ok_or("objs entry is not a pair")?;
        let (fam, n) = obj_ref(&pair[0])?;
        // Strict decode of every object's custody edges (ADR-0002 §3), counted only for the
        // pooled family in the pool container — the model's `poolPopulation` filter.
        let parent = opt_obj_ref(field(&pair[1], "parent")?)?;
        let allocated = opt_obj_ref(field(&pair[1], "allocatedBy")?)?;
        if fam != POOL_FAMILY || parent != Some(POOL_CONTAINER) {
            continue;
        }
        population += 1;
        let is_drawn = allocated.is_some();
        let is_managed = managed.contains(&(fam, n));
        if is_drawn {
            drawn += 1;
            if n == BORN_ORDINAL {
                born_drawn += 1; // a drawn DPL-born nets out of the draw guard (V-POOL-6; pool-objects design D3)
            } else if !is_managed {
                foreign_drawn += 1;
            }
        } else {
            free += 1;
            if n == BORN_ORDINAL {
                born += 1;
            } else if !is_managed {
                foreign_free += 1;
            }
        }
    }
    if population > CEILING {
        return Err(format!(
            "pool population {population} exceeds the dpbp ceiling {CEILING} (DPBP-I7): no MC-legal state creates past the census floor"
        ));
    }
    Ok((
        PoolCensus::new(population, free, drawn, born, foreign_free, born_drawn),
        foreign_drawn,
    ))
}

/// One frozen step as a [`PoolStep`].
fn pool_step(state: &Value) -> Result<PoolStep, String> {
    // A `.fail()` step froze only `#meta`; state_var finds no var and this is the sentinel.
    let Ok(conv) = state_var(state, "conv") else {
        return Ok(PoolStep::Refused);
    };
    let s = state_var(state, "s")?;
    let managed = managed_set(conv)?;
    let (census, foreign_drawn) = census_of(s, &managed)?;
    Ok(PoolStep::World(PoolWorld {
        census,
        derived_req: int64(field(conv, "derivedReq")?)?,
        managed_count: i64::try_from(managed.len()).unwrap_or(i64::MAX),
        foreign_drawn,
        refusal: field(conv, "refusal")?
            .as_bool()
            .ok_or("refusal is not a bool")?,
    }))
}

/// Parses a frozen pool-lifecycle trace into its per-state [`PoolStep`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch, an unknown family tag, or a pool
/// population over the family ceiling (the DPBP-I7 envelope — a model↔core divergence).
pub fn parse_pool_trace(json: &str) -> Result<Vec<PoolStep>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(pool_step)
        .collect()
}
