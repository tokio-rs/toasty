use super::{
    ddb_expression, item_to_record, operation, stmt, Connection, ExprAttrs, Result, Schema,
};
use std::sync::Arc;
use toasty_core::{driver::Response, stmt::ExprContext};

impl Connection {
    pub(crate) async fn exec_query_pk(
        &mut self,
        schema: &Arc<Schema>,
        op: operation::QueryPk,
    ) -> Result<Response> {
        let table = schema.table(op.table);
        let cx = ExprContext::new_with_target(&**schema, table);

        let mut expr_attrs = ExprAttrs::default();
        let key_expression = ddb_expression(&cx, &mut expr_attrs, true, &op.pk_filter);

        let filter_expression = op
            .filter
            .as_ref()
            .map(|expr| ddb_expression(&cx, &mut expr_attrs, false, expr));

        let res = self
            .client
            .query()
            .table_name(&table.name)
            .key_condition_expression(key_expression)
            .set_filter_expression(filter_expression)
            .set_expression_attribute_names(Some(expr_attrs.attr_names))
            .set_expression_attribute_values(Some(expr_attrs.attr_values))
            .send()
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        let schema = schema.clone();

        Ok(Response::value_stream(stmt::ValueStream::from_iter(
            res.items.into_iter().flatten().map(move |item| {
                item_to_record(
                    &item,
                    op.select.iter().map(|column_id| schema.column(*column_id)),
                )
            }),
        )))
    }
}
