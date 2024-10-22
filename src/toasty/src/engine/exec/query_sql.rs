use super::*;

impl<'a> Exec<'a> {
    pub(super) async fn exec_query_sql(&mut self, action: &plan::QuerySql<'a>) -> Result<()> {
        let mut sql = action.sql.clone();

        if !action.input.is_empty() {
            assert_eq!(action.input.len(), 1);

            let input = self.collect_input(&action.input[0]).await?;

            swap_args(&mut sql, &input);
        }

        let res = self
            .db
            .driver
            .exec(
                &self.db.schema,
                operation::QuerySql {
                    stmt: sql,
                    ty: action.output.as_ref().map(|o| o.ty.clone()),
                }
                .into(),
            )
            .await?;

        let Some(out) = &action.output else {
            // Should be no output
            let _ = res.collect().await;
            return Ok(());
        };

        // TODO: don't clone
        let project = out.project.clone();

        let res = ValueStream::from_stream(async_stream::try_stream! {
            for await value in res {
                let value = value?;
                yield project.eval(&value)?;
            }
        });

        self.vars.store(out.var, res);

        Ok(())
    }
}

// TODO: so much code duplication
fn swap_args<'stmt>(sql: &mut sql::Statement<'stmt>, values: &Vec<stmt::Value<'stmt>>) {
    match sql {
        sql::Statement::Query(query) => match &mut *query.body {
            sql::ExprSet::Select(select) => {
                if let Some(expr) = &mut select.selection {
                    swap_args_expr(expr, values);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn swap_args_expr<'stmt>(sql: &mut sql::Expr<'stmt>, values: &Vec<stmt::Value<'stmt>>) {
    use sql::Expr::*;

    match sql {
        And(sql::ExprAnd { operands }) => {
            for operand in operands {
                swap_args_expr(operand, values);
            }
        }
        Or(sql::ExprOr { operands }) => {
            for operand in operands {
                swap_args_expr(operand, values);
            }
        }
        BinaryOp(sql::ExprBinaryOp { lhs, rhs, .. }) => {
            swap_args_expr(&mut *lhs, values);
            swap_args_expr(&mut *rhs, values);
        }
        Like { .. } => {}
        InList(sql::ExprInList { list, .. }) => match list {
            sql::ExprList::Placeholder(expr_placeholder) => {
                assert_eq!(expr_placeholder.position, 0);
                *list = sql::ExprList::Expr(
                    values
                        .iter()
                        .map(|value| match value {
                            stmt::Value::Record(values) => sql::Expr::Value(values[0].clone()),
                            _ => sql::Expr::Value(value.clone()),
                        })
                        .collect(),
                );
            }
            _ => {}
        },
        _ => {}
    }
}
