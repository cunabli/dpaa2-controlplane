//! The intent vocabulary's name strings as distinct domain types (feedback:
//! customise string so names cannot be confused among one another; ADR-0013 §2).
//!
//! An intent and its derived plan carry three kinds of name string that keep
//! company inside one struct — a [`ProvenanceKey`](crate::compiled::ProvenanceKey)
//! is `{ tenant, rule, construct }`, a [`Refusal`](crate::refuse::Refusal) payload
//! sets a `tenant` beside a `construct` — and a bare `String` in each slot lets a
//! tenant name be passed where a construct name is meant with no compiler word. The
//! `resource_name!` macro mints a newtype per slot so the wrong name is a type error,
//! not a silent swap; every type derives the same ordering/hashing surface (so it
//! keys the same `BTreeSet`/`BTreeMap` a `String` did) and the same string-facing
//! conveniences (so call sites and tests stay terse). No `Deref<Target = str>`: a
//! name is not a string, and hiding the distinction behind auto-deref is the very
//! confusion this module removes. Stdlib only (design D10: `dpaa2-api` stays
//! serde-free and dependency-light).

/// Defines a string newtype for one name slot of the intent vocabulary.
///
/// The generated type wraps a [`String`] and derives `Debug, Clone, PartialEq, Eq,
/// PartialOrd, Ord, Hash` (so it stands in for a `String` key in the plan's ordered
/// collections) plus, by hand, [`Display`](core::fmt::Display), [`AsRef<str>`],
/// `From<&str>`/`From<String>`/`From<&Self>` (so `.into()` and `From<&str>` keep
/// construction terse), and `as_str`/`is_empty` accessors. It deliberately omits
/// `Deref`, so a value of one name type never coerces into another or into a raw
/// `&str` argument.
macro_rules! resource_name {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// The name as a string slice — the last-resort accessor for comparing
            /// against a bare `&str` (e.g. the [`KERNEL`](crate::intent::KERNEL)
            /// literal) without reopening the type distinction.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Whether the name is empty — the sentinel an absent optional name
            /// carries (a non-restricted tenant's `pool`, a tenant-level provenance
            /// `construct`).
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl From<&$name> for $name {
            fn from(s: &$name) -> Self {
                s.clone()
            }
        }
    };
}

resource_name! {
    /// A tenant's name: the key namespace of every object a tenant draws, and the
    /// thing a port, link end, fabric owner, crypto block, extra or `pool` names
    /// when it refers to a tenant (design D1; `types.qnt` `Tenant`). Distinct from
    /// [`ConstructName`] so the tenant slot of a [`ProvenanceKey`](crate::compiled::ProvenanceKey)
    /// or a [`Refusal`](crate::refuse::Refusal) can never take a construct name by
    /// mistake.
    TenantName
}

resource_name! {
    /// A declared construct's name: a port, link or fabric identity, and the
    /// polymorphic `construct` a derived value bottoms out in — a tenant-level count
    /// carries the empty name, a per-construct rule the port/fabric/link name
    /// (design D6; `derive.qnt` `ProvenanceKey`/`dpniConstructs`). One type spans
    /// all three construct kinds because they share a single declaration namespace
    /// and flow together through the provenance `constructs` set and the refusal
    /// payloads; the [`Member`](crate::intent::Member) enum and the struct field
    /// names carry which kind a given slot expects.
    ConstructName
}

resource_name! {
    /// A derivation rule's name: the token a [`ProvenanceNode`](crate::compiled::ProvenanceNode)
    /// and its [`ProvenanceKey`](crate::compiled::ProvenanceKey) address it by
    /// (`"dpio"`, `"T"`, `"port-edge"`, …; design D6). Distinct from the tenant and
    /// construct it sits beside in a key.
    RuleName
}
