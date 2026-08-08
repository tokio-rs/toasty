use toasty_core::stmt;

/// A numeric type that has a representable "one" value.
///
/// Used as a bound on [`crate::stmt::increment`] and [`crate::stmt::decrement`]
/// so the literal `1` is encoded as a [`stmt::Value`] variant matching the
/// field type — both backend binding and driver-side arithmetic require it.
///
/// Implemented for every primitive numeric type ([`i8`]–[`i64`], [`u8`]–[`u64`],
/// [`f32`], [`f64`]). Implement for custom newtypes around supported numeric
/// values (e.g. wrapping `rust_decimal::Decimal`) to use them with
/// [`increment`] / [`decrement`].
///
/// [`increment`]: crate::stmt::increment
/// [`decrement`]: crate::stmt::decrement
pub trait Numeric {
    /// Return the value `1` for this type as a [`stmt::Value`].
    fn one() -> stmt::Value;
}

macro_rules! impl_numeric {
    ($($t:ty),* $(,)?) => {
        $(
            impl Numeric for $t {
                fn one() -> stmt::Value {
                    stmt::Value::from(1 as $t)
                }
            }
        )*
    };
}

impl_numeric!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
