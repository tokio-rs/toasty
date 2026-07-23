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

    /// Signed integer storage with a non-standard byte width.
    pub struct Int<const BYTES: u8>;
    /// Unsigned integer storage with a non-standard byte width.
    pub struct UInt<const BYTES: u8>;

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
    pub struct Json;
    pub struct Jsonb;

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

/// Marker implemented by the integer storage tags accepted for embedded enum
/// discriminant columns.
pub trait IntegerStorage {}

impl IntegerStorage for tag::I8 {}
impl IntegerStorage for tag::I16 {}
impl IntegerStorage for tag::I32 {}
impl IntegerStorage for tag::I64 {}
impl IntegerStorage for tag::U8 {}
impl IntegerStorage for tag::U16 {}
impl IntegerStorage for tag::U32 {}
impl IntegerStorage for tag::U64 {}
impl<const BYTES: u8> IntegerStorage for tag::Int<BYTES> {}
impl<const BYTES: u8> IntegerStorage for tag::UInt<BYTES> {}

// `Option<T>` is compatible with the same storage tags as `T`. The schema
// tracks nullability separately, so the storage type is unaffected by the
// option wrapper.
impl<T, S> CompatibleWith<S> for Option<T> where T: CompatibleWith<S> {}

// Deferred fields and smart pointers are transparent storage wrappers.
impl<T, S> CompatibleWith<S> for crate::Deferred<T> where T: CompatibleWith<S> {}
impl<T, S> CompatibleWith<S> for Box<T> where T: CompatibleWith<S> {}
impl<T, S> CompatibleWith<S> for std::rc::Rc<T> where T: CompatibleWith<S> {}
impl<T, S> CompatibleWith<S> for std::sync::Arc<T> where T: CompatibleWith<S> {}

// A unit enum is both `Embed` and `Scalar`. Integer storage selected on a
// `Vec<unit-enum>` field describes each discriminant, not the collection as a
// whole. Keep these implementations limited to enum collections so `Vec<u8>`
// retains its binary-blob meaning.
macro_rules! impl_integer_enum_collection_compat {
    ($($storage:ty),* $(,)?) => {
        $(
            impl<T> CompatibleWith<$storage> for Vec<T>
            where
                T: crate::schema::Embed
                    + crate::schema::Scalar
                    + CompatibleWith<$storage>,
            {}
        )*
    };
}

impl_integer_enum_collection_compat!(
    tag::I8,
    tag::I16,
    tag::I32,
    tag::I64,
    tag::U8,
    tag::U16,
    tag::U32,
    tag::U64,
);

impl<T, const BYTES: u8> CompatibleWith<tag::Int<BYTES>> for Vec<T> where
    T: crate::schema::Embed + crate::schema::Scalar + CompatibleWith<tag::Int<BYTES>>
{
}

impl<T, const BYTES: u8> CompatibleWith<tag::UInt<BYTES>> for Vec<T> where
    T: crate::schema::Embed + crate::schema::Scalar + CompatibleWith<tag::UInt<BYTES>>
{
}

impl CompatibleWith<tag::Boolean> for bool {}

impl CompatibleWith<tag::I8> for i8 {}
impl CompatibleWith<tag::I16> for i16 {}
impl CompatibleWith<tag::I32> for i32 {}
impl CompatibleWith<tag::I64> for i64 {}

impl CompatibleWith<tag::U8> for u8 {}
impl CompatibleWith<tag::U16> for u16 {}
impl CompatibleWith<tag::U32> for u32 {}
impl CompatibleWith<tag::U64> for u64 {}

// `int(N)` and `uint(N)` have no exact Rust counterpart when N is not a
// power of two. Preserve the integer-family check while the engine performs
// the value-level checked cast.
macro_rules! impl_custom_integer_compat {
    ($storage:ident => $($ty:ty),* $(,)?) => {
        $(
            #[diagnostic::do_not_recommend]
            impl<const BYTES: u8> CompatibleWith<tag::$storage<BYTES>> for $ty {}
        )*
    };
}

impl_custom_integer_compat!(Int => i8, i16, i32, i64);
impl_custom_integer_compat!(UInt => u8, u16, u32, u64);

impl CompatibleWith<tag::F32> for f32 {}
impl CompatibleWith<tag::F64> for f64 {}
// Both directions are allowed — drivers convert at the boundary, and the
// storage choice is the user's explicit decision.
impl CompatibleWith<tag::F32> for f64 {}
impl CompatibleWith<tag::F64> for f32 {}

impl CompatibleWith<tag::Text> for String {}
impl CompatibleWith<tag::VarChar> for String {}

// JSON fields serialize through Toasty's string expression type. They support
// text storage and the native JSON types recognized by the SQL drivers.
#[cfg(feature = "serde")]
impl<T> CompatibleWith<tag::Text> for crate::Json<T> {}
#[cfg(feature = "serde")]
impl<T> CompatibleWith<tag::VarChar> for crate::Json<T> {}
#[cfg(feature = "serde")]
impl<T> CompatibleWith<tag::Json> for crate::Json<T> {}
#[cfg(feature = "serde")]
impl<T> CompatibleWith<tag::Jsonb> for crate::Json<T> {}
#[cfg(feature = "serde")]
impl CompatibleWith<tag::Text> for serde_json::Value {}
#[cfg(feature = "serde")]
impl CompatibleWith<tag::VarChar> for serde_json::Value {}
#[cfg(feature = "serde")]
impl CompatibleWith<tag::Json> for serde_json::Value {}
#[cfg(feature = "serde")]
impl CompatibleWith<tag::Jsonb> for serde_json::Value {}

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
