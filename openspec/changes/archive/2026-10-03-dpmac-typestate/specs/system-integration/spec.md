# system-integration — dpmac-typestate delta

## ADDED Requirements

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
