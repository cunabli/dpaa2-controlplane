# intent-compiler delta: vocabulary-v2-followups

## ADDED Requirements

### Requirement: The reserved kernel resolves at every reference site

The reserved kernel SHALL be a resolving referent at every site that can
name a tenant, on both boundaries: at the `TenantRef` sites this holds by
construction (the `Kernel` case carries no name to be absent), and at the
three `TenantName` reference sites — a fabric's forwarder, a crypto
allocation's tenant, an extra allocation's tenant — compile SHALL treat
the reserved kernel name as declared, exactly as the parse boundary's
resolution rule already does. No `TenantAbsent` refusal SHALL ever carry
the reserved kernel name. Whether a kernel object materialises for such a
reference remains owned by the existing materialisation triggers; this
requirement governs resolution only.

#### Scenario: A kernel-forwarded fabric with no kernel port compiles past resolution

- **WHEN** an intent declares a hardware fabric whose `forwarded_by` names
  the reserved kernel, with tenant-only members and no port owned by the
  kernel
- **THEN** compile emits no `TenantAbsent` refusal for the fabric's
  forwarder, and the refusal set for the intent is otherwise unchanged

#### Scenario: A kernel crypto allocation resolves

- **WHEN** an intent carries a crypto allocation whose tenant names the
  reserved kernel and no kernel port exists
- **THEN** compile emits no `TenantAbsent` refusal for that allocation

#### Scenario: Duplicate construct names remain a parse-boundary refusal

- **WHEN** a programmatic intent carries two constructs of different
  families sharing one name
- **THEN** compile emits no duplicate-name refusal — the raw parse layer
  deliberately owns that check, and the one-sided posture is recorded
  against design D11 in ADR-0013 §5
