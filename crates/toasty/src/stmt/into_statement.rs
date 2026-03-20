use super::{List, Statement};

use std::marker::PhantomData;

use toasty_core::stmt;

/// Convert a value into a [`Statement`].
///
/// This trait bridges query builders to [`Statement<T>`](Statement). The
/// associated [`Returning`](IntoStatement::Returning) type encodes what the
/// statement produces when executed:
///
/// | Builder | `Returning` |
/// |---|---|
/// | [`Query<M>`] | [`List<M>`] |
/// | [`Delete<M>`] | `()` |
/// | [`Association<M>`] | [`List<M>`] |
/// | Create builders (generated) | `M` |
/// | Tuples of builders | `(R1, R2, …)` |
/// | `Vec<Q>` / `[Q; N]` | [`List<Q::Returning>`](List) |
///
/// Tuples, `Vec`, and arrays batch multiple statements into a single
/// round-trip. Each sub-statement runs independently; the combined result is
/// a tuple or list of individual results.
pub trait IntoStatement {
    /// The type this statement produces when executed.
    type Returning;

    /// Consume `self` and produce the [`Statement`].
    fn into_statement(self) -> Statement<Self::Returning>;
}

macro_rules! impl_into_statement_for_tuple {
    ( $( $Q:ident ),+ ; $n:tt ; $( $idx:tt ),+ ) => {
        impl< $( $Q: IntoStatement ),+ > IntoStatement for ( $( $Q, )+ ) {
            type Returning = ( $( $Q::Returning, )+ );

            fn into_statement(self) -> Statement<Self::Returning> {
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
fn batch_from_iter<Q: IntoStatement>(
    iter: impl Iterator<Item = Q>,
) -> Statement<List<Q::Returning>> {
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
    type Returning = List<Q::Returning>;

    fn into_statement(self) -> Statement<Self::Returning> {
        batch_from_iter(self.into_iter())
    }
}

/// Dynamic batch via `[Q; N]`: fixed-size array of homogeneous queries, returns `List<Q::Output>`.
impl<Q: IntoStatement, const N: usize> IntoStatement for [Q; N] {
    type Returning = List<Q::Returning>;

    fn into_statement(self) -> Statement<Self::Returning> {
        batch_from_iter(self.into_iter())
    }
}
