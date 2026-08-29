# dpmac baseline

<!-- Instantiated from _template.md; every section mandatory, empty sections
     state so explicitly (spec: object-baseline). -->

Claim markers, used per claim throughout: **[read]** = derived from source or
manual, not yet observed; **[verified]** = observed on the board against the
pinned reference environment (see `reference-environment.md`).

Findings are written to be provable: behavioral claims name their
observables and are distilled into the Invariant candidates section as
propositions a Quint model can carry — invariant-bearing or
invariant-breaking.

Cross-family relationships are mapped in [object-model.md](object-model.md);
the board scenarios that will settle this document's open items are
classified in [traffic-inventory.md](traffic-inventory.md).

The DPMAC represents a physical MAC/SerDes port. It is the identity anchor
of the whole control plane (ADR-0001 §3): dpmac ids are fixed by the DPC,
never renumber, and a port is keyed by its dpmac, with the dpni derived by
connection edge. Unlike every other family, a dpmac is a thin handle over
DPC-fixed hardware — its create config is a single field (`mac_id`);
everything it *is* (interface type, link type, max rate, FEC, SerDes
settings) is born in the DPC and read-only thereafter [read].

## Command surface

restool v2.4 exposes 3 verbs (`dpmac_commands.c:711-729`) [read]:

| Command | MC interaction | Notes |
|---|---|---|
| `info <dpmac.N> [--verbose]` | get_attributes, get_api_version, **get_mac_addr**, get_counter ×62, `dprc_get_connection` | prints id, plugged state, endpoint + "link is up/down" (which is the **DPRC connection state, not MAC link state**), link type, eth interface, MAC address, max rate, counters |
| `create --mac-id=<n> [--container=<c>]` | `dpmac_create` | `--mac-id` mandatory, the only cfg field; an escape hatch — every shipped DPL pre-declares dpmacs, and what MC does with a mac-id that has no DPC port entry is unknown |
| `destroy <dpmac.N>` | `dpmac_destroy` | refuses if driver-bound |

Not exposed by restool (MC has it): link cfg/state, protocol change, IPG
params, MDIO read/write, reset, bulk statistics. The /dev/dprc.N whitelist
does allow `GET_COUNTER` and `GET_MAC_ADDR` unprivileged — which is how
restool's info works through the kernel [read]. restool dropped MC v9
support for this family entirely; a dangling `dpmac_commands_v9` extern
remains [read].

## Option inventory: used vs available

**No script creates or destroys a dpmac deliberately.** The ls-scripts
consume dpmacs as user-supplied endpoints, validated by regex only, and
discover them by grepping `dprc show` output [read]:

- ls-addni/ls-addmux accept `dprc.N/dpmac.M` full-path endpoints, but the
  full-path chain is broken end-to-end (type derivation produces
  `dprc.N/dpmac`, restool errors, the error is swallowed, and the script
  proceeds) — the three entry points even use inconsistent regexes
  (ls-addsw refuses the prefix the others accept) [read].
- `ls-listmac` = `dprc list --full-path` → `dprc show | grep dpmac` →
  two `dpmac info` calls per dpmac (label + endpoint). On current restool
  each info also pulls all 62 counters — 124 MC round-trips per dpmac
  [read].
- ls-append-dpl cannot recreate a dpmac from a DPL (it emits no
  `--mac-id`); it can only *connect* to existing ones via the
  `dpmac@N`→`dpmac.N` name fallback [read].
- `restool dprc generate-dpl` emits empty `dpmac@N {}` nodes —
  correct-by-accident (the parse function is a no-op stub) but matching
  what shipped DPLs look like [read].

DPL/DPC surface [read, mc-utils config corpus]: DPL declares dpmacs by id
only (empty nodes; at most `compatible = "fsl,dpmac"`). The DPC's
`board_info/ports/mac@N` nodes — same id space — carry the physical port
description; the only properties in the entire corpus are `link_type`
(PHY/FIXED/BACKPLANE), `enet_if` (only "USXGMII" ever appears; other
modes come from the RCW SerDes protocol), and `fec_mode` ("none"). The
SerDes eq and IPG settings visible in `dpmac_attr` have no DPC syntax
anywhere — their provenance is unresolved (unknown register).

## Attribute mutability

- Create-time: `mac_id` only. There is no other create input [read].
- DPC-born, read-only in `dpmac_attr`: `max_rate`, `link_type`,
  `fec_mode`, `serdes_cfg`, `ifg_cfg` — with two runtime exceptions:
  `dpmac_set_protocol` changes `eth_if` (MC ≥ 10.32) and
  `dpmac_set_params` changes IPG mode/length [read].
- **The MAC address has no setter anywhere in the API** —
  `dpmac_get_mac_addr` is read-only; nothing in restool, the flib, or the
  kernel writes it. It is programmed pre-Linux (DPC and/or bootloader;
  which is authoritative is outside the corpus). This is the stable,
  globally-unique address the dpni inherits at connect (ADR-0001 C2)
  [verified].
- Link state is runtime but split by direction (MC API notes below):
  there is no `dpmac_set_link_cfg` and no `dpmac_get_link_state`.

## MC API notes

10.32→10.39 delta [read, mc-utils diff]: **one addition** —
`dpmac_get_statistics` (bulk counter fetch via two IOVAs, the scalable
replacement for N× `get_counter`), API 4.8 → 4.10 (4.9 at 10.37 was a
firmware-side-only bump; no client change). Nothing else changed; no
version bumps of existing commands. No MACsec exists in this family at
any release.

**Counter skew, the inverse of the usual direction**: restool's counter
enum carries 62 ids — identical to mc-utils **10.40**, i.e. restool runs
*ahead* of our 10.39 firmware, whose flib defines only the first 28
(size buckets, pause, byte/frame classes). The 34 extras (egress size
buckets, FCS/VLAN/control, per-priority PFC 0–7 both directions) exist
only from 10.40. restool prints whatever succeeds and silently skips
refusals, so on this board `dpmac info` shows 28 counters with no
indication 34 were refused — and a transient portal error is
indistinguishable from "not supported" [read].

The link API is a **directional pair** [read]:

- `dpmac_get_link_cfg` — requests flowing *down*: what the peer wants
  (the connected dpni's `dpni_set_link_cfg` lands here), pulled by the
  MAC driver on a `LINK_CFG_REQ` interrupt.
- `dpmac_set_link_state` — reality flowing *up*: the PHY-observed state
  pushed into MC, surfaced to the peer as `dpni_get_link_state`.

`GET_ATTR` is a V3 command and `GET_LINK_CFG`/`SET_LINK_STATE` are V2;
what the earlier wire versions carried is not in the corpus.
`dpmac_mdio_read/write` are compiled but undeclared in the public header
in both snapshots — unreachable without a private prototype [read].

## Kernel-side behavior (Linux 6.6.52)

**Two consumers, explicit arbitration** [read, `.build/src/linux`]:

- The only driver that *binds* dpmac devices is the vendor standalone
  `fsl_dpaa2_mac` (not upstream). It **defers forever** if the dpmac's
  endpoint is a dpni/dpsw in the *same* container (the eth/switch driver
  will own the MAC through its own portal, device left driverless).
- On dpni↔dpmac connect, `dpaa2-eth` force-unbinds the standalone driver
  (`dpaa2_mac_driver_detach`) and takes the MAC over phylink; on
  disconnect it hands it back. The DPMAC has **no endpoint-changed event
  of its own** — connect/disconnect is only observable from the peer's
  ENDPOINT_CHANGED interrupt.
- A dpmac whose peer lives in a **different container** (VFIO/VPP dpnis:
  `fsl_mc_get_endpoint` returns `-EPERM`) is treated as unconnected —
  the standalone driver binds and drives the PHY/SFP while the datapath
  is owned elsewhere. This is the load-bearing arrangement for our
  board: dpmacs stay in the kernel container, VPP owns the dpnis in the
  child [verified in production use].

`CONFIG_FSL_DPAA2_MAC_NETDEVS=y` (this build) gives each standalone-bound
dpmac a `macN` netdev: carrier via phylink, ethtool stats (the 28
counters), link settings — but tx is a drop stub, the MAC address is
all-zero (never read from the object), there are no packet stats, and its
mii ioctl hook is dead code (`ndo_do_ioctl` is never routed in 6.6)
[read].

Kernel link-state pushes always carry `state_valid = 0`, `supported = 0`,
`advertising = 0` — only `up`, `rate`, and duplex/pause option bits are
ever populated, and `link_down` re-pushes the stale rate/options from the
last link-up. Whether MC honors `up` with `state_valid=0` is firmware
behavior outside the corpus [read]. The standalone driver's inbound
direction has a real duplex inversion bug (`LINK_CFG_REQ` with
half-duplex requested is applied as full and vice versa) [read].

The `eth_if`→`phy_interface_t` map is asymmetric: `1000BASEX` (and
MII/RMII/SMII/GMII/XAUI) have no forward mapping — a DPC declaring one
of these on a dpmac with no DT `phy-mode` fails connect with a bare
`-EINVAL` that never names the offending value. DT `phy-mode` wins over
the DPC value when present; dpmac↔DT matching is by the `reg` property
of `dpmacs/ethernet@N` nodes equal to the dpmac id [read]. Feature
gates: protocol change needs DPMAC API ≥ 4.8, bundled statistics ≥ 4.10
— our 10.39 firmware reports 4.10, so both should be live; the kernel
never logs the version it found (unknown register) [read].

The kernel never reads the dpmac's MAC address — the port MAC reaches
Linux only through `dpni_get_port_mac_addr` on the connected dpni
(dpni.md, DPNI-I3) [read].

## Lifecycle ordering and dependencies

Dpmacs are created by the MC at boot from the DPC and normally live
forever in the root container [read]; on this board dpmacs 3–10 and 17
exist, with 17 kernel-connected and 7/9 connected to VPP-owned dpnis
across the container boundary [verified]. The runtime lifecycle is
therefore **connect/disconnect, not create/destroy**:

1. Unconnected dpmac → standalone driver binds (PHY managed, `macN`
   netdev if enabled).
2. `dprc connect` dpni↔dpmac → peer's ENDPOINT_CHANGED fires; same-
   container kernel dpni: eth driver takeover; cross-container consumer:
   standalone driver keeps the PHY, peer sees link via
   `dpni_get_link_state` [read/verified as above].
3. `dprc disconnect` → handback to the standalone driver.
4. `dpmac destroy` exists but is an off-nominal path: `ls-delete all` is
   the only script that reaches it, and it discards the result while
   reporting the dpmac deleted (silent-failure notes).

## Intent mapping

The dpmac *is* the physical-port construct — the operator-facing key of
the topology (ADR-0001 §3, `topology.toml` keys ports by dpmac). The
intent layer never creates or configures dpmacs; it consumes them as
fixed anchors and derives connection edges. The ADR-0003 port safety
matrix is expressed entirely in dpmac terms (dpmac.3 and dpmac.17
total-deny, dpmac.7/9 flagged, others lifecycle-only) [verified,
board-validated]. FEC/protocol/IPG mutation is out of intent scope until
a concrete need appears.

## Silent-failure notes

- `dpmac info` prints only the counters the firmware accepts, skipping
  refusals silently (28 of 62 on this firmware) — version gap and
  transient errors are invisible [read].
- The endpoint block prints an **uninitialized** `state` value before
  checking the `dprc_get_connection` error; and endpoint types with a
  nonzero interface id other than dpsw/dpdmux produce a malformed line
  that ls-main's greps silently read as "no endpoint" [read].
- `destroy` in a child container overwrites the destroy error with the
  `dprc_close` result — same lying-exit-status shape as dpseci [read].
- `ls-delete all` enumerates dpmacs, destroys them with output and exit
  status discarded, and prints them as deleted regardless [read].
- `object_exists` is an unanchored `grep | wc -l`: `dpmac.1` matches
  `dpmac.10`–`dpmac.18` — wrong on every LX2160A (high macs 17/18); the
  same substring pattern underlies `process_listmac` [read].
- `restool dpmac info` **aborts via `assert(false)`** on an eth_if or
  link_type value beyond its enum — and the 10.40 API already grows the
  enum space [read].
- Kernel: for a FIXED/NONE-type dpmac the standalone driver installs the
  MC interrupt handler without a phylink guard — a link request from MC
  dereferences NULL (oops, not degradation); a missing DT node for a
  non-PHY dpmac produces no diagnostic at all [read].

The family-level pattern: **the observation surface is weaker than the
state**. Endpoint "link is up" is not MAC link state; counter absence is
not zero; exit 0 is not destroyed. The reconciler must observe dpmacs
via attributes + connection queries, never via restool's rendered text.
Weaker is not unrelated, though: on a bound, enabled pair the endpoint
line's `, link is up/down` text was observed co-varying with a physical
cable flap at both ends while the connection edge itself persisted
(peers still named) — so it is not a static rendering of the edge
either, and reading it as evidence of *anything* stable is unsafe
[verified 2026-08-24, V-LINK-2 rev 3].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPMAC-I1 | Identity permanence: dpmac ids are DPC-fixed and never renumber across reboots or any action sequence; the set of dpmacs is constant at runtime (create/destroy are off-nominal) | `dprc show` dpmac set across reboots | verified (ADR-0001 §3) |
| DPMAC-I2 | The dpmac MAC address is immutable through every API surface (no setter exists) and survives all connect/disconnect sequences; it is the address the connected dpni inherits (DPNI-I3) | `get_mac_addr` before/after suites; dpni primary MAC after connect | verified (ADR-0001 C2) |
| DPMAC-I3 | Attribute immutability with two exceptions: `dpmac_attr` fields are constant except `eth_if` (via `set_protocol`) and IPG (via `set_params`) | `get_attributes` before/after | candidate |
| DPMAC-I4 | Link channels are directional and distinct: `get_link_cfg` carries peer *requests* (from `dpni_set_link_cfg`), `set_link_state` carries PHY *reality* (to `dpni_get_link_state`); the model must not conflate them into one link variable | both queries under a forced peer request | candidate |
| DPMAC-I5 | **Breaking:** the model must NOT read `dpmac info`'s "link is up" as MAC link state — it is the DPRC connection state; MAC link state has no restool observable at all | info output vs peer `dpni_get_link_state` with cable pulled | verified 2026-08-24 (V-LINK-2 rev 3): never read as MAC link state — but on a bound, enabled pair the connection-state text co-varies with the cable flap, so the two are not independent |
| DPMAC-I6 | Driver arbitration: standalone driver bound ⟺ no same-container host-managed peer connected; cross-container peers leave the standalone driver owning the PHY while the datapath is remote | driver symlink under `/sys/bus/fsl-mc/devices/dpmac.N/`; `macN` presence | verified (in production use on this board) |
| DPMAC-I7 | **Breaking:** the model must NOT assume the counter vocabulary: available counters are firmware-versioned (28 at 10.39, 62 at 10.40+) and refusals are silent; absence ≠ zero | per-counter MC status vs restool output | verified 2026-08-29 (V-DPMAC-1 rev 1): 28 of restool's 62 counters printed on every port, the rest refused and skipped without a trace in the output |
| DPMAC-I8 | **Breaking:** exit 0 ⇒ destroyed is false in child containers (error overwritten) and always false under `ls-delete all` (result discarded) | object presence after "successful" destroy | candidate |
| DPMAC-I9 | Every kernel link-state push carries `state_valid=0` and empty supported/advertising; the model carries emitted fields per action and treats MC's interpretation as an environment choice until board-probed | wire fields; peer-visible link state | candidate |

## Unknown / unverified register

1. Does MC reject `dpmac create --mac-id=N` when the DPC has no
   `mac@N` port entry? (No firmware source; restool create is otherwise
   an escape hatch of unknown semantics.)
2. MC semantics of `state_valid = 0` in `set_link_state` — does the `up`
   bit take effect? Every kernel push depends on the answer.
   **Answered on the kernel path** — board suite V-LINK-2 rev 3,
   2026-08-24: with a physical cable pull on a wired pair, the peer's
   `dpni info` `link status:` tracked PHY reality down and back up.
   Since every kernel push carries `state_valid = 0`, the `up` bit does
   take effect — with a propagation lag, the MC-visible state trailing
   the local carrier flag by enough that a probe fired at the moment of
   the flap reads the stale value. The same sitting showed an
   admin-down of the peer's interface is not a link-down stimulus on
   this wiring: it never drops the light the peer transmits. The direct
   raw probe (issuing `SET_LINK_STATE` ourselves with chosen field
   values) stays open — V-LINK-3, deferred to the online driver.
3. Provenance of `serdes_eq_settings` and `ifg_cfg` (no DPC property
   exists; `set_params` covers IPG only) — RCW/firmware defaults
   presumed.
4. Which of DPC `mac_addr` vs bootloader programs the burned-in MAC on
   this board (outside the corpus; the inheritance chain itself is
   verified).
5. ~~Board's DPMAC API version as reported (expected 4.10; kernel never
   logs it — one `dpmac info` line settles it and DPMAC-I7's count).~~ —
   resolved: 4.10 [board-observed 2026-08-25, clean-boot snapshot,
   reference-environment.md]; DPMAC-I7's counter half stays with
   unknown 7.
6. Wire-format history of `GET_ATTR` v1→v3 and `GET_LINK_CFG`/
   `SET_LINK_STATE` v1→v2 (only the newest layout per snapshot).
7. ~~Whether counters ≥ 28 are refused cleanly by 10.39 firmware (feeds
   DPMAC-I7's observable).~~ **Answered** — board plan V-DPMAC-1 rev 1,
   2026-08-29: restool 2.4 asks for 62 counters and `dpmac info` prints
   28 on every port, 25G and 10G alike — the 34 counters 10.39.0 does not
   carry are refused and skipped silently (`dpmac_commands.c` drops the
   error), so the refusal is clean but invisible and the row count is
   the only observable. The vocabulary is firmware-wide, not per port.
8. `eth_if` resolution when the DPC omits `enet_if` (RCW SerDes protocol
   mapping happens outside both repos).
