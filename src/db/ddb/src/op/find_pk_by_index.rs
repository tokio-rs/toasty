use super::*;

impl DynamoDB {
    pub(crate) async fn exec_find_pk_by_index<'stmt>(
        &self,
        schema: &schema::Schema,
        op: operation::FindPkByIndex<'_>,
    ) -> Result<stmt::ValueStream<'stmt>> {
        let table = schema.table(op.table);
        let index = schema.index(op.index);

        let mut expr_attrs = ExprAttrs::default();
        let key_expression = ddb_expression(schema, &mut expr_attrs, false, &op.filter);

        let res = if index.unique {
            self.client
                .query()
                .table_name(self.index_table_name(index))
                .key_condition_expression(key_expression)
                .set_expression_attribute_names(Some(expr_attrs.attr_names))
                .set_expression_attribute_values(Some(expr_attrs.attr_values))
                .send()
                .await?
        } else {
            self.client
                .query()
                .table_name(self.table_name(table))
                .index_name(self.index_table_name(index))
                .key_condition_expression(key_expression)
                .set_expression_attribute_names(Some(expr_attrs.attr_names))
                .set_expression_attribute_values(Some(expr_attrs.attr_values))
                .send()
                .await?
        };

        let schema = schema.clone();

        Ok(stmt::ValueStream::from_iter(
            res.items.into_iter().flatten().map(move |item| {
                let table = schema.table(op.table);
                item_to_record(&item, table.primary_key_columns())
            }),
        ))
    }
}
