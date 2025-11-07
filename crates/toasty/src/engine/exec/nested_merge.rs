use super::{plan, Exec, Result};
use toasty_core::driver::Rows;
use toasty_core::stmt;

impl Exec<'_> {
    pub(super) async fn action_nested_merge(&mut self, action: &plan::NestedMerge) -> Result<()> {
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
        level: &plan::NestedLevel,
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
        qual: &plan::MergeQualification,
        row: &RowStack<'_>,
    ) -> Result<bool> {
        match qual {
            plan::MergeQualification::Predicate(func) => func.eval_bool(row),
        }
    }
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
