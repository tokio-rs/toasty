use super::*;

impl DynamoDB {
    pub(crate) async fn exec_query_pk<'a>(
        &self,
        schema: &'a schema::Schema,
        op: operation::QueryPk<'_>,
    ) -> Result<stmt::ValueStream<'a>> {
        let table = schema.table(op.table);

        let mut expr_attrs = ExprAttrs::default();
        let key_expression = ddb_expression(schema, &mut expr_attrs, true, &op.pk_filter);

        let filter_expression = if let Some(expr) = &op.filter {
            Some(ddb_expression(schema, &mut expr_attrs, false, expr))
        } else {
            None
        };

        println!("client.query()");
        println!("  + op = {:#?}", op);
        println!("  + table = {:#?}", table);
        println!("  + key_condition_expr = {:#?}", key_expression);
        println!("  + filter_expression = {:#?}", filter_expression);
        println!("  + expr_attr_names = {:#?}", expr_attrs.attr_names);
        println!("  + expr_attr_values = {:#?}", expr_attrs.attr_values);

        let res = self
            .client
            .query()
            .table_name(&self.table_name(&table))
            .key_condition_expression(key_expression)
            .set_filter_expression(filter_expression)
            .set_expression_attribute_names(Some(expr_attrs.attr_names))
            .set_expression_attribute_values(Some(expr_attrs.attr_values))
            .send()
            .await?;

        Ok(stmt::ValueStream::from_iter(
            res.items.into_iter().flatten().map(move |item| {
                item_to_record(
                    &item,
                    op.select.iter().map(|column_id| schema.column(column_id)),
                )
            }),
        ))
    }
}
