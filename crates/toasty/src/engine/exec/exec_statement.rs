use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    stmt,
};

use crate::{
    Result,
    engine::{
        eval,
        exec::{Action, Exec, Output, VarId},
    },
};

/// Configuration for pagination at the execution level.
#[derive(Debug, Clone)]
pub(crate) struct PaginationConfig {
    /// Number of items per page
    pub page_size: i64,
    /// Function to extract cursor from a row (SQL only).
    /// For NoSQL drivers, this is None (driver provides cursor).
    pub extract_cursor: Option<eval::Func>,
}

/// Information about a MySQL INSERT with RETURNING that needs special handling.
///
/// MySQL doesn't support RETURNING clauses, but we can work around this for
/// auto-increment columns by using LAST_INSERT_ID().
#[derive(Debug)]
struct MySQLInsertReturning {
    /// Number of rows being inserted
    num_rows: u64,

    /// The original returning expression that was removed from the statement
    returning_expr: stmt::Expr,

    /// The type of the auto-increment column
    auto_column_type: stmt::Type,
}

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

    /// Pagination configuration (None if not paginated)
    pub pagination: Option<PaginationConfig>,
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
                let response = self.vars.load(*var_id).await?;
                let values = response.values.collect_as_value().await?;
                input_values.push(values);
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

        // MySQL does not support returning clauses with insert statements,
        // which adds a wrinkle when we want to get the IDs for autoincrement
        // IDs.
        let mysql_insert_returning = self.process_stmt_insert_with_returning_on_mysql(&mut stmt);

        // Short circuit if we can statically determine there are no results
        if let stmt::Statement::Query(query) = &stmt
            && let stmt::ExprSet::Values(values) = &query.body
            && values.is_empty()
        {
            assert!(!action.conditional_update_with_no_returning);

            let rows = if action.output.ty.is_some() {
                Rows::Stream(stmt::ValueStream::default())
            } else {
                Rows::Count(0)
            };

            self.vars.store(
                action.output.output.var,
                action.output.output.num_uses,
                ExecResponse::from_rows(rows),
            );

            return Ok(());
        }

        let params = self.engine.extract_params(&mut stmt);

        let op = operation::QuerySql {
            stmt,
            params,
            ret: if action.conditional_update_with_no_returning {
                Some(vec![stmt::Type::I64, stmt::Type::I64])
            } else if mysql_insert_returning.is_some() {
                // For MySQL INSERT with RETURNING, we don't send RETURNING to the database
                // (it doesn't support it). The driver will fetch auto-increment IDs using LAST_INSERT_ID().
                None
            } else {
                action.output.ty.clone()
            },
            last_insert_id_hack: mysql_insert_returning.as_ref().map(|info| info.num_rows),
        };

        let mut res = self.connection.exec(&self.engine.schema, op.into()).await?;

        if action.conditional_update_with_no_returning {
            let Rows::Stream(rows) = res.values else {
                return Err(toasty_core::Error::invalid_result(format!(
                    "conditional update expected Stream, got {:?}",
                    res.values
                )));
            };

            let rows = rows.collect().await?;
            assert_eq!(rows.len(), 1);

            let stmt::Value::Record(record) = &rows[0] else {
                return Err(toasty_core::Error::invalid_result(format!(
                    "conditional update expected Record, got {:?}",
                    rows[0]
                )));
            };

            assert_eq!(record.len(), 2);

            if record[0] != record[1] {
                return Err(toasty_core::Error::condition_failed(
                    "update condition did not match",
                ));
            }

            res.values = Rows::Count(record[0].to_u64_unwrap());
        } else if let Some(mysql_info) = mysql_insert_returning {
            res.values = mysql_info.reconstruct_returning(res.values).await?;
        }

        // Apply pagination if configured
        if let Some(pagination) = &action.pagination {
            assert!(res.next_cursor.is_none() && res.prev_cursor.is_none());
            res.values.buffer().await?;
            self.apply_sql_pagination(&mut res, pagination)?;
        }

        self.vars
            .store(action.output.output.var, action.output.output.num_uses, res);

        Ok(())
    }

    /// Apply SQL pagination by extracting cursor from last row.
    /// If we got a full page (page_size rows), extract cursor for potential next page.
    /// The client will naturally discover there's no more data when the next request returns empty.
    ///
    /// The response values must already be buffered (via `Rows::buffer()`).
    fn apply_sql_pagination(
        &mut self,
        res: &mut ExecResponse,
        pagination: &PaginationConfig,
    ) -> Result<()> {
        let Some(extract_cursor) = &pagination.extract_cursor else {
            return Ok(());
        };

        let Rows::Value(stmt::Value::List(ref row_vec)) = res.values else {
            return Ok(());
        };

        let page_size = pagination.page_size as usize;

        // Extract cursors for potential next/prev pages
        res.next_cursor = if row_vec.len() == page_size {
            let cursor_row = &row_vec[page_size - 1];
            Some(extract_cursor.eval(std::slice::from_ref(cursor_row))?)
        } else {
            // Got fewer than page_size rows, no more data
            None
        };

        // Extract prev cursor from first row (for backward pagination)
        res.prev_cursor = if !row_vec.is_empty() {
            let cursor_row = &row_vec[0];
            Some(extract_cursor.eval(std::slice::from_ref(cursor_row))?)
        } else {
            None
        };

        Ok(())
    }
}

impl Exec<'_> {
    /// Processes INSERT statements with RETURNING on MySQL, which doesn't support RETURNING.
    ///
    /// Returns information needed to reconstruct the RETURNING results using LAST_INSERT_ID()
    /// if this is a MySQL INSERT with RETURNING. Returns None otherwise.
    ///
    /// # Panics
    ///
    /// Panics if the RETURNING clause includes non-auto-increment columns, as MySQL doesn't
    /// support RETURNING and we can only work around it for auto-increment columns.
    fn process_stmt_insert_with_returning_on_mysql(
        &self,
        stmt: &mut stmt::Statement,
    ) -> Option<MySQLInsertReturning> {
        if self.engine.capability().returning_from_mutation {
            return None;
        }

        let stmt::Statement::Insert(insert) = stmt else {
            return None;
        };

        let returning = insert.returning.take()?;

        // Verify that all columns in the RETURNING clause are auto-increment columns.
        // This is required because MySQL doesn't support RETURNING, but we can work around
        // this limitation for auto-increment columns by using LAST_INSERT_ID().
        let cx = self.engine.expr_cx_for(&*insert);

        let mut ref_count = 0;
        let mut auto_column_type = None;
        stmt::visit::for_each_expr(&returning, |expr| {
            if let stmt::Expr::Reference(expr_ref) = expr {
                let column = cx.resolve_expr_reference(expr_ref).as_column_unwrap();

                assert!(
                    column.auto_increment,
                    "MySQL does not support RETURNING clause for non-auto-increment columns. \
                     Column '{}' in table '{}' is not auto-increment. \
                     Only auto-increment columns can be returned from INSERT statements on MySQL.",
                    column.name, self.engine.schema.db.tables[column.id.table.0].name
                );

                auto_column_type = Some(column.ty.clone());
                ref_count += 1;
            }
        });

        assert_eq!(
            ref_count, 1,
            "MySQL INSERT with RETURNING must have exactly one auto-increment column reference, found {ref_count}"
        );

        let auto_column_type = auto_column_type.expect("auto_column_type should be set");

        // Extract the expression from the RETURNING clause and replace ExprReference with ExprArg
        let mut returning_expr = match returning {
            stmt::Returning::Expr(expr) => expr,
            _ => panic!(
                "MySQL INSERT with RETURNING must have an Expr, got: {:#?}",
                returning
            ),
        };

        // Replace the ExprReference with ExprArg(position: 0) so we can pass the ID as a positional argument
        stmt::visit_mut::for_each_expr_mut(&mut returning_expr, |expr| {
            if matches!(expr, stmt::Expr::Reference(_)) {
                *expr = stmt::Expr::Arg(stmt::ExprArg {
                    position: 0,
                    nesting: 0,
                });
            }
        });

        // Count the number of rows being inserted
        let num_rows = match &insert.source.body {
            stmt::ExprSet::Values(values) => values.rows.len() as u64,
            _ => {
                panic!(
                    "MySQL INSERT with RETURNING only supports VALUES, got: {:#?}",
                    insert.source.body
                );
            }
        };

        Some(MySQLInsertReturning {
            num_rows,
            returning_expr,
            auto_column_type,
        })
    }
}

impl From<ExecStatement> for Action {
    fn from(value: ExecStatement) -> Self {
        Self::ExecStatement(value.into())
    }
}

impl MySQLInsertReturning {
    /// Reconstructs RETURNING results from the ID rows returned by the driver.
    ///
    /// MySQL doesn't support RETURNING, but we fetch auto-increment IDs using LAST_INSERT_ID().
    /// This method takes the ID rows returned by the driver and evaluates the original RETURNING
    /// expression for each ID to produce the expected results.
    async fn reconstruct_returning(self, rows: Rows) -> Result<Rows> {
        // The driver executed SELECT LAST_INSERT_ID() and returned rows with IDs.
        let Rows::Stream(id_rows) = rows else {
            return Err(toasty_core::Error::invalid_result(format!(
                "MySQL INSERT RETURNING expected Stream, got {:?}",
                rows
            )));
        };

        let id_values = id_rows.collect().await?;
        assert_eq!(
            id_values.len(),
            self.num_rows as usize,
            "Expected {} ID rows from driver, got {}",
            self.num_rows,
            id_values.len()
        );

        // Reconstruct the RETURNING results by evaluating the original returning expression
        // for each ID row returned by the driver
        let mut returning_rows = Vec::with_capacity(self.num_rows as usize);

        for id_value_raw in id_values {
            // The driver returns a record with one field containing the ID.
            // Extract the ID value from the record wrapper.
            let stmt::Value::Record(record) = id_value_raw else {
                return Err(toasty_core::Error::invalid_result(format!(
                    "MySQL INSERT RETURNING expected Record from driver, got {:?}",
                    id_value_raw
                )));
            };

            assert_eq!(
                record.fields.len(),
                1,
                "Expected record with one field from driver"
            );

            // Cast the ID to the correct type for the auto-increment column
            let id_value = self.auto_column_type.cast(record.fields[0].clone())?;
            let input = vec![id_value];

            // Evaluate the returning expression with the auto-increment ID
            let row_value = self.returning_expr.eval(&input)?;

            returning_rows.push(row_value);
        }

        Ok(Rows::Stream(stmt::ValueStream::from_iter(
            returning_rows.into_iter().map(Ok),
        )))
    }
}
