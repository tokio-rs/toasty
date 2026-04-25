use super::{
    Connection, ExprAttrs, Result, Schema, ddb_expression, deserialize_ddb_cursor, item_to_record,
    operation, serialize_ddb_cursor, stmt,
};
use std::sync::Arc;
use toasty_core::driver::operation::QueryPkLimit;
use toasty_core::{driver::ExecResponse, stmt::ExprContext};

impl Connection {
    pub(crate) async fn exec_query_pk(
        &mut self,
        schema: &Arc<Schema>,
        op: operation::QueryPk,
    ) -> Result<ExecResponse> {
        let table = schema.db.table(op.table);
        let cx = ExprContext::new_with_target(&schema.db, table);

        let mut expr_attrs = ExprAttrs::default();

        // When querying an index, use index filter logic (not primary key logic)
        let is_primary_key = op.index.is_none();
        let key_expression = ddb_expression(&cx, &mut expr_attrs, is_primary_key, &op.pk_filter);

        let filter_expression = op
            .filter
            .as_ref()
            .map(|expr| ddb_expression(&cx, &mut expr_attrs, false, expr));

        // Build base query
        let mut query = self
            .client
            .query()
            .table_name(&table.name)
            .key_condition_expression(key_expression)
            .set_filter_expression(filter_expression)
            .set_expression_attribute_names(Some(expr_attrs.attr_names))
            .set_expression_attribute_values(Some(expr_attrs.attr_values));

        if let Some(index_id) = op.index {
            let index = schema.db.index(index_id);
            if index.unique {
                return Err(toasty_core::Error::from_args(format_args!(
                    "Unique index {} doesn't have fields.",
                    index.name
                )));
            }
            tracing::trace!(table_name = %table.name, index_name = %index.name, "querying secondary index");
            query = query.index_name(&index.name);
        } else {
            tracing::trace!(table_name = %table.name, "querying primary key");
        }

        if let Some(ref direction) = op.order {
            query = query.scan_index_forward(*direction == stmt::Direction::Asc);
        }

        match op.limit {
            None => {
                // No limit — return all results in a single call.
                let schema = schema.clone();
                let res = query
                    .send()
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;

                let cursor = res.last_evaluated_key.as_ref().map(serialize_ddb_cursor);

                let rows = stmt::ValueStream::from_iter(res.items.into_iter().flatten().map(
                    move |item| {
                        item_to_record(
                            &item,
                            op.select
                                .iter()
                                .map(|column_id| schema.db.column(*column_id)),
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

            Some(QueryPkLimit::Cursor { page_size, after }) => {
                // Cursor-based pagination: single call, return one page.
                query = query.limit(page_size as i32);
                if let Some(cursor_value) = after {
                    query =
                        query.set_exclusive_start_key(Some(deserialize_ddb_cursor(&cursor_value)));
                }

                let schema = schema.clone();
                let res = query
                    .send()
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;

                let cursor = res.last_evaluated_key.as_ref().map(serialize_ddb_cursor);

                let rows = stmt::ValueStream::from_iter(res.items.into_iter().flatten().map(
                    move |item| {
                        item_to_record(
                            &item,
                            op.select
                                .iter()
                                .map(|column_id| schema.db.column(*column_id)),
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

            Some(QueryPkLimit::Offset { limit, offset }) => {
                // Offset-based pagination: stream items, discard the first
                // `offset` in-place, then collect exactly `limit` items.
                let skip = offset.unwrap_or(0) as usize;
                let need = limit as usize + skip;
                // This may process unneeded item - if offset is large relative to limit, or if the
                // filter expression does not filter many items server side. For minimal extra reads, use
                // pagination instead.
                let mut stream = query.into_paginator().page_size(need as i32).items().send();

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
                let mut rows: Vec<toasty_core::stmt::Value> = Vec::with_capacity(limit as usize);
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
                                op.select
                                    .iter()
                                    .map(|column_id| schema.db.column(*column_id)),
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
