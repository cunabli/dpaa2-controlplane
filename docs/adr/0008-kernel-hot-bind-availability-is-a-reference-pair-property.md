# ADR-0008: Kernel hot-bind availability is a per-family property of the reference pair

- **Status:** Accepted — board sitting 2026-08-23 (V-LIFE-DPIO-1,
  V-LIFE-DPSECI-1, V-LIFE-DPDMAI-1, rev 2), extended the same day with
  the bisect that root-caused the bystander unbind (§4–§6)
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
a bind. The remaining true rows (dpsw, dpdmux, dprtc, dpmac, dprc) stand
on the driver being present and are not yet board-anchored.

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

### 6. Sitting risk

Any suite that destroys objects in the Linux root can, through the race
above, silently unbind an arbitrary bystander in that container — a
dpni carrying traffic just as easily as a dpseci. The unbind leaves no
log entry and the object's directory stays in place, so only a driver
link check finds it. Post-sitting health checks should read the boot
dpseci's driver link explicitly, and any anomaly means a reboot before
the next sitting, since neither the crypto namespace nor a detached
driver recovers within a boot.

## Open questions and revisit triggers

- **Why does the firmware hand back a stale plugged bit?** Everything up
  to that point is anchored in the kernel source; this last link is not
  checkable from it. The working explanation is that fetching objects by
  index while destroy commands are in flight can return a torn or stale
  descriptor, because the object table shifts underneath the index. To
  confirm the timing end to end, rerun the sequence that killed the
  bystander with pauses between the teardown's unplug and destroy
  commands: if the kill stops happening, the race is established. Any
  firmware update re-anchors this question from scratch.
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
- **The unanchored true rows.** dpsw and dpdmux are marked available on
  the strength of their drivers being loaded, and their positive faces
  are batch 3. dprtc and dpmac are boot-born, so their rows are vacuous
  until something creates one at runtime.
