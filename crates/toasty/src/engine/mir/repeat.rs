use toasty_core::stmt;

use crate::engine::mir;

/// Produces `value` once per row of `input`.
///
/// The input supplies only a cardinality — its rows are never read, and a
/// write that reports just an affected-row count works the same as one that
/// returns rows. Used for a returning clause whose value is fully known
/// client-side (e.g. an UPDATE whose assignments were all literals): the
/// database is not asked for any columns, and the result is the value
/// repeated once per affected row.
#[derive(Debug)]
pub(crate) struct Repeat {
    /// The node whose cardinality drives the repetition.
    pub(crate) input: mir::NodeId,

    /// The value produced for each input row.
    pub(crate) value: stmt::Value,

    /// Output type: `List<value's type>`.
    pub(crate) ty: stmt::Type,
}

impl From<Repeat> for mir::Node {
    fn from(value: Repeat) -> Self {
        mir::Operation::Repeat(value).into()
    }
}
