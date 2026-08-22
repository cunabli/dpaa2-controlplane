# dpci baseline

<!-- Instantiated from _template.md; every section mandatory, empty sections
     state so explicitly (spec: object-baseline). -->

Claim markers, used per claim throughout: **[read]** = derived from source or
manual, not yet observed; **[verified]** = observed on the board against the
pinned reference environment (see `reference-environment.md`).

Findings are written to be provable: behavioral claims name their
observables and are distilled into the Invariant candidates section as
propositions a Quint model can carry — invariant-bearing or
invariant-breaking.

The DPCI is generic frame-based **inter-partition communication** over
QBMan: two dpcis connected in a pair form a link between software
contexts, with a completely free protocol (manual ch. 12). It is *not*
AIOP-only — the manual explicitly allows GPP↔GPP, and NXP ships DPLs with
same-container dpci loopback pairs. Tier C question answered up front:
**board-exercisable, yes** (create + `dprc connect`, no AIOP needed, no
platform gate anywhere in the corpus) — but **nobody consumes it on this
board today**: the kernel has no dpci driver, and the DPDK consumers are
eventdev (uses an *unconnected* dpci as a soft event queue) and the
GPP↔AIOP cmdif rawdev (needs the AIOP peer we don't have) [read].

## Command surface

restool v2.4: 3 verbs (`dpci_commands.c:541-559`) [read]. `create` takes
`--num-priorities` (1–2), `--options` (the two OPR flags), `--container`.
`info` is unusually rich: it queries **peer attributes and link state**
(`connected peer` / `no peer`, peer's priority count, `link status`) —
the only family whose restool info shows connection liveness. `destroy`
guards only against a bound kernel driver, not an existing peer.
Connect/disconnect are dprc verbs and type-agnostic: restool accepts any
`type.id` endpoint pair; all validation is MC-side [read].

No enable/disable/reset/set-rx-queue verbs — a restool-created dpci is
never enabled by restool; enable + queue binding are consumer-side (DPDK
does set_rx_queue ×2 → enable) [read].

## Option inventory: used vs available

**Used by ls-main: nothing** (no dpci path at all). ls-append-dpl reaches
dpci only generically — and brokenly (silent-failure notes). The corpus
recipe is DPDK's `dynamic_dpl.sh`: 8 dpcis per container,
`--num-priorities=2 --options=DPCI_OPT_HAS_OPR,DPCI_OPT_OPR_SHARED`,
left unconnected for eventdev [read].

Available-unused: everything, including `--options` — which is worse
than unused, it is **non-functional** (below).

## Attribute mutability

Create-time immutable: `num_of_priorities` (restool caps 1–2; the header
advertises `DPCI_PRIO_NUM 4`), `options`. Read-only: id, peer id, peer's
priority count, link state. Externally mutable: the **peer**, set and
cleared by `dprc connect`/`disconnect` — connection state is mutable
object state owned by a *different* family's verbs. The OPR options are
**write-only**: `DPCI_GET_ATTR` returns no options field, so they are
unreadable by any tool in the corpus [read].

Asymmetric peering is legal and directional: local Rx priorities = own
count, local **Tx priorities = the peer's count**; Tx FQID reads
`DPCI_FQID_NOT_VALID` when unconnected or when the priority exceeds the
peer's [read].

## MC API notes

The dpci flib is **byte-identical from MC 10.32.0 through 10.39.0**
(frozen since ≤ 10.32; only the header-layout move) [read, mc-utils
diff]. The changelog's *only* dpci entry ever is 10.4.0 ("support for
order preservation and multiple priorities") — nothing in the window
[read, MC changelog]. No LX2160A DPC/DPL provisions dpci; repo-wide it
appears only in the two AIOP DPLs (paired across containers) and the
LS2088A nadk DPL (10 dpcis paired as same-container GPP loopbacks) [read].
DPAA2UM rev 53 Table 2-1 lists DPCI platform support as **All** — the
platform question does not arise for this family [read, manual].

## Kernel-side behavior (Linux 6.6.52)

No dpci driver — bus plumbing only (`fsl_mc_bus_dpci_type`, open cmd
table); not allocatable (pools are dpbp/dpmcp/dpcon). A created dpci
enumerates unbound and does nothing. The uapi allowlist explicitly
whitelists `DPCI_GET_LINK_STATE` and `DPCI_GET_PEER_ATTR`, plus the
generic create/destroy and **`DPRC_CONNECT`/`DISCONNECT`** — so the
whole create-connect-inspect cycle is drivable from userspace (root)
[read].

Kernel visibility gap: `dpci create` does **not** trigger a bus rescan
(`dprc create` does, unconditionally) — the new object exists in MC but
not in sysfs until `restool --rescan` or autorescan [read].

## Lifecycle ordering and dependencies

create (unconnected, disabled) → create the second endpoint → `dprc
connect <common-ancestor> --endpoint1=dpci.A --endpoint2=dpci.B`
(endpoints need not share a container; must be disconnected first) →
consumer opens, binds rx queues to DPIO/DPCON, enables → link up.
Teardown: unbind driver (restool's only guard) → destroy on the parent
token; restool does not force a disconnect first — whether MC requires
one is an open item [read].

## Intent mapping

The natural carrier for the **pseudo-physical-link construct** (ADR-0005):
a dpci pair is the only DPAA2 object that *is* a link between two
software contexts by definition — the cross-dprc kernel↔VPP link the
intent layer wants to express maps onto dpni↔dpni or dpci↔dpci pairs,
with dpci the datapath-free control variant. Derivation: one pair per
declared inter-context channel; priorities default 2. Blocked in practice
by the consumer gap (no kernel driver) — an intent that compiles to dpci
must also name the userspace consumer, or be refused.

## Silent-failure notes

- **`--options` is silently discarded by every flib in existence**:
  restool's, DPDK's, and MC's own reference flib all marshal *only*
  `num_of_priorities` in CREATE — the options word is never written, MC
  always receives 0. `--options=DPCI_OPT_HAS_OPR,...` parses, validates,
  prints nothing, does nothing — and is unverifiable after the fact
  because GET_ATTR has no options field. `dynamic_dpl.sh`'s
  `DPCI_OPTIONS` default is therefore also a no-op [read].
- Corollary trap: restool's `dpci_cfg` is **not zero-initialized** (every
  sibling family's is) — the day the flib marshalling is fixed, create
  without `--options` starts sending stack garbage [read].
- **ls-append-dpl cannot create a dpci from a DPL**: the DPL property is
  `num_of_priorities` → mapped to `--num-of-priorities`, but restool's
  flag is `--num-priorities` (dpcon's DPL property matches its flag; dpci
  is the odd one out) — mid-script usage error, exit status unchecked
  [read].
- A created-but-unconnected dpci looks healthy everywhere except the
  `no peer`/`link status: 0` lines nothing ever checks; DPDK stores the
  invalid Tx FQID unchecked (harmless for eventdev, latent for cmdif)
  [read].
- No rescan after create: object exists in MC, invisible to the bus
  [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPCI-I1 | Pair symmetry: connection is one peer exactly, set only via dprc connect; Rx capacity is own priority count, Tx capacity is the peer's — asymmetric pairs give asymmetric directions | `dpci info` both ends; Tx FQID validity per priority | candidate |
| DPCI-I2 | **Breaking:** the model must NOT carry `options` as configurable state — the flag is discarded on the wire by every flib and unreadable back; model dpci as options-less until a fixed flib exists | flib marshalling; GET_ATTR shape | candidate (corpus-proven) |
| DPCI-I3 | **Breaking:** the model must NOT equate object existence with bus visibility — dpci create performs no rescan; MC state and sysfs state diverge until an explicit rescan | sysfs before/after `--rescan` | candidate |
| DPCI-I4 | Exercisability without AIOP: create + connect of a GPP↔GPP pair succeeds on this DPC (no platform gate exists for dpci, unlike dpaiop) | create/connect/`link status` on the board | board-pending |
| DPCI-I5 | Consumer-required liveness: link state reflects consumer enable, not mere connection — a restool-only pair may never leave link-down | `dpci info` after connect, before any enable | board-pending (decides the intent-layer contract) |

## Unknown / unverified register

1. Does link state go up on connect alone, or only after both ends issue
   `DPCI_ENABLE`? (Decides DPCI-I5; restool never enables.)
2. Does destroy of a still-connected dpci succeed, or does MC demand a
   disconnect first? (restool doesn't guard it.)
3. Same-container connect in the *root* dprc — all corpus examples pair
   in children or across containers.
4. Per-container/global dpci ceiling (no resource cap is expressed in the
   DPC).
5. Asymmetric-pair connect (2 vs 1 priorities): rejected at connect, or
   accepted with one direction short?
6. Indirect probe for DPCI-I2 on hardware: does OPR config succeed on an
   object created "with" the flag (proving MC defaulted it off)?
