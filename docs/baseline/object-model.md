# Object model: cross-family relationship map

Stub — populated by task 6.1 (spec: object-baseline, "A cross-object
relationship map exists"). Target content, precise enough to seed typestate
design and Quint lifecycle models without re-reading C:

- Containment: DPRC parent/child structure and what crossing a container
  boundary means.
- Connect edges: which family pairs `dprc connect` accepts and what a
  connection implies for each side.
- Create vs allocate: for every consumer path, whether an actor creates a
  new object or a driver claims an existing pooled one — and the pool's name.
- Allocation pools: which families are pooled, who draws from them, sizing
  couplings.
- Lifecycle ordering: what must exist, be connected, or be plugged before
  each transition is legal.

_Not yet populated._
