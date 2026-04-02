use super::{
    Connection, ExprAttrs, Result, Schema, ddb_expression, item_to_record, operation, stmt,
};
use std::sync::Arc;
use toasty_core::{driver::ExecResponse, stmt::ExprContext};

impl Connection {
    pub(crate) async fn exec_find_pk_by_index(
        &mut self,
        schema: &Arc<Schema>,
        op: operation::FindPkByIndex,
    ) -> Result<ExecResponse> {
        let table = schema.db.table(op.table);
        let index = schema.db.index(op.index);
        let cx = ExprContext::new_with_target(&schema.db, table);

        let mut expr_attrs = ExprAttrs::default();
        let key_expression = ddb_expression(&cx, &mut expr_attrs, false, &op.filter);

        let res = if index.unique {
            tracing::trace!(index_name = %index.name, "querying unique index as table");
            self.client
                .query()
                .table_name(&index.name)
                .key_condition_expression(key_expression)
                .set_expression_attribute_names(Some(expr_attrs.attr_names))
                .set_expression_attribute_values(Some(expr_attrs.attr_values))
                .send()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?
        } else {
            tracing::trace!(table_name = %table.name, index_name = %index.name, "querying secondary index");
            self.client
                .query()
                .table_name(&table.name)
                .index_name(&index.name)
                .key_condition_expression(key_expression)
                .set_expression_attribute_names(Some(expr_attrs.attr_names))
                .set_expression_attribute_values(Some(expr_attrs.attr_values))
                .send()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?
        };

        let schema = schema.clone();

        Ok(ExecResponse::value_stream(stmt::ValueStream::from_iter(
            res.items.into_iter().flatten().map(move |item| {
                let table = schema.db.table(op.table);
                item_to_record(&item, table.primary_key_columns())
            }),
        )))
    }
}
