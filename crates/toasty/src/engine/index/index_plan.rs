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

    /// Literal key values for direct `GetByKey` routing: a `Value::List` of
    /// `Value::Record` entries (one per lookup), populated when every index key
    /// column has a literal equality predicate. When `Some`, the planner can
    /// route to `GetByKey` (e.g. DynamoDB `BatchGetItem`) instead of a query.
    pub(crate) key_values: Option<stmt::Value>,
}

impl IndexPlan<'_> {
    pub(crate) fn table_id(&self) -> TableId {
        self.index.on
    }
}
