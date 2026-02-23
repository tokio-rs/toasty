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
/// 2. Building all hash indexes from the pre-computed `indexes` list
/// 3. Processing each root row:
///    - For each nested child relationship at this level:
///      - Filters batch-loaded child data using the qualification (hash lookup
///        or scan predicate)
///      - Recursively merges each matching child row with its own children
///      - Collects results into a list, or a single value if `single` is `true`
///    - Projects the final row by applying the projection function with the
///      current row and all nested children
///    - Adds the projected row to output
/// 4. Returning all merged rows with their nested data
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

    /// Flat list of hash indexes to build before the merge, computed at plan time.
    ///
    /// Each entry describes which input to index and which fields form the key.
    /// `MergeQualification::HashLookup` references entries by position in this vec.
    pub(crate) hash_indexes: Vec<MergeIndex>,

    /// Flat list of sorted indexes to build before the merge, computed at plan time.
    ///
    /// `MergeQualification::SortLookup` references entries by position in this vec.
    pub(crate) sort_indexes: Vec<MergeIndex>,
}

/// Describes one hash index to build over an input before the merge begins.
///
/// Computed entirely at plan time; execution builds the index mechanically.
#[derive(Debug, Clone)]
pub(crate) struct MergeIndex {
    /// Index into `NestedMerge::inputs` — which input to build the index over.
    pub(crate) source: usize,

    /// Which field(s) of the child record form the hash key.
    pub(crate) child_projections: Vec<stmt::Projection>,
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

    /// True when the parent model's field for this child is nullable (`Option<T>`).
    /// When true and `single=true`, NestedMerge emits the Option encoding:
    /// `Record([0])` for None (no match), `Record([1, value])` for Some(value).
    pub(crate) nullable: bool,
}

/// How to filter nested records for a parent record
#[derive(Debug, Clone)]
pub(crate) enum MergeQualification {
    /// Parent selects all records provided from sub statements.
    ///
    /// This is typically used when the parent query returns only one row, so all
    /// nested records end up associated with that single parent record.
    All,

    /// Hash index lookup — O(1) per parent row after a one-time O(M) index build.
    ///
    /// For `single=true` (has_one / belongs_to) relationships only, since
    /// `HashIndex` requires unique keys across the child set.
    ///
    /// `index` is a position into `NestedMerge::hash_indexes`.
    /// `lookup_key` evaluates against the ancestor row stack to produce the key.
    HashLookup {
        /// Position in `NestedMerge::hash_indexes`.
        index: usize,
        /// Evaluates against the ancestor `RowStack` to produce the lookup key.
        lookup_key: eval::Func,
    },

    /// Sorted index lookup — O(log M + k) per parent row after a one-time O(M log M) build.
    ///
    /// For `single=false` (has_many) relationships. Supports duplicate keys:
    /// `find_range(Included(key), Included(key))` returns all matching children.
    ///
    /// `index` is a position into `NestedMerge::sort_indexes`.
    SortLookup {
        /// Position in `NestedMerge::sort_indexes`.
        index: usize,
        /// Evaluates against the ancestor `RowStack` to produce the lookup key.
        lookup_key: eval::Func,
    },

    /// General predicate evaluation (nested loop — O(N×M)).
    ///
    /// Args: [ancestor_stack..., nested_record] -> bool
    ///
    /// The ancestor stack contains all ancestors from root to immediate parent:
    /// [root, child, grandchild, ..., immediate_parent, nested_record]
    ///
    /// Execution: For each parent record, evaluate predicate against all
    /// nested records. No indexing.
    Scan(eval::Func),
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

struct Indices<'a> {
    hash: Vec<stmt::HashIndex<'a>>,
    sort: Vec<stmt::SortedIndex<'a>>,
}

impl Exec<'_> {
    pub(super) async fn action_nested_merge(&mut self, action: &NestedMerge) -> Result<()> {
        // Load all input data upfront
        let mut inputs = Vec::with_capacity(action.inputs.len());

        for var_id in &action.inputs {
            inputs.push(match self.vars.load(*var_id).await? {
                Rows::Count(count) => Input::Count(count),
                Rows::Value(value) => Input::Value(match value {
                    stmt::Value::List(items) => items,
                    value => vec![value],
                }),
                Rows::Stream(value_stream) => Input::Value(value_stream.collect().await?),
            });
        }

        // Build all hash and sorted indexes from the plan-time-computed flat lists.
        let indices = Indices {
            hash: action
                .hash_indexes
                .iter()
                .map(|mi| {
                    let Input::Value(values) = &inputs[mi.source] else {
                        panic!("HashLookup source must be a Value input, not Count")
                    };
                    stmt::HashIndex::new(values, &mi.child_projections)
                })
                .collect(),
            sort: action
                .sort_indexes
                .iter()
                .map(|mi| {
                    let Input::Value(values) = &inputs[mi.source] else {
                        panic!("SortLookup source must be a Value input, not Count")
                    };
                    stmt::SortedIndex::new(values, &mi.child_projections)
                })
                .collect(),
        };

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
                    merged_rows.push(self.merge_nested_row(
                        &row_stack,
                        &action.root,
                        &inputs,
                        &indices,
                    )?);
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
                    merged_rows.push(self.merge_nested_row(
                        &stack,
                        &action.root,
                        &inputs,
                        &indices,
                    )?);
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
    /// 2. **Filters child rows**: For each child, uses either a pre-built hash index
    ///    (O(1) lookup) or a linear scan with the qualification predicate
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
    /// * `inputs` - All batch-loaded data for the entire merge, indexed by source
    /// * `indexes` - Pre-built hash indexes, indexed by position in `NestedMerge::indexes`
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
        indices: &Indices<'_>,
    ) -> Result<stmt::Value> {
        // Collected all nested rows for this row.
        let mut nested = vec![];

        for nested_child in &level.nested {
            // Find the batch-loaded input
            let Input::Value(nested_input) = &inputs[nested_child.level.source] else {
                todo!("input={:#?}", inputs[nested_child.level.source])
            };
            let mut nested_rows_projected = vec![];

            // Process a single matching child row: recurse and collect the result.
            let mut process = |nested_row: &stmt::Value| -> Result<()> {
                let nested_stack = RowStack {
                    parent: Some(row_stack),
                    row: nested_row,
                    position: row_stack.position + 1,
                };
                nested_rows_projected.push(self.merge_nested_row(
                    &nested_stack,
                    &nested_child.level,
                    inputs,
                    indices,
                )?);
                Ok(())
            };

            match &nested_child.qualification {
                MergeQualification::All => {
                    for row in nested_input {
                        process(row)?;
                    }
                }
                MergeQualification::HashLookup { index, lookup_key } => {
                    let key_val = lookup_key.eval(row_stack)?;
                    if let Some(row) = indices.hash[*index].find(key_as_slice(&key_val)) {
                        process(row)?;
                    }
                }
                MergeQualification::SortLookup { index, lookup_key } => {
                    let key_val = lookup_key.eval(row_stack)?;
                    let key = key_as_slice(&key_val);
                    for row in indices.sort[*index].find_range(
                        std::ops::Bound::Included(key),
                        std::ops::Bound::Included(key),
                    ) {
                        process(row)?;
                    }
                }
                MergeQualification::Scan(func) => {
                    for row in nested_input {
                        let stack = RowStack {
                            parent: Some(row_stack),
                            row,
                            position: row_stack.position + 1,
                        };
                        if func.eval_bool(&stack)? {
                            process(row)?;
                        }
                    }
                }
            }

            nested.push(if nested_child.single {
                assert!(nested_rows_projected.len() <= 1, "TODO: error handling");

                if let Some(row) = nested_rows_projected.into_iter().next() {
                    if nested_child.nullable {
                        // Some(value) encoding: List([value])
                        stmt::Value::List(vec![row])
                    } else {
                        row
                    }
                } else if nested_child.nullable {
                    // None encoding: List([])
                    stmt::Value::List(vec![])
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
}

/// Extract a `&[Value]` key from a parent key value.
///
/// `Record` values are flattened to their field slice (composite key).
/// All other values are wrapped in a single-element slice (scalar key).
fn key_as_slice(val: &stmt::Value) -> &[stmt::Value] {
    match val {
        stmt::Value::Record(r) => r.as_slice(),
        v => std::slice::from_ref(v),
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
