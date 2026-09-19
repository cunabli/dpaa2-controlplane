# ADR-0018: The module tree speaks sans-io hexagonal

- **Status:** Accepted — grilling session 2026-09-17 (bead
  dpaa2-controlplane-yfg; no openspec change because as purely
  mechanical moves carry no spec nor behavioral change)
- **Date:** 2026-09-17
- **Supersedes / relates to:** the intent-layer review's structure
  leave-it ("keep-flat for dpaa2-api/config/mc/tools", synthesis of
  2026-09-06-intent-layer) — superseded for dpaa2-api, re-affirmed
  with named triggers for the adapter crates; ADR-0014 (the copy-drift
  law that makes the models/ mirror load-bearing);
  2026-09-15-dprc-encapsulation task 2.1 (the module-namespaced
  `dpaa2_api::dprc::*` precedent this ADR generalizes);
  bead dpaa2-controlplane-26v
  (the dpaa2-verify board/ vs intent/ split, the same principle
  applied to the verify crate)

## Context

dpaa2-api is the hexagon's core: the domain model, the trait seams,
and the pure reconcile functions. It is ~10k lines across 17 flat
`src/` modules, and tiles #5–8 multiply per-family sums, typestates,
refusal payloads, and planners. Restructuring after those tiles land
means moving everything twice; restructuring without a recorded shape
means every tile re-litigates where its pieces go. The architecture
is sans-io hexagonal by declaration (README, crate docs), but a flat
module list cannot show it: nothing in the tree distinguishes a
domain entity from a trait seam an adapter must implement, and the
critical seams dilute into the file list.

The quint side is already shaped: `models/core/` holds the generic
domain machine (types, lifecycle, containment, pools, companions) and
`models/families/` holds all 16 per-object-family models. The R14
rust-copies ledger and ADR-0014 tie the Rust and quint sides
together, so a Rust tree that mirrors the model tree makes the
correspondence self-evident and lintable.

## Decision

dpaa2-api's module tree names the hexagon's parts. Root is `lib.rs`
only; every module lives in one of five namespaces:

- **`core/`** — the pure domain: entities, value types, shared sums,
  errors, and the macros shared across MC object families (today:
  types, error, model, family, port, inventory). Nothing in `core/`
  names IO or an adapter, ever. One shared home — there is no
  sibling `model/` namespace to file against, so "core or model?" is
  never a question.
- **`contract/`** — the trait seams adapters implement or consume
  (`McControl` and kin), plus `contract/fake.rs`, the in-crate
  reference implementation for tests. The name is deliberate: these
  traits are quint-able contracts — their obligations are what
  `models/core/` constrains, dpaa2-verify replays frozen traces
  through their implementations, and the tile-#10 differential gate
  will hold two implementations (restool shim, ioctl portal) to the
  same surface. Not `ports/` (a port is a domain noun in DPAA2 —
  network ports, `core/port`), not `io/` (DPIO is an object family,
  and a sans-io crate must not export `::io`), not `adapters/`
  (adapters are the implementations and live outside this crate).
- **`families/`** — per-object-family vocabulary: sums, typestates,
  verb surfaces, and (as tiles land) per-family refusal payload
  types. Mirrors `models/families/` one-to-one: `families/dprc.rs` ↔
  `families/dprc.qnt`. dprc migrates first (the task-2.1 precedent);
  dpni, the pool quartet, dpmac, dpseci land here as tiles #5–8
  arrive.
- **`plan/`** — the pure planning half, per-family where the planner
  is family-specific: plan, matcher, reconcile at the namespace root;
  `plan/dprc.rs` (today's dprc_plan) as the pattern for future
  family planners.
- **`intent/`** — the intent-compile pipeline: intent, derive,
  compiled, and the refusal machinery (see below).

**Roots declare, files define.** Module files are mod.rs style — a
`foo.rs` beside a `foo/` is denied workspace-wide
(`clippy::self_named_module_files`). A namespace's `mod.rs` carries
only its module doc, the `mod`/`pub mod` declarations, and the
re-exports that fix its public paths; the items live in subject-named
files that own a domain — `contract/mc.rs` holds `McControl` and the
MC-side seams to come — never one file per item, never a
`types`/`misc` grab-bag.

**The public API is namespaced-only.** `dpaa2_api::contract::McControl`,
`dpaa2_api::families::dprc::*` — no flat root re-exports. This is a
one-time break of today's flat paths, paid before any release,
precisely so the surface never moves again after one.
`clippy::wildcard_imports` warns at the crate root; `use super::*`
survives only in `#[cfg(test)] mod tests` blocks.

**Refusals split by construct, land by family.** The `Refusal`,
`Referrer`, and `Warning` enums and shared machinery stay in
`intent/refuse.rs` — their variant lists are triple-linted against
`models/intent/refuse.qnt` and docs (ADR-0014), and restructuring the
enums would drag the models in lockstep, which this change refuses.
What splits out now is per-construct validation logic and fixtures.
The landing convention for tiles #5–8: each new family's refusal
payload types live in `families/<f>.rs`, referenced by new top-level
`Refusal` variants, so no future family grows `refuse.rs` again.

## The broad map (all six crates)

- **dpaa2-api** — the hexagon: `core/` and `contract/` are the
  stable heart; `families/`, `plan/`, `intent/` are the domain growth
  areas. Depends on no adapter.
- **dpaa2-mc** — southbound adapter; implements
  `dpaa2_api::contract::*` over the restool shim and dpaa2-hal's
  kernel primitives, and owns all policy (retry, tolerance, error
  mapping). Stays flat (5 modules). Restructure trigger: tile #10
  (`mc-portal-backend`), when `restool/` and `portal/` become sibling
  backends behind the unchanged contract, differential-tested.
- **dpaa2-hal** — typed, policy-free primitives for the kernel
  interfaces the hardware is reached through (ADR-0001 §6; carved
  from dpaa2-mc in commit 5139cf2): fsl-mc sysfs today; VFIO,
  netlink, and the MC-portal ioctl transport join only with the
  change that consumes them. Zero dependencies, plain `io::Error`,
  no trait seams — those belong above. It follows the embedded-Rust
  HAL pattern (embedded-hal, esp-hal and kin): the one crate where
  low-level code with narrow constraints — `unsafe`, interior
  mutability, invariant-guarding wrappers — is allowed to live, so
  everything above it builds on a safe typed surface and the
  workspace's unsafe review burden stays in one place (roadmap
  row 10's ioctl transport, the single unsafe module, lands here).
- **dpaa2-config** — northbound adapter; parses declarative intent
  into the backend-neutral model. Stays flat (3 modules).
  Restructure trigger: tile #5+, when per-family intent options give
  `schema`/`parse` a families echo worth mirroring.
- **dpaa2-tools** — imperative shell driving convergence. Stays flat
  (6 modules). Restructure trigger: a second frontend (the TUI).
- **dpaa2-verify** — test harness, already split by this principle
  (bead dpaa2-controlplane-26v, commit 8cdd61b): `board/` renders
  board-mutating artifacts inside the ADR-0003 envelope, `intent/` is
  pure offline oracles, `itf.rs` the shared substrate.

## Consequences

- Tile #5–8 artifacts cite this layout; their pieces have a named
  home before they exist (bead yfg's original purpose).
- The seams cannot dilute: an adapter's obligation is whatever
  `contract/` exports, and the import path says so at every use site.
- The models/ mirror becomes structural: `families/<f>.rs` ↔
  `families/<f>.qnt` is checkable by path, not by convention.
- The one-time public-path break lands in the yfg parcels, gated by
  an API-surface diff review, the full suite, quality floor, and the
  frozen-trace replays staying green.
- Adapter crates do not restructure speculatively; each carries a
  named trigger, recorded here so the decision is not re-litigated
  per tile.
- The restool shim's `create_dpni` companion-provisioning chain (dpio
  top-up, the dpbp/dpmcp/dpcon dependencies, rollback) is southbound
  *policy*, not a 1:1 firmware verb; when tile #10 adds the portal
  backend the chain lives once — a policy module or trait-default —
  shared by both backends rather than forked (dpni-typestate review
  synthesis row 12).
