# ADR-0008: Kernel hot-bind availability is a per-family property of the reference pair

- **Status:** Accepted — board sitting 2026-08-23 (V-LIFE-DPIO-1,
  V-LIFE-DPSECI-1, V-LIFE-DPDMAI-1, rev 2), extended the same day with
  the bisect that root-caused the bystander unbind (§4–§6) and with the
  batch-3 sittings (V-DPSW-1, V-DPDMUX-1) that anchored the two
  remaining available rows and settled the spacing experiment;
  extended 2026-08-29 (V-DPDMUX-2 rev 5) with §7 — a hook that runs
  its own destroys re-opens the window the teardown's spacing had
  closed
- **Date:** 2026-08-23
- **Supersedes / relates to:** OpenSpec change `verify-foundation` (task
  5.2, the per-family lifecycle suites); ADR-0003 §2 (evidence is
  read-back against a stamped reference pair)

## Context

Three lifecycle suites drove the canonical forward order for a
root-resident object — companion created and plugged, consumer created
and plugged, bus rescan — and then waited for the kernel's own probe.
All three read back an unbound object. The model expected a bind, so the
harness scored three failures.

The model was wrong, and in the same way each time: it read
"this family has a kernel driver" as "this family's objects bind". On
the stamped reference pair (MC firmware 10.39.0, kernel 6.6.52) that
inference does not hold. A driver can be loaded, correct, and still
refuse an object created after boot — for three different reasons, none
of them an MC or provisioning fault:

1. **dpio — the seats are already taken.** The probe logged
   `probe failed. Number of DPIOs exceeds NR_CPUS.` and returned -34.
   The kernel binds one dpio per CPU; the boot layout already provisions
   exactly that many, so every seat is occupied before userspace runs.
2. **dpseci — the registration is already claimed.** The boot-time
   dpseci probed successfully about ten seconds into the boot and
   registered its algorithms. Ours probed about three minutes in and was
   refused -17 (EEXIST) on every algorithm it offered. The crypto API's
   algorithm names are a single global namespace, and the first dpseci
   to probe in a boot takes them.
3. **dpdmai — the driver is not there.** No qdma driver is registered on
   the bus at all, and nothing about dpdmai or qdma appears in the kernel
   log. The reference kernel simply carries no consumer for this family.

In each case the probe *ran*: the driver was matched, took what it
needed, failed, and released it. The read-back after the failure shows
the companion still free and untouched, and the object unbound — the
kernel cleans up after its own failed probe.

## Decision

### 1. Availability is a parameter, not an inference

`FamilyParams` gains `hotBindAvailable`: does the reference kernel bind
an object of this family that was created, plugged and rescanned at
*runtime*? It is independent of `hasKernelDriver`, which stays a
statement about the driver's existence. dpni is true and board-anchored
(V-LIFE-DPNI-1). dpio, dpseci and dpdmai are false, each for the
mechanism above. Every driver-less family is trivially false, pooled
families included: allocator custody is modeled as pool membership, not
a bind. dpsw and dpdmux are true and board-anchored (V-DPSW-1,
V-DPDMUX-1) — with a condition on the dpsw row worth stating plainly:
its driver takes a runtime-created object only when that object was
built in the shape it accepts, control interface on and both the
flooding and broadcast domains scoped per FDB. Those are not the
defaults the command-line tool applies when unasked, so a
default-created dpsw is refused at probe, and the refusal is logged
rather than silent. dprtc, dpmac and dprc remain true on the strength
of their drivers being present; the first two are boot-born, so nothing
creates one at runtime to test.

### 2. A probe that cannot succeed changes nothing

`kernelBindAt` keeps every guard it had — the driver still needs a free
companion to attempt the probe, so the action is enabled exactly where
it was, and traces keep their shape. Only the transform is conditional:
where `hotBindAvailable` is false the state is left untouched. No bind,
and no census draw, because the failing driver gives back what it took.

### 3. The suites assert the negative face as conformance

The expected driver-link read-back is taken from the trace's post-state
rather than from the fact that a bind step was attempted. A suite for a
family that cannot hot-bind therefore *expects* an unbound object, and
passes when the board reports one. The three rev-2 captures stand as
passing evidence under this reading: nothing about the board's behavior
was reinterpreted, only the model's expectation of it. The traces were
re-frozen and the suites regenerated; the action sequences are
byte-identical to the ones the board ran, so the captures still answer
the scripts that produced them.

### 4. Destroys in the root container can silently unbind a bystander

The first write of this ADR could not say what had unbound the
boot-time dpseci. A bisect over fresh boots now names the chain, and it
is a bus-level hazard rather than anything about dpseci in particular.

A bare rescan on its own is harmless: the boot dpseci stayed bound with
its full set of registered algorithms. Running a lifecycle suite is not.
Both suites that ran destroys in the Linux root logged the same pair of
errors, in each case while their teardown was destroying objects:
`dprc_get_obj(i=98) failed: -119` and
`1 out of 99 devices could not be retrieved`. The second suite's run
ended with the boot dpseci's device directory still present, its driver
link gone, and its registered algorithms down from seventy-four to
twelve — with no kernel-log entry about that object at all beyond its
original boot probe.

Each step of that is anchored in the reference kernel tree
(`drivers/bus/fsl-mc/`):

- **The enumeration is not atomic.** `dprc_scan_objects`
  (`dprc-driver.c:245`) asks the firmware how many objects a container
  holds, then fetches them one index at a time. Nothing holds the
  firmware still in between. When a fetch fails the entry is marked
  invalid and the loop carries on (`dprc-driver.c:287-298`); the two
  messages quoted above are the `dev_err` sites at
  `dprc-driver.c:288` and `dprc-driver.c:317`. The `-119` is `-ENAVAIL`,
  which `mc-sys.c:55` maps from the firmware's "no resources" status —
  an index the firmware could not serve at that instant.
- **The racing scan is interrupt-driven, not command-driven.** The
  command device exposes no rescan at all
  (`fsl-mc-uapi.c` contains no scan call). Two paths do scan: the
  sysfs bus attribute, which is what `restool dprc sync` writes to
  (`rescan_store`, `fsl-mc-bus.c:235`, reached from restool's
  `echo 1 > /sys/bus/fsl-mc/rescan`) and which runs synchronously with
  that write; and the container's own threaded interrupt handler
  (`dprc_irq0_handler_thread`, registered at `dprc-driver.c:528`),
  which rescans whenever the firmware reports an object created,
  destroyed, added or removed (`dprc-driver.c:437-443`). Every destroy
  raises that event, so the handler thread is scanning the container
  while the next destroy is already on its way — no explicit sync
  needed, and nothing the caller can serialize against.
- **A stale plugged bit detaches the driver, silently.**
  `check_plugged_state_change` (`dprc-driver.c:145`) is reached for
  every object the scan already knows. If the freshly fetched
  descriptor has the plugged bit clear while Linux has it set, it calls
  `device_release_driver` (`dprc-driver.c:164`). That is the only place
  on this bus that detaches a driver while leaving the device
  registered, and it logs nothing — matching the symptom exactly.
  Removal proper (`fsl_mc_device_remove`) would have deleted the device
  directory, which the read-back shows still present, so that path is
  ruled out.

The one link that cannot be checked from the kernel tree is why a
descriptor for a plugged, untouched object came back with its plugged
bit clear. It is left as an open question below.

A later suite showed how much worse the same window can be. Its
teardown ran just two destroys, and a single scan window took the
drivers off *three* boot residents at once — a network object, a
physical port object, and the crypto object — leaving the crypto
algorithm list completely empty rather than partially so, and the
network object's detach taking the management interface down with it.
The same run also removed a boot portal object outright and re-added
it, which is the first direct sighting of the removal arm rather than
the silent-detach arm: two different endings from one race. It
reproduced on two consecutive fresh boots, so it is near-deterministic
rather than a rare coincidence.

What distinguishes that suite from the earlier ones, whose bystanders
survived, is the state of the object being destroyed: it was both
driver-bound and connected. Tearing that down raises more firmware
events than destroying a bare object does, so more of them land while a
scan is already walking. The window is not a fixed size — it widens
with how much the destroy has to undo.

### 5. Why the damage is permanent, and total

Two properties of the SEC driver turn a single stray detach into a
condition that lasts until reboot (`drivers/crypto/caam/caamalg_qi2.c`):

- **Registration state is driver-global, not per-instance.** The
  algorithm tables (`driver_algs[]` at line 1933, `driver_aeads[]` at
  2058, `driver_hash[]` at 4713) are static and shared by every dpseci
  the driver handles, including the `registered` flag on each entry
  (lines 56, 62) and the device back-pointer the probe writes into them
  (lines 5568, 5618). So one instance's removal unregisters the
  family's algorithms for everyone, and a second instance's probe
  collides with the first instance's registrations rather than making
  its own. This is the mechanism behind the availability row: the first
  dpseci of a boot owns the namespace, and it owns it on behalf of the
  driver, not itself.
- **A failing probe destroys the working instance's memory cache.** The
  probe overwrites a single static cache pointer unconditionally
  (line 42, assigned at 5486), and every error label in the probe falls
  through to destroying it (line 5700) — the same cache the healthy
  first instance is still using. Removal destroys it too (line 5745).

Three separate upstream-reportable defects fall out of this: the
non-atomic bus enumeration together with the silent driver release on a
stale plugged bit; the SEC driver's driver-global registration state;
and its cache lifetime. None is introduced by this port — all three sit
in the reference kernel as shipped.

### 6. Teardown spaces its destroys; sitting risk is contained, not gone

Any suite that destroys objects in the Linux root can, through the race
above, silently unbind an arbitrary bystander in that container — a
dpni carrying traffic just as easily as a dpseci. The unbind leaves no
log entry and the object's directory stays in place, so only a driver
link check finds it.

The fix follows from the mechanism rather than from guessing: if the
damage comes from a destroy landing while a scan is still walking, then
waiting for the walk should end it. Generated teardowns now pause after
every destroy, and only after a destroy — the unbind and unplug steps
raise no object event and need no pause. A re-sitting of the suite that
had cost three bystanders their drivers ran five destroys with no
enumeration error logged at all, no bystander touched, and the object
count back at its exact starting value. That is the whole fix, and it
belongs to the generator, so every suite gets it.

The risk is contained rather than eliminated. The pause is a timing
accommodation, not a lock: nothing prevents the race, and a slow enough
scan would still meet the next destroy. Post-sitting health checks
should still read the boot dpseci's driver link explicitly, and any
anomaly still means a reboot before the next sitting, since neither the
crypto namespace nor a detached driver recovers within a boot.

### 7. A hook that destroys widens the window the teardown had closed

The spacing of §6 is a property of the generated teardown, and only of
it. A suite hook runs before that teardown, with its own body of
commands, and nothing spaces those. V-DPDMUX-2 rev 5 proved the cost.
Its hook carried a phase-4 that destroyed a connected dpni↔dpdmux pair
and re-created it; the EXIT-trap teardown then destroyed the re-created
pair a second time. Four root-container destroys landed in about nine
seconds, two of them on a pairing the firmware had already refused to
undo (ADR-0009) — the densest burst of root destroys any sitting had
run. It hit the §4 race: 5.5 s after the run's marker the boot dpni's
netdev logged link down, and 0.6 s later a boot dpcon was re-added to
the bus (`Adding to iommu group`). The boot dpni lost its driver
silently — the MC still shows it plugged and connected to its dpmac,
because no command in the run named either object and the MC edge was
never touched — and the management interface was down until the driver was
rebound (a rebind or a reboot).

Two things are new for the record. The destroyed pair was connected but
*not* driver-bound, where §4's earlier casualties were both connected
and bound: an unbound connected object is enough to raise the events
that feed the race, so bindedness is not the threshold §4 read it as.
And the boot dpcon's remove-and-re-add is the second sighting of the
removal arm rather than the silent-detach arm — the first was the
two-destroy teardown §4 describes — so both endings of the one race are
now twice observed.

The root cause is script design, not firmware: the hook did by hand,
unspaced and doubled, the destroys the teardown is built to do once and
spaced. The rule that follows is a placement rule. A hook never
destroys or re-creates an object in the root container; only the
script's own teardown does, once per run, with the §6 spacing. A suite
whose *teardown* must itself destroy a connected pair is flagged in the
suite ledger as a **management-link risk**: run it last in a sitting,
and expect a rebind or a reboot after it. V-DPDMUX-2's phase 4 is
removed from the committed hook (the suite's other agent), leaving the
teardown as the only place a destroy happens.

## Open questions and revisit triggers

- **Why does the firmware hand back a stale plugged bit?** Everything up
  to that point is anchored in the kernel source; this last link is not
  checkable from it. The working explanation is that fetching objects by
  index while destroy commands are in flight can return a torn or stale
  descriptor, because the object table shifts underneath the index.
  The timing half of that is now settled — the spaced-destroy experiment
  below confirms the concurrency — but not the firmware half: nothing
  explains why one walk reported three live, untouched objects as
  unplugged in the same pass. Spacing makes the question unreachable in
  practice, since the condition no longer occurs, so answering it would
  need a deliberate reproduction rather than a sitting. Any firmware
  update re-anchors it from scratch.
- **Registrations outlive the binding.** Twelve algorithm entries
  remained registered after the holder was detached, which fits the
  driver-global tables above: the detach ran the removal path for some
  entries, and nothing frees a name that stays flagged as registered.
  Freeing the first dpseci therefore does not free the namespace, and no
  dpseci can bind for the rest of that boot — a reboot is the only
  reset. Revisit if a kernel upgrade changes the unregister path.
- **The table is pinned to one reference pair.** A kernel upgrade
  re-anchors every row: a qdma driver appearing would flip dpdmai, a
  change in how dpio seats relate to CPU count would flip dpio, and a
  fix to the crypto unregister path would flip dpseci. Treat the whole
  table as evidence about this pair, not as a property of the hardware.
- **The unanchored true rows.** dpsw and dpdmux are settled on the board
  (V-DPSW-1, V-DPDMUX-1). What remains unanchored is dprc, and dprtc and
  dpmac, whose rows are vacuous while nothing creates one at runtime.
- **Does spacing hold under a heavier teardown?** Partly answered by
  §7. A connected, unbound pair destroyed *twice* in one run — the
  hook's phase-4 destroy-and-recreate plus the teardown's second
  destroy, four root destroys in nine seconds — did not hold: it tripped
  the race and took the boot dpni's driver. What still holds is the
  single spaced teardown of a pair of that shape, which survived three
  times (V-DPDMUX-2 revs 2, 3, 4). So the open half is narrower now:
  spacing survives one teardown of a connected pair, but not a hook that
  destroys ahead of it, and a teardown that removes a container still
  holding residents remains unmeasured. Revisit when a suite of that
  last shape is authored.
