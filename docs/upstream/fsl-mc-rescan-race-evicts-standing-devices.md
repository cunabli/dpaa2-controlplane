# fsl-mc container rescan races object removal and evicts a standing device

Finding 53 of `docs/upstream/findings.md`; the full control-plane
diagnosis lives on ADR-0008 (§4, §9, §10) and this file is the
self-standing kernel-facing description. It is the same race finding 39
names, widened by a cross-container dpni↔dpni sitting to two verbatim
signatures and the probabilistic, probe-independent shape.

## Environment

- LX2160A-class board (DT), Linux 6.6.52 LF tree, MC firmware 10.39.0,
  restool v2.4.
- A root container (`dprc.1`) holding, among its boot residents, the
  management interface dpni.0 bound to `fsl_dpaa2_eth`.

## Reproduction

A management flow runs a burst of back-to-back bus-removing operations in
the root container — driver unbind, `dprc assign --plugged=0`, and
`<type> destroy` in any mix, several within a few seconds. Each removal
raises a DPRC object-removed interrupt; the container's threaded IRQ
handler rescans the container's object table while the next removal is
already in flight. Any restool script that tears down several root objects
in a tight sequence reproduces it.

## Observed traces

The scan walks the object table one index at a time and a fetch that
fails mid-walk is logged and skipped. Two sittings of the same suite:

```
dprc_get_obj(i=110) failed: -119
1 out of 111 devices could not be retrieved
```

```
dprc_get_obj(i=94) failed: -119
dprc_get_obj(i=115) failed: -119
2 out of 116 devices could not be retrieved
```

`-119` is `-ENAVAIL`, the firmware's "no resources" status for an index
it cannot serve at that instant. Shortly after the `-119` lines an
*unrelated standing* object loses its driver: the management interface
dpni.0, with the signature

```
dpni.0 eth0: Link is Down
fsl_dpaa2_eth dpni.0: driver ... -> (none)
<max frame length reset 10240 -> 1536>
```

within tens of milliseconds of the enumeration errors, and its DPL
allocatables released back to the bus. The eviction is probabilistic: one
sitting lost dpni.0, another ran the same sequence and survived because
its removals were refused and raised no events, thinning the burst. It
does not require a failing probe in the burst — a burst with no probe
evicted dpni.0 all the same.

## Analysis pointers

- `dprc_scan_objects` (`drivers/bus/fsl-mc/dprc-driver.c`) asks the
  firmware how many objects the container holds, then fetches them by
  index with nothing holding the firmware still between fetches; the
  object table shifts under the index as removals land, so a fetch
  returns `-ENAVAIL` (the `dev_err` sites log the two messages above) and
  the loop carries on with a short table.
- The racing scan is interrupt-driven, not command-driven: the
  container's `dprc_irq0_handler_thread` rescans on every object-added/
  removed event, so it is already walking when the next removal arrives —
  nothing the caller can serialize against.
- `check_plugged_state_change` then sees a plugged object whose freshly
  fetched descriptor (from the torn walk) reads unplugged and calls
  `device_release_driver`, detaching the driver while leaving the device
  registered and logging nothing — matching dpni.0 losing its driver with
  no log line of its own. The index shift is why the loss lands on a
  bystander rather than on a removed object.
- Suspected locus: the dprc IRQ-driven rescan racing the non-atomic
  per-index `dprc_get_obj` enumeration. The one link not checkable from
  the kernel tree is why a plugged, untouched object's descriptor comes
  back unplugged (ADR-0008 open questions).

## Mitigation

Spacing removals apart (pause after each until the walk finishes) lowers
the probability but does not close the race — it is a timing
accommodation, not a lock, and bursts on object-add events are exposed
too. The root-cause fix is kernel-side: re-read the descriptor before
`device_release_driver` releases a bound driver on a plugged→unplugged
transition, so a torn read becomes a no-op (the finding-39 minimal shape,
`dprc-driver.c`). Recovery on the running board is a reboot, which
restores the DPL and rebinds the evicted interface; a sysfs rebind of the
boot dpni also re-attaches it.

## Status

Candidate. Bus-level code (`drivers/bus/fsl-mc/`), so upstream-relevant
beyond the LF tree. Same race as finding 39; this note carries the
cross-container-sitting signatures and the probabilistic, probe-
independent shape. Evidence: the V-TRAF-1 ledger row in
`models/board/README.md`.
