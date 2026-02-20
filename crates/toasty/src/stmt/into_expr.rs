use std::{rc::Rc, sync::Arc};

use super::{Expr, Value};
use toasty_core::stmt;

pub trait IntoExpr<T: ?Sized> {
    fn into_expr(self) -> Expr<T>;

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

impl IntoExpr<usize> for usize {
    fn into_expr(self) -> Expr<usize> {
        Expr::from_value(Value::from(self as u64))
    }

    fn by_ref(&self) -> Expr<usize> {
        Expr::from_value(Value::from(*self as u64))
    }
}

impl<T: ?Sized> IntoExpr<T> for Expr<T> {
    fn into_expr(self) -> Self {
        self
    }

    fn by_ref(&self) -> Self {
        self.clone()
    }
}

impl<T: IntoExpr<T> + ?Sized> IntoExpr<T> for &T {
    fn into_expr(self) -> Expr<T> {
        self.by_ref()
    }

    fn by_ref(&self) -> Expr<T> {
        (*self).by_ref()
    }
}

impl<T: IntoExpr<T>> IntoExpr<[T]> for &T {
    fn into_expr(self) -> Expr<[T]> {
        self.by_ref().cast()
    }

    fn by_ref(&self) -> Expr<[T]> {
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

impl<T: IntoExpr<T>> IntoExpr<Option<T>> for T {
    fn into_expr(self) -> Expr<Option<T>> {
        self.into_expr().cast()
    }

    fn by_ref(&self) -> Expr<Option<T>> {
        self.by_ref().cast()
    }
}

impl<T: IntoExpr<T>> IntoExpr<Option<T>> for &T {
    fn into_expr(self) -> Expr<Option<T>> {
        self.by_ref().cast()
    }

    fn by_ref(&self) -> Expr<Option<T>> {
        (*self).by_ref().cast()
    }
}

impl<T: IntoExpr<T>> IntoExpr<T> for &Option<T> {
    fn into_expr(self) -> Expr<T> {
        match self {
            Some(value) => value.into_expr(),
            None => Expr::from_value(Value::Null),
        }
    }

    fn by_ref(&self) -> Expr<T> {
        match self {
            Some(value) => value.by_ref(),
            None => Expr::from_value(Value::Null),
        }
    }
}

impl IntoExpr<String> for &str {
    fn into_expr(self) -> Expr<String> {
        Expr::from_value(Value::from(self))
    }

    fn by_ref(&self) -> Expr<String> {
        Expr::from_value(Value::from(*self))
    }
}

impl IntoExpr<Option<String>> for &str {
    fn into_expr(self) -> Expr<Option<String>> {
        Expr::from_value(Value::from(self))
    }

    fn by_ref(&self) -> Expr<Option<String>> {
        Expr::from_value(Value::from(*self))
    }
}

impl IntoExpr<Self> for String {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value(self.into())
    }
}

impl IntoExpr<Self> for Vec<u8> {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value(self.clone().into())
    }
}

#[cfg(feature = "rust_decimal")]
impl IntoExpr<Self> for rust_decimal::Decimal {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value((*self).into())
    }
}

#[cfg(feature = "bigdecimal")]
impl IntoExpr<Self> for bigdecimal::BigDecimal {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value(self.clone().into())
    }
}

impl<T, U, const N: usize> IntoExpr<[T]> for [U; N]
where
    U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| U::by_ref(item).untyped),
        ))
    }

    fn by_ref(&self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| U::by_ref(item).untyped),
        ))
    }
}

impl<T, U, const N: usize> IntoExpr<[T]> for &[U; N]
where
    U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| U::by_ref(item).untyped),
        ))
    }

    fn by_ref(&self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| U::by_ref(item).untyped),
        ))
    }
}

impl<T, E: IntoExpr<T>> IntoExpr<[T]> for &[E] {
    fn into_expr(self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| E::by_ref(item).untyped),
        ))
    }

    fn by_ref(&self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| E::by_ref(item).untyped),
        ))
    }
}

impl<T, U> IntoExpr<[T]> for Vec<U>
where
    U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.into_iter().map(|item| item.into_expr().untyped),
        ))
    }

    fn by_ref(&self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.iter().map(|item| item.by_ref().untyped),
        ))
    }
}

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
    fn assert_into_expr<T: ?Sized, E: IntoExpr<T>>() {}

    assert_into_expr::<i64, i64>();
    assert_into_expr::<(String, String), (&String, &String)>();
    assert_into_expr::<[(String, String)], &[(&String, &String)]>();
    assert_into_expr::<[(String, String)], [(&String, &String); 3]>();
    assert_into_expr::<[(String, String)], &[(&String, &String); 3]>();
}
