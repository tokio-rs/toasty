use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{eval, mir};

/// Keeps the input rows for which `predicate` holds.
///
/// The predicate's `arg(0)` is the current row; `arg(1 + i)` is the whole
/// output of `args[i]`. Every input row is read (the whole input must be
/// scanned), so the input — and each attached arg — is an
/// [`Always`](mir::operation::InputRead::Always) read even though evaluation
/// is per-row. A predicate that reads only attached args (a key-value
/// pre-filter) ignores `arg(0)`: when it is false every row is dropped, so
/// downstream operations see an empty list.
///
/// Used when the database cannot apply all filter conditions natively (e.g.,
/// NoSQL drivers with limited query capabilities).
#[derive(Debug)]
pub(crate) struct Filter {
    /// The node producing the records to filter.
    pub(crate) input: mir::NodeId,

    /// Nodes whose whole outputs the predicate reads as `arg(1 + i)`.
    pub(crate) args: IndexSet<mir::NodeId>,

    /// The predicate: `arg(0)` = current row, `arg(1 + i)` = `args[i]`; keep
    /// the row when true.
    pub(crate) predicate: eval::Func,

    /// The output type (same as input, but potentially fewer rows).
    pub(crate) ty: stmt::Type,
}

impl From<Filter> for mir::Node {
    fn from(value: Filter) -> Self {
        mir::Operation::Filter(value).into()
    }
}
