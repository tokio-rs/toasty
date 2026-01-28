use super::{
    ddb_expression, ddb_key, operation, Connection, Delete, ExprAttrs, Result, Schema, SdkError,
    TransactWriteItem,
};
use std::collections::HashMap;
use toasty_core::{driver::Response, stmt::ExprContext};

impl Connection {
    pub(crate) async fn exec_delete_by_key(
        &mut self,
        schema: &Schema,
        op: operation::DeleteByKey,
    ) -> Result<Response> {
        use aws_sdk_dynamodb::operation::delete_item::DeleteItemError;

        let table = schema.table(op.table);
        let cx = ExprContext::new_with_target(schema, table);

        let mut expr_attrs = ExprAttrs::default();
        let mut filter_expression = None;

        if let Some(filter) = &op.filter {
            filter_expression = Some(ddb_expression(&cx, &mut expr_attrs, false, filter));
        }

        let unique_indices = table
            .indices
            .iter()
            .filter(|index| !index.primary_key && index.unique)
            .collect::<Vec<_>>();

        if unique_indices.len() > 1 {
            panic!("TODO: support more than 1 unique index");
        }

        if unique_indices.is_empty() {
            if op.keys.len() == 1 {
                let key = &op.keys[0];

                let res = self
                    .client
                    .delete_item()
                    .table_name(&table.name)
                    .set_key(Some(ddb_key(table, key)))
                    .set_expression_attribute_names(if filter_expression.is_some() {
                        Some(expr_attrs.attr_names)
                    } else {
                        None
                    })
                    .set_expression_attribute_values(if filter_expression.is_some() {
                        Some(expr_attrs.attr_values)
                    } else {
                        None
                    })
                    .set_condition_expression(filter_expression)
                    .send()
                    .await;

                if let Err(SdkError::ServiceError(e)) = res {
                    if let DeleteItemError::ConditionalCheckFailedException(_) = e.err() {
                        return Ok(Response::count(0));
                    }

                    return Err(toasty_core::Error::driver_operation_failed(
                        SdkError::ServiceError(e),
                    ));
                }

                assert!(res.is_ok());

                return Ok(Response::count(1));
            } else {
                let mut transact_items = vec![];

                for key in &op.keys {
                    transact_items.push(
                        TransactWriteItem::builder()
                            .delete(
                                Delete::builder()
                                    .table_name(&table.name)
                                    .set_key(Some(ddb_key(table, key)))
                                    .set_expression_attribute_names(
                                        if filter_expression.is_some() {
                                            Some(expr_attrs.attr_names.clone())
                                        } else {
                                            None
                                        },
                                    )
                                    .set_expression_attribute_values(
                                        if filter_expression.is_some() {
                                            Some(expr_attrs.attr_values.clone())
                                        } else {
                                            None
                                        },
                                    )
                                    .set_condition_expression(filter_expression.clone())
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

                return Ok(Response::count(op.keys.len() as _));
            }
        }

        let [key] = &op.keys[..] else {
            panic!("only 1 key supported so far")
        };

        if filter_expression.is_some() {
            todo!()
        }

        let index = &unique_indices[0];

        let attributes_to_get = index
            .columns
            .iter()
            .map(|index_column| {
                let column = schema.column(index_column.column);
                column.name.clone()
            })
            .collect();

        // First, we need to read the current value for the unique attributes
        let res = self
            .client
            .get_item()
            .table_name(&table.name)
            .set_key(Some(ddb_key(table, key)))
            .set_attributes_to_get(Some(attributes_to_get))
            .send()
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        let Some(curr_unique_values) = res.item else {
            return Ok(Response::count(0));
        };

        // Now we must both delete from the main table **and** the unique index
        // while ensuring the unique attributes have not been mutated.
        let mut transact_items = vec![];

        let mut expression_names = HashMap::new();
        let mut expression_values = HashMap::new();
        let mut condition_expression = String::new();

        for (name, value) in &curr_unique_values {
            let expr_name = format!("#{name}");
            let expr_value_name = format!(":{name}");
            condition_expression = format!("{expr_name} = {expr_value_name}");
            expression_names.insert(expr_name, name.clone());
            expression_values.insert(expr_value_name, value.clone());
        }

        transact_items.push(
            TransactWriteItem::builder()
                .delete(
                    Delete::builder()
                        .table_name(&table.name)
                        .set_key(Some(ddb_key(table, key)))
                        .condition_expression(condition_expression)
                        .set_expression_attribute_names(Some(expression_names))
                        .set_expression_attribute_values(Some(expression_values))
                        .build()
                        .unwrap(),
                )
                .build(),
        );

        for (name, value) in curr_unique_values {
            transact_items.push(
                TransactWriteItem::builder()
                    .delete(
                        Delete::builder()
                            .table_name(&index.name)
                            .key(name, value)
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
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        Ok(Response::count(1))
    }
}
