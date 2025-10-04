use super::{eval, stmt, Action, VarId};
use std::collections::HashMap;

/// Nested merge operation - combines parent and child materializations
///
/// The nested merge algorithm is recursive.
/// * First, we need to get the batch-loaded records for all statements.
/// * Then, starting at the root we:
///     * Iterate over each record.
///         * The record is not the final projection, it may include extra fields.
///     * For each nested stmt, filter records for the curent row
///     * Perform a recursive nested merge.
///     * Take the results of of the recursive nested merge
///     * project the curent row
///     * Store in vec to return.
#[derive(Debug, Clone)]
pub(crate) struct NestedMerge {
    /// Input sources. NestedLevel will reference their inputs by index in this vec.
    pub(crate) inputs: Vec<VarId>,

    /// Output variable, where to store the merged values
    pub(crate) output: VarId,

    /// The root level
    pub(crate) root: NestedLevel,
}

/// A single level in the nesting hierarchy
///
/// Each level represents one child relationship and contains:
/// - How to load the child data
/// - How to filter it for each parent (can reference ANY ancestor)
/// - How to project it (may include ExprArgs for its own children)
/// - Its own children (recursive nesting)
#[derive(Debug, Clone)]
pub(crate) struct NestedLevel {
    /// Input for this level as an index in `NestedMerge::inputs`
    pub(crate) source: usize,

    /// Projection for this level (before passing to parent) Argument 0 is the
    /// row for the current level, all other arguments are the results of the
    /// recursive nested merge.
    pub(crate) projection: eval::Func,

    /// This level's children (recursive nesting)
    /// Empty for leaf nodes
    pub(crate) nested: Vec<NestedChild>,
}

#[derive(Debug, Clone)]
pub(crate) struct NestedChild {
    /// The nested level for this child
    pub(crate) level: NestedLevel,

    /// How to filter rows to match the parent request
    pub(crate) qualification: MergeQualification,
}

/// How to filter nested records for a parent record
#[derive(Debug, Clone)]
pub(crate) enum MergeQualification {
    /// Equality on specific columns (uses hash index)
    ///
    /// root_columns can reference ANY ancestor in the context stack.
    /// Each entry is (levels_up, column_index):
    ///   - levels_up: 0 = immediate parent, 1 = grandparent, 2 = great-grandparent, etc.
    ///   - column_index: which column from that ancestor record
    ///
    /// Example: Tags referencing both Post and User
    ///   root_columns: [(0, 0), (1, 0)]  // Post.id (0 up), User.id (1 up)
    ///   index_id: VarId for Tags data (indexes HashMap will have entry for this VarId)
    ///
    /// During planning: The nested_columns are collected and stored in NestedMerge.indexes
    /// During execution: Use the pre-built index from NestedMerge.indexes[index_id]
    Equality {
        /// Which ancestor levels and columns to extract for the lookup key
        /// Vec<(levels_up, column_index)>
        root_columns: Vec<(usize, usize)>,

        /// Which VarId's index to use (references NestedMerge.indexes)
        index_id: VarId,
    },

    /// General predicate evaluation (uses nested loop)
    /// Args: [ancestor_stack..., nested_record] -> bool
    ///
    /// The ancestor stack contains all ancestors from root to immediate parent:
    /// [root, child, grandchild, ..., immediate_parent, nested_record]
    ///
    /// Execution: For each parent record, evaluate predicate against all
    /// nested records. No indexing.
    Predicate(eval::Func),
}

impl From<NestedMerge> for Action {
    fn from(src: NestedMerge) -> Self {
        Self::NestedMerge(src)
    }
}
