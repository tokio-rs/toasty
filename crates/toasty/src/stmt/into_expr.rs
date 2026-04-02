use std::{rc::Rc, sync::Arc};

use super::assignment::impl_assign_via_expr;
use super::{Expr, List, Value};
use toasty_core::stmt;

/// Convert a value into an [`Expr<T>`].
///
/// This trait is the primary way Toasty coerces Rust values into query
/// expressions. It is implemented for all scalar types (`i64`, `String`,
/// `bool`, `uuid::Uuid`, …), `Option<T>`, tuples, slices, `Vec`, and arrays.
///
/// Generated code uses `IntoExpr` bounds on filter and setter methods so that
/// callers can pass either a raw value or an already-constructed [`Expr`]:
///
/// ```
/// # use toasty::stmt::{Expr, IntoExpr};
/// // Both &str and Expr<String> implement IntoExpr<String>:
/// let _e1: Expr<String> = "Alice".into_expr();
/// let _e2: Expr<String> = Expr::<String>::from_untyped(
///     toasty_core::stmt::Value::from("Bob"),
/// ).into_expr();
/// ```
///
/// # Implementing for custom types
///
/// If you have a newtype that wraps a supported scalar, implement `IntoExpr`
/// by converting to the inner type first, then calling [`Expr::from_value`].
pub trait IntoExpr<T> {
    /// Consume `self` and produce an [`Expr<T>`].
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty::stmt::{Expr, IntoExpr};
    ///
    /// let _expr: Expr<i64> = 42_i64.into_expr();
    /// let _expr: Expr<String> = "hello".into_expr();
    /// ```
    fn into_expr(self) -> Expr<T>;

    /// Produce an [`Expr<T>`] from a reference without consuming `self`.
    ///
    /// For [`Copy`] types this clones the value. For non-`Copy` types like
    /// `String` this clones the underlying data.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty::stmt::{Expr, IntoExpr};
    ///
    /// let value = 42_i64;
    /// let _expr: Expr<i64> = value.by_ref();
    /// // `value` is still usable
    /// assert_eq!(value, 42);
    /// ```
    fn by_ref(&self) -> Expr<T>;
}

macro_rules! impl_into_expr_for_copy {
    ( $( $var:ident($t:ty) ;)* ) => {
        $(
            impl IntoExpr<$t> for $t {
                fn into_expr(self) -> Expr<$t> {
                    Expr::from_value(Value::from(self))
                }

                fn by_ref(&self) -> Expr<$t> {
                    Expr::from_value(Value::from(self.clone()))
                }
            }

            impl_assign_via_expr!($t => $t);
        )*
    };
}

impl_into_expr_for_copy! {
    Bool(bool);
    I8(i8);
    I16(i16);
    I32(i32);
    I64(i64);
    U8(u8);
    U16(u16);
    U32(u32);
    U64(u64);
    Uuid(uuid::Uuid);
}

#[cfg(feature = "jiff")]
impl_into_expr_for_copy! {
    Timestamp(jiff::Timestamp);
    Zoned(jiff::Zoned);
    Date(jiff::civil::Date);
    Time(jiff::civil::Time);
    DateTime(jiff::civil::DateTime);
}

// Pointer-sized integers convert through their fixed-size equivalents
impl IntoExpr<isize> for isize {
    fn into_expr(self) -> Expr<isize> {
        Expr::from_value(Value::from(self as i64))
    }

    fn by_ref(&self) -> Expr<isize> {
        Expr::from_value(Value::from(*self as i64))
    }
}
impl_assign_via_expr!(isize => isize);

impl IntoExpr<usize> for usize {
    fn into_expr(self) -> Expr<usize> {
        Expr::from_value(Value::from(self as u64))
    }

    fn by_ref(&self) -> Expr<usize> {
        Expr::from_value(Value::from(*self as u64))
    }
}
impl_assign_via_expr!(usize => usize);

impl<T> IntoExpr<T> for Expr<T> {
    fn into_expr(self) -> Self {
        self
    }

    fn by_ref(&self) -> Self {
        self.clone()
    }
}
impl_assign_via_expr!({T} Expr<T> => T);

impl<T: IntoExpr<T>> IntoExpr<T> for &T {
    fn into_expr(self) -> Expr<T> {
        self.by_ref()
    }

    fn by_ref(&self) -> Expr<T> {
        (*self).by_ref()
    }
}
impl_assign_via_expr!({T: IntoExpr<T>} &T => T);

impl<T: IntoExpr<T>> IntoExpr<List<T>> for &T {
    fn into_expr(self) -> Expr<List<T>> {
        self.by_ref().cast()
    }

    fn by_ref(&self) -> Expr<List<T>> {
        (*self).by_ref().cast()
    }
}

impl<T: IntoExpr<T>> IntoExpr<Self> for Option<T> {
    fn into_expr(self) -> Expr<Self> {
        match self {
            Some(value) => value.into_expr().cast(),
            None => Expr::from_value(Value::Null),
        }
    }

    fn by_ref(&self) -> Expr<Self> {
        match self {
            Some(value) => value.by_ref().cast(),
            None => Expr::from_value(Value::Null),
        }
    }
}
impl_assign_via_expr!({T: IntoExpr<T>} Option<T> => Option<T>);

impl<T: IntoExpr<T>> IntoExpr<Option<T>> for T {
    fn into_expr(self) -> Expr<Option<T>> {
        self.into_expr().cast()
    }

    fn by_ref(&self) -> Expr<Option<T>> {
        self.by_ref().cast()
    }
}
impl_assign_via_expr!({T: IntoExpr<T>} T => Option<T>);

impl<T: IntoExpr<T>> IntoExpr<Option<T>> for &T {
    fn into_expr(self) -> Expr<Option<T>> {
        self.by_ref().cast()
    }

    fn by_ref(&self) -> Expr<Option<T>> {
        (*self).by_ref().cast()
    }
}
impl_assign_via_expr!({T: IntoExpr<T>} &T => Option<T>);

impl IntoExpr<String> for &str {
    fn into_expr(self) -> Expr<String> {
        Expr::from_value(Value::from(self))
    }

    fn by_ref(&self) -> Expr<String> {
        Expr::from_value(Value::from(*self))
    }
}
impl_assign_via_expr!(&str => String);

impl IntoExpr<Option<String>> for &str {
    fn into_expr(self) -> Expr<Option<String>> {
        Expr::from_value(Value::from(self))
    }

    fn by_ref(&self) -> Expr<Option<String>> {
        Expr::from_value(Value::from(*self))
    }
}
impl_assign_via_expr!(&str => Option<String>);

impl IntoExpr<Self> for String {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value(self.into())
    }
}
impl_assign_via_expr!(String => String);

impl IntoExpr<Self> for Vec<u8> {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value(self.clone().into())
    }
}
impl_assign_via_expr!(Vec<u8> => Vec<u8>);

#[cfg(feature = "rust_decimal")]
impl IntoExpr<Self> for rust_decimal::Decimal {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value((*self).into())
    }
}
#[cfg(feature = "rust_decimal")]
impl_assign_via_expr!(rust_decimal::Decimal => rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl IntoExpr<Self> for bigdecimal::BigDecimal {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value(self.clone().into())
    }
}
#[cfg(feature = "bigdecimal")]
impl_assign_via_expr!(bigdecimal::BigDecimal => bigdecimal::BigDecimal);

impl<T, U, const N: usize> IntoExpr<List<T>> for [U; N]
where
    U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<List<T>> {
        Expr::from_untyped(stmt::Expr::list(
            self.into_iter().map(|item| item.into_expr().untyped),
        ))
    }

    fn by_ref(&self) -> Expr<List<T>> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| U::by_ref(item).untyped),
        ))
    }
}
impl_assign_via_expr!({T, U: IntoExpr<T>, const N: usize} [U; N] => List<T>);

impl<T, U, const N: usize> IntoExpr<List<T>> for &[U; N]
where
    U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<List<T>> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| U::by_ref(item).untyped),
        ))
    }

    fn by_ref(&self) -> Expr<List<T>> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| U::by_ref(item).untyped),
        ))
    }
}
impl_assign_via_expr!({T, U: IntoExpr<T>, const N: usize} &[U; N] => List<T>);

impl<T, E: IntoExpr<T>> IntoExpr<List<T>> for &[E] {
    fn into_expr(self) -> Expr<List<T>> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| E::by_ref(item).untyped),
        ))
    }

    fn by_ref(&self) -> Expr<List<T>> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| E::by_ref(item).untyped),
        ))
    }
}
impl_assign_via_expr!({T, E: IntoExpr<T>} &[E] => List<T>);

impl<T, U> IntoExpr<List<T>> for Vec<U>
where
    U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<List<T>> {
        Expr::from_untyped(stmt::Expr::list(
            self.into_iter().map(|item| item.into_expr().untyped),
        ))
    }

    fn by_ref(&self) -> Expr<List<T>> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| item.by_ref().untyped),
        ))
    }
}
impl_assign_via_expr!({T, U: IntoExpr<T>} Vec<U> => List<T>);

macro_rules! forward_impl {
    ( $( $ty:ty ,) *) => {
        $(
            impl<T> IntoExpr<$ty> for T
            where
                T: IntoExpr<T>,
            {
                fn into_expr(self) -> Expr<$ty> {
                    <Self as IntoExpr<Self>>::into_expr(self).cast()
                }

                fn by_ref(&self) -> Expr<$ty> {
                    <Self as IntoExpr<Self>>::by_ref(self).cast()
                }
            }
        ) *
    };
}

forward_impl!(Arc<T>, Box<T>, Rc<T>,);
impl_assign_via_expr!({T: IntoExpr<T>} T => Arc<T>);
impl_assign_via_expr!({T: IntoExpr<T>} T => Box<T>);
impl_assign_via_expr!({T: IntoExpr<T>} T => Rc<T>);

macro_rules! impl_into_expr_for_tuple {
    (! $( $n:tt $t:ident $e:ident )* ) => {
        impl<$( $t, $e ),*> IntoExpr<($( $t, )*)> for ($( $e, )*)
        where
            $( $e: IntoExpr<$t>, )*
        {
            fn into_expr(self) -> Expr<($( $t, )*)> {
                let record = stmt::ExprRecord::from_vec(vec![
                    $( self.$n.into_expr().untyped, )*
                ]);
                let untyped = stmt::Expr::Record(record);
                Expr::from_untyped(untyped)
            }

            fn by_ref(&self) -> Expr<($( $t, )*)> {
                let record = stmt::ExprRecord::from_vec(vec![
                    $( self.$n.by_ref().untyped, )*
                ]);
                let untyped = stmt::Expr::Record(record);
                Expr::from_untyped(untyped)
            }
        }

        impl_assign_via_expr!({$( $t, $e: IntoExpr<$t> ),*} ($( $e, )*) => ($( $t, )*));
    };

    (
        ( $( $n_base:tt $t_base:ident $e_base:ident )* )
        $n:tt $t:ident $e:ident
        $( $rest:tt )*
    ) => {
        // Implement for tuples at this level
        impl_into_expr_for_tuple!(! $( $n_base $t_base $e_base )* $n $t $e);

        // Recurse
        impl_into_expr_for_tuple!(
            ( $( $n_base $t_base $e_base )* $n $t $e )
            $( $rest )*
        );
    };

    ( ( $( $n:tt $t:ident $e:ident )* ) ) => {}
}

impl_into_expr_for_tuple! {
    ()
    0 T0 E0
    1 T1 E1
    2 T2 E2
    3 T3 E3
    4 T4 E4
    5 T5 E5
    6 T6 E6
    7 T7 E7
    8 T8 E8
    9 T9 E9
}

#[test]
fn assert_bounds() {
    fn assert_into_expr<T, E: IntoExpr<T>>() {}

    assert_into_expr::<i64, i64>();
    assert_into_expr::<(String, String), (&String, &String)>();
    assert_into_expr::<List<(String, String)>, &[(&String, &String)]>();
    assert_into_expr::<List<(String, String)>, [(&String, &String); 3]>();
    assert_into_expr::<List<(String, String)>, &[(&String, &String); 3]>();
}
