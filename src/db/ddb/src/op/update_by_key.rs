use super::*;

impl DynamoDB {
    pub(crate) async fn exec_update_by_key<'a>(
        &self,
        schema: &schema::Schema,
        op: operation::UpdateByKey<'a>,
    ) -> Result<stmt::ValueStream<'a>> {
        let table = schema.table(op.table);

        let mut expr_attrs = ExprAttrs::default();

        let unique_indices = table
            .indices
            .iter()
            .filter(|index| {
                if !index.primary_key && index.unique {
                    index.columns.iter().any(|index_column| {
                        let column = index_column.table_column(schema);
                        op.assignments
                            .iter()
                            .any(|assignment| assignment.target == column.id)
                    })
                } else {
                    false
                }
            })
            .collect::<Vec<_>>();

        let filter_expression = match (&op.filter, &op.condition) {
            (Some(filter), None) => Some(ddb_expression(schema, &mut expr_attrs, false, filter)),
            (None, Some(condition)) => {
                Some(ddb_expression(schema, &mut expr_attrs, false, condition))
            }
            (Some(_), Some(_)) => {
                todo!()
            }
            _ => None,
        };

        let mut update_expression_set = String::new();
        let mut update_expression_remove = String::new();
        let mut ret = vec![];

        for assignment in &op.assignments {
            let value = match &assignment.value {
                sql::Expr::Value(value) => value,
                _ => todo!("assignment = {:#?}", assignment),
            };

            ret.push(value.clone());

            let column = expr_attrs
                .column(schema.column(assignment.target))
                .to_string();

            if value.is_null() {
                if !update_expression_remove.is_empty() {
                    write!(update_expression_remove, ", ").unwrap();
                }

                write!(update_expression_remove, "{column}").unwrap();
            } else {
                let value = expr_attrs.value(value);

                if !update_expression_set.is_empty() {
                    write!(update_expression_set, ", ").unwrap();
                }

                write!(update_expression_set, "{column} = {value}").unwrap();
            }
        }

        let mut update_expression = String::new();

        if !update_expression_set.is_empty() {
            write!(update_expression, "SET {update_expression_set}").unwrap();
        }

        if !update_expression_remove.is_empty() {
            write!(update_expression, " REMOVE {update_expression_remove}").unwrap();
        }

        match &unique_indices[..] {
            [] => {
                if op.keys.len() == 1 {
                    let key = &op.keys[0];

                    let res = self
                        .client
                        .update_item()
                        .table_name(self.table_name(table))
                        .set_key(Some(ddb_key(&table, key)))
                        .set_update_expression(Some(update_expression))
                        .set_expression_attribute_names(Some(expr_attrs.attr_names))
                        .set_expression_attribute_values(if !expr_attrs.attr_values.is_empty() {
                            Some(expr_attrs.attr_values)
                        } else {
                            None
                        })
                        .set_condition_expression(filter_expression)
                        .return_values_on_condition_check_failure(
                            ReturnValuesOnConditionCheckFailure::AllOld,
                        )
                        .send()
                        .await;

                    if let Err(SdkError::ServiceError(e)) = res {
                        if let UpdateItemError::ConditionalCheckFailedException(_e) = e.err() {
                            /*
                            let record =
                                item_to_record(e.item.as_ref().unwrap(), table.columns.iter())
                                    .unwrap();
                                */

                            // First, if there is a filter, we need to check if the
                            // filter matches it. If it doesn't, then the update did
                            // not apply to the record.
                            if let Some(_) = op.filter {
                                // TODO: can't support both for now
                                assert!(op.condition.is_none());
                                /*
                                if !filter.eval_bool(&record).unwrap() {
                                    return Ok(stmt::ValueStream::new());
                                }
                                */
                                return Ok(stmt::ValueStream::new());
                            }

                            // At this point, there should be a condition
                            // let condition = op.condition.as_ref().unwrap();
                            assert!(op.condition.is_some());

                            // The condition must not have matched...
                            // TODO: can we check?
                            // assert!(!condition.eval_bool(&record).unwrap());

                            // TODO: probably map the error, but for now fall through
                        }

                        return Err(SdkError::ServiceError(e).into());
                    }
                } else {
                    let mut transact_items = vec![];

                    for key in &op.keys {
                        transact_items.push(
                            TransactWriteItem::builder()
                                .update(
                                    Update::builder()
                                        .table_name(self.table_name(table))
                                        .set_key(Some(ddb_key(&table, key)))
                                        .set_update_expression(Some(update_expression.clone()))
                                        .set_expression_attribute_names(Some(
                                            expr_attrs.attr_names.clone(),
                                        ))
                                        .set_expression_attribute_values(
                                            if !expr_attrs.attr_values.is_empty() {
                                                Some(expr_attrs.attr_values.clone())
                                            } else {
                                                None
                                            },
                                        )
                                        .set_condition_expression(filter_expression.clone())
                                        .return_values_on_condition_check_failure(
                                            ReturnValuesOnConditionCheckFailure::AllOld,
                                        )
                                        .build()
                                        .unwrap(),
                                )
                                .build(),
                        );
                    }

                    let res = self
                        .client
                        .transact_write_items()
                        .set_transact_items(Some(transact_items))
                        .send()
                        .await;

                    if let Err(SdkError::ServiceError(e)) = res {
                        todo!("err={:#?}", e);
                    }
                }
            }
            [index] => {
                assert!(op.keys.len() == 1, "TODO: handle multiple keys");
                let key = &op.keys[0];

                let mut transact_items = vec![];

                let attributes_to_get = index
                    .columns
                    .iter()
                    .map(|index_column| index_column.table_column(schema).name.clone())
                    .collect();

                // Records that have had their unique values set initially
                // (previously were null).
                let mut set_unique_attrs = HashMap::new();
                // Records that have had their unique attribute update from a
                // previous value.
                let mut updated_unique_attrs = HashMap::new();

                // First, we need to read the current value for the unique attributes
                let res = self
                    .client
                    .get_item()
                    .table_name(self.table_name(table))
                    .set_key(Some(ddb_key(&table, key)))
                    .set_attributes_to_get(Some(attributes_to_get))
                    .send()
                    .await?;

                let Some(mut curr_unique_values) = res.item else {
                    anyhow::bail!("item not found")
                };

                // Which unique attributes are being updated
                for index_column in &index.columns {
                    let column = index_column.table_column(schema);

                    for assignment in &op.assignments {
                        if column.id == assignment.target {
                            if let Some(prev) = curr_unique_values.remove(&column.name) {
                                let value = assignment.value.as_value();

                                // TODO: this probably could be made cheaper if needed
                                if ddb_to_val(&column.ty, &prev) != *value {
                                    updated_unique_attrs.insert(column.id, prev);
                                }
                            } else {
                                set_unique_attrs.insert(column.id, ());
                            }
                        }
                    }
                }

                if updated_unique_attrs.is_empty() && set_unique_attrs.is_empty() {
                    todo!()
                } else {
                    assert!(
                        updated_unique_attrs.len() + set_unique_attrs.len() == 1,
                        "TODO: support more than one unique attr"
                    );

                    let mut condition_expression = String::new();

                    for column_id in set_unique_attrs.keys() {
                        let column = expr_attrs.column(schema.column(column_id)).to_string();
                        condition_expression = format!("attribute_not_exists({column})");
                    }

                    for (column_id, prev) in &updated_unique_attrs {
                        let column = expr_attrs.column(schema.column(column_id)).to_string();
                        let value = expr_attrs.ddb_value(prev.clone());

                        condition_expression = format!("{column} = {value}");
                    }

                    if let Some(filter_expression) = filter_expression {
                        condition_expression.push_str(" AND ");
                        condition_expression.push_str(&filter_expression);
                    }

                    // Insert the update op
                    transact_items.push(
                        TransactWriteItem::builder()
                            .update(
                                Update::builder()
                                    .table_name(self.table_name(table))
                                    .set_key(Some(ddb_key(&table, key)))
                                    .condition_expression(condition_expression)
                                    .set_update_expression(Some(update_expression))
                                    .set_expression_attribute_names(Some(expr_attrs.attr_names))
                                    .set_expression_attribute_values(Some(expr_attrs.attr_values))
                                    .build()
                                    .unwrap(),
                            )
                            .build(),
                    );

                    for (column_id, prev) in &updated_unique_attrs {
                        let name = &schema.column(column_id).name;
                        // Delete the index entry for all rows that are updating
                        // their unique attribute.
                        transact_items.push(
                            TransactWriteItem::builder()
                                .delete(
                                    Delete::builder()
                                        .table_name(self.index_table_name(index))
                                        .key(name.clone(), prev.clone())
                                        .build()
                                        .unwrap(),
                                )
                                .build(),
                        );
                    }

                    for column_id in updated_unique_attrs.keys().chain(set_unique_attrs.keys()) {
                        let name = &schema.column(column_id).name;

                        // Create the new entry if there is one.
                        let mut index_insert_items = HashMap::new();

                        for index_column in &index.columns {
                            let column = index_column.table_column(schema);
                            let assignment = op
                                .assignments
                                .iter()
                                .find(|assignment| *column_id == assignment.target)
                                .unwrap();

                            let value = assignment.value.as_value();

                            if !value.is_null() {
                                index_insert_items.insert(column.name.clone(), ddb_val(value));
                            }
                        }

                        // This will be empty if **unsetting** a unique attribute.
                        if index_insert_items.is_empty() {
                            continue;
                        }

                        // Add primary keys
                        for (index, column) in table.primary_key_columns().enumerate() {
                            let key_field = match key {
                                stmt::Value::Record(record) => &record[index],
                                value => value,
                            };
                            index_insert_items.insert(column.name.clone(), ddb_val(key_field));
                        }

                        // Ensure value is unique
                        let mut expression_names = HashMap::new();
                        let expr_name = format!("#{name}");

                        let condition_expression = format!("attribute_not_exists({expr_name})");
                        expression_names.insert(expr_name, name.clone());

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

                    let res = self
                        .client
                        .transact_write_items()
                        .set_transact_items(Some(transact_items))
                        .send()
                        .await;

                    if let Err(SdkError::ServiceError(e)) = res {
                        // TODO: do some checks on the error
                        anyhow::bail!("failed to update = {:#?}", e);
                    }

                    assert!(res.is_ok());
                }
            }
            _ => todo!(),
        }

        // If we get here, then returning should be false
        Ok(if op.returning {
            stmt::ValueStream::from_value(stmt::Record::from_vec(ret))
        } else {
            stmt::ValueStream::new()
        })
    }
}
