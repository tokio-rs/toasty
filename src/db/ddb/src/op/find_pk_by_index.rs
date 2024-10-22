use super::*;

impl DynamoDB {
    pub(crate) async fn exec_find_pk_by_index<'a>(
        &self,
        schema: &'a schema::Schema,
        op: operation::FindPkByIndex<'_>,
    ) -> Result<stmt::ValueStream<'a>> {
        let table = schema.table(op.table);
        let index = schema.index(op.index);

        let mut expr_attrs = ExprAttrs::default();
        let key_expression = ddb_expression(schema, &mut expr_attrs, false, &op.filter);

        println!(
            "index.unique={:#?}; key_condition_expression={:#?}; names={:#?}",
            index.unique, key_expression, expr_attrs.attr_names
        );

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

        Ok(stmt::ValueStream::from_iter(
            res.items
                .into_iter()
                .flatten()
                .map(|item| item_to_record(&item, table.primary_key_columns())),
        ))
    }
}
