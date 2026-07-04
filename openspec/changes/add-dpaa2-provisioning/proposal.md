## Why

On the SolidRun ClearFog LX2160A, DPAA2 network interfaces do not exist as kernel
netdevs until the NXP Management Complex (MC) firmware is up and DPAA2 objects
(DPNI, DPMAC) have been created and connected via the MC portal, then bound by the
`fsl_dpaa2_eth` driver. This provisioning is *hardware configuration*, must run
before `networkd`/NetworkManager, must be idempotent and reliable across reboots,
and today has no strongly-typed, reconciliation-based tool to perform it. This
change specifies that tool as a small Rust control plane.

## What Changes

- Introduce a **declarative, intent-based control plane** that reads a desired
  topology and reconciles the MC firmware to match it (level-triggered, hardware
  authoritative — no external state store).
- Model the topology as a **graph of MC objects** (each with a provisioning
  lifecycle) keyed by their **stable DPMAC anchors**, not by MC-assigned DPNI
  indices.
- Provide a **hexagonal architecture** with two swappable seams: a northbound
  config source (`topology.toml` now, gNMI/YANG later) and a southbound device
  backend (`restool` shim now, direct `/dev/mc_cmd` ioctl portal later), around a
  **pure reconciliation core**.
- Deliver **stable interface naming** via deterministic/known MAC + stock
  `systemd.link` — udev/networkd is presentation only, never the reconciliation
  trigger.
- Provide a CLI (`scan | ensure | status | dry-run`) exposing observed state and
  the desired-vs-actual delta as a first-class surface.
- Provide **system integration** (MC-ready trigger, ordering before `networkd`,
  `.link` naming) that starts the reconciler causally when `dprc.1` appears.

## Capabilities

### New Capabilities
- `topology-config`: Parse and validate the declarative desired topology and
  convert it into the backend-neutral desired-state model. (crate: `dpaa2-config`)
- `mc-backend`: The southbound port (`McControl` + `KernelControl` traits) and its
  first implementation, the `restool` shim. (crate: `dpaa2-mc`)
- `reconciler`: The backend-neutral domain model (object-graph state machine,
  ownership, drift) and the pure `reconcile()` engine. (crate: `dpaa2-api`)
- `provisioning-cli`: The binary entry points and the imperative shell that drives
  observe → reconcile → act → wait → re-observe to convergence. (crate: `dpaa2-tools`)
- `system-integration`: MC-ready trigger, ordering relative to `networkd`, and
  `systemd.link`-based stable naming. (packaging/units, no single crate)

### Modified Capabilities
- (none — greenfield)

## Impact

- New/filled crates: `dpaa2-config`, `dpaa2-mc`, `dpaa2-api`, `dpaa2-tools`
  (currently empty stubs).
- New workspace dependencies: `serde`/`toml` (config), an error crate, a logging
  crate; `restool` becomes a **runtime dependency** for phase 1 only.
- Workspace lint `unsafe_code = "forbid"` is **retained**; the future ioctl portal
  will relax it in `dpaa2-mc` only (out of scope here).
- Deployment artifacts on Debian: the binary, a `systemd` unit, a udev/`.link`
  trigger, and `/etc/dpaa2/topology.toml`.
- Hands off at the netdev boundary to L3 consumers (e.g. `holo`, `networkd`); no
  coupling downward into the MC.
