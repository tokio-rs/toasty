use super::*;

impl DynamoDB {
    pub(crate) async fn exec_insert<'stmt>(
        &self,
        schema: &schema::Schema,
        insert: sql::Insert<'stmt>,
    ) -> Result<stmt::ValueStream<'stmt>> {
        let table = &schema.table(insert.table);

        let unique_indices = table
            .indices
            .iter()
            .filter(|index| {
                if !index.primary_key && index.unique {
                    // Don't update the index if the value is not included.
                    index.columns.iter().all(|index_column| {
                        let column = schema.column(index_column.column);
                        insert
                            .columns
                            .iter()
                            .any(|column_id| *column_id == column.id)
                    })
                } else {
                    false
                }
            })
            .collect::<Vec<_>>();

        // Create the item map
        let mut insert_items = vec![];
        let mut ret = vec![];

        let source = insert.source.into_values();

        for row in source.rows {
            let mut values = vec![];
            let mut items = HashMap::new();

            for (i, column_id) in insert.columns.iter().enumerate() {
                let column = schema.column(*column_id);

                if let Some(expr) = row.get(i) {
                    let val = expr.as_value();

                    if !val.is_null() {
                        items.insert(column.name.clone(), ddb_val(val));
                    }

                    values.push(val.clone());
                }
            }
            ret.push(stmt::Record::from_vec(values).into());
            insert_items.push(items);
        }

        match &unique_indices[..] {
            [] => {
                if insert_items.len() == 1 {
                    let insert_items = insert_items.into_iter().next().unwrap();

                    self.client
                        .put_item()
                        .table_name(self.table_name(table))
                        .set_item(Some(insert_items))
                        .send()
                        .await?;
                } else {
                    let mut request_items = HashMap::new();
                    request_items.insert(
                        self.table_name(table),
                        insert_items
                            .into_iter()
                            .map(|insert_item| {
                                WriteRequest::builder()
                                    .put_request(
                                        PutRequest::builder()
                                            .set_item(Some(insert_item))
                                            .build()
                                            .unwrap(),
                                    )
                                    .build()
                            })
                            .collect(),
                    );

                    self.client
                        .batch_write_item()
                        .set_request_items(Some(request_items))
                        .send()
                        .await?;
                }
            }
            [index] => {
                let mut transact_items = vec![];

                for insert_items in insert_items {
                    let mut index_insert_items = HashMap::new();
                    let mut expression_names = HashMap::new();
                    let mut condition_expression = String::new();
                    let mut nullable = false;

                    for index_column in &index.columns {
                        let column = schema.column(index_column.column);

                        if !insert_items.contains_key(&column.name) {
                            nullable = true;
                            break;
                        }

                        index_insert_items
                            .insert(column.name.clone(), insert_items[&column.name].clone());

                        if condition_expression.is_empty() {
                            let name = format!("#{}", column.id.index);
                            condition_expression = format!("attribute_not_exists({name})");
                            expression_names.insert(name, column.name.clone());
                        }
                    }

                    if !nullable {
                        // Add primary key values
                        for column in table.primary_key_columns() {
                            let name = &column.name;
                            index_insert_items.insert(name.clone(), insert_items[name].clone());
                        }

                        transact_items.push(
                            TransactWriteItem::builder()
                                .put(
                                    Put::builder()
                                        .table_name(self.index_table_name(index))
                                        .set_item(Some(index_insert_items))
                                        .condition_expression(condition_expression)
                                        .set_expression_attribute_names(Some(expression_names))
                                        .build()
                                        .unwrap(),
                                )
                                .build(),
                        );
                    }

                    transact_items.push(
                        TransactWriteItem::builder()
                            .put(
                                Put::builder()
                                    .table_name(self.table_name(table))
                                    .set_item(Some(insert_items))
                                    .build()
                                    .unwrap(),
                            )
                            .build(),
                    );
                }

                self.client
                    .transact_write_items()
                    .set_transact_items(Some(transact_items))
                    .send()
                    .await?;
            }
            _ => todo!(),
        }

        Ok(stmt::ValueStream::from_vec(ret))
    }
}
