use super::{eval, stmt, Action, VarId};
use std::collections::HashMap;

/// Nested merge operation - combines parent and child materializations
///
/// Handles the ENTIRE nesting hierarchy for a root statement, not just one level.
/// This is necessary because deeply nested statements can reference data from any
/// ancestor in the hierarchy.
#[derive(Debug)]
pub(crate) struct NestedMerge {
    /// Root materialization variable (parent records)
    pub root: VarId,

    /// Nested hierarchy - children and their descendants
    /// Multiple entries at this level = siblings (e.g., User has Posts AND Comments)
    pub nested: Vec<NestedLevel>,

    /// Indexes to build upfront before execution
    /// Map from VarId (source data) to columns to index by
    /// Built during planning, used during execution
    pub indexes: HashMap<VarId, Vec<usize>>,

    /// Output variable (projected result with nested structure)
    pub output: VarId,

    /// Final projection to apply at root level
    /// Args: [root_record, filtered_collection_0, filtered_collection_1, ...]
    /// The filtered collections are bound to ExprArgs in the returning clause
    pub projection: eval::Func,
}

/// A single level in the nesting hierarchy
///
/// Each level represents one child relationship and contains:
/// - How to load the child data
/// - How to filter it for each parent (can reference ANY ancestor)
/// - How to project it (may include ExprArgs for its own children)
/// - Its own children (recursive nesting)
#[derive(Debug)]
pub(crate) struct NestedLevel {
    /// Source data (from child's ExecStatement)
    pub source: VarId,

    /// Which ExprArg in parent's projection this binds to
    pub arg_index: usize,

    /// How to filter nested records for each parent record
    /// Can reference ANY ancestor in the context stack, not just immediate parent
    pub qualification: MergeQualification,

    /// Projection for this level (before passing to parent)
    /// Contains ExprArgs for this level's children
    pub projection: eval::Func,

    /// This level's children (recursive nesting)
    /// Empty for leaf nodes
    pub nested: Vec<NestedLevel>,
}

/// How to filter nested records for a parent record
#[derive(Debug)]
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

impl Action {
    pub(crate) fn into_nested_merge(self) -> NestedMerge {
        match self {
            Self::NestedMerge(action) => action,
            _ => panic!(),
        }
    }
}

impl From<NestedMerge> for Action {
    fn from(src: NestedMerge) -> Self {
        Self::NestedMerge(src)
    }
}
