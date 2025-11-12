use toasty_core::driver::Rows;
use toasty_core::stmt;

use crate::engine::eval;
use crate::engine::exec::{Action, Exec, Output, VarId};
use crate::Result;

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

#[derive(Debug)]
struct RowStack<'a> {
    parent: Option<&'a RowStack<'a>>,
    row: &'a stmt::Value,
    /// Matches `position` from ExprArg
    position: usize,
}

#[derive(Debug)]
struct RowAndNested<'a> {
    row: &'a stmt::Value,
    nested: &'a [stmt::Value],
}

impl Exec<'_> {
    pub(super) async fn action_nested_merge(&mut self, action: &NestedMerge) -> Result<()> {
        // Load all input data upfront
        let mut input = Vec::with_capacity(action.inputs.len());

        for var_id in &action.inputs {
            // TODO: make loading input concurrent
            let data = self
                .vars
                .load(*var_id)
                .await?
                .into_values()
                .collect()
                .await?;
            input.push(data);
        }

        // Load the root rows
        let root_rows = &input[action.root.source];
        let mut merged_rows = vec![];

        // Iterate over each record to perform the nested merge
        for row in root_rows {
            let stack = RowStack {
                parent: None,
                row,
                position: 0,
            };
            merged_rows.push(self.materialize_nested_row(&stack, &action.root, &input)?);
        }

        // Store the output
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            Rows::value_stream(merged_rows),
        );

        Ok(())
    }

    fn materialize_nested_row(
        &self,
        row_stack: &RowStack<'_>,
        level: &NestedLevel,
        input: &[Vec<stmt::Value>],
    ) -> Result<stmt::Value> {
        // Collected all nested rows for this row.
        let mut nested = vec![];

        for nested_child in &level.nested {
            // Find the batch-loaded input
            let nested_input = &input[nested_child.level.source];
            let mut nested_rows_projected = vec![];

            for nested_row in nested_input {
                let nested_stack = RowStack {
                    parent: Some(row_stack),
                    row: nested_row,
                    position: row_stack.position + 1,
                };

                // Filter the input
                if !self.eval_merge_qualification(&nested_child.qualification, &nested_stack)? {
                    continue;
                }

                // Recurse nested merge and track the result
                nested_rows_projected.push(self.materialize_nested_row(
                    &nested_stack,
                    &nested_child.level,
                    input,
                )?);
            }

            nested.push(if nested_child.single {
                assert!(nested_rows_projected.len() <= 1, "TODO: error handling");

                if let Some(row) = nested_rows_projected.into_iter().next() {
                    row
                } else {
                    stmt::Value::Null
                }
            } else {
                stmt::Value::List(nested_rows_projected)
            });
        }

        // Project the row with the nested data as arguments.
        let eval_input = RowAndNested {
            row: row_stack.row,
            nested: &nested[..],
        };

        level.projection.eval(&eval_input)
    }

    fn eval_merge_qualification(
        &self,
        qual: &MergeQualification,
        row: &RowStack<'_>,
    ) -> Result<bool> {
        match qual {
            MergeQualification::Predicate(func) => func.eval_bool(row),
        }
    }
}

impl stmt::Input for &RowStack<'_> {
    fn resolve_arg(
        &mut self,
        expr_arg: &stmt::ExprArg,
        projection: &stmt::Projection,
    ) -> Option<stmt::Expr> {
        let mut current: &RowStack<'_> = self;

        // Find the stack level that corresponds with the argument.
        loop {
            if current.position == expr_arg.position {
                break;
            }

            let Some(parent) = current.parent else {
                todo!()
            };
            current = parent;
        }

        // Get the value and apply projection
        Some(current.row.entry(projection).to_expr())
    }
}

impl stmt::Input for &RowAndNested<'_> {
    fn resolve_arg(
        &mut self,
        expr_arg: &stmt::ExprArg,
        projection: &stmt::Projection,
    ) -> Option<stmt::Expr> {
        let base = if expr_arg.position == 0 {
            self.row
        } else {
            &self.nested[expr_arg.position - 1]
        };

        Some(base.entry(projection).to_expr())
    }
}

impl From<NestedMerge> for Action {
    fn from(src: NestedMerge) -> Self {
        Self::NestedMerge(src)
    }
}
