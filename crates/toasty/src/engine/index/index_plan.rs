use toasty_core::{
    schema::db::{Index, TableId},
    stmt,
};

#[derive(Debug)]
pub(crate) struct IndexPlan<'a> {
    /// The index to use to execute the query
    pub(crate) index: &'a Index,

    /// Filter to apply to the index
    pub(crate) index_filter: stmt::Expr,

    /// How to filter results after applying the index filter
    pub(crate) result_filter: Option<stmt::Expr>,

    /// True if we have to apply the result filter our self
    pub(crate) post_filter: Option<stmt::Expr>,

    /// Key expression for direct `GetByKey` routing. Populated when every index
    /// key column has an exact predicate (equality or IN). Two forms:
    ///
    /// - `Expr::Value(Value::List([Value::Record([...]), ...]))` — all key values
    ///   are literals known at plan time; the planner emits a constant `GetByKey`.
    /// - `Expr::Arg(0)` — key values come from a runtime input (e.g. `pk IN
    ///   (arg[0])` batch-load); the planner wires the input node directly.
    ///
    /// When `Some`, the planner routes to `GetByKey` instead of `QueryPk`.
    pub(crate) key_values: Option<stmt::Expr>,

    /// True when this plan targets the primary key and `key_values` was populated.
    /// Captured before `key_values` can be consumed via `.take()`.
    pub(crate) has_pk_keys: bool,
}

impl IndexPlan<'_> {
    pub(crate) fn table_id(&self) -> TableId {
        self.index.on
    }
}
