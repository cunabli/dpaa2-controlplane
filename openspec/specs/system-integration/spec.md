# system-integration Specification

## Purpose
Define how the reconciler is triggered causally on MC readiness, ordered before
network configuration, and how stable interface names are applied via
`systemd.link`.
## Requirements
### Requirement: Reconciler is triggered causally on MC readiness
The reconciler SHALL be started by a systemd unit gated on the appearance of the MC
root container (`dprc.1`) and on a liveness probe that issues an MC command and
retries until it responds, rather than a timer or arbitrary temporal ordering. The
probe SHALL NOT depend on a `firmware_version` sysfs attribute, which is absent on
the target MC. The unit SHALL run to completion.

#### Scenario: Waits for MC readiness
- **WHEN** `dprc.1` is enumerated but not yet responsive
- **THEN** the reconciler does not begin provisioning until the readiness probe
  passes

### Requirement: Provisioning is ordered before network configuration
The provisioning unit SHALL complete before `networkd`/NetworkManager configures the
resulting interfaces, ordered before `network-pre.target`.

#### Scenario: Interfaces provisioned before network stack
- **WHEN** the system boots
- **THEN** DPNI provisioning completes before the network management stack attempts
  to configure the DPAA2 interfaces

### Requirement: Stable naming via MAC match, presentation-only udev
Stable interface names SHALL be applied by `systemd.link` files that match each
port's MAC address and set the desired name. These files SHALL be generated at
runtime from the topology into `/run/systemd/network/` (not shipped as static files
and not produced by a systemd generator). Because a port's matchable MAC lives on its
DPNI and does not exist until the DPNI is provisioned — by which point the kernel has
already named the netdev — the reconciler SHALL generate the `.link` files after
convergence and SHALL apply the rename to the existing interface via a per-interface
udev retrigger. udev/`systemd.link` SHALL be used only for renaming (presentation)
and SHALL NOT be part of the reconciliation trigger or fan-out.

#### Scenario: Netdev renamed by MAC match
- **WHEN** a provisioned DPNI's netdev carries the port's inherited MAC
- **THEN** it is renamed to that port's configured name via the generated
  `systemd.link` file in `/run/systemd/network/`

#### Scenario: Generated link config takes precedence
- **WHEN** a generated `10-dpaa2-<name>.link` and the stock `99-default.link` both
  exist
- **THEN** the generated file wins (first match) and applies the stable name

#### Scenario: udev does not drive reconciliation
- **WHEN** DPAA2 objects are created and emit uevents
- **THEN** no udev rule performs provisioning work in response

### Requirement: Fixed-link ports are handled without a rename stage
The system SHALL treat a fixed-link port as provisioned upon connection where the
connected DPMAC is fixed-link and `dpaa2-eth` does not create a netdev, and SHALL
NOT block or fail waiting for a netdev to rename.

#### Scenario: Fixed-link port needs no rename
- **WHEN** a fixed-link port is connected and produces no netdev
- **THEN** system integration reports it provisioned without attempting a rename

### Requirement: Board milestone covers lifecycle, VFIO, and end-to-end convergence
The change's board milestone SHALL comprise operator-launched suites for: the
full container lifecycle on scratch children (create, populate, plug, lock,
unlock, evict, destroy), an active VFIO bind/unbind on a scratch child
(driver_override, bind, override propagation to a subsequently-added child,
unbind, teardown), and end-to-end consumer convergence (declared intent →
`dpaa2-tools` converges the container; re-run converges to zero actions).
Suites are scratch-first and self-cleaning; use of the live VPP container is
permitted where a face requires it, is deliberate, and is recorded in the
suite's plan. Scripts assert the reference pair (MC 10.39.0 + Linux 6.6.52)
before running.

#### Scenario: End-to-end convergence diffs clean
- **WHEN** the operator runs the convergence suite with one declared consumer
- **THEN** the first run creates the container, the second run plans zero actions, and the read-back matches the derived model

#### Scenario: VFIO suite leaves no residue
- **WHEN** the VFIO suite completes (pass or fail)
- **THEN** its scratch containers and overrides are removed by the suite itself and the board object census matches the pre-suite baseline

### Requirement: The board milestone extends the V-DPNI series and amends the baseline
The change's board milestone SHALL be one operator sitting: a named
batch suite extending the V-DPNI series (option-profile creation walks,
sizing-field probes, `HAS_REPLICATION` accept/reject, unread-flag
probes, primary-MAC mutation) plus the online-MBT learning session.
Every probe outcome SHALL amend `docs/baseline/dpni.md` in the same
change — answers move items off the unknown-register list, silence is
recorded with a revisit trigger — and the restool-unreachable unknowns
SHALL be verified as deferral rows: the runtime `dpni_set_*` surface and
TX_CONFIRMATION_MODE to `mc-portal-backend` (#10) — emit v2 with an explicit
channel index, probe v1-handler retention (register #1) — `num_rx_tcs`-via-
DPL to `dpl-tape-out` (#14), table-write and traffic-dependent items to
their earliest reachable tile.

#### Scenario: Suite results close or defer every targeted unknown
- **WHEN** the board sitting completes and results are diffed
- **THEN** each targeted unknown-register item is either amended in the
  baseline with its evidence tag or carried as a named deferral row, and
  no targeted item is left unaccounted

#### Scenario: Suites leave the board clean
- **WHEN** any dpni suite finishes, pass or refuse
- **THEN** every scratch object it created is destroyed and the
  recovery baseline still verifies

### Requirement: One intent file converges the MVP setup end to end
From a single declarative intent file, the tool SHALL converge the
board to the first MVP workable setup: a kernel tenant's dpni in
dprc.1 with its regime-derived pool companions, connected to a
permitted dpmac and kernel-attached as a live network interface; and a
userspace tenant's child dprc.N populated with a dpni and its derived
companions and VFIO-bound, consumable by any userspace dataplane. The
convergence SHALL be idempotent and level-triggered with no persisted
state, and a re-run over the converged board SHALL be a no-op.
Sustained traffic is out of scope (roadmap #9); liveness means
kernel-attached and link-connected within the dpmac safety
constraints. At root scope drift heals upward only — root pool
capacity is grow-only at runtime — and teardown returns the DPL
baseline modulo the typed reboot-required residue (ADR-0020).

#### Scenario: Fresh board converges to both tenants
- **WHEN** the intent declaring one kernel tenant and one userspace
  tenant is applied to the reference baseline
- **THEN** dprc.1 gains the live kernel interface, dprc.N exists
  populated and VFIO-bound, and a second run emits an empty plan

#### Scenario: Drift heals to the intent
- **WHEN** pool objects are added or removed out of band (within the
  free set) and the tool re-runs
- **THEN** deficit recreated; undeclared never-plugged surplus
  pruned; plugged surplus reported as the typed reboot-required
  residue, never destroyed (ADR-0020); drawn and DPL-born objects
  untouched

#### Scenario: Teardown returns the baseline
- **WHEN** the tenants are removed from the intent and the tool
  re-runs
- **THEN** every consumer (the kernel dpni, the populated child
  dprc) and the child's residents are destroyed; runtime-created
  root pool capacity that was plugged — every family, not only the
  grow-only dpio seats — is reported as the typed reboot-required
  residue, observed vs. required stated and the reconciliation path
  named (ADR-0020, widening ADR-0003 §7), and that residue is the
  only census delta against the DPL baseline, which the closing
  reboot restores

### Requirement: The port surface is witnessed end to end on the board

The board milestone SHALL witness the full product path in one sitting:
one intent file anchored on a wired dpmac converges the kernel regime
through the shipped `dpaa2ctl`; the hooks read the typed surface
(arbitration state, MAC relation, attribute constancy, vocabulary
counters, carrier); the teardown exercises the typed sever-then-unbind
edge law; and the RemoteOwned arrangement (cross-container consumer
dpni, standalone driver holding the PHY) is read back across the
container boundary. Divergences SHALL feed back under the
validation-gaps triage order (implementation first, board state second,
characterization last), and baseline amendments from the sitting
(V-DPMAC-2's answer to unknown #1, the carrier observability rows) land
in the same change.

#### Scenario: One intent, both port regimes, idempotent

- **WHEN** the operator runs the Suite A ensure, then re-runs ensure
  and dry-run
- **THEN** the first run converges the port (kernel dpni bound,
  arbitration `KernelOwned`, MAC `Inherited`), and the re-runs plan
  zero actions with the port-detail view unchanged

#### Scenario: The sitting closes clean

- **WHEN** the sitting's suites complete and the closing census and
  reboot-recovery diff run
- **THEN** residue is zero against the clean-boot reference, the dpmac
  set is unchanged across the reboot (DPMAC-I1), and every verdict is
  recorded in VERDICTS.json with the evidence archived per board rules
