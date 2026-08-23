# ADR-0004: Southbound evolution — staged dual backend, MC v10 pin, one unsafe module

- **Status:** Accepted — scoping session 2026-08-22
- **Date:** 2026-08-22
- **Supersedes / relates to:** OpenSpec change `restool-baseline` (design D6);
  ADR-0001 §1 (the `McControl` seam this evolution happens behind);
  ADR-0003 (auditability motivates keeping restool board-side)

## Context

Today the southbound adapter (`dpaa2-mc`) shells out to `restool` v2.4. The
end state of the series is a native Rust transport speaking the MC's command
format over the DPRC device-node ioctl — restool without restool. Cutting
over in one step would replace the one component every board session can
audit (a human-readable command line) with a binary wire format, at exactly
the time the models are still learning what the MC does. The transition must
preserve the oracle while the replacement earns trust.

## Decision

### 1. The restool shim stays as executor and oracle

The restool-based `McControl` remains the board-side executor for MBT
(ADR-0003 depends on its auditability: every step is a visible command) and
the reference implementation the native transport is measured against. It is
not deprecated by the portal's arrival; it is the other half of the
differential gate.

### 2. The MC-portal transport lands mid-series, as its own change

The Rust ioctl transport (`mc-portal-backend`, change #10 in
`docs/ROADMAP.md`) lands only after the Tier-A models have stabilized —
after dprc, dpni, pool objects, dpmac, and dpseci each have a model that
survived board suites. The portal is then validated against *known* semantics
rather than discovering semantics and transport bugs at once.

### 3. Per-family migration behind the unchanged trait, gated differentially

`McControl` does not change shape for the migration. Families move from the
restool backend to the portal one at a time, each migration gated by
differential testing: the same plan executed through both backends must
produce identical observed state (and equivalent error surfaces) on the
frozen traces of that family. A family that fails the gate stays on restool
until the divergence is explained — in the model, the baseline document, or
the portal code.

### 4. MC v10 only, single version, asserted at startup

The portal implements the MC v10 command format exclusively — the format
this board's firmware (10.39.0) speaks and the one restool's `mc_v10/` tree
anchors. At startup the transport reads the firmware version and refuses to
run against anything it was not built for. Multi-version marshalling is a
recorded non-goal; the revisit trigger is a second board on different
firmware.

### 5. Unsafe is confined to the single ioctl call-site module

The workspace-wide `unsafe_code = "forbid"` (ADR-0001 C4) stands everywhere
except one module in `dpaa2-hal` (the kernel-primitives crate added by the
2026-08-23 amendment, ADR-0001 §6; this section originally said `dpaa2-mc`):
the ioctl call site. That module carries the
crate-level opt-out, the safety comments, and nothing else. Command
marshalling — the bulk of the portal — is safe, explicit little-endian
serialization; no `#[repr(C)]` transmutes, no pointer casts outside the one
module.

### 6. Notes carried to change #10 (amended 2026-08-23)

Triaged from the pre-series port notes and recorded here so #10's
just-in-time proposal starts from them rather than rediscovering them:

- **Struct layout fidelity** is the core hazard of hand-written
  little-endian serialization (§5): every command's field/bit layout must be
  anchored to restool's `mc_v10/` tree and covered by encode tests.
  Byte-level fixtures come from recorded restool traces (captured ioctl
  payloads), not from linking the C flib — an FFI reference oracle was
  considered and rejected, because the restool binary already *is* the C
  code behind a process boundary and the differential gate (§3) covers it.
- **ioctl numbers and magic values** must match the kernel driver exactly.
  They live outside the MC command format itself, so they need their own
  checked constants; nothing else in this ADR covers them.
- **The error-surface mapping** (MC status → errno-style → reported error)
  must be enumerated explicitly: §3's "equivalent error surfaces" clause is
  only checkable against an enumerated mapping.
- **Handle lifecycle is a typestate.** The MC ABI is token-based (open →
  command-by-token → close); the typed handle witnesses a successful open
  per ADR-0002 §3, making use-after-close and double-close unrepresentable —
  the C tool avoids them only by bool-flag convention.
- **miri and fuzzing were considered and dropped**: miri cannot execute the
  ioctl the single unsafe module exists to make, and the portal decodes
  responses only from firmware the §4 startup assertion pins. The lean
  equivalent is property-based encode/decode round-trips in #10's normal
  test suite.

## Consequences

**Positive**

- The board keeps an auditable executor for the whole learning phase; the
  binary transport is only trusted family-by-family, on evidence.
- A portal bug cannot masquerade as an MC discovery: the differential gate
  attributes divergence to a backend.
- The unsafe surface is one module to review, forever.

**Negative / to watch**

- Two backends must be maintained until the last family migrates; the
  unchanged trait and shared frozen traces bound that cost.
- The single-version pin means a firmware update on the board is a stop-the-
  world event for the portal (the startup assertion will refuse); this is
  deliberate — silent misinterpretation of a changed wire format is the
  failure mode being bought off.

## References

- OpenSpec change `restool-baseline`, `design.md` D6.
- `docs/ROADMAP.md` — change #10 `mc-portal-backend` and its dependencies.
- ADR-0001 — the `McControl` trait seam and the workspace unsafe policy.
- `src/restool` `mc_v10/` — the wire-format anchor for the portal.
