use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{exec, mir};

/// Merges child records into parent records.
///
/// Used to combine the results of parent and child queries when loading
/// associations (e.g., users with their todos). The merge produces nested
/// records where each parent contains its associated children.
#[derive(Debug)]
pub(crate) struct NestedMerge {
    /// The nodes providing parent and child data to merge.
    /// [`NestedLevel`](exec::NestedLevel) references them by position.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// Configuration for how to perform the merge at each nesting level.
    pub(crate) root: exec::NestedLevel,

    /// Flat list of hash indexes to build before the merge, computed at plan time.
    pub(crate) hash_indexes: Vec<exec::MergeIndex>,

    /// Flat list of sorted indexes to build before the merge, computed at plan time.
    pub(crate) sort_indexes: Vec<exec::MergeIndex>,

    /// Output type: `List<root.projection.ret>`.
    pub(crate) ty: stmt::Type,
}

impl NestedMerge {
    pub(crate) fn new(
        inputs: IndexSet<mir::NodeId>,
        root: exec::NestedLevel,
        hash_indexes: Vec<exec::MergeIndex>,
        sort_indexes: Vec<exec::MergeIndex>,
    ) -> Self {
        let ty = stmt::Type::list(root.projection.ret.clone());
        NestedMerge {
            inputs,
            root,
            hash_indexes,
            sort_indexes,
            ty,
        }
    }
}

impl From<NestedMerge> for mir::Node {
    fn from(value: NestedMerge) -> Self {
        mir::Operation::NestedMerge(value).into()
    }
}
