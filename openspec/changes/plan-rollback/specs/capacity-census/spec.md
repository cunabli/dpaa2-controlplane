# Spec Delta

## ADDED Requirements

### Requirement: A live free count is headroom, never a population bracket

A family whose ceiling observation is a live free count (dpbp's `bp`
pool row, ADR-0011 decision 1) SHALL be judged by that count alone: the
free units are the remaining headroom, a create plan admits while the
count is above zero, and a growth plan draws at most the observed free
count. Population SHALL be compared only against static brackets
(`Observed` ceilings: the per-boot dpmcp budget, the measured dpni
ceiling).

#### Scenario: Zero free refuses at any population

- **WHEN** a family's observed live free count is zero and the intent
  requires another object of that family
- **THEN** admission refuses the create naming the budget, the draw,
  and the zero headroom, whatever the family's population (the
  V-CEIL-1 law — the board refuses a dpbp create exactly at zero free)

### Requirement: An unsatisfiable draw refuses, never a silent zero-create plan

No plan SHALL silently drop a required draw because population outgrew
the remaining free units: an unsatisfiable draw surfaces as a refusal
naming the budget, the draw, and the headroom (the established refusal
voice), never as a zero-create plan.

#### Scenario: An aged board still plans the draw

- **WHEN** a family's population meets or exceeds its observed live
  free count while the intent requires more objects and free units
  remain
- **THEN** the plan creates up to the observed free count and the
  admission face renders the family's budget rows — the draw never
  vanishes from the dry-run
