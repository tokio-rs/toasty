use super::{List, Query, Statement};

/// Convert a query into a scope statement over model `M`.
///
/// `IntoScope<M>` captures "this is a query targeting model `M`, regardless of
/// cardinality." It admits the three forms a relation accessor can produce —
/// `Query<List<M>>`, `Query<Option<M>>`, and `Query<M>` — and lowers each to a
/// `Statement<List<M>>` suitable for use as an insert scope (see
/// [`Insert::set_scope`](crate::stmt::Insert::set_scope)).
///
/// Compare to [`IntoStatement`](crate::stmt::IntoStatement), which preserves the
/// query's returning type and so distinguishes the three forms above. `IntoScope`
/// erases that distinction, since the scope of an insert only cares about which
/// rows the query addresses, not how many it would return on execution.
///
/// For `Query<Option<M>>` and `Query<M>` — produced by [`Query::first`] and
/// [`Query::one`] respectively — the `single` flag and the paired `LIMIT 1`
/// added by those narrowing methods are cleared, since `Statement<List<M>>`
/// would otherwise be self-contradictory ("a list that returns one row").
///
/// [`Query::first`]: crate::stmt::Query::first
/// [`Query::one`]: crate::stmt::Query::one
pub trait IntoScope<M> {
    /// Lower `self` into a `Statement<List<M>>` for use as an insert scope.
    fn into_scope(self) -> Statement<List<M>>;
}

impl<M> IntoScope<M> for Query<List<M>> {
    fn into_scope(self) -> Statement<List<M>> {
        Statement::from_untyped_stmt(self.untyped.into())
    }
}

impl<M> IntoScope<M> for Query<Option<M>> {
    fn into_scope(self) -> Statement<List<M>> {
        Statement::from_untyped_stmt(widen(self.untyped).into())
    }
}

impl<M> IntoScope<M> for Query<M> {
    fn into_scope(self) -> Statement<List<M>> {
        Statement::from_untyped_stmt(widen(self.untyped).into())
    }
}

fn widen(mut query: toasty_core::stmt::Query) -> toasty_core::stmt::Query {
    if query.single {
        query.single = false;
        query.limit = None;
    }
    query
}
