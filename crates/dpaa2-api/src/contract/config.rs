use crate::core::error::Error;
use crate::intent::Intent;

/// Northbound config source producing the neutral [`Intent`].
///
/// TOML implements this now; a gNMI/YANG frontend can implement it later and feed
/// the same pure core (design D0). The frontend parses and validates *intent* only;
/// it does not compile — turning an [`Intent`] plus an [`Inventory`](crate::core::inventory::Inventory) into the
/// object plan is [`compile`](crate::compile)'s, not the frontend's (design D10).
pub trait ConfigSource {
    /// Loads and validates the declared intent.
    ///
    /// # Errors
    /// Returns an error if the source is unreadable or fails validation.
    fn load(&self) -> Result<Intent, Error>;
}
