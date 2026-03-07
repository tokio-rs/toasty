use super::{IntoSelect, Statement};

use std::marker::PhantomData;

use toasty_core::stmt;

/// Convert a value into a [`Statement`].
///
/// This trait bridges query builders to `Statement<T>`. It is blanket-
/// implemented for anything that implements [`IntoSelect`].
pub trait IntoStatement<T> {
    fn into_statement(self) -> Statement<T>;
}

/// Blanket implementation: any `IntoSelect` type can produce a `Statement`
/// for its model type.
impl<T: IntoSelect> IntoStatement<T::Model> for T {
    fn into_statement(self) -> Statement<T::Model> {
        self.into_select().into()
    }
}

macro_rules! impl_into_statement_for_tuple {
    ( $( $T:ident : $Q:ident ),+ ; $n:tt ; $( $idx:tt ),+ ) => {
        impl< $( $T, $Q ),+ > IntoStatement<( $( Vec<$T>, )+ )> for ( $( $Q, )+ )
        where
            $( $Q: IntoStatement<$T>, )+
        {
            fn into_statement(self) -> Statement<( $( Vec<$T>, )+ )> {
                let exprs: Vec<stmt::Expr> = vec![
                    $( stmt::Expr::stmt(self.$idx.into_statement().untyped), )+
                ];

                let query = stmt::Query::new_single(
                    stmt::Values::from(stmt::Expr::record(exprs)),
                );

                Statement {
                    untyped: query.into(),
                    _p: PhantomData,
                }
            }
        }
    };
}

impl_into_statement_for_tuple!(T1: Q1, T2: Q2; 2; 0, 1);
impl_into_statement_for_tuple!(T1: Q1, T2: Q2, T3: Q3; 3; 0, 1, 2);
impl_into_statement_for_tuple!(T1: Q1, T2: Q2, T3: Q3, T4: Q4; 4; 0, 1, 2, 3);
impl_into_statement_for_tuple!(T1: Q1, T2: Q2, T3: Q3, T4: Q4, T5: Q5; 5; 0, 1, 2, 3, 4);
impl_into_statement_for_tuple!(T1: Q1, T2: Q2, T3: Q3, T4: Q4, T5: Q5, T6: Q6; 6; 0, 1, 2, 3, 4, 5);
impl_into_statement_for_tuple!(T1: Q1, T2: Q2, T3: Q3, T4: Q4, T5: Q5, T6: Q6, T7: Q7; 7; 0, 1, 2, 3, 4, 5, 6);
impl_into_statement_for_tuple!(T1: Q1, T2: Q2, T3: Q3, T4: Q4, T5: Q5, T6: Q6, T7: Q7, T8: Q8; 8; 0, 1, 2, 3, 4, 5, 6, 7);
