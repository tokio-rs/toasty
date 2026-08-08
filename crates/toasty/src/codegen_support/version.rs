//! Compile-time validation and runtime support for `#[version]` fields.
//!
//! `#[derive(Model)]` emits a `_check::<FieldType>()` obligation bounded by
//! [`Version`] for every `#[version]` field. The concrete impl covers
//! `u64` directly; the blanket propagates through tuple-newtype embeds of any
//! depth, mirroring the [`Auto`] blanket in
//! [`crate::codegen_support::newtype`].
//!
//! At runtime, generated code calls [`Version::as_u64`] to extract the
//! raw counter for use in OCC condition expressions.

use super::newtype::NewtypeOf;

/// A field type that can be used as an OCC version counter.
///
/// Only `u64` and tuple-newtype embeds that wrap a `Version` type (directly
/// or transitively) satisfy this bound.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as a `#[version]` field",
    label = "invalid type for `#[version]`",
    note = "only `u64` and tuple-newtype embeds of `u64` are supported"
)]
pub trait Version: Copy {
    /// Return the raw `u64` counter value stored in this field.
    fn as_u64(self) -> u64;

    /// Construct a field value from a raw `u64` counter, wrapping through any
    /// newtype layers. Used by generated code to build the next-version
    /// expression in the correct shape for the update assignment.
    fn from_u64(v: u64) -> Self;
}

impl Version for u64 {
    fn as_u64(self) -> u64 {
        self
    }

    fn from_u64(v: u64) -> Self {
        v
    }
}

// Propagate through tuple-newtype embeds of any depth.
//
// `do_not_recommend` keeps the blanket out of error suggestions so the
// `Version` `#[diagnostic::on_unimplemented]` message wins for users
// who hit a missing-`Version` error on an embed.
#[diagnostic::do_not_recommend]
impl<T> Version for T
where
    T: NewtypeOf + Copy,
    <T as NewtypeOf>::Inner: Version,
{
    fn as_u64(self) -> u64 {
        self.into_inner().as_u64()
    }

    fn from_u64(v: u64) -> Self {
        T::from_inner(<T::Inner as Version>::from_u64(v))
    }
}
