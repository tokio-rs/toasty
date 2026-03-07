use super::{IntoSelect, Statement};

use std::marker::PhantomData;

use toasty_core::stmt;

/// Convert a value into a [`Statement`].
///
/// This trait bridges query builders to `Statement<T>`. The associated
/// `Output` type encodes what the statement returns when executed:
/// - Select queries: `Output = Vec<M>` (returns a list)
/// - Create builders: `Output = M` (returns a single item)
/// - Tuples: `Output = (Q1::Output, Q2::Output, ...)` (composed naturally)
pub trait IntoStatement {
    type Output;
    fn into_statement(self) -> Statement<Self::Output>;
}

/// Blanket implementation: any `IntoSelect` type produces a `Statement`
/// whose output is `Vec<Model>` — select queries return lists.
impl<T: IntoSelect> IntoStatement for T {
    type Output = Vec<T::Model>;

    fn into_statement(self) -> Statement<Vec<T::Model>> {
        Statement {
            untyped: self.into_select().untyped.into(),
            _p: PhantomData,
        }
    }
}

macro_rules! impl_into_statement_for_tuple {
    ( $( $Q:ident ),+ ; $n:tt ; $( $idx:tt ),+ ) => {
        impl< $( $Q: IntoStatement ),+ > IntoStatement for ( $( $Q, )+ ) {
            type Output = ( $( $Q::Output, )+ );

            fn into_statement(self) -> Statement<Self::Output> {
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

impl_into_statement_for_tuple!(Q1, Q2; 2; 0, 1);
impl_into_statement_for_tuple!(Q1, Q2, Q3; 3; 0, 1, 2);
impl_into_statement_for_tuple!(Q1, Q2, Q3, Q4; 4; 0, 1, 2, 3);
impl_into_statement_for_tuple!(Q1, Q2, Q3, Q4, Q5; 5; 0, 1, 2, 3, 4);
impl_into_statement_for_tuple!(Q1, Q2, Q3, Q4, Q5, Q6; 6; 0, 1, 2, 3, 4, 5);
impl_into_statement_for_tuple!(Q1, Q2, Q3, Q4, Q5, Q6, Q7; 7; 0, 1, 2, 3, 4, 5, 6);
impl_into_statement_for_tuple!(Q1, Q2, Q3, Q4, Q5, Q6, Q7, Q8; 8; 0, 1, 2, 3, 4, 5, 6, 7);
