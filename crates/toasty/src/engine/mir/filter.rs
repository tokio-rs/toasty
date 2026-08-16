use toasty_core::stmt;

use crate::engine::{eval, mir};

/// Keeps the input rows for which `predicate` holds.
///
/// The predicate is a function of one argument: `arg(0)` is the current row.
/// Every input row is read (the whole input must be scanned), so the input
/// is an [`Always`](mir::operation::InputRead::Always) read even though
/// evaluation is per-row.
///
/// Used when the database cannot apply all filter conditions natively (e.g.,
/// NoSQL drivers with limited query capabilities).
#[derive(Debug)]
pub(crate) struct Filter {
    /// The node producing the records to filter.
    pub(crate) input: mir::NodeId,

    /// The predicate: `arg(0)` = current row; keep the row when true.
    pub(crate) predicate: eval::Func,

    /// The output type (same as input, but potentially fewer rows).
    pub(crate) ty: stmt::Type,
}

impl From<Filter> for mir::Node {
    fn from(value: Filter) -> Self {
        mir::Operation::Filter(value).into()
    }
}
