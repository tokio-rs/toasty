use super::{
    Connection, ExprAttrs, Put, Result, SdkError, TransactWriteItem, Value, db, item_to_record,
    operation, stmt,
};
use aws_sdk_dynamodb::types::{AttributeValue, ReturnValue};
use std::{collections::HashMap, fmt::Write};
use toasty_core::driver::ExecResponse;

impl Connection {
    pub(crate) async fn exec_upsert(
        &mut self,
        schema: &db::Schema,
        op: operation::Upsert,
    ) -> Result<ExecResponse> {
        let insert = op.stmt;
        let target = insert.target.as_table_unwrap();
        let table = schema.table(target.table);
        let upsert = insert
            .upsert
            .as_ref()
            .expect("upsert operation without clause");
        let stmt::UpsertTarget::Columns(conflict_columns) = &upsert.target else {
            panic!("DynamoDB upsert target was not lowered")
        };
        assert_eq!(
            conflict_columns.as_slice(),
            table
                .primary_key_columns()
                .map(|column| column.id)
                .collect::<Vec<_>>(),
            "DynamoDB upsert must target the primary key"
        );

        let stmt::ExprSet::Values(values) = &insert.source.body else {
            return Err(toasty_core::Error::invalid_statement(
                "DynamoDB upsert requires a VALUES source",
            ));
        };
        let [row] = values.rows.as_slice() else {
            return Err(toasty_core::Error::invalid_statement(
                "DynamoDB upsert requires exactly one row",
            ));
        };
        let mut item = HashMap::new();
        for (position, column_id) in target.columns.iter().enumerate() {
            let value = row_value(row, position)?;
            if !value.is_null() {
                item.insert(
                    schema.column(*column_id).name.clone(),
                    Value::from(value.clone()).to_ddb(),
                );
            }
        }

        let returning = returning_columns(schema, table, insert.returning.as_ref());
        match upsert.action {
            stmt::UpsertAction::Ignore => {
                self.exec_upsert_ignore(schema, table, item, returning.as_deref())
                    .await
            }
            stmt::UpsertAction::Update => {
                let mut key = HashMap::new();
                for column in table.primary_key_columns() {
                    key.insert(
                        column.name.clone(),
                        item.get(&column.name)
                            .expect("upsert primary key missing from source")
                            .clone(),
                    );
                }

                let mut attrs = ExprAttrs::default();
                let mut sets = String::new();
                let mut removes = String::new();

                // Values present only on the create side become `if_not_exists`.
                for (position, column_id) in target.columns.iter().enumerate() {
                    let column = schema.column(*column_id);
                    if conflict_columns.contains(&column.id)
                        || upsert.shared.contains(&[column.id.index])
                        || row_value(row, position)?.is_null()
                    {
                        continue;
                    }
                    let name = attrs.column(column).to_string();
                    let value = attrs.value(row_value(row, position)?);
                    comma(&mut sets);
                    write!(sets, "{name} = if_not_exists({name}, {value})").unwrap();
                }

                for (projection, assignment) in upsert.shared.iter() {
                    render_assignment(
                        table,
                        &mut attrs,
                        projection,
                        assignment,
                        upsert.defaults.get(projection),
                        &mut sets,
                        &mut removes,
                    )?;
                }

                let mut expression = String::new();
                if !sets.is_empty() {
                    write!(expression, "SET {sets}").unwrap();
                }
                if !removes.is_empty() {
                    write!(expression, " REMOVE {removes}").unwrap();
                }

                let output = self
                    .client
                    .update_item()
                    .table_name(&table.name)
                    .set_key(Some(key))
                    .update_expression(expression)
                    .set_expression_attribute_names(Some(attrs.attr_names))
                    .set_expression_attribute_values(
                        (!attrs.attr_values.is_empty()).then_some(attrs.attr_values),
                    )
                    .return_values(ReturnValue::AllNew)
                    .send()
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;

                if let Some(columns) = returning {
                    let record = item_to_record(
                        output
                            .attributes()
                            .expect("UpdateItem ALL_NEW response omitted attributes"),
                        columns.iter().copied(),
                    )?;
                    Ok(ExecResponse::value_stream(stmt::ValueStream::from_value(
                        stmt::Value::Record(record),
                    )))
                } else {
                    Ok(ExecResponse::count(1))
                }
            }
        }
    }

    async fn exec_upsert_ignore(
        &mut self,
        schema: &db::Schema,
        table: &db::Table,
        item: HashMap<String, AttributeValue>,
        returning: Option<&[&db::Column]>,
    ) -> Result<ExecResponse> {
        let first_pk = table.primary_key_columns().next().unwrap();
        let pk_alias = format!("#pk_{}", first_pk.id.index);
        let condition = format!("attribute_not_exists({pk_alias})");
        let names = HashMap::from([(pk_alias, first_pk.name.clone())]);

        let unique_indices = table
            .indices
            .iter()
            .filter(|index| !index.primary_key && index.unique)
            .collect::<Vec<_>>();

        if unique_indices.is_empty() {
            let result = self
                .client
                .put_item()
                .table_name(&table.name)
                .set_item(Some(item.clone()))
                .condition_expression(condition)
                .set_expression_attribute_names(Some(names))
                .send()
                .await;
            if let Err(SdkError::ServiceError(error)) = result {
                if matches!(
                    error.err(),
                    aws_sdk_dynamodb::operation::put_item::PutItemError::ConditionalCheckFailedException(_)
                ) {
                    return Ok(ExecResponse::empty_value_stream());
                }
                return Err(toasty_core::Error::driver_operation_failed(
                    SdkError::ServiceError(error),
                ));
            } else if let Err(error) = result {
                return Err(toasty_core::Error::driver_operation_failed(error));
            }
        } else {
            let mut writes = Vec::new();
            for index in unique_indices {
                let mut index_item = HashMap::new();
                let mut index_names = HashMap::new();
                let mut index_condition = None;
                let mut nullable = false;
                for index_column in &index.columns {
                    let column = index_column.table_column(schema);
                    let Some(value) = item.get(&column.name) else {
                        nullable = true;
                        break;
                    };
                    index_item.insert(column.name.clone(), value.clone());
                    if index_condition.is_none() {
                        let alias = format!("#unique_{}", column.id.index);
                        index_names.insert(alias.clone(), column.name.clone());
                        index_condition = Some(format!("attribute_not_exists({alias})"));
                    }
                }
                if nullable {
                    continue;
                }
                for column in table.primary_key_columns() {
                    index_item.insert(column.name.clone(), item[&column.name].clone());
                }
                writes.push(
                    TransactWriteItem::builder()
                        .put(
                            Put::builder()
                                .table_name(&index.name)
                                .set_item(Some(index_item))
                                .set_condition_expression(index_condition)
                                .set_expression_attribute_names(Some(index_names))
                                .build()
                                .unwrap(),
                        )
                        .build(),
                );
            }
            writes.push(
                TransactWriteItem::builder()
                    .put(
                        Put::builder()
                            .table_name(&table.name)
                            .set_item(Some(item.clone()))
                            .condition_expression(condition)
                            .set_expression_attribute_names(Some(names))
                            .build()
                            .unwrap(),
                    )
                    .build(),
            );

            let result = self
                .client
                .transact_write_items()
                .set_transact_items(Some(writes))
                .send()
                .await;
            if let Err(SdkError::ServiceError(error)) = result {
                if let super::TransactWriteItemsError::TransactionCanceledException(cancelled) =
                    error.err()
                {
                    let selected_conflict = cancelled
                        .cancellation_reasons()
                        .last()
                        .and_then(|reason| reason.code())
                        == Some("ConditionalCheckFailed");
                    if selected_conflict {
                        return Ok(ExecResponse::empty_value_stream());
                    }
                }
                return Err(toasty_core::Error::driver_operation_failed(
                    SdkError::ServiceError(error),
                ));
            } else if let Err(error) = result {
                return Err(toasty_core::Error::driver_operation_failed(error));
            }
        }

        if let Some(columns) = returning {
            let record = item_to_record(&item, columns.iter().copied())?;
            Ok(ExecResponse::value_stream(stmt::ValueStream::from_value(
                stmt::Value::Record(record),
            )))
        } else {
            Ok(ExecResponse::count(1))
        }
    }
}

fn returning_columns<'a>(
    schema: &'a db::Schema,
    table: &'a db::Table,
    returning: Option<&stmt::Returning>,
) -> Option<Vec<&'a db::Column>> {
    let returning = returning?;
    let expr = returning
        .as_project()
        .expect("lowered upsert returning projection");
    let record = expr.as_record().expect("upsert returning must be a record");
    Some(
        record
            .fields
            .iter()
            .map(|expr| {
                let stmt::Expr::Reference(reference) = expr else {
                    panic!("upsert returning item is not a column: {expr:#?}")
                };
                let column = reference.as_expr_column_unwrap();
                schema.column(table.columns[column.column].id)
            })
            .collect(),
    )
}

fn row_value(row: &stmt::Expr, position: usize) -> Result<&stmt::Value> {
    match row.entry(position) {
        Some(stmt::Entry::Value(value)) | Some(stmt::Entry::Expr(stmt::Expr::Value(value))) => {
            Ok(value)
        }
        _ => Err(toasty_core::Error::invalid_statement(format!(
            "DynamoDB upsert row entry did not lower to a literal: row={row:#?}; position={position}"
        ))),
    }
}

fn render_assignment(
    table: &db::Table,
    attrs: &mut ExprAttrs,
    projection: &stmt::Projection,
    assignment: &stmt::Assignment,
    default: Option<&stmt::Assignment>,
    sets: &mut String,
    removes: &mut String,
) -> Result<()> {
    if let stmt::Assignment::Batch(batch) = assignment {
        for assignment in batch {
            render_assignment(table, attrs, projection, assignment, default, sets, removes)?;
        }
        return Ok(());
    }
    let column = table.resolve(projection);
    let name = attrs.column(column).to_string();
    match assignment {
        stmt::Assignment::Set(stmt::Expr::Value(value)) if value.is_null() => {
            comma(removes);
            removes.push_str(&name);
        }
        stmt::Assignment::Set(stmt::Expr::Value(value)) => {
            let value = attrs.value(value);
            comma(sets);
            write!(sets, "{name} = {value}").unwrap();
        }
        stmt::Assignment::Append(stmt::Expr::Value(value)) => {
            let value = attrs.value(value);
            let default = render_default(attrs, default)?;
            comma(sets);
            write!(
                sets,
                "{name} = list_append(if_not_exists({name}, {default}), {value})"
            )
            .unwrap();
        }
        stmt::Assignment::Add(stmt::Expr::Value(value)) => {
            let value = attrs.value(value);
            let default = render_default(attrs, default)?;
            comma(sets);
            write!(sets, "{name} = if_not_exists({name}, {default}) + {value}").unwrap();
        }
        stmt::Assignment::Subtract(stmt::Expr::Value(value)) => {
            let value = attrs.value(value);
            let default = render_default(attrs, default)?;
            comma(sets);
            write!(sets, "{name} = if_not_exists({name}, {default}) - {value}").unwrap();
        }
        other => {
            return Err(toasty_core::Error::unsupported_feature(format!(
                "DynamoDB upsert assignment is not supported: {other:#?}"
            )));
        }
    }
    Ok(())
}

fn render_default(attrs: &mut ExprAttrs, default: Option<&stmt::Assignment>) -> Result<String> {
    let Some(stmt::Assignment::Set(stmt::Expr::Value(value))) = default else {
        return Err(toasty_core::Error::invalid_statement(
            "DynamoDB shared upsert mutations require a literal #[default] value",
        ));
    };
    Ok(attrs.value(value).to_string())
}

fn comma(dst: &mut String) {
    if !dst.is_empty() {
        dst.push_str(", ");
    }
}
