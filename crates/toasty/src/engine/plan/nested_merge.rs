use crate::engine::{
    eval,
    plan::{Action, Output, VarId},
};

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
    pub(crate) output: Output,

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

    /// True if single value
    pub(crate) single: bool,
}

/// How to filter nested records for a parent record
#[derive(Debug, Clone)]
pub(crate) enum MergeQualification {
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
