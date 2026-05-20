//! Compile-time validation for `#[version]` fields.
//!
//! When `#[derive(Model)]` sees a `#[version]` attribute, the generated code
//! emits a one-shot obligation that the field's Rust type implements
//! [`VersionCounter`]. The Rust type checker resolves the field type, so type
//! aliases and re-exports compose naturally — the macro never inspects the raw
//! type token and so cannot be defeated by a rename the way static analysis of
//! the `u64` identifier would be.
//!
//! Mirrors the pattern in [`crate::codegen_support::storage`] and
//! [`crate::codegen_support::auto`].

/// Asserts that a Rust field type can back a `#[version]` optimistic-concurrency
/// counter.
///
/// Only `u64` qualifies: the runtime increments the counter and matches the
/// previous value as a `Value::U64`. Unlike the `storage` and `auto` markers
/// there is no `Option<T>` forwarding impl — a version counter is never
/// nullable.
#[diagnostic::on_unimplemented(
    message = "`#[version]` field type `{Self}` must be `u64`",
    label = "invalid Rust type for `#[version]`",
    note = "the version counter is incremented and compared as a `u64`"
)]
pub trait VersionCounter {
    /// The counter as a `u64`. Generated code routes every read of a
    /// `#[version]` field through this so the `u64` constraint is enforced by
    /// trait resolution rather than by the macro inspecting the field type.
    #[doc(hidden)]
    fn into_u64(self) -> u64;
}

impl VersionCounter for u64 {
    fn into_u64(self) -> u64 {
        self
    }
}
