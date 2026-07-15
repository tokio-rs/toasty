//! Compile-time validation that enum variant fields sharing a column agree on
//! their type.
//!
//! When two `#[derive(Embed)]` enum variants declare the same shared logical
//! field via `#[shared(<ident>)]`, the generated code emits a
//! [`SameColumnType`] obligation between the two field types. The trait is
//! implemented only reflexively, so the obligation holds exactly when the two
//! types are identical; otherwise the `#[diagnostic::on_unimplemented]`
//! message points the user at the mismatch.
//!
//! The schema builder enforces the same rule at runtime as a backstop for
//! schemas built without the derive macro.

/// Asserts that two enum variant fields mapped to the same column have the same
/// Rust type.
///
/// Implemented only reflexively (`impl<T> SameColumnType<T> for T`), so
/// `A: SameColumnType<B>` holds iff `A` and `B` are the same type. Macro-
/// generated code emits a `_check::<A, B>()` bounded by this trait for each pair
/// of fields that land on one shared column.
#[diagnostic::on_unimplemented(
    message = "enum variant fields sharing a column must have the same type",
    label = "`{Self}` does not match `{Other}`, the type another variant shares with this column",
    note = "give both variants' fields the same type, or use distinct `#[shared(...)]` identifiers so they map to separate columns"
)]
pub trait SameColumnType<Other> {}

impl<T> SameColumnType<T> for T {}
