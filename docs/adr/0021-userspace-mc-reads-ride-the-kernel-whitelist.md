# ADR-0021: Userspace MC reads ride the kernel whitelist

- **Status:** Accepted — dpseci-typestate task 3.1 (bead
  dpaa2-controlplane-lbk; the MC-ioctl read-slice decision settled in the
  proposing grilling session)
- **Date:** 2026-10-03
- **Supersedes / relates to:** ADR-0018 (names dpaa2-hal the workspace's
  single unsafe crate and the home of "roadmap row 10's ioctl transport";
  this record lands the read slice of that transport ahead of the tile);
  ADR-0003 §5 (the fsl-mc-uapi whitelist as the capability frontier);
  ADR-0007 (restool exits 0 on an MC refusal — why the header status, not
  the exit code, carries the verdict); `docs/baseline/mc-ioctl-policy.md`
  (the whitelist table, the single source of what the primitive may ever
  encode); `docs/baseline/mc-status.md` (the MC status vocabulary); the
  `mc-portal-backend` change (roadmap #10), which inherits this primitive
  as its read core and owns all growth of the command vocabulary

## Context

A dpseci object's deciding hazard — whether its create options carry the
congestion backstop (DPSECI-I3) — is observable only from the object's
`GET_ATTR` options mask. restool fetches that mask and discards it at
print, so no restool verb can witness it. The control plane therefore
needs a userspace path to the MC that restool does not give it.

The kernel already fixes the boundary of what any userspace client may
ask the MC. The fsl-mc-uapi driver checks every command sent through a
`/dev/dprc.N` node against a fixed whitelist (`fsl_mc_command_check`) and
refuses an off-list command with `-EACCES`, regardless of privilege
(ADR-0003 §5; `docs/baseline/mc-ioctl-policy.md`). restool crosses that
same ioctl path, so the whitelist is an objective capability frontier,
not a restool artifact.

The full ioctl portal backend — writes, the differential gate against the
restool shim, per-family migration — is roadmap #10. dpseci needs only
the read slice now, and bringing the whole backend forward to serve one
observable would fragment #10's design.

## Decision

Userspace MC reads ride the kernel's own whitelist. `dpaa2-hal` gains the
`/dev/dprc.N` MC-command primitive (`portal.rs`) as the proven read core
that #10 inherits, fenced structurally rather than procedurally.

**The command vocabulary is a closed read sum.** The primitive encodes
exactly the whitelisted reads a dpseci read-back consumes — OPEN,
GET_ATTR, GET_API_VERSION, DPSECI_GET_TX_QUEUE, CLOSE — as the variants of
one Rust enum. No variant carries a create, destroy, or set command id, so
a write is unrepresentable by construction; the fence is the type, not a
review rule. `docs/baseline/mc-ioctl-policy.md` is the single source of
what the primitive may ever encode, and #10 owns every addition to the
vocabulary.

**The primitive is policy-free (ADR-0018).** It encodes the 64-byte
command, performs the ioctl, and types three outcomes, judging none:

1. a decoded response, when the MC answers `OK`;
2. an MC status other than `OK`, read from the response header (the
   firmware judged the command and refused — restool sees this as exit 0,
   ADR-0007); the status vocabulary is `docs/baseline/mc-status.md`;
3. a transport refusal — the kernel whitelist `-EACCES`, or
   `ENOENT`/permission on the device node — surfaced as the `io::Error`
   of the result.

Retry, tolerance, and error mapping belong to `dpaa2-mc`, as with the
restool shim.

**Unsafe is confined and asserted.** This is the workspace's unsafe debut.
`dpaa2-hal` restates the workspace lints with `unsafe_code = "deny"` in
place of the workspace-wide `forbid` (the other five crates keep `forbid`
— a crate scope cannot loosen `forbid`), and the sole `#[allow]` sits on
the one ioctl call. Encoding and decoding are transmute-free byte
operations (`to_le_bytes`/`from_le_bytes` over a `[u8; 64]`); no `repr(C)`
struct and no transmute are needed. Every layout constant — the ioctl
request code, the command ids, the header and parameter offsets — is
asserted against the pinned kernel/restool sources and golden-tested on
fixture frames, so a wrong layout fails a unit test, never the board.

## Consequences

- The dpseci options mask (DPSECI-I3) becomes an implemented convergence
  observable; the HAS_CG backstop is read, not trusted.
- The read slice is board-witnessed (V-DPSECI-3 rev 2: `GET_ATTR` +
  `GET_API_VERSION` over `/dev/dprc.1` on a live object), and the routing
  law — the read rides the root container's node, never a child's — is
  recorded at the shim.
- #10 starts from a read-only primitive proven against the board, with no
  write or differential machinery pre-decided by its shape.
- The one unsafe site in the workspace is a single reviewed ioctl call;
  the other crates stay `unsafe_code = "forbid"`.
- A new whitelisted read for another family extends the closed sum under
  #10's ownership; the primitive never grows a write path without an
  amendment here.

## Alternatives rejected

- **restool-pure.** Keep every observation on restool. The safety bit
  (DPSECI-I3) then stays write-trusted for the whole dpseci tile — a
  hollow end-to-end witness, since the deciding hazard is never actually
  read back.
- **Reads-plus-validation-probe.** Debut the transport with a mutation,
  to also settle MC-layer create validation (baseline unknown #1). That
  makes the first command through a hand-rolled transport a write; the
  unknown is not worth the mutation-debut risk. CREATE stays restool until
  #10's differential gate backs it.
