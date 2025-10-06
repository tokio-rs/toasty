use super::{plan, Exec, Result};
use toasty_core::stmt;
use toasty_core::stmt::ValueStream;

struct RowStack<'a> {
    parent: Option<&'a RowStack<'a>>,
    row: &'a stmt::Value,
}

impl Exec<'_> {
    pub(super) async fn action_nested_merge(&mut self, action: &plan::NestedMerge) -> Result<()> {
        // Load all input data upfront
        let mut input = Vec::with_capacity(action.inputs.len());

        for var_id in &action.inputs {
            // TODO: make loading input concurrent
            let data = self.vars.load(*var_id).collect().await?;
            input.push(data);
        }

        // Load the root rows
        let root_rows = &input[action.root.source];
        let mut merged_rows = vec![];

        // Iterate over each record to perform the nested merge
        for row in root_rows {
            let stack = RowStack { parent: None, row };
            merged_rows.push(self.materialize_nested_row(&stack, &action.root, &input)?);
        }

        // Store the output
        self.vars
            .store(action.output, ValueStream::from_vec(merged_rows));

        Ok(())
    }

    fn materialize_nested_row(
        &self,
        row: &RowStack<'_>,
        level: &plan::NestedLevel,
        input: &[Vec<stmt::Value>],
    ) -> Result<stmt::Value> {
        // Collected all nested rows for this row.
        let mut nested = vec![];

        for nested_child in &level.nested {
            // Find the batch-loaded input
            let nested_input = &input[nested_child.level.source];

            for nested_row in nested_input {
                let nested_stack = RowStack {
                    parent: Some(row),
                    row: nested_row,
                };

                // Filter the input
                if !self.eval_merge_qualification(&nested_child.qualification, &nested_stack) {
                    continue;
                }

                // Recurse nested merge and track the result
                nested.push(self.materialize_nested_row(
                    &nested_stack,
                    &nested_child.level,
                    input,
                )?);
            }
        }

        // Now build the row by performing the projection with the collected data
        todo!("projection={:#?}", level.projection);
    }

    fn eval_merge_qualification(
        &self,
        qual: &plan::MergeQualification,
        row: &RowStack<'_>,
    ) -> bool {
        match qual {
            plan::MergeQualification::Predicate(func) => {
                assert_eq!(2, func.args.len());
                todo!("qual={qual:#?}");
            }
        }
    }
}
