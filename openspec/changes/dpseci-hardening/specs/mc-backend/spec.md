# mc-backend — dpseci-hardening delta

## ADDED Requirements

### Requirement: An unknown firmware option bit is attributed, never merged and never erased

The observe-side dpseci options decode SHALL carry an unnamed firmware
bit through as a raw escape that preserves the bit's identity, alongside
every named option it decodes. The decode MUST NOT collapse the mask to
an undifferentiated unknown, and MUST NOT merge an unnamed bit into any
named option (design D4 of the dpseci-typestate archive; review synthesis
S1 arm (i)). The escape is display-face information only: the census
projection and the family judgment signature are unchanged by its
presence.

#### Scenario: A readback with an unnamed bit keeps its identity

- **WHEN** a raw GET_ATTR readback carries a named option plus one bit no
  vocabulary name covers
- **THEN** the decoded mask holds the named option and a raw escape
  identifying that bit, and the census projection for the object is the
  same as before this change

## MODIFIED Requirements

None.
