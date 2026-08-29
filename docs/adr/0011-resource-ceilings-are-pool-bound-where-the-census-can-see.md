# ADR-0011: Resource ceilings are pool-bound where the census can see, and two places it cannot

- **Status:** Accepted — board sitting 2026-08-29 (task 5.11, suite
  V-CEIL-1); §3 was ambivalent after rev 1 and settled by the rev 2
  sitting the same day; §2 stays an open question rather than a forced
  pass/fail cell
- **Date:** 2026-08-29
- **Supersedes / relates to:** OpenSpec change `verify-foundation` tasks
  5.11 and 6.3; ADR-0006 (single writer; the uapi finding in its
  amendment); ADR-0010 (ids are reused); `docs/baseline/dprc.md`
  (`mc.global --resources`), `docs/baseline/dpmcp.md`, `docs/baseline/dpni.md`

## Context

The firmware's own container answers `dprc show mc.global --resources`
with per-pool free counts (task 5.10: buffer pools, MC portals, frame
queues, congestion groups, queuing destinations, and a dozen WRIOP
tables). A reconciler that creates objects needs to know when a create
will be refused and whether a destroy gives the resource back —
otherwise "converge to N dpnis" is a plan it cannot judge before
issuing. Task 5.11 ran create-until-refused per family in a scratch
child container, reading the pools before, at the ceiling, and after
the family's destroys, for dpbp, dpcon, dpmcp, dpci, dpdmai, dpdcei and
dpni, with a cap of 64 per family.

## What the board answered

- **A dpbp is refused exactly when the buffer-pool count reaches zero.**
  The pool read 63 free; 63 creates succeeded, the 64th was refused
  with `No resources` (status 0x8), the count read zero at the ceiling,
  and every destroy returned its unit. The pool count is the ceiling,
  and the census can predict the refusal to the object.
- **dpcon, dpmcp, dpci, dpdmai and dpdcei reach 64 without a refusal**
  in a scratch child, and — dpmcp aside, below — their pools return to
  the pre-family value after the destroys. dpci's ceiling, which task
  5.10 left at "above 19", is now "above 64"; the cap, not a pool, ended
  each family.
- **A dpni is refused at the 18th object on the board** (17 created on
  top of the boot dpni) with `No resources`, and every pool the
  firmware lists still had headroom at that moment: frame queues 1913
  of 1981, congestion groups and queuing destinations 219 of 253, the
  WRIOP key-profile and policy tables all above 180. Each dpni drew a
  fixed slice — 4 frame queues, 2 congestion groups, 2 queuing
  destinations, 3 policy entries, 1 of several per-port tables — and
  all of it came back on destroy. Whatever refused the 18th is not in
  the listing.

## Decision

1. **The model treats a family's ceiling as its gating pool's free
   count when the listing names that pool, and as unknown otherwise.**
   dpbp's law is anchored; the other families' pools are large enough
   that a scratch container never meets them, and the model carries no
   number for them. The reconciler plans against the census's free
   counts and treats a refusal below the count as drift to report, not
   a state to model.

2. **The dpni ceiling is an unlisted resource until named.** The
   working explanation is a fixed per-object table the firmware does
   not expose through `--resources` — the DPC's object budget for the
   family, or a per-WRIOP-port resource with a count of 18 on this
   board. It is not modeled as a pool. The reconciler must expect a
   `No resources` on a dpni create with every listed pool showing room,
   and report it as the family's ceiling, not as a pool exhaustion.

3. **Destroying a dpmcp does not return its MC portal for the rest of
   the boot — a firmware leak, not a container quota.** The portal count
   read 200 before the dpmcp family, 138 at the ceiling (64 creates, 62
   drawn — the last two were satisfied from somewhere the listing does
   not show), and 138 again after all 64 destroys succeeded without an
   error. Rev 1 could not tell a leak from a quota held by the scratch
   child; the rev 2 sitting read the pools after the child itself was
   destroyed — still 138 — and after the reboot — 203. The reconciler
   therefore treats dpmcp create/destroy churn as consuming a fixed,
   unreplenished budget of 203 portals per boot: portals are created
   once and reused, never recycled through destroy. Every other family
   returns to its pre-value on destroy.

## Consequences

- `models/board/baselines/snapshot.sh` captures `dprc show mc.global
  --resources` from this sitting on, and the snapshot JSON carries the
  pool counts, so every future post-teardown and post-reboot diff shows
  whether a pool came back. That is the instrument that settled §3 on
  the rev 2 sitting; the reference snapshot carries the post-reboot
  pool counts so the comparison always has both sides.
- `docs/baseline/dpmcp.md` carries the leak as DPMCP-I6 (board-settled)
  and `dpni.md` the ceiling of 18 as an unknown, with the numbers above,
  so the next reader does not re-derive them.
- The dpbp law lands in `dpbp.md`'s invariant table as verified; the
  "above 64" ceilings replace the earlier bounds in `dpci.md`.

## Open questions and revisit triggers

- **Answered (rev 2, 2026-08-29): a destroyed dpmcp's portals do not
  return when its container is destroyed.** The checkpoint after the
  scratch child's teardown read 138, the post-reboot capture 203. A
  firmware leak within a boot; the reconciler never destroys a dpmcp.
  Worth reporting upstream with the numbers.
- **What refuses the 18th dpni?** A DPC read (the object budget) or a
  create with a smaller shape (fewer queues, no per-port tables) that
  reaches a different count would name it. Until then, 18 is the
  number for this board's DPC and nothing else.
- **Why 62 for 64 dpmcp creates, and why 203 to 200 before the family
  runs?** Both sittings read 200 at their first checkpoint, after five
  (rev 2) and six (rev 1) dpmcps had been created and destroyed by
  earlier suites — neither one per object nor zero. Either the listing
  lags, or some portals come from a reserve it does not count. A run
  that reads the pool after each create and each destroy resolves it.
- Any firmware or DPC change re-anchors all three numbers.
