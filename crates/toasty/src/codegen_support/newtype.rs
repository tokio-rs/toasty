//! Marker trait emitted by `#[derive(Embed)]` for tuple-newtype embeds.
//!
//! The companion blanket impl turns any newtype embed whose inner type
//! implements [`Auto`] into an [`Auto`] type itself, so a user does not need
//! to repeat the strategy on the wrapper:
//!
//! ```ignore
//! #[derive(toasty::Embed)]
//! struct UserId(uuid::Uuid);
//!
//! // Automatically: `UserId: Auto` because `Uuid: Auto`.
//! ```
//!
//! Newtypes around non-`Auto` types stay non-`Auto` — the blanket's
//! `T::Inner: Auto` bound is on a generic associated type, so it is properly
//! conditional and doesn't error at impl declaration.
//!
//! `NewtypeOf` is local to toasty. Combined with the orphan rule, that keeps
//! the blanket from overlapping with the concrete `impl Auto for u64`,
//! `impl Auto for uuid::Uuid`, etc.: no downstream crate can ever add
//! `impl NewtypeOf for u64`, and toasty itself never will.

use crate::schema::{Auto, Field};
use toasty_core::schema::app::AutoStrategy;

/// Marker for a tuple-newtype embedded type, carrying its single inner field
/// type as an associated type. Emitted by `#[derive(Embed)]` for tuple
/// structs with exactly one field.
pub trait NewtypeOf {
    /// The inner field's type.
    type Inner;
}

// `do_not_recommend` keeps the blanket out of error suggestions so the
// `Auto` trait's `#[diagnostic::on_unimplemented]` message wins for users
// who hit a missing-`Auto` error on an embed.
#[diagnostic::do_not_recommend]
impl<T> Auto for T
where
    T: NewtypeOf + Field,
    <T as NewtypeOf>::Inner: Auto,
{
    const STRATEGY: AutoStrategy = <<T as NewtypeOf>::Inner as Auto>::STRATEGY;
}
