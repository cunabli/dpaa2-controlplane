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

- [ ] 1.1 Define neutral `DesiredTopology` graph (objects + connection edges), no
      serde, keyed by DPMAC anchors
- [ ] 1.2 Define `ObservedTopology`, `ObjectKind`, and `Lifecycle` states
- [ ] 1.3 Define `Transition` and `Plan` types
- [ ] 1.4 Define `McControl` and `KernelControl` southbound traits at MC-command
      granularity
- [ ] 1.5 Define the northbound config-source trait producing `DesiredTopology`
- [ ] 1.6 Model actuatable-vs-assert-only fields and ownership metadata

## 2. Reconciliation engine (dpaa2-api, pure)

- [ ] 2.1 Implement edge-based desired↔observed matching (index-independent)
- [ ] 2.2 Implement `reconcile()` producing an ordered `Plan`
- [ ] 2.3 Enforce ownership: only touch the configured subgraph; never
      enumerate-and-delete
- [ ] 2.4 Implement immutable-attribute drift detection (report, refuse)
- [ ] 2.5 Implement assert-only verification (report mismatch, no actuation)
- [ ] 2.6 Build a fake in-memory MC test double implementing `McControl`/
      `KernelControl` over an `ObservedTopology` (D10) — the hardware-free test seam
- [ ] 2.7 Unit-test the engine via the fake backend: idempotence, absent→create+
      connect, renumber-stable, foreign-preserved, drift-refused, assert-only,
      convergence/requeue

## 3. Config frontend (dpaa2-config)

- [ ] 3.1 Define the TOML schema (DPMAC-keyed ports, name, MAC, MAC mode)
- [ ] 3.2 Deserialize TOML and convert into neutral `DesiredTopology`
- [ ] 3.3 Reject DPNI-index pinning and validate names/MACs/uniqueness
- [ ] 3.4 Unit-test parsing, conversion, and validation errors

## 4. Southbound: restool shim (dpaa2-mc)

- [ ] 4.1 Implement `McControl::observe` by invoking/parsing `restool`
- [ ] 4.2 Implement create / connect / set-MAC / disconnect / destroy
- [ ] 4.3 Implement `KernelControl` bind + netdev observation (sysfs/netlink)
- [ ] 4.4 Handle fixed-link ports (report no-netdev, do not error)
- [ ] 4.5 Confirm the crate compiles under `unsafe_code = "forbid"`
- [ ] 4.6 Parse tests over recorded restool v2.4 output fixtures (golden files);
      assert command construction matches the shim recipe

## 5. Imperative shell / CLI (dpaa2-tools)

- [ ] 5.1 Wire crates; implement `scan`/`ensure`/`status`/`dry-run` subcommands
- [ ] 5.2 Implement observe → reconcile → act → wait → re-observe convergence loop
      with deadline
- [ ] 5.3 Implement `status` (lifecycle + delta, non-zero on divergence) and
      `dry-run` (print plan, apply nothing)
- [ ] 5.4 Add structured logging of observed state, plan, and applied actions
- [ ] 5.5 Verify idempotence and retry-safety on re-run
- [ ] 5.6 Test the convergence loop and exit codes against the fake backend
      (converges, deadline→non-zero, dry-run applies nothing) — no board

## 6. System integration and packaging

- [ ] 6.1 systemd unit gated on `dprc.1` readiness, ordered `Before=network-pre.target`
- [ ] 6.2 One-line udev poke on `dprc.1` add (trigger once; no reconciliation in udev)
- [ ] 6.3 Generate `systemd.link` files (MAC → stable name) into
      `/run/systemd/network/` from the topology; `udevadm control --reload` before
      provisioning (reconciler-owned; not a systemd generator)
- [ ] 6.4 Handle fixed-link ports with no rename stage
- [ ] 6.5 Install layout: binary, unit, `.link`, `/etc/dpaa2/topology.toml`
- [ ] 6.6 End-to-end validation on the board (cold boot → named interfaces)
