## 0. Board verification (empirical — de-risk before/while building)

- [x] 0.1 E2: enumerate real dpmac indices — 25G=dpmac.3–6 (CAUI), 10G=dpmac.7–10
      (XFI), dpmac.17=1G mgmt (already dpni.0). Doc's original mapping superseded.
- [x] 0.2 E1: link type — all in-scope DPMACs are `LINK_TYPE_PHY` (none fixed)
- [x] 0.3 E5: no `firmware_version` attr on MC 10.32.0 — readiness must probe via an
      MC command (e.g. `dprc info`) and retry
- [x] 0.4 E3: netdev inherits the DPMAC's stable MAC (no actuation needed); naming
      keys on it via `systemd.link`
- [x] 0.5 E4: minimal DPNI + connect yields a working netdev; driver auto-allocates
      DPBP/DPCON/DPMCP — no buffer-pool provisioning needed
- [x] 0.6 restool v2.4 present (MC 10.32.0 compatible)
- [x] 0.7 E6: `macN` = `fsl_dpaa2_mac` kernel name kept by stock `99-default.link`;
      zero permanent MAC → no collision with our MAC-keyed `10-dpaa2-*.link`
- [x] 0.8 E7: `ethtool -P eth1` = DPMAC MAC = permanent address → `systemd.link`
      `[Match] MACAddress=` matches
- [x] 0.9 shim recipe captured in design.md (`--script` create + assign+plug +
      connect + sync; observe via `dprc show`/`dpni info`/sysfs `net/`)

## 1. Core model and traits (dpaa2-api)

- [x] 1.1 Define neutral `DesiredTopology` graph (objects + connection edges), no
      serde, keyed by DPMAC anchors
- [x] 1.2 Define `ObservedTopology`, `ObjectKind`, and `Lifecycle` states
- [x] 1.3 Define `Transition` and `Plan` types
- [x] 1.4 Define `McControl` and `KernelControl` southbound traits at MC-command
      granularity
- [x] 1.5 Define the northbound config-source trait producing `DesiredTopology`
- [x] 1.6 Model actuatable-vs-assert-only fields and ownership metadata

## 2. Reconciliation engine (dpaa2-api, pure)

- [x] 2.1 Implement edge-based desired↔observed matching (index-independent)
- [x] 2.2 Implement `reconcile()` producing an ordered `Plan`
- [x] 2.3 Enforce ownership: only touch the configured subgraph; never
      enumerate-and-delete
- [x] 2.4 Implement immutable-attribute drift detection (report, refuse)
- [x] 2.5 Implement assert-only verification (report mismatch, no actuation)
- [x] 2.6 Build a fake in-memory MC test double implementing `McControl`/
      `KernelControl` over an `ObservedTopology` (D10) — the hardware-free test seam
- [x] 2.7 Unit-test the engine via the fake backend: idempotence, absent→create+
      connect, renumber-stable, foreign-preserved, drift-refused, assert-only,
      convergence/requeue

## 3. Config frontend (dpaa2-config)

- [x] 3.1 Define the TOML schema (DPMAC-keyed ports, name, MAC, MAC mode)
- [x] 3.2 Deserialize TOML and convert into neutral `DesiredTopology`
- [x] 3.3 Reject DPNI-index pinning and validate names/MACs/uniqueness
- [x] 3.4 Unit-test parsing, conversion, and validation errors

## 4. Southbound: restool shim (dpaa2-mc)

- [x] 4.1 Implement `McControl::observe` by invoking/parsing `restool`
- [x] 4.2 Implement create / connect / set-MAC / disconnect / destroy
- [x] 4.3 Implement `KernelControl` bind + netdev observation (sysfs/netlink)
- [x] 4.4 Handle fixed-link ports (report no-netdev, do not error)
- [x] 4.5 Confirm the crate compiles under `unsafe_code = "forbid"`
- [x] 4.6 Parse tests over recorded restool v2.4 output fixtures (golden files);
      assert command construction matches the shim recipe

## 5. Imperative shell / CLI (dpaa2-tools)

- [x] 5.1 Wire crates; implement `scan`/`ensure`/`status`/`dry-run` subcommands
- [x] 5.2 Implement observe → reconcile → act → wait → re-observe convergence loop
      with deadline
- [x] 5.3 Implement `status` (lifecycle + delta, non-zero on divergence) and
      `dry-run` (print plan, apply nothing)
- [x] 5.4 Add structured logging of observed state, plan, and applied actions
- [x] 5.5 Verify idempotence and retry-safety on re-run
- [x] 5.6 Test the convergence loop and exit codes against the fake backend
      (converges, deadline→non-zero, dry-run applies nothing) — no board

## 6. System integration and packaging

- [x] 6.1 systemd unit gated on `dprc.1` readiness, ordered `Before=network-pre.target`
- [x] 6.2 One-line udev poke on `dprc.1` add (trigger once; no reconciliation in udev)
- [x] 6.3 Generate `systemd.link` files (MAC → stable name) into
      `/run/systemd/network/` from the topology; `udevadm control --reload` before
      provisioning (reconciler-owned; not a systemd generator)
- [x] 6.4 Handle fixed-link ports with no rename stage
- [x] 6.5 Install layout: binary, unit, `.link`, `/etc/dpaa2/topology.toml`
- [x] 6.6 End-to-end validation on the board (cold boot → named interfaces)
      — VALIDATED on the LX2160A board, 2026-07-05. Cold boot: `dpaa2-eth`
      probes `dpni.1` as `eth1`; `dpaa2ctl` writes `10-dpaa2-wan0.link` matching
      the DPNI's inherited board MAC `d0:63:b4:04:96:25` and the `udevadm`
      retrigger renames `eth1 → wan0` the same boot; systemd-networkd brings the
      25G link up and DHCP configures it (85.195.223.32). `networkctl status
      wan0` shows the generated `.link` as the applied Link File.
