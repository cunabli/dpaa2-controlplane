# dpaa2-controlplane

[![Crates.io](https://img.shields.io/crates/v/dpaa2-controlplane.svg)](https://crates.io/crates/dpaa2-controlplane)
[![Docs.rs](https://docs.rs/dpaa2-controlplane/badge.svg)](https://docs.rs/dpaa2-controlplane)
[![CI](https://github.com/cunabli/dpaa2-controlplane/workflows/CI/badge.svg)](https://github.com/cunabli/dpaa2-controlplane/actions)

A declarative, intent-based control plane for the DPAA2 dataplane on NXP's LX2160A.
You describe *which physical port should carry which stably-named network interface*;
the tool converges the NXP Management Complex (MC) to
that intent — creating and wiring the DPNI↔DPMAC objects `fsl_dpaa2_eth` needs, then
renaming the resulting netdev — idempotently, at every boot, with no persisted state.

It replaces hand-run `restool`/`ls-addni` sequences with a level-triggered reconciler:
state is read fresh each pass and compared against `topology.toml`, so a partial or
failed run self-heals on the next one and interface names never drift from intent.

## How it works

- **Intent in, convergence out.** `topology.toml` declares network constructs — tenants
  (a dataplane with a core budget), ports keyed by their stable DPMAC anchor, links,
  fabrics, crypto engines — never MC object counts; `dpaa2ctl ensure` compiles that
  intent into the full object plan and drives the board toward it, safe to re-run.
- **Identity by connection edge, not index.** A managed interface is matched by its
  DPMAC edge, so an MC-reassigned DPNI index across reboots still resolves correctly.
- **Pure core, thin adapters.** A hardware-free reconciler (`reconcile(desired,
  observed) -> Plan`) sits behind trait seams, with `restool`/sysfs and TOML as
  swappable adapters — so the whole loop is testable without a board.
- **Stable naming via stock `systemd.link`.** Names are generated at runtime and applied
  during early boot; no custom udev helpers, no marker files.

## Example

A `topology.toml` states capacity and who consumes it — never a dpio, dpbp, dpcon,
dpmcp, queue, or worker count. The compiler derives every MC object and size, and
`dpaa2ctl dry-run` prints each one with the rule and construct it came from.

```toml
[intent]
schema = 1                 # the version hook; the only document-level property today

# A userspace poll-mode dataplane (VPP/DPDK) in its own isolated container.
[[tenant]]
name = "router"
dataplane = "userspace-poll"   # kernel-netlink | userspace-poll | userspace-event
max_cores = 16                 # a budget the derived thread count must fit under
isolation = "isolated"         # public | restricted | isolated (default isolated)

# Two 10G ports the router terminates, each anchored on a stable DPMAC.
[[port]]
name = "wan0"
dpmac = "dpmac.9"
rate = 10000                   # Mbps, the unit `dpmac info` reports maxima in
tenant = "router"

[[port]]
name = "wan1"
dpmac = "dpmac.10"
rate = 10000
tenant = "router"

# A management port with no tenant: the kernel's own driver terminates it.
[[port]]
name = "mgmt"
dpmac = "dpmac.7"
rate = 10000

# One crypto accelerator for the router; `flows` is a demand, not a queue count.
[[crypto]]
tenant = "router"
flows = 2

# The raise-only escape hatch: add companions on top of the derived request.
[[extra]]
tenant = "router"
family = "dpio"
count = 2
```

From this the compiler derives the router's child DPRC, its dpni per port, its
companion pool (dpio, dpbp, dpcon, dpmcp) sized by the poll-mode regime and thread
count, one dpseci sized to `flows`, and the kernel-terminated `mgmt` port — or, if a
request cannot fit the board, the complete list of refusals naming each one. The full
vocabulary, its derived quantities, and its refusals are
[ADR-0013](docs/adr/0013-accepted-intent-vocabulary.md).

## Workspace

| Crate | Role |
|-------|------|
| `dpaa2-api` | Neutral topology model, trait seams, and the pure reconciler (the hexagon's core). |
| `dpaa2-mc` | Southbound adapter over `restool` and the fsl-mc sysfs bus. |
| `dpaa2-config` | Northbound `topology.toml` frontend. |
| `dpaa2-tools` | The `dpaa2ctl` binary: the imperative shell and stable-naming stage. |

## Documentation

- **Architecture & rationale** — [`docs/adr/0001-dpaa2-provisioning-control-plane-architecture.md`](docs/adr/0001-dpaa2-provisioning-control-plane-architecture.md),
  including the non-obvious hardware behaviours the design had to accommodate.
- **Behavioural contract** — the capability specs under [`openspec/specs/`](openspec/specs/).
- **Object baselines** — one document per DPAA2 object family under
  [`docs/baseline/`](docs/baseline/), joined by the
  [cross-family relationship map](docs/baseline/object-model.md) and the
  [validation traffic inventory](docs/baseline/traffic-inventory.md), all
  pinned to the [reference environment](docs/baseline/reference-environment.md).
- **Port series roadmap** — [`docs/ROADMAP.md`](docs/ROADMAP.md); process
  decisions as numbered ADRs under [`docs/adr/`](docs/adr/); upstream-shareable
  divergences in [`docs/upstream/findings.md`](docs/upstream/findings.md).
- **On-board deployment** (systemd unit, udev trigger, install layout) — [`packaging/README.md`](packaging/README.md).
- **API reference** — [docs.rs](https://docs.rs/dpaa2-controlplane), or `cargo doc --open`.

## Status

Early development. Validated end-to-end on an LX2160A board (cold boot → named,
DHCP-configured 25G interface). Interfaces and file formats may change before 1.0.

## Installation

The provisioning binary is `dpaa2ctl`. For an ad-hoc build:

```sh
cargo install dpaa2-tools     # or: cargo build --release
```

For a full on-board install (binary + systemd unit + udev trigger + example config),
see [`packaging/README.md`](packaging/README.md).

## License

Licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE) or
http://www.apache.org/licenses/LICENSE-2.0).

## Contribution

See [CONTRIBUTING.md](CONTRIBUTING.md).
