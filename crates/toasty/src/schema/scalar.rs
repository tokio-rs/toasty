//! Marker trait identifying scalar field types eligible to be the element
//! type of a collection field (e.g. `Vec<T>`).
//!
//! The macro never inspects user types directly — when it sees a model field
//! `tags: Vec<String>` it expands to `<Vec<String> as Field>::field_ty(...)`
//! and lets the trait system pick the right behavior. Trait coherence then
//! requires a way to distinguish "scalar `T`" (where `Vec<T>` should be a
//! collection field) from `u8` (where `Vec<u8>` is a bytes blob) and from
//! relation / embed types. `Scalar` is that marker.
//!
//! `Scalar` is sealed: callers cannot implement it. New scalar types must be
//! added here.

mod sealed {
    pub trait Sealed {}
}

/// Marker trait for primitive types that can be the element type of a
/// `Vec<T>` collection field.
///
/// Implemented for the textual, numeric, and UUID primitives. Not
/// implemented for `u8` — `Vec<u8>` keeps its bytes-blob meaning. Not
/// implemented for embed types or relations.
pub trait Scalar: sealed::Sealed {}

macro_rules! impl_scalar {
    ($($ty:ty),* $(,)?) => {
        $(
            impl sealed::Sealed for $ty {}
            impl Scalar for $ty {}
        )*
    };
}

impl_scalar!(
    bool,
    i8,
    i16,
    i32,
    i64,
    u16,
    u32,
    u64,
    f32,
    f64,
    isize,
    usize,
    String,
    uuid::Uuid,
);

#[cfg(feature = "rust_decimal")]
impl_scalar!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_scalar!(bigdecimal::BigDecimal);
