use toasty_core::driver::Rows;
use toasty_core::stmt;

use crate::engine::eval;
use crate::engine::exec::{Action, Exec, Output, VarId};
use crate::Result;

/// Combines parent and child data into nested structures.
///
/// The nested merge algorithm processes hierarchical data by:
///
/// 1. Loading all batch data upfront - fetches all input data for all levels
///    before processing
/// 2. Processing each root row:
///    - For each nested child relationship at this level:
///      - Filters batch-loaded child data to find matching rows using the
///        qualification predicate
///      - Recursively merges each matching child row with its own children
///      - Collects results into a list, or a single value if `single` is `true`
///    - Projects the final row by applying the projection function with the
///      current row and all nested children
///    - Adds the projected row to output
/// 3. Returning all merged rows with their nested data
///
/// # Note
///
/// Rows loaded from batch queries are not the final projection. They may include
/// extra fields needed for filtering or projecting nested children.
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
    /// Parent selects all records provided from sub statements.
    ///
    /// This is typically used when the parent query returns only one row, so all
    /// nested records end up associated with that single parent record.
    All,

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

#[derive(Debug)]
enum Input {
    Count(u64),
    Value(Vec<stmt::Value>),
}

impl Exec<'_> {
    pub(super) async fn action_nested_merge(&mut self, action: &NestedMerge) -> Result<()> {
        // Load all input data upfront
        let mut inputs = Vec::with_capacity(action.inputs.len());

        for var_id in &action.inputs {
            inputs.push(match self.vars.load(*var_id).await? {
                Rows::Count(count) => Input::Count(count),
                Rows::Value(value) => Input::Value(value.unwrap_list()),
                Rows::Stream(value_stream) => Input::Value(value_stream.collect().await?),
            });
        }

        // Load the root rows
        let mut merged_rows = vec![];

        match &inputs[action.root.source] {
            Input::Count(count) => {
                let row_stack = RowStack {
                    parent: None,
                    // Bit of a hack
                    row: &stmt::Value::Null,
                    position: 0,
                };

                for _ in 0..*count {
                    merged_rows.push(self.merge_nested_row(&row_stack, &action.root, &inputs)?);
                }
            }
            Input::Value(root_rows) => {
                // Iterate over each record to perform the nested merge
                for row in root_rows {
                    let stack = RowStack {
                        parent: None,
                        row,
                        position: 0,
                    };
                    merged_rows.push(self.merge_nested_row(&stack, &action.root, &inputs)?);
                }
            }
        }

        // Store the output
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            Rows::value_stream(merged_rows),
        );

        Ok(())
    }

    /// Recursively merges a single row with its nested child data.
    ///
    /// This is the core recursive function that processes one row at a time through
    /// the nesting hierarchy. For each row, it:
    ///
    /// 1. **Processes each child relationship**: Iterates through all nested children
    ///    defined at this level
    /// 2. **Filters child rows**: For each child, scans the batch-loaded child data
    ///    and applies the qualification predicate to find matching rows
    /// 3. **Recursively merges**: For each matching child row, recursively calls
    ///    `merge_nested_row` to process its own children
    /// 4. **Collects results**: Gathers all processed child rows into a list (or
    ///    single value if `single: true`)
    /// 5. **Projects final result**: Applies the projection function with the current
    ///    row and all nested children as arguments to produce the final output
    ///
    /// # Arguments
    ///
    /// * `row_stack` - The ancestor stack containing this row and all parent rows,
    ///   used for evaluating predicates that reference parent data
    /// * `level` - The current nesting level descriptor with source, projection, and children
    /// * `input` - All batch-loaded data for the entire merge, indexed by source
    ///
    /// # Returns
    ///
    /// The projected value for this row, with all nested children merged in according
    /// to the projection function.
    fn merge_nested_row(
        &self,
        row_stack: &RowStack<'_>,
        level: &NestedLevel,
        inputs: &[Input],
    ) -> Result<stmt::Value> {
        // Collected all nested rows for this row.
        let mut nested = vec![];

        for nested_child in &level.nested {
            // Find the batch-loaded input
            let Input::Value(nested_input) = &inputs[nested_child.level.source] else {
                todo!("input={:#?}", inputs[nested_child.level.source])
            };
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
                nested_rows_projected.push(self.merge_nested_row(
                    &nested_stack,
                    &nested_child.level,
                    inputs,
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
            MergeQualification::All => Ok(true),
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
