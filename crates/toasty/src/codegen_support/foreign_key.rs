//! Compile-time validation that a `belongs_to` key field has the same type as
//! the target field it references.
//!
//! For each `(key, references)` pair of a `belongs_to` declared inside a
//! `#[derive(Embed)]` enum variant, the generated code passes the target
//! model's field accessor to [`target_field`], which recovers the referenced
//! field's type through the accessor's [`FieldPath`] impl, then calls
//! [`TargetField::check_key`] with the key field's type. The [`SameKeyType`]
//! obligation fails with a readable message when the two disagree.
//!
//! Root models get the same guarantee from the generated relation accessor,
//! whose filter compares the key field to the target path.

use crate::stmt::Path;
use std::marker::PhantomData;

/// Recovers the Rust type a field accessor points at.
///
/// Implemented for [`Path<Origin, T>`] (primitive fields) and for the
/// generated `{Type}Fields<Origin>` structs of embedded types, so
/// `Target::fields().id()` resolves to the type of `Target::id` without
/// spelling it out.
pub trait FieldPath {
    /// The referenced field's type.
    type Ty;
}

impl<Origin, T> FieldPath for Path<Origin, T> {
    type Ty = T;
}

/// Asserts that a `belongs_to` key field can hold the value of the field it
/// references.
///
/// Implemented only reflexively (`impl<T> SameKeyType<T> for T`), so
/// `A: SameKeyType<B>` holds iff `A` and `B` are the same type.
#[diagnostic::on_unimplemented(
    message = "`belongs_to` key field has type `{Self}`, but the field it references has type `{Target}`",
    label = "this key field must have type `{Target}`",
    note = "a foreign key stores the referenced field's value, so both fields must have the same type"
)]
pub trait SameKeyType<Target> {}

impl<T> SameKeyType<T> for T {}

/// The type of a `belongs_to` target field, recovered from its accessor by
/// [`target_field`].
pub struct TargetField<T>(PhantomData<T>);

/// Resolves the type of the field `target` points at.
pub fn target_field<P: FieldPath>(_target: P) -> TargetField<P::Ty> {
    TargetField(PhantomData)
}

impl<T> TargetField<T> {
    /// Type-checks one `(key, references)` pair. `Key` is the key field's
    /// type with any `Option` wrapper removed. Never called at runtime.
    ///
    /// A method on an already-resolved `TargetField<T>` rather than a free
    /// function generic over both types: with `T` still an inference variable,
    /// rustc would infer `T = Key` from the sole `SameKeyType` impl and report
    /// a bare type mismatch instead of the `on_unimplemented` message.
    pub fn check_key<Key>(self)
    where
        Key: SameKeyType<T>,
    {
    }
}
