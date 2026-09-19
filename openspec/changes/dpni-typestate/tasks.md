# dpni-typestate tasks

Land order per design D8/Migration: model first, pure core, compiler,
shim, verify, board, close-out. One bead at a time through acceptance.

## 1. Model gate

- [x] 1.1 Grow `models/families/dpni.qnt` with the create-option surface
  (twelve live options with ranges, typed flag vocabulary + raw-mask
  escape, PMD/kernel profiles) and named invariants (create-range
  refusal, profile totality, dead-option parity, write-only field law);
  Apalache-mark; CI ladder green

## 2. Core typestates

- [ ] 2.1 `dpaa2_api::families::dpni` create-surface typestates:
  `dpni_cfg` as immutable type parameter, refined range types, typed
  option set with provenance-carrying raw-mask constructor; runtime
  state slot holding the primary MAC
- [ ] 2.2 Dead-option and `num_rx_tcs` parity refusals; `dist_key_size`
  write-only by construct (excluded from observation comparison); cfg
  drift plans destroy + create, MAC-only drift plans the mutation

## 3. Intent derivation and hazard closure

- [ ] 3.1 Compiler derives options purely from `Dataplane` + interface
  construct (PMD/kernel profile map, one function, dry-run rule
  provenance); no TOML or vocabulary surface for options
- [ ] 3.2 Close the `intent::tenant` typestate hazards: no zero-value
  `Default` path on `Isolation`/`Intent`, empty `TenantName`
  unconstructible outside its sentinel role; frozen-trace replay and
  public-surface diff green

## 4. Southbound shim

- [ ] 4.1 restool shim dpni create at full option granularity (computed
  raw mask only), primary-MAC set, observation mapping of the
  `dpni_attr` asymmetries
- [ ] 4.2 `observe_container(id)` beside the enumerate verb; planning
  re-observation uses it per candidate; OI-3 dpmcp-budget outcome cited
  in the disposition (rider z5z)

## 5. Verify and board

- [ ] 5.1 `dpaa2-verify`: frozen-trace MBT twins for the dpni model;
  batch-suite generation for the option-profile walks, sizing-field
  probes (#3), `HAS_REPLICATION` accept/reject (#8), unread-flag probes
  (#6), primary-MAC mutation — scratch-first, self-cleaning, reference
  pair asserted
- [ ] 5.2 Board sitting: batch suite extending the V-DPNI series plus
  the online-MBT per-step learning session; results diffed clean or
  divergences fed back to the model

## 6. Close-out

- [ ] 6.1 Docs close-out: `docs/baseline/dpni.md` amendments for every
  probe outcome; deferral rows verified (#10 runtime setters + TX_CONF
  v2, #14 num_rx_tcs-via-DPL, table/traffic items to earliest
  reachability); ADR for anything that solidified or died on the board;
  roadmap row #5; CHANGELOG via cliff; full quality floor
