use toasty_core::{
    driver::{operation, Rows},
    stmt,
};

use crate::{
    engine::exec::{Action, Exec, Output, VarId},
    Result,
};

#[derive(Debug)]
pub(crate) struct ExecStatement {
    /// Where to get arguments for this action.
    pub input: Vec<VarId>,

    /// How to handle output
    pub output: ExecStatementOutput,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement,

    /// When true, the statement is a conditional update without any returning.
    pub conditional_update_with_no_returning: bool,
}

#[derive(Debug)]
pub(crate) struct ExecStatementOutput {
    /// Databases always return rows as a vec of values. This specifies the type
    /// of each value.
    pub ty: Option<Vec<stmt::Type>>,
    pub output: Output,
}

impl Exec<'_> {
    pub(super) async fn action_exec_statement(&mut self, action: &ExecStatement) -> Result<()> {
        let mut stmt = action.stmt.clone();

        // Collect input values and substitute into the statement
        if !action.input.is_empty() {
            let mut input_values = Vec::new();
            for var_id in &action.input {
                let values = self
                    .vars
                    .load(*var_id)
                    .await?
                    .into_values()
                    .collect()
                    .await?;
                input_values.push(stmt::Value::List(values));
            }

            stmt.substitute(&input_values);

            self.engine.simplify_stmt(&mut stmt);
        }

        debug_assert!(
            stmt.returning()
                .and_then(|returning| returning.as_expr())
                .map(|expr| expr.is_record())
                .unwrap_or(true),
            "stmt={stmt:#?}"
        );

        // Short circuit if we can statically determine there are no results
        if let stmt::Statement::Query(query) = &stmt {
            if let stmt::ExprSet::Values(values) = &query.body {
                if values.is_empty() {
                    assert!(!action.conditional_update_with_no_returning);

                    let rows = if action.output.ty.is_some() {
                        Rows::Values(stmt::ValueStream::default())
                    } else {
                        Rows::Count(0)
                    };

                    self.vars.store(
                        action.output.output.var,
                        action.output.output.num_uses,
                        rows,
                    );

                    return Ok(());
                }
            }
        }

        let op = operation::QuerySql {
            stmt,
            ret: if action.conditional_update_with_no_returning {
                Some(vec![stmt::Type::I64, stmt::Type::I64])
            } else {
                action.output.ty.clone()
            },
        };

        let mut res = self
            .connection
            .exec(&self.engine.schema.db, op.into())
            .await?;

        if action.conditional_update_with_no_returning {
            let Rows::Values(rows) = res.rows else {
                return Err(anyhow::anyhow!(
                    "conditional_update_with_no_returning: expected values, got {res:#?}"
                ));
            };

            let rows = rows.collect().await?;
            assert_eq!(rows.len(), 1);

            let stmt::Value::Record(record) = &rows[0] else {
                return Err(anyhow::anyhow!(
                    "conditional_update_with_no_returning: expected record, got {rows:#?}"
                ));
            };

            assert_eq!(record.len(), 2);

            if record[0] != record[1] {
                anyhow::bail!("update condition did not match");
            }

            res.rows = Rows::Count(record[0].to_u64_unwrap());
        }

        self.vars.store(
            action.output.output.var,
            action.output.output.num_uses,
            res.rows,
        );

        Ok(())
    }
}

impl From<ExecStatement> for Action {
    fn from(value: ExecStatement) -> Self {
        Self::ExecStatement(value.into())
    }
}
