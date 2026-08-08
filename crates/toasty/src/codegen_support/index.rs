//! Compile-time validation that a field used in an index maps to a single
//! index column.
//!
//! When `#[derive(Model)]` sees a field participating in a secondary index or
//! unique constraint (`#[index]`, `#[unique]`, or a model-level
//! `#[index(...)]` / `#[unique(...)]`), the generated code emits a one-shot
//! obligation that the field's Rust type implements [`IndexableField`].
//!
//! Scalars implement it directly; tuple-newtype embeds and unit (data-less)
//! enums via impls emitted by `#[derive(Embed)]`. Data-carrying enums and
//! multi-field embedded structs map to more than one column, so they do not
//! implement it and naming one in an index is a compile error rather than a
//! runtime panic.
//!
//! Mirrors the pattern in [`crate::codegen_support::storage`] and
//! [`crate::codegen_support::auto`]: a trait carrying a
//! `#[diagnostic::on_unimplemented]` message and concrete impls for the
//! supported scalar types. Unlike those, newtype embeds get a per-type impl
//! from the derive rather than a `NewtypeOf` blanket — a blanket would conflict
//! with the `Box<T>` forwarding impl below, since `Box` is `#[fundamental]`.

/// Asserts that a Rust field type can serve as a single index column.
///
/// Macro-generated code emits a `_check::<FieldType>()` call bounded by
/// `IndexableField`. The Rust type checker — not the macro — resolves the field
/// type, so type aliases and newtype embeds are handled correctly.
#[diagnostic::on_unimplemented(
    message = "field type `{Self}` cannot be used in an index",
    label = "not indexable",
    note = "only scalar fields, newtype embeds, and unit (data-less) enums can be indexed; \
            data-carrying enums and multi-field embedded structs span multiple columns and \
            have no single index column"
)]
pub trait IndexableField {}

// `Option<T>` is indexable wherever `T` is. The schema tracks nullability
// separately, so the option wrapper does not affect indexability.
impl<T> IndexableField for Option<T> where T: IndexableField {}

// Smart-pointer wrappers (used for boxed foreign keys) are transparent for
// storage, so they are indexable wherever the pointee is.
impl<T> IndexableField for Box<T> where T: IndexableField {}
impl<T> IndexableField for std::rc::Rc<T> where T: IndexableField {}
impl<T> IndexableField for std::sync::Arc<T> where T: IndexableField {}

// A `Deferred<T>` field is loaded lazily but stores the same single column as
// `T`, so it is indexable wherever `T` is.
impl<T> IndexableField for crate::Deferred<T> where T: IndexableField {}

impl IndexableField for bool {}

impl IndexableField for i8 {}
impl IndexableField for i16 {}
impl IndexableField for i32 {}
impl IndexableField for i64 {}
impl IndexableField for isize {}

impl IndexableField for u8 {}
impl IndexableField for u16 {}
impl IndexableField for u32 {}
impl IndexableField for u64 {}
impl IndexableField for usize {}

impl IndexableField for f32 {}
impl IndexableField for f64 {}

impl IndexableField for String {}
impl IndexableField for Vec<u8> {}

impl IndexableField for uuid::Uuid {}

#[cfg(feature = "rust_decimal")]
impl IndexableField for rust_decimal::Decimal {}

#[cfg(feature = "bigdecimal")]
impl IndexableField for bigdecimal::BigDecimal {}

#[cfg(feature = "jiff")]
mod jiff_impls {
    use super::IndexableField;

    impl IndexableField for jiff::Timestamp {}
    impl IndexableField for jiff::civil::Date {}
    impl IndexableField for jiff::civil::Time {}
    impl IndexableField for jiff::civil::DateTime {}
}

// Tuple-newtype embeds get a per-type `IndexableField` impl from
// `#[derive(Embed)]` (see `expand_embedded_indexable_impl` in toasty-macros),
// forwarding to their inner type. A `NewtypeOf` blanket would be more concise
// but conflicts with the `Box<T>` impl above, since `Box` is `#[fundamental]`.
