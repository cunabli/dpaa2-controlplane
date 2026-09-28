//! The intent-layer half of the harness: the frozen-ITF trace readers for the
//! intent, raw-surface, edits, and dprc-lifecycle alphabets, and the intent-layer
//! copy lint (R11–R16).

/// Reader for frozen dpio-lifecycle ITF traces (pool-objects task 4.1); the seat-typed
/// stepped-machine twin of [`pool_itf`], replayed by `tests/dpio_replay.rs`.
pub mod dpio_itf;
/// Reader for frozen dpni create-surface ITF traces (dpni-typestate task 5.1); the
/// stepped-machine twin of [`dprc_itf`], replayed by `tests/dpni_replay.rs`.
pub mod dpni_itf;
/// Reader for frozen dprc-lifecycle ITF traces (2026-09-15-dprc-encapsulation task 2.3); the
/// stepped-machine twin of [`intent_itf`], replayed by `tests/dprc_replay.rs`.
pub mod dprc_itf;
pub mod edits_itf;
pub mod intent_itf;
/// The intent-layer copy lint (R11–R16): every ADR/COVERAGE enumeration that
/// restates the `models/intent/` model is a linted copy, cross-checked here so a
/// drift fails in CI (ADR-0014).
pub mod lint;
/// Reader for frozen pool-lifecycle ITF traces (pool-objects task 4.1); the allocator-custody
/// stepped-machine twin of [`dpni_itf`], replayed by `tests/pool_replay.rs`.
pub mod pool_itf;
/// Reader, TOML emitter, and verdict matcher for frozen raw-conformance ITF
/// traces (intent-layer task 3.3e).
pub mod raw_itf;
