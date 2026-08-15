use super::NodeId;

/// A condition gating a [`Node`](super::Node)'s execution.
///
/// Deliberately not `Copy`: future variants may carry non-copyable data
/// (e.g. a boolean `eval::Func`, the planned replacement for the `Guard`
/// action). Consumers borrow conds rather than clone them.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Cond {
    /// The referenced node produced at least one row.
    NonEmpty(NodeId),
}
