use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    schema::db,
    stmt,
};

use crate::{
    Result,
    engine::{eval, exec::Exec, mir},
};

/// How to interpret a statement's output rows.
///
/// A conditional write (the SQL `#[version]` / OCC path compiled as a single
/// CTE statement) prefixes its result with two probe columns: the number of
/// rows matching the filter and, of those, the number satisfying the condition.
/// The write applied only when the two agree; a mismatch is a condition
/// failure, and zero matched rows means the record no longer exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConditionalOutput {
    /// Not a conditional write. Output is passed through unchanged.
    None,

    /// Conditional write with no `RETURNING`. The two probe columns are the
    /// only output; the action reports the matched-row count.
    Count,

    /// Conditional write with a `RETURNING`. The two probe columns are followed
    /// by the changed rows' columns, which become the action's output.
    Returning,
}

/// Configuration for pagination at the execution level.
#[derive(Debug, Clone)]
pub(crate) struct PaginationConfig {
    /// Number of items per page
    pub page_size: i64,
    /// Whether the query starts after a cursor and can have a previous page.
    pub has_previous_page: bool,
    /// Function to extract cursor from a row (SQL only).
    /// For NoSQL drivers, this is None (driver provides cursor).
    pub extract_cursor: Option<eval::Func>,
}

/// An `INSERT` whose `RETURNING` the backend cannot serve, but whose one
/// generated value the backend still names exactly.
///
/// MySQL has no `RETURNING` on `INSERT`. It does report the auto-increment
/// value a statement generated, through `LAST_INSERT_ID()` on the same
/// connection, and for a one-row insert that value identifies the row. The
/// returning clause is stripped from the statement and evaluated here against
/// the reported value.
///
/// Every other shape is rejected by
/// [`Exec::strip_insert_returning_without_capability`] rather than
/// reconstructed.
#[derive(Debug)]
struct LastInsertId {
    /// The stripped returning expression, with the auto-increment column
    /// reference replaced by `Expr::Arg(0)`.
    returning_expr: stmt::Expr,

    /// The type of the auto-increment column.
    auto_column_type: stmt::Type,
}

/// Information about a MySQL UPDATE with RETURNING that needs special handling.
///
/// MySQL doesn't support `RETURNING` on `UPDATE`. The workaround is to strip
/// the returning, run the UPDATE, then run a follow-up `SELECT` over the same
/// table and filter to fetch the post-update column values. The two
/// statements are not atomic relative to concurrent writers — see #881 for
/// the broader design discussion.
#[derive(Debug)]
pub(super) struct MySQLUpdateReturning {
    /// The `SELECT` statement that returns the post-update values. Carries
    /// the same filter as the original `UPDATE` plus the projected
    /// returning expression.
    select_stmt: stmt::Statement,
}

impl Exec<'_> {
    pub(super) async fn exec_statement(
        &mut self,
        action: &mir::ExecStatement,
    ) -> Result<ExecResponse> {
        // Databases always return rows as a vec of values; this specifies the
        // type of each value. `None` means the statement returns only a count.
        let output_ty = mir::row_field_types(&action.ty);

        let mut stmt = action.stmt.clone();

        // Collect input values and substitute into the statement
        if !action.inputs.is_empty() {
            let input_values = self.collect_input(action.inputs.iter().copied()).await?;
            stmt.substitute(&input_values);

            self.engine.simplify_stmt(&mut stmt);
        }

        debug_assert!(
            stmt.returning()
                .and_then(|returning| returning.as_project())
                .map(|expr| expr.is_record())
                .unwrap_or(true),
            "stmt={stmt:#?}"
        );

        // A backend without `RETURNING` on mutations cannot report a value the
        // database computed. Strip the returning when the backend names that
        // value exactly; reject the statement when it does not.
        let last_insert_id = self.strip_insert_returning_without_capability(&mut stmt)?;

        // MySQL does not support `RETURNING` on `UPDATE`. Strip the returning
        // and capture an equivalent `SELECT` to run after the UPDATE.
        let mysql_update_returning = self.process_stmt_update_with_returning_on_mysql(&mut stmt);

        // Short circuit if we can statically determine there are no results
        if let stmt::Statement::Query(query) = &stmt
            && let stmt::ExprSet::Values(values) = &query.body
            && values.is_empty()
        {
            assert_eq!(action.conditional, ConditionalOutput::None);

            let rows = if output_ty.is_some() {
                Rows::Stream(stmt::ValueStream::default())
            } else {
                Rows::Count(0)
            };

            return Ok(ExecResponse::from_rows(rows));
        }

        // Legalize the statement for the target backend and extract bind
        // parameters (SQL drivers only; key-value drivers read values
        // directly from the statement).
        let params = self.engine.prepare_for_driver(&mut stmt);

        let ret = match action.conditional {
            // A conditional write prefixes its result with two `I64` probe
            // counts; the `Returning` variant follows them with the changed
            // rows' columns.
            ConditionalOutput::Count => Some(vec![stmt::Type::I64, stmt::Type::I64]),
            ConditionalOutput::Returning => {
                let mut tys = vec![stmt::Type::I64, stmt::Type::I64];
                tys.extend(
                    output_ty
                        .clone()
                        .expect("conditional write with RETURNING has output columns"),
                );
                Some(tys)
            }
            ConditionalOutput::None if last_insert_id.is_some() => {
                // The RETURNING was stripped; the driver reports the value the
                // insert generated instead of a row count.
                None
            }
            ConditionalOutput::None if mysql_update_returning.is_some() => {
                // The UPDATE has had its RETURNING stripped; the driver runs
                // a plain UPDATE that returns no rows. The follow-up SELECT
                // below produces the returning values.
                None
            }
            ConditionalOutput::None => output_ty.clone(),
        };

        let op = operation::QuerySql {
            stmt,
            params,
            ret,
            last_insert_id: last_insert_id.is_some(),
        };

        let mut res = self.connection.exec(&self.engine.schema, op.into()).await?;

        match action.conditional {
            ConditionalOutput::None => {
                if let Some(last_insert_id) = last_insert_id {
                    res.values = last_insert_id.eval_returning(res.values).await?;
                } else if let Some(mysql_update) = mysql_update_returning {
                    res = self
                        .run_mysql_update_returning_select(mysql_update, output_ty.clone())
                        .await?;
                }
            }
            ConditionalOutput::Count | ConditionalOutput::Returning => {
                let rows = collect_conditional_probe(res.values).await?;
                let (matched, conditioned) = conditional_probe_counts(&rows[0])?;

                // A conditional write targets a row the caller holds an
                // instance of: zero matched rows means it has since been
                // deleted.
                if matched == 0 {
                    return Err(toasty_core::Error::record_not_found(
                        "conditional write matched no rows",
                    ));
                }
                if matched != conditioned {
                    return Err(toasty_core::Error::condition_failed(
                        "write condition did not match",
                    ));
                }

                res.values = match action.conditional {
                    ConditionalOutput::Count => Rows::Count(matched as u64),
                    _ => {
                        // The probe locked the matched rows, so the write
                        // applied to exactly those rows and every result row is
                        // a real changed row — strip the two leading probe
                        // columns.
                        let changed = rows
                            .into_iter()
                            .map(|row| {
                                let stmt::Value::Record(record) = row else {
                                    return Err(toasty_core::Error::invalid_result(
                                        "conditional write expected Record",
                                    ));
                                };
                                Ok(stmt::Value::record_from_vec(
                                    record.fields.into_iter().skip(2).collect(),
                                ))
                            })
                            .collect::<Result<Vec<_>>>()?;

                        Rows::value_stream(changed)
                    }
                };
            }
        }

        // Apply pagination if configured
        if let Some(pagination) = &action.pagination {
            assert!(res.is_unpaginated());
            res.values.buffer().await?;
            self.apply_sql_pagination(&mut res, pagination)?;
        }

        Ok(res)
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
            Some(Box::new(extract_cursor.eval(
                &self.engine.schema,
                std::slice::from_ref(cursor_row),
            )?))
        } else {
            // Got fewer than page_size rows, no more data
            None
        };

        // Extract a previous cursor only when this query started after another page.
        res.prev_cursor = if pagination.has_previous_page
            && !row_vec.is_empty()
            && self.engine.capability().backward_pagination
        {
            let cursor_row = &row_vec[0];
            Some(Box::new(extract_cursor.eval(
                &self.engine.schema,
                std::slice::from_ref(cursor_row),
            )?))
        } else {
            None
        };

        Ok(())
    }
}

impl Exec<'_> {
    /// Detects an UPDATE with a non-empty `RETURNING` on a MySQL backend
    /// and rewrites the statement for the workaround path:
    ///
    /// - The returning clause is stripped from the UPDATE so the SQL
    ///   serializer doesn't reject it.
    /// - An equivalent `SELECT` over the same table + filter is captured,
    ///   carrying the original returning expression as its projection.
    ///
    /// Returns `None` when the backend supports `RETURNING` natively (PG,
    /// SQLite) or when the statement is not an UPDATE with a returning
    /// project. The two-statement path is not atomic relative to concurrent
    /// writers — see #881.
    pub(super) fn process_stmt_update_with_returning_on_mysql(
        &self,
        stmt: &mut stmt::Statement,
    ) -> Option<MySQLUpdateReturning> {
        if self.engine.capability().returning_from_mutation || !self.engine.capability().sql() {
            return None;
        }

        let stmt::Statement::Update(update) = stmt else {
            return None;
        };

        let table_id = match &update.target {
            stmt::UpdateTarget::Table(table_id) => *table_id,
            _ => return None,
        };

        let returning = update.returning.take()?;

        let select = stmt::Select {
            returning,
            source: stmt::Source::table(table_id),
            filter: update.filter.clone(),
            distinct: false,
        };
        let select_stmt =
            stmt::Statement::Query(stmt::Query::new(stmt::ExprSet::Select(Box::new(select))));

        Some(MySQLUpdateReturning { select_stmt })
    }

    /// Runs the follow-up `SELECT` for a MySQL UPDATE with stripped
    /// `RETURNING`. The driver receives a plain query whose result rows
    /// take the place of the original RETURNING output.
    pub(super) async fn run_mysql_update_returning_select(
        &mut self,
        mysql_update: MySQLUpdateReturning,
        ret_ty: Option<Vec<stmt::Type>>,
    ) -> Result<toasty_core::driver::ExecResponse> {
        let mut select_stmt = mysql_update.select_stmt;
        let select_params = self.engine.prepare_for_driver(&mut select_stmt);

        let op = operation::QuerySql {
            stmt: select_stmt,
            params: select_params,
            ret: ret_ty,
            last_insert_id: false,
        };

        self.connection.exec(&self.engine.schema, op.into()).await
    }

    /// Strips the `RETURNING` from an `INSERT` on a backend that cannot serve
    /// one, or rejects the statement.
    ///
    /// [`Capability::returning_from_mutation`] records whether a backend can
    /// return values from a mutation. Without it, Toasty can produce a result
    /// only when it derives every value from the statement's own input —
    /// lowering folds those into a constant returning clause before the
    /// statement reaches here — or when the backend names the value exactly.
    /// MySQL's `LAST_INSERT_ID()` is such a value: on the same connection it
    /// reports the auto-increment value a one-row `INSERT` generated.
    ///
    /// Everything else is an error rather than a reconstruction. Deriving a
    /// multi-row insert's ids as `first_id + offset` is wrong under a session
    /// `auto_increment_increment` above one, under `innodb_autoinc_lock_mode=2`
    /// with concurrent inserts, and for any insert whose affected-row count
    /// differs from its input-row count.
    ///
    /// [`Capability::returning_from_mutation`]: toasty_core::driver::Capability::returning_from_mutation
    fn strip_insert_returning_without_capability(
        &self,
        stmt: &mut stmt::Statement,
    ) -> Result<Option<LastInsertId>> {
        let capability = self.engine.capability();

        if !capability.sql() {
            return Ok(None);
        }

        if capability.returning_from_mutation {
            return Ok(None);
        }

        let stmt::Statement::Insert(insert) = stmt else {
            return Ok(None);
        };

        if insert.returning.is_none() {
            return Ok(None);
        }

        let driver = capability.driver_name;

        // Every column the returning clause reads has to come back from the
        // database, so each one is a candidate for rejection. Collect them
        // before the statement is mutated; duplicates name the same value.
        let mut columns: Vec<&db::Column> = vec![];
        {
            let cx = self.engine.expr_cx_for(&*insert);
            stmt::visit::for_each_expr(insert.returning.as_ref().unwrap(), |expr| {
                if let stmt::Expr::Reference(expr_reference) = expr {
                    let column = cx.resolve_expr_reference(expr_reference).as_column_unwrap();
                    if !columns.iter().any(|c| c.id == column.id) {
                        columns.push(column);
                    }
                }
            });
        }

        let describe = |column: &db::Column| {
            format!(
                "{}.{}",
                self.engine.schema.db.table(column.id.table).name,
                column.name
            )
        };

        if let Some(column) = columns.iter().find(|column| !column.auto_increment) {
            return Err(toasty_core::Error::unsupported_feature(format!(
                "{driver} cannot return `{}` from an INSERT: the backend has no \
                 RETURNING on mutations and Toasty cannot derive the value from \
                 the statement's input. Set the field from the application, or \
                 use a backend that supports RETURNING.",
                describe(column)
            )));
        }

        let [column] = columns[..] else {
            return Err(toasty_core::Error::unsupported_feature(format!(
                "{driver} has no RETURNING on mutations: this INSERT needs {} \
                 values back from the database, and the backend reports only the \
                 value an auto-increment column generated.",
                columns.len()
            )));
        };

        // `LAST_INSERT_ID()` names one row, so the insert has to be one row of
        // literal values. An `INSERT ... SELECT` has no row count to check.
        let stmt::ExprSet::Values(values) = &insert.source.body else {
            return Err(toasty_core::Error::unsupported_feature(format!(
                "{driver} cannot return `{}` from an INSERT reading its rows from \
                 a query: the backend has no RETURNING on mutations.",
                describe(column)
            )));
        };

        if values.rows.len() != 1 {
            return Err(toasty_core::Error::unsupported_feature(format!(
                "{driver} cannot return `{}` from an INSERT of {} rows: the \
                 backend has no RETURNING on mutations, and the generated value \
                 it does report names one row only. Insert the rows one at a \
                 time, set the key from the application, or use a backend that \
                 supports RETURNING.",
                describe(column),
                values.rows.len()
            )));
        }

        // The one row still has to be a row this statement wrote. An upsert
        // that takes its conflict branch generates nothing, and
        // `LAST_INSERT_ID()` then reports a value from an earlier statement.
        // The verifier rejects every upsert on a backend without the
        // capability, so this does not fire today; it states the invariant
        // where the value is read.
        if insert.upsert.is_some() {
            return Err(toasty_core::Error::unsupported_feature(format!(
                "{driver} cannot return `{}` from an upsert: the backend has no \
                 RETURNING on mutations, and the value it does report names a \
                 row only when the statement inserted one.",
                describe(column)
            )));
        }

        let auto_column_type = column.ty.clone();

        let stmt::Returning::Project(mut returning_expr) = insert.returning.take().unwrap() else {
            return Err(toasty_core::Error::invalid_statement(
                "an INSERT reaching the driver returns a projection or nothing",
            ));
        };

        // The driver reports the generated value as the sole argument.
        stmt::visit_mut::for_each_expr_mut(&mut returning_expr, |expr| {
            if matches!(expr, stmt::Expr::Reference(_)) {
                *expr = stmt::Expr::Arg(stmt::ExprArg {
                    position: 0,
                    nesting: 0,
                });
            }
        });

        Ok(Some(LastInsertId {
            returning_expr,
            auto_column_type,
        }))
    }
}

/// Collects a conditional write's result rows. The probe (a `COUNT` aggregate)
/// always yields at least one row, so an empty result is a driver bug.
async fn collect_conditional_probe(rows: Rows) -> Result<Vec<stmt::Value>> {
    let Rows::Stream(rows) = rows else {
        return Err(toasty_core::Error::invalid_result(format!(
            "conditional write expected Stream, got {rows:?}"
        )));
    };

    let rows = rows.collect().await?;
    if rows.is_empty() {
        return Err(toasty_core::Error::invalid_result(
            "conditional write probe returned no rows",
        ));
    }

    Ok(rows)
}

/// Reads the two leading probe counts (`matched`, `conditioned`) from a
/// conditional write's result row.
fn conditional_probe_counts(row: &stmt::Value) -> Result<(i64, i64)> {
    let stmt::Value::Record(record) = row else {
        return Err(toasty_core::Error::invalid_result(format!(
            "conditional write expected Record, got {row:?}"
        )));
    };

    match (record.fields.first(), record.fields.get(1)) {
        (Some(stmt::Value::I64(matched)), Some(stmt::Value::I64(conditioned))) => {
            Ok((*matched, *conditioned))
        }
        _ => Err(toasty_core::Error::invalid_result(format!(
            "conditional write probe columns are not I64; row={row:?}"
        ))),
    }
}

impl LastInsertId {
    /// Rebuilds the stripped `RETURNING` from the value the driver reported.
    ///
    /// The insert was checked to write exactly one row, so the driver returns
    /// exactly one row holding exactly one value.
    async fn eval_returning(self, rows: Rows) -> Result<Rows> {
        let Rows::Stream(rows) = rows else {
            return Err(toasty_core::Error::invalid_result(format!(
                "INSERT reporting a generated value expected Stream, got {rows:?}"
            )));
        };

        let rows = rows.collect().await?;

        let [stmt::Value::Record(record)] = &rows[..] else {
            return Err(toasty_core::Error::invalid_result(format!(
                "INSERT reporting a generated value expected one Record, got {rows:?}"
            )));
        };

        let [id] = &record.fields[..] else {
            return Err(toasty_core::Error::invalid_result(format!(
                "INSERT reporting a generated value expected one field, got {record:?}"
            )));
        };

        // An auto-increment value is a scalar, so the cast is schema-free.
        let id = self.auto_column_type.cast(&(), id.clone())?;
        let row = self.returning_expr.eval(&[id])?;

        Ok(Rows::value_stream(vec![row]))
    }
}
