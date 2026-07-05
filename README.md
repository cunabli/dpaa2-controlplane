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

- **Intent in, convergence out.** `topology.toml` declares ports keyed by their stable
  DPMAC anchor; `dpaa2ctl ensure` drives the board toward it and is safe to re-run.
- **Identity by connection edge, not index.** A managed interface is matched by its
  DPMAC edge, so an MC-reassigned DPNI index across reboots still resolves correctly.
- **Pure core, thin adapters.** A hardware-free reconciler (`reconcile(desired,
  observed) -> Plan`) sits behind trait seams, with `restool`/sysfs and TOML as
  swappable adapters — so the whole loop is testable without a board.
- **Stable naming via stock `systemd.link`.** Names are generated at runtime and applied
  during early boot; no custom udev helpers, no marker files.

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
