use super::{
    ddb_expression, item_to_record, operation, stmt, Connection, ExprAttrs, Result, Schema,
};
use std::sync::Arc;
use toasty_core::{driver::Response, stmt::ExprContext};

impl Connection {
    pub(crate) async fn exec_find_pk_by_index(
        &mut self,
        schema: &Arc<Schema>,
        op: operation::FindPkByIndex,
    ) -> Result<Response> {
        let table = schema.table(op.table);
        let index = schema.index(op.index);
        let cx = ExprContext::new_with_target(&**schema, table);

        let mut expr_attrs = ExprAttrs::default();
        let key_expression = ddb_expression(&cx, &mut expr_attrs, false, &op.filter);

        let res = if index.unique {
            self.client
                .query()
                .table_name(&index.name)
                .key_condition_expression(key_expression)
                .set_expression_attribute_names(Some(expr_attrs.attr_names))
                .set_expression_attribute_values(Some(expr_attrs.attr_values))
                .send()
                .await
                .map_err(toasty_core::Error::driver)?
        } else {
            self.client
                .query()
                .table_name(&table.name)
                .index_name(&index.name)
                .key_condition_expression(key_expression)
                .set_expression_attribute_names(Some(expr_attrs.attr_names))
                .set_expression_attribute_values(Some(expr_attrs.attr_values))
                .send()
                .await
                .map_err(toasty_core::Error::driver)?
        };

        let schema = schema.clone();

        Ok(Response::value_stream(stmt::ValueStream::from_iter(
            res.items.into_iter().flatten().map(move |item| {
                let table = schema.table(op.table);
                item_to_record(&item, table.primary_key_columns())
            }),
        )))
    }
}
