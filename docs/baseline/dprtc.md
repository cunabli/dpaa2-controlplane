# dprtc baseline

<!-- Instantiated from _template.md; every section mandatory, empty sections
     state so explicitly (spec: object-baseline). -->

Claim markers, used per claim throughout: **[read]** = derived from source or
manual, not yet observed; **[verified]** = observed on the board against the
pinned reference environment (see `reference-environment.md`).

Findings are written to be provable: every behavioral claim names its
**observables** (which query shows it) and, where it constrains or enables a
transition, is distilled into the Invariant candidates section below as a
precise proposition a Quint model can carry — either invariant-bearing (a
property the model must uphold) or invariant-breaking (a plausible belief
the corpus shows is false, which the model must not encode).

## Command surface

What restool exposes for this family (`create`, `destroy`, `info`, `update`,
…), with the ioctl/MC command each maps to where known.

_Not yet populated._

## Option inventory: used vs available

Every option of every command, each marked **used** (by which of
`ls-main`/`ls-debug`/`ls-append-dpl`, with the default value it passes) or
**available-but-unused** (with semantics from source or manual).

_Not yet populated._

## Attribute mutability

Create-time-immutable vs mutable-at-runtime, per attribute. Immutable-
attribute drift is refused, never repaired, by the reconciler; this
classification feeds the typestate design.

_Not yet populated._

## MC API notes

MC 10.x command-format details, version-gated behavior from `mc-utils/api`
deltas, firmware-side semantics not visible in restool's C.

_Not yet populated._

## Kernel-side behavior (Linux 6.6.52)

Driver binding, allocation from container pools, sysfs/netdev surfaces, udev
reactions — where the kernel, not restool, defines the observable semantics.

_Not yet populated._

## Lifecycle ordering and dependencies

What must exist first, what allocates vs creates, ordering constraints for
create/connect/plug/destroy.

_Not yet populated._

## Intent mapping

Which network construct(s) of the intent layer this family serves, and the
derivation rules that size or place it.

_Not yet populated._

## Silent-failure notes

Known ways this family fails quietly (misconfiguration accepted, drops with
no counter, state that looks converged but is not). Feeds the loudness
invariants of the models.

_Not yet populated._

## Invariant candidates

The section's findings distilled into checkable propositions, one row per
candidate: a stable id (`dprtc-I<n>`), the proposition (state predicate,
transition precondition/postcondition, or temporal property — precise enough
to transcribe into Quint), the observables that check it, and its status
(candidate / board-pending / verified / refuted). Negative knowledge is
listed too, as invariant-breaking entries ("the model must NOT assume …").

_Not yet populated._

## Unknown / unverified register

Claims that could not be established from the corpus, recorded as candidates
for board validation — never omitted, never guessed.

_Not yet populated._
