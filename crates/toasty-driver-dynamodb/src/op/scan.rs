use super::{
    Connection, ExprAttrs, Result, Schema, ddb_expression, deserialize_ddb_cursor, item_to_record,
    operation, serialize_ddb_cursor, stmt,
};
use std::sync::Arc;
use toasty_core::{
    driver::{ExecResponse, operation::Pagination},
    schema::db::ColumnId,
    stmt::ExprContext,
};

impl Connection {
    pub(crate) async fn exec_scan(
        &mut self,
        schema: &Arc<Schema>,
        op: operation::Scan,
    ) -> Result<ExecResponse> {
        let table = schema.db.table(op.table);
        let cx = ExprContext::new_with_target(&schema.db, table);

        let mut expr_attrs = ExprAttrs::default();

        let filter_expression = op
            .filter
            .as_ref()
            .map(|expr| ddb_expression(&cx, &mut expr_attrs, false, expr));

        tracing::trace!(table_name = %table.name, "scanning table");

        // Build the base scan with filter and expression attributes.
        let scan = self
            .client
            .scan()
            .table_name(&table.name)
            .set_filter_expression(filter_expression)
            .set_expression_attribute_names(Some(expr_attrs.attr_names))
            .set_expression_attribute_values(Some(expr_attrs.attr_values));

        match op.limit {
            None => {
                // No limit — stream all items across 1 MB DynamoDB pages.
                let mut stream = scan.into_paginator().items().send();

                let mut rows: Vec<stmt::Value> = Vec::new();
                while let Some(item) = stream
                    .next()
                    .await
                    .transpose()
                    .map_err(toasty_core::Error::driver_operation_failed)?
                {
                    let value = item_to_record(
                        &item,
                        op.columns.iter().map(|&col_idx| {
                            schema.db.column(ColumnId {
                                table: op.table,
                                index: col_idx,
                            })
                        }),
                    )
                    .map(stmt::Value::from)?;
                    rows.push(value);
                }

                Ok(ExecResponse {
                    values: toasty_core::driver::Rows::Stream(stmt::ValueStream::from_vec(rows)),
                    next_cursor: None,
                    prev_cursor: None,
                })
            }

            Some(Pagination::Cursor { page_size, after }) => {
                // Cursor-based pagination: single call returning one page.
                let scan = scan.limit(page_size as i32);
                let scan = if let Some(cursor_value) = after {
                    scan.set_exclusive_start_key(Some(deserialize_ddb_cursor(&cursor_value)))
                } else {
                    scan
                };

                let schema = schema.clone();
                let res = scan
                    .send()
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;

                let cursor = res.last_evaluated_key.as_ref().map(serialize_ddb_cursor);

                let rows = stmt::ValueStream::from_iter(res.items.into_iter().flatten().map(
                    move |item| {
                        item_to_record(
                            &item,
                            op.columns.iter().map(|&col_idx| {
                                schema.db.column(ColumnId {
                                    table: op.table,
                                    index: col_idx,
                                })
                            }),
                        )
                        .map(stmt::Value::from)
                    },
                ));

                Ok(ExecResponse {
                    values: toasty_core::driver::Rows::Stream(rows),
                    next_cursor: cursor,
                    prev_cursor: None,
                })
            }

            Some(Pagination::Offset { limit, offset }) => {
                // Offset-based: stream items, discard the first `offset`, then
                // collect exactly `limit`.
                let skip = offset.unwrap_or(0) as usize;
                let need = limit as usize + skip;
                let mut stream = scan.into_paginator().page_size(need as i32).items().send();

                // Discard offset items without storing them.
                let mut skipped = 0;
                while skipped < skip {
                    match stream
                        .next()
                        .await
                        .transpose()
                        .map_err(toasty_core::Error::driver_operation_failed)?
                    {
                        Some(_) => skipped += 1,
                        None => break,
                    }
                }

                // Collect up to `limit` items.
                let mut rows: Vec<stmt::Value> = Vec::with_capacity(limit as usize);
                while rows.len() < limit as usize {
                    match stream
                        .next()
                        .await
                        .transpose()
                        .map_err(toasty_core::Error::driver_operation_failed)?
                    {
                        Some(item) => {
                            let value = item_to_record(
                                &item,
                                op.columns.iter().map(|&col_idx| {
                                    schema.db.column(ColumnId {
                                        table: op.table,
                                        index: col_idx,
                                    })
                                }),
                            )
                            .map(stmt::Value::from)?;
                            rows.push(value);
                        }
                        None => break,
                    }
                }

                Ok(ExecResponse {
                    values: toasty_core::driver::Rows::Stream(stmt::ValueStream::from_vec(rows)),
                    next_cursor: None,
                    prev_cursor: None,
                })
            }
        }
    }
}
