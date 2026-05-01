use crate::sort_key_columns;

use super::{
    Connection, Delete, ExprAttrs, Result, ReturnValuesOnConditionCheckFailure, SdkError,
    TransactWriteItem, db, ddb_expression, ddb_key, item_to_record, operation,
};
use std::collections::HashMap;
use toasty_core::{driver::ExecResponse, stmt::ExprContext};

impl Connection {
    pub(crate) async fn exec_delete_by_key(
        &mut self,
        schema: &db::Schema,
        op: operation::DeleteByKey,
    ) -> Result<ExecResponse> {
        use aws_sdk_dynamodb::operation::delete_item::DeleteItemError;

        let table = schema.table(op.table);
        let sk_cols = sort_key_columns(table);
        let cx = ExprContext::new_with_target(schema, table);

        let mut expr_attrs = ExprAttrs::default();

        let condition_expression = match (&op.filter, &op.condition) {
            (Some(filter), None) => Some(ddb_expression(&cx, &mut expr_attrs, false, filter)),
            (None, Some(condition)) => Some(ddb_expression(&cx, &mut expr_attrs, false, condition)),
            (Some(filter), Some(condition)) => {
                let f = ddb_expression(&cx, &mut expr_attrs, false, filter);
                let c = ddb_expression(&cx, &mut expr_attrs, false, condition);
                Some(format!("({f}) AND ({c})"))
            }
            _ => None,
        };

        let has_condition = op.condition.is_some();
        let filter_expr = op.filter.as_ref();

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

                let mut req = self
                    .client
                    .delete_item()
                    .table_name(&table.name)
                    .set_key(Some(ddb_key(table, key)))
                    .set_expression_attribute_names(if condition_expression.is_some() {
                        Some(expr_attrs.attr_names)
                    } else {
                        None
                    })
                    .set_expression_attribute_values(if condition_expression.is_some() {
                        Some(expr_attrs.attr_values)
                    } else {
                        None
                    })
                    .set_condition_expression(condition_expression);

                if has_condition || filter_expr.is_some() {
                    req = req.return_values_on_condition_check_failure(
                        ReturnValuesOnConditionCheckFailure::AllOld,
                    );
                }

                let res = req.send().await;

                if let Err(SdkError::ServiceError(e)) = res {
                    if let DeleteItemError::ConditionalCheckFailedException(cce) = e.err() {
                        if !has_condition {
                            // Pure filter failure → count 0
                            return Ok(ExecResponse::count(0));
                        }

                        if let Some(filter) = filter_expr {
                            // Both filter and condition set — check if filter matched
                            if let Some(old_item) = cce.item() {
                                let record =
                                    item_to_record(old_item, table.columns.iter(), &sk_cols)
                                        .unwrap();
                                use toasty_core::stmt;
                                struct RecordInput<'a>(&'a stmt::ValueRecord);
                                impl stmt::Input for RecordInput<'_> {
                                    fn resolve_ref(
                                        &mut self,
                                        expr_reference: &stmt::ExprReference,
                                        projection: &stmt::Projection,
                                    ) -> Option<stmt::Expr> {
                                        match expr_reference {
                                            stmt::ExprReference::Column(col) => Some(
                                                self.0.fields[col.column]
                                                    .entry(projection)
                                                    .to_expr(),
                                            ),
                                            _ => None,
                                        }
                                    }
                                }
                                if !filter.eval_bool(RecordInput(&record)).unwrap_or(false) {
                                    return Ok(ExecResponse::count(0));
                                }
                            } else {
                                // Record gone — filter trivially didn't match
                                return Ok(ExecResponse::count(0));
                            }
                        }

                        return Err(toasty_core::Error::condition_failed(
                            "DynamoDB conditional check failed",
                        ));
                    }

                    return Err(toasty_core::Error::driver_operation_failed(
                        SdkError::ServiceError(e),
                    ));
                }

                return Ok(ExecResponse::count(1));
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
                                        if condition_expression.is_some() {
                                            Some(expr_attrs.attr_names.clone())
                                        } else {
                                            None
                                        },
                                    )
                                    .set_expression_attribute_values(
                                        if condition_expression.is_some() {
                                            Some(expr_attrs.attr_values.clone())
                                        } else {
                                            None
                                        },
                                    )
                                    .set_condition_expression(condition_expression.clone())
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
                    if has_condition {
                        return Err(toasty_core::Error::condition_failed(
                            "DynamoDB conditional check failed",
                        ));
                    }
                    todo!("err={:#?}", e);
                }

                return Ok(ExecResponse::count(op.keys.len() as _));
            }
        }

        let [key] = &op.keys[..] else {
            panic!("only 1 key supported so far")
        };

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
            return Ok(ExecResponse::count(0));
        };

        // Now we must both delete from the main table **and** the unique index
        // while ensuring the unique attributes have not been mutated.
        let mut transact_items = vec![];

        let mut expression_names = HashMap::new();
        let mut expression_values = HashMap::new();
        let mut unique_condition_expression = String::new();

        for (name, value) in &curr_unique_values {
            let expr_name = format!("#{name}");
            let expr_value_name = format!(":{name}");
            unique_condition_expression = format!("{expr_name} = {expr_value_name}");
            expression_names.insert(expr_name, name.clone());
            expression_values.insert(expr_value_name, value.clone());
        }

        // AND in the version condition if present
        if let Some(cond_expr) = &condition_expression {
            if unique_condition_expression.is_empty() {
                unique_condition_expression = cond_expr.clone();
                expression_names.extend(expr_attrs.attr_names.clone());
                expression_values.extend(expr_attrs.attr_values.clone());
            } else {
                unique_condition_expression =
                    format!("({unique_condition_expression}) AND ({cond_expr})");
                expression_names.extend(expr_attrs.attr_names.clone());
                expression_values.extend(expr_attrs.attr_values.clone());
            }
        }

        transact_items.push(
            TransactWriteItem::builder()
                .delete(
                    Delete::builder()
                        .table_name(&table.name)
                        .set_key(Some(ddb_key(table, key)))
                        .condition_expression(unique_condition_expression)
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

        let res = self
            .client
            .transact_write_items()
            .set_transact_items(Some(transact_items))
            .send()
            .await;

        if let Err(e) = res {
            if has_condition {
                return Err(toasty_core::Error::condition_failed(
                    "DynamoDB conditional check failed",
                ));
            }
            return Err(toasty_core::Error::driver_operation_failed(e));
        }

        Ok(ExecResponse::count(1))
    }
}
