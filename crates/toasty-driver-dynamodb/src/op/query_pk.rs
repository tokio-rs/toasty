use super::{
    Connection, ExprAttrs, Result, Schema, ddb_expression, ddb_key, deserialize_ddb_cursor,
    item_to_record, operation, serialize_ddb_cursor, stmt,
};
use std::sync::Arc;
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

        // Build the query based on whether we're querying primary key or an index
        let result = if let Some(index_id) = op.index {
            let index = schema.db.index(index_id);

            if index.unique {
                use toasty_core::Error;
                let err = Error::from_args(format_args!(
                    "Unique index {} doesn't have fields.",
                    index.name
                ));
                Err(err)
            } else {
                tracing::trace!(table_name = %table.name, index_name = %index.name, "querying secondary index");
                let mut query = self
                    .client
                    .query()
                    .table_name(&table.name)
                    .index_name(&index.name)
                    .key_condition_expression(key_expression)
                    .set_filter_expression(filter_expression)
                    .set_expression_attribute_names(Some(expr_attrs.attr_names))
                    .set_expression_attribute_values(Some(expr_attrs.attr_values));

                // Apply pagination parameters.
                // If op.pagination is present, use it; otherwise fall back to op.limit/cursor
                if let Some(ref pagination) = op.pagination {
                    query = query.limit(pagination.page_size as i32);
                    if let Some(ref cursor_value) = pagination.cursor {
                        query = query
                            .set_exclusive_start_key(Some(deserialize_ddb_cursor(cursor_value)));
                    }
                } else {
                    if let Some(limit) = op.limit {
                        query = query.limit(limit as i32);
                    }
                    if let Some(ref start_key) = op.cursor {
                        query = query.set_exclusive_start_key(Some(ddb_key(table, start_key)));
                    }
                }
                if let Some(ref direction) = op.order {
                    query = query.scan_index_forward(*direction == stmt::Direction::Asc);
                }

                query
                    .send()
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)
            }
        } else {
            tracing::trace!(table_name = %table.name, "querying primary key");
            let mut query = self
                .client
                .query()
                .table_name(&table.name)
                .key_condition_expression(key_expression)
                .set_filter_expression(filter_expression)
                .set_expression_attribute_names(Some(expr_attrs.attr_names))
                .set_expression_attribute_values(Some(expr_attrs.attr_values));

            // Apply pagination parameters.
            // If op.pagination is present, use it; otherwise fall back to op.limit/cursor
            if let Some(ref pagination) = op.pagination {
                query = query.limit(pagination.page_size as i32);
                if let Some(ref cursor_value) = pagination.cursor {
                    query =
                        query.set_exclusive_start_key(Some(deserialize_ddb_cursor(cursor_value)));
                }
            } else {
                if let Some(limit) = op.limit {
                    query = query.limit(limit as i32);
                }
                if let Some(ref start_key) = op.cursor {
                    query = query.set_exclusive_start_key(Some(ddb_key(table, start_key)));
                }
            }
            if let Some(ref direction) = op.order {
                query = query.scan_index_forward(*direction == stmt::Direction::Asc);
            }

            query
                .send()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)
        };

        let schema = schema.clone();
        let res = result?;

        // Capture LastEvaluatedKey for pagination
        let cursor = res.last_evaluated_key.as_ref().map(serialize_ddb_cursor);

        let rows = stmt::ValueStream::from_iter(res.items.into_iter().flatten().map(move |item| {
            item_to_record(
                &item,
                op.select
                    .iter()
                    .map(|column_id| schema.db.column(*column_id)),
            )
        }));

        Ok(ExecResponse {
            values: toasty_core::driver::Rows::Stream(rows),
            next_cursor: cursor,
            prev_cursor: None,
        })
    }
}
