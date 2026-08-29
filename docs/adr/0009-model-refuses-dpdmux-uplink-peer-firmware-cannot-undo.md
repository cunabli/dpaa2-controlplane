# ADR-0009: The model refuses a dpdmux uplink peer the firmware accepts but cannot undo

- **Status:** Accepted — board sitting 2026-08-29 (V-DPDMUX-2 rev 1
  through rev 5, all diverged from the model); rev 5 is final
- **Date:** 2026-08-29
- **Supersedes / relates to:** OpenSpec change `verify-foundation` task
  5.9; `models/core/connect.qnt` `legalPorts`; ADR-0003 (board evidence
  protocol); ADR-0007 (single-hop moves and creator-bound destroy)

## Context

The model forbids connecting a dpni to a dpdmux uplink: the uplink
(interface 0) may only face a dpmac. V-DPDMUX-2 was authored to issue
that model-forbidden pairing on the board through a suite hook (the
illegal action is a disabled action, so it cannot be traced) and record
what the firmware actually does. Two revisions ran on the pinned pair
(MC 10.39.0, kernel 6.6.52), and both diverged from what the model
predicts.

The observed behaviour is a one-way door. MC 10.39 **accepts**
`dprc connect` of a dpni onto the dpdmux uplink named by its bare object
name; `dpdmux info` then reads `interface 0: connection: dpni.N`. The
disconnect is then **refused** — from the demux end (rev 1 and rev 2)
and from the dpni end (rev 2 as well), each with Configuration error
(status 0x6). The pairing stands until an object is destroyed (the
teardown destroys were accepted). The downlink control was inconclusive
in both revisions because the dpni was still busy on the uplink: a
connect to interface 1 came back Configuration error, and a disconnect
of the unconnected interface 1 from the demux end came back No resources
(status 0x8).

Rev 3 (2026-08-29) tried the downlink control from a fresh state with no
uplink populated, and found a refused connect that still connects.
Asking to connect the dpni onto the downlink (interface 1 — restool's
endpoint parser sends `if_id=1` for the second endpoint) came back
Configuration error (0x6), yet the read-back immediately showed
`interface 0: connection: dpni.N` and interface 1 still none: the refused
command had landed the dpni on the uplink. The firmware's answer and its
state disagree — a refusal that still leaves a connection behind — which
is exactly the class of behaviour ADR-0003's read-back rule exists for.
Everything after matched revs 1 and 2: the disconnect was refused from
either end, the subsequent bare-name uplink connect was refused (the dpni
already busy), and the pairing stood until the teardown destroys removed
it.

Rev 5 (2026-08-29) ran from a fresh boot, 97 s in, and settled the
downlink cleanly for the first time. Before any connect the dpdmux read
interface 0 none, interface 1 none, and the dpni endpoint none: no
pairing survives a reboot, which refutes rev 4's standing hypothesis
that one might. The downlink connect (`dprc connect … --endpoint1=
dpdmux.N.1 --endpoint2=dpni.M`) was **accepted** with no stderr, and
this time the answer and the state agreed — `dpdmux info` read
interface 1 as the dpni with interface 0 still none, and `dpni info`
read its endpoint as `dpdmux.N.1`. The disconnect was then refused from
every end: from the dpni end Configuration error (0x6), from the demux
downlink end (`dpdmux.N.1`) Configuration error (0x6), and from the
demux uplink end (the bare name = interface 0, where nothing is
connected) No resources (0x8). So on this firmware no end can
disconnect a dpni from a dpdmux, uplink or downlink alike. Destroying
both objects and re-creating them brought the same ids back with no
pairing on them, so a pairing does not survive destroy-and-recreate
either. Rev 3's shape — a refused connect that still left a connection
behind — did not reproduce on this fresh boot; it and rev 5's clean
accept both stand as observed, unexplained, on different boots. Rev 5's
hook then ran an in-run phase-4 destroy that tripped ADR-0008's rescan
race and took the management interface down; that incident and the rule
it produced are recorded in ADR-0008 §7, and phase 4 is removed from the
committed hook.

The refusal is the firmware's, not a kernel driver's. restool prints the
errno of the `/dev/dprc` ioctl; the kernel's `fsl-mc-uapi.c` returns
only `-EACCES`/`-EPERM`/`-EFAULT`/`-ENOMEM` of its own, while `-ENXIO`
("Configuration error") comes only from `mc_status_to_error()` mapping
the MC response header. Both objects were unplugged and dmesg was silent
throughout, so no driver was involved — the acceptance and the refusal
are both the MC's own answer.

Three sources disagree about whether the pairing should be legal at all.
The DPAA2 manual (dpdmux chapter) says the uplink "can be an internal or
external interface". The MC firmware changelog for 10.37 says it
"imposed the connection restrictions of a DPDMUX … a DPDMUX uplink was
only supposed to be connected to a DPMAC object". The pinned 10.39
accepts the dpni and then cannot undo it — matching neither the manual's
permissiveness nor the changelog's restriction cleanly.

## Decision

### 1. `legalPorts` keeps the dpmac-only uplink rule

The control plane refuses the dpni-on-uplink pairing ahead of the
firmware. The firmware accepts a pairing it cannot undo, so a reconciler
that let the connect through would reach a state it can leave only by
destroying an object. Keeping the model stricter than the firmware is
the safe choice: the control plane never issues a connect it could not
later reverse.

### 2. The suite asserts the observed behaviour as conformance evidence

V-DPDMUX-2 records the firmware's actual answer — accepted, then
un-disconnectable from any end — as the conformance oracle. The
refusal the model predicts is issued by the model, not by the board, so
the suite passes when the board reproduces the observed accept/refuse
shape. Rev 5 is the suite's final shape: it asserts what rev 5 observed
— a fresh-boot downlink accepted with the state agreeing, no end able to
disconnect, and no pairing surviving a reboot or a destroy-and-recreate
— and no further revision runs. Revs 1 through 4 stand as the divergence
and the successive controls that motivated this record.

### 3. The reconciler treats "dpni on an uplink" as an unreachable state

If a read-back ever shows a dpni sitting on a dpdmux interface — uplink
(interface 0) or downlink (interface 1) alike — the only convergence
path is destroy-and-recreate, because the firmware accepts the pairing
and then refuses the disconnect from every end. That must surface as an
explicit plan step, never as a silent disconnect attempt the firmware
would reject. The reconciler carries any dpni-on-a-dpdmux state as
unreachable-by-construction and, if reality contradicts that, plans the
destroy openly.

## Consequences

The cost is representational: a topology the manual calls legal (a dpni
on the uplink) is unrepresentable in the model, so the tool cannot build
one even where a future firmware might support it cleanly. The benefit
is that no reconciler run can leave a stuck pairing behind — the control
plane never enters a state whose only exit is destroying an object,
because it never issues the connect that reaches it. Given the firmware
cannot undo the pairing, a stricter control plane is worth the lost
expressiveness.

The downlink is a separate case the model does not guard. `legalPorts`
only restricts the uplink, so a dpni on a dpdmux downlink
(`dpdmux.N.1`) is legal in the model, and rev 5 confirms the firmware
accepts it — but it is now known to be equally un-disconnectable. The
model is left as it is; the consequence is that any dpdmux↔dpni edge,
whichever interface it lands on, is destroy-only on this firmware, so
the reconciler must plan its removal as a destroy rather than a
disconnect. `dpdmux-typestate` (#12) owns encoding that downlink edge as
permanent-until-destroy when it takes up the family's typestate.

## Open questions and revisit triggers

- **A newer MC firmware changes the answer.** If a later release refuses
  the connect, or lets the disconnect succeed, the one-way door closes
  and the guard can relax. Re-anchor from scratch on any firmware
  update.
- **The ioctl portal with an explicit endpoint.** restool's endpoint
  parser turns a bare object name into `if_id` 0 for connect and
  disconnect alike, so every disconnect this suite issued named
  interface 0 explicitly. Whether the raw portal, sending
  `DPRC_DISCONNECT` with a different `if_id` or endpoint ordering, gets
  a different answer is untested — confirm it when
  `mc-portal-backend` (#10) lands.
- **Rev 3's refused connect that left state behind.** Rev 3 saw a
  downlink connect refused (Configuration error) leave the dpni on the
  uplink anyway; rev 5, from a fresh boot, saw the downlink connect
  accepted cleanly with the state agreeing, and the rev-3 shape did not
  reproduce. The two stand on different boots. Before treating rev 3 as
  real behaviour rather than a one-boot artifact, reproduce it on a boot
  that has seen prior create/destroy churn — the condition rev 3's boot
  had and rev 5's fresh boot did not.
- **Does a dpmac on the uplink disconnect cleanly at all?** V-DPDMUX-1
  only ever destroyed its dpmac-uplink pairing, never disconnected it,
  so whether the legal uplink peer can be disconnected is still unknown.
  Rev 4 was to probe it, but its dpmac-uplink connect was refused
  because the rev-3 ghost still held the uplink, so the disconnect was
  never issued; rev 5 skipped the uplink face entirely (the dpni pairing
  still stood). The question is unissued, deferred to #12.
- **The same shape for dpsw ports.** Whether a dpsw port exhibits the
  same accept-then-cannot-undo behaviour is unprobed; revisit when a
  dpsw port-connect suite is authored.
