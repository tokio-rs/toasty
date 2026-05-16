//! Compile-time validation for `#[column(type = ...)]`.
//!
//! When `#[derive(Model)]` sees an explicit storage type on a field, the
//! generated code emits a one-shot obligation that the field's Rust type
//! implements `CompatibleWith<Tag>` for the matching marker `Tag`. The Rust
//! type checker — not the macro — resolves the field type, so type aliases
//! and generic parameters are handled correctly.

/// Marker types corresponding to each `#[column(type = ...)]` storage tag.
///
/// These types are zero-sized and only ever appear as the `S` parameter to
/// [`CompatibleWith`]. Macro-generated code names them by absolute path.
#[allow(missing_docs, non_camel_case_types)]
pub mod tag {
    pub struct Boolean;

    pub struct I8;
    pub struct I16;
    pub struct I32;
    pub struct I64;

    pub struct U8;
    pub struct U16;
    pub struct U32;
    pub struct U64;

    pub struct F32;
    pub struct F64;

    pub struct Text;
    pub struct VarChar;

    pub struct Binary;
    pub struct Blob;

    pub struct Timestamp;
    pub struct Date;
    pub struct Time;
    pub struct DateTime;
}

/// Asserts that a Rust field type can be stored as the given storage tag.
///
/// Macro-generated code emits a `_check::<FieldType>()` call where `_check`
/// is bounded by `CompatibleWith<Tag>`. Trait resolution happens after type
/// alias substitution, so `type String = u32;` cannot defeat the check the
/// way it would defeat any analysis of the raw `String` token in the macro.
#[diagnostic::on_unimplemented(
    message = "field type `{Self}` is not compatible with the requested column storage `{Storage}`",
    label = "incompatible Rust type for `#[column(type = ...)]`",
    note = "see `toasty::codegen_support::storage` for the compatibility table"
)]
pub trait CompatibleWith<Storage> {}

// `Option<T>` is compatible with the same storage tags as `T`. The schema
// tracks nullability separately, so the storage type is unaffected by the
// option wrapper.
impl<T, S> CompatibleWith<S> for Option<T> where T: CompatibleWith<S> {}

impl CompatibleWith<tag::Boolean> for bool {}

impl CompatibleWith<tag::I8> for i8 {}
impl CompatibleWith<tag::I16> for i16 {}
impl CompatibleWith<tag::I32> for i32 {}
impl CompatibleWith<tag::I64> for i64 {}

impl CompatibleWith<tag::U8> for u8 {}
impl CompatibleWith<tag::U16> for u16 {}
impl CompatibleWith<tag::U32> for u32 {}
impl CompatibleWith<tag::U64> for u64 {}

impl CompatibleWith<tag::F32> for f32 {}
impl CompatibleWith<tag::F64> for f64 {}
// Both directions are allowed — drivers convert at the boundary, and the
// storage choice is the user's explicit decision.
impl CompatibleWith<tag::F32> for f64 {}
impl CompatibleWith<tag::F64> for f32 {}

impl CompatibleWith<tag::Text> for String {}
impl CompatibleWith<tag::VarChar> for String {}

impl CompatibleWith<tag::Binary> for Vec<u8> {}
impl CompatibleWith<tag::Blob> for Vec<u8> {}

// UUIDs may be stored as their native form, as text, or as raw bytes.
impl CompatibleWith<tag::Text> for uuid::Uuid {}
impl CompatibleWith<tag::VarChar> for uuid::Uuid {}
impl CompatibleWith<tag::Blob> for uuid::Uuid {}
impl CompatibleWith<tag::Binary> for uuid::Uuid {}

#[cfg(feature = "rust_decimal")]
impl CompatibleWith<tag::Text> for rust_decimal::Decimal {}
#[cfg(feature = "rust_decimal")]
impl CompatibleWith<tag::VarChar> for rust_decimal::Decimal {}

#[cfg(feature = "bigdecimal")]
impl CompatibleWith<tag::Text> for bigdecimal::BigDecimal {}
#[cfg(feature = "bigdecimal")]
impl CompatibleWith<tag::VarChar> for bigdecimal::BigDecimal {}

#[cfg(feature = "jiff")]
mod jiff_impls {
    use super::{CompatibleWith, tag};

    impl CompatibleWith<tag::Timestamp> for jiff::Timestamp {}
    impl CompatibleWith<tag::Date> for jiff::civil::Date {}
    impl CompatibleWith<tag::Time> for jiff::civil::Time {}
    impl CompatibleWith<tag::DateTime> for jiff::civil::DateTime {}

    // Temporal types may also be serialized as text (ISO 8601 form). The
    // driver handles the conversion at the boundary.
    impl CompatibleWith<tag::Text> for jiff::Timestamp {}
    impl CompatibleWith<tag::VarChar> for jiff::Timestamp {}
    impl CompatibleWith<tag::Text> for jiff::civil::Date {}
    impl CompatibleWith<tag::VarChar> for jiff::civil::Date {}
    impl CompatibleWith<tag::Text> for jiff::civil::Time {}
    impl CompatibleWith<tag::VarChar> for jiff::civil::Time {}
    impl CompatibleWith<tag::Text> for jiff::civil::DateTime {}
    impl CompatibleWith<tag::VarChar> for jiff::civil::DateTime {}
}
