# Deployment layout

The DPAA2 control plane installs four kinds of artifact on the Debian target. Naming
(`.link`) files are **not** shipped — the reconciler generates them at runtime into
`/run/systemd/network/` from `topology.toml` (design D3), so they can never drift.

| Source                                   | Installed path                                   | Purpose |
|------------------------------------------|--------------------------------------------------|---------|
| `target/release/dpaa2ctl`                | `/usr/bin/dpaa2ctl`                              | The provisioning binary |
| `packaging/systemd/dpaa2-provision.service` | `/etc/systemd/system/dpaa2-provision.service` | Oneshot unit, MC-ready gated, ordered before `network-pre.target` |
| `packaging/udev/99-dpaa2-provision.rules`| `/etc/udev/rules.d/99-dpaa2-provision.rules`    | One-shot poke that starts the unit when `dprc.1` appears |
| `packaging/dpaa2/topology.toml`          | `/etc/dpaa2/topology.toml`                       | Operator intent (edit for your board) |

## Trigger chain (design D4/D5)

```
dprc.1 add ──udev poke──▶ dpaa2-provision.service
                              │ ExecStartPre: dpaa2ctl wait-ready   (probe MC command, retry)
                              │ ExecStart:    dpaa2ctl ensure       (generate .link, reload udev, converge)
                              ▼
                         named ethX before network-pre.target
```

udev does exactly one job here (a one-shot start); it performs **no** provisioning
work and is never the reconciliation loop.

Both `wait-ready` and `ensure` self-limit to 60s; systemd adds `TimeoutStartSec=90`
as a safety net in case either ever hangs past its own deadline (this unit runs
`Before=network-pre.target`, so an unbounded hang would stall boot). If the unit
still fails, `Restart=on-failure` retries it in-boot (5s backoff, capped at 3
attempts per 60s window) rather than leaving provisioning permanently failed for
the boot.

## Install

```sh
sudo ./packaging/install.sh          # build --release and install all artifacts
sudo systemctl enable dpaa2-provision.service
```

Removing the unit stops provisioning; existing MC objects persist untouched
(additive, idempotent — see the change proposal's rollback note).
