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

            /// Checks the name is a valid Linux interface name that cannot be mistaken
            /// for a runtime handle: non-empty, at most 15 bytes (`IFNAMSIZ`), neither
            /// `.` nor `..`, free of `/` and whitespace, and never matching the reserved
            /// `family.N` pattern (ADR-0015 decision 13). A valid name serves verbatim
            /// as the netdev name and its restool label, so name and label stay lossless
            /// and id-confusion is unrepresentable at the parse boundary. Construction
            /// stays infallible (an empty name is the absent-optional sentinel); a
            /// frontend calls this on the names an operator DECLARES.
            ///
            /// # Errors
            /// Returns the first [`NameError`] the name trips, naming the offending
            /// detail — the length, the character, or the reserved family token.
            pub fn validate(&self) -> Result<(), NameError> {
                validate_interface_name(&self.0)
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

/// The construct-name view of a tenant name: the MC label ADR-0015 decision 9 stamps
/// on a tenant-owned object (its child dprc, companions, dpseci), whose owning construct
/// *is* the tenant. A tenant name is already a valid construct name — one declaration
/// namespace (ADR-0013 §2) — so the crossing is lossless. It is spelled out rather than
/// hopping through `From<&str>` so the tenant→label seam reads at the call site.
impl From<&TenantName> for ConstructName {
    fn from(t: &TenantName) -> Self {
        Self(t.0.clone())
    }
}

resource_name! {
    /// A derivation rule's name: the token a [`ProvenanceNode`](crate::compiled::ProvenanceNode)
    /// and its [`ProvenanceKey`](crate::compiled::ProvenanceKey) address it by
    /// (`"dpio"`, `"T"`, `"port-edge"`, …; design D6). Distinct from the tenant and
    /// construct it sits beside in a key.
    RuleName
}

/// Why a name is not a valid interface name (ADR-0015 decision 13). Each variant
/// carries the offending detail — the length, the character, or the reserved MC
/// family token — so its [`Display`](core::fmt::Display) is an actionable message a
/// caller quotes verbatim in a refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    /// The name is empty; an interface name names something.
    Empty,
    /// The name exceeds the 15-byte `IFNAMSIZ` limit (the byte length is carried).
    TooLong(usize),
    /// The name is `.` or `..`, which the kernel's `dev_valid_name` refuses because
    /// the name serves verbatim as a netdev (and thus a path) component.
    DotComponent,
    /// The name carries a `/` or a whitespace character (the offending char is carried).
    ForbiddenChar(char),
    /// The name matches the reserved `family.N` pattern — an MC family token dotted
    /// with digits (the token is carried) — which would collide with a runtime handle.
    ReservedPattern(&'static str),
}

impl core::fmt::Display for NameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => f.write_str("the name is empty"),
            Self::TooLong(n) => write!(f, "{n} characters exceeds the 15-character limit"),
            Self::DotComponent => f.write_str("a name of `.` or `..` is reserved"),
            Self::ForbiddenChar(c) => {
                write!(
                    f,
                    "character {c:?} is not allowed (a name carries no `/` or whitespace)"
                )
            }
            Self::ReservedPattern(token) => write!(
                f,
                "matches the reserved `{token}.N` pattern (an MC family token dotted with digits \
                 collides with a runtime handle)"
            ),
        }
    }
}

impl std::error::Error for NameError {}

/// Checks a raw name against the sealed interface-name contract (ADR-0015 decision 13):
/// the kernel's `dev_valid_name` rules — non-empty, ≤ 15 bytes, not `.`/`..`, no `/` or
/// whitespace — plus the reserved-pattern exclusion. The single implementation the
/// per-slot [`validate`](TenantName::validate) methods share.
fn validate_interface_name(s: &str) -> Result<(), NameError> {
    if s.is_empty() {
        return Err(NameError::Empty);
    }
    if s.len() > 15 {
        return Err(NameError::TooLong(s.len()));
    }
    if s == "." || s == ".." {
        return Err(NameError::DotComponent);
    }
    if let Some(c) = s.chars().find(|&c| c == '/' || c.is_whitespace()) {
        return Err(NameError::ForbiddenChar(c));
    }
    if let Some(token) = reserved_family_token(s) {
        return Err(NameError::ReservedPattern(token));
    }
    Ok(())
}

/// The MC family token a name reserves by matching `<family>.<digits>` to end of
/// string, or `None`. Reuses [`ALL_FAMILIES`](crate::family::ALL_FAMILIES) for the
/// token set (ADR-0014: one enumeration, not a second copy to keep in step). A simple
/// prefix/suffix scan — no regex, no new dependency (design D10).
fn reserved_family_token(s: &str) -> Option<&'static str> {
    for family in crate::family::ALL_FAMILIES {
        let token = family.as_str();
        if let Some(digits) = s
            .strip_prefix(token)
            .and_then(|rest| rest.strip_prefix('.'))
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
        {
            return Some(token);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    //! The interface-name validator over the sealed contract (ADR-0015 decision 13);
    //! one type stands in for all three, the `validate` method being macro-shared.

    use super::{ConstructName, NameError};
    use crate::family::ALL_FAMILIES;

    fn err(name: &str) -> NameError {
        ConstructName::from(name)
            .validate()
            .expect_err("name should be rejected")
    }

    #[test]
    fn accepts_valid_interface_names() {
        for ok in ["vpp", "abcdefghijklmno", "dpni4", "dpni.4x", "dpnix.4"] {
            assert!(ok.len() <= 15, "fixture `{ok}` is within IFNAMSIZ");
            ConstructName::from(ok)
                .validate()
                .unwrap_or_else(|e| panic!("`{ok}` should be valid: {e}"));
        }
    }

    #[test]
    fn rejects_empty_and_overlong() {
        assert_eq!(err(""), NameError::Empty);
        // 16 bytes — one past IFNAMSIZ.
        assert_eq!(err("abcdefghijklmnop"), NameError::TooLong(16));
    }

    #[test]
    fn rejects_dot_components() {
        assert_eq!(err("."), NameError::DotComponent);
        assert_eq!(err(".."), NameError::DotComponent);
    }

    #[test]
    fn rejects_slash_and_whitespace() {
        assert_eq!(err("a/b"), NameError::ForbiddenChar('/'));
        assert_eq!(err("a b"), NameError::ForbiddenChar(' '));
        assert_eq!(err("a\tb"), NameError::ForbiddenChar('\t'));
    }

    #[test]
    fn rejects_every_reserved_family_pattern() {
        for family in ALL_FAMILIES {
            let name = format!("{}.0", family.as_str());
            assert_eq!(
                err(&name),
                NameError::ReservedPattern(family.as_str()),
                "`{name}` must be refused as a reserved handle"
            );
        }
    }
}
