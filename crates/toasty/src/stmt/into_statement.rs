use super::{IntoSelect, List, Statement};

use std::marker::PhantomData;

use toasty_core::stmt;

/// Convert a value into a [`Statement`].
///
/// This trait bridges query builders to `Statement<T>`. The associated
/// `Output` type encodes what the statement returns when executed:
/// - Select queries: `Output = List<M>` (returns a list)
/// - Create builders: `Output = M` (returns a single item)
/// - Tuples: `Output = (Q1::Output, Q2::Output, ...)` (composed naturally)
/// - Homogeneous batches: `Output = List<M>` (list encoding)
pub trait IntoStatement {
    type Output;
    fn into_statement(self) -> Statement<Self::Output>;
}

/// Blanket implementation: any `IntoSelect` type produces a `Statement`
/// whose output is `List<Model>` — select queries return lists.
impl<T: IntoSelect> IntoStatement for T {
    type Output = List<T::Model>;

    fn into_statement(self) -> Statement<List<T::Model>> {
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
                    $( {
                        let mut untyped = self.$idx.into_statement().untyped;
                        ensure_batch_returning(&mut untyped);
                        stmt::Expr::stmt(untyped)
                    }, )+
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

/// Ensure a sub-statement has a returning clause for batch composition.
///
/// Mutation statements (delete, update) without a returning clause default to
/// returning a count. In a batch, every sub-statement must produce a value, so
/// we set an empty record returning (`Returning::Value(Expr::record([]))`)
/// which represents unit.
fn ensure_batch_returning(stmt: &mut stmt::Statement) {
    if !stmt.is_query() && stmt.returning().is_none() {
        stmt.set_returning(stmt::Returning::Value(stmt::Expr::record::<stmt::Expr>([])));
    }
}

/// Helper to build a batched statement from an iterator of queries.
fn batch_from_iter<Q: IntoStatement>(iter: impl Iterator<Item = Q>) -> Statement<List<Q::Output>> {
    let exprs: Vec<stmt::Expr> = iter
        .map(|q| {
            let mut untyped = q.into_statement().untyped;
            ensure_batch_returning(&mut untyped);
            stmt::Expr::stmt(untyped)
        })
        .collect();

    let query = stmt::Query::new_single(stmt::Values::from(stmt::Expr::record(exprs)));

    Statement {
        untyped: query.into(),
        _p: PhantomData,
    }
}

/// Dynamic batch via `Vec<Q>`: all queries have the same type, returns `List<Q::Output>`.
impl<Q: IntoStatement> IntoStatement for Vec<Q> {
    type Output = List<Q::Output>;

    fn into_statement(self) -> Statement<Self::Output> {
        batch_from_iter(self.into_iter())
    }
}

/// Dynamic batch via `[Q; N]`: fixed-size array of homogeneous queries, returns `List<Q::Output>`.
impl<Q: IntoStatement, const N: usize> IntoStatement for [Q; N] {
    type Output = List<Q::Output>;

    fn into_statement(self) -> Statement<Self::Output> {
        batch_from_iter(self.into_iter())
    }
}
