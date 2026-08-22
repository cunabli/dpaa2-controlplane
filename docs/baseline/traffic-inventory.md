# Traffic inventory: validation scenarios vs the port safety matrix

Stub — populated by task 6.2 (spec: object-baseline, "Validation scenarios
are traffic-classified against the port matrix"). Every planned validation
scenario in the series is classified here as exactly one of:

- **object-lifecycle-only** — MC-bus mutations only, no link semantics;
- **link-signaling** — asserts or observes link state, no frames;
- **traffic-bearing** — frames on a wire; explicitly flagged, allowed ports
  only.

Port safety matrix (ADR-0003): dpmac.3 and dpmac.17/dpni.0 total-deny — no
scenario of any class may name them; dpmac.4–6 and dpmac.8/10 are unwired,
lifecycle-only (link-up cannot be asserted there); dpmac.7/9 carry
link-signaling and traffic-bearing scenarios only when explicitly flagged.

_No scenarios recorded yet._
