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
}

impl IndexPlan<'_> {
    pub(crate) fn table_id(&self) -> TableId {
        self.index.on
    }
}
