use toasty_core::stmt;

use super::NodeId;

/// Conditionally execute an operation only when a guard query returned rows.
///
/// `guard` is a dependency whose result is checked at runtime. If non-empty,
/// the `then_node` operation executes. Otherwise execution skips `then_node`.
///
/// **Important:** `then_node` is NOT in this node's `deps`. It is emitted
/// inside the then-block during exec planning, not in the normal topological
/// order.
#[derive(Debug)]
pub(crate) struct IfNonEmpty {
    /// The guard query whose output is tested for emptiness.
    pub(crate) guard: NodeId,

    /// The operation to execute conditionally.
    pub(crate) then_node: NodeId,

    /// The type of this node's output.
    pub(crate) ty: stmt::Type,
}
