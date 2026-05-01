use super::{
    Connection, Delete, ExprAttrs, Put, Result, ReturnValuesOnConditionCheckFailure, SdkError,
    TransactWriteItem, TransactWriteItemsError, Update, UpdateItemError, Value, db, ddb_expression,
    ddb_key, item_to_record, operation, stmt,
};
use aws_sdk_dynamodb::types::{AttributeValue, CancellationReason};
use std::{collections::HashMap, fmt::Write};
use toasty_core::{driver::ExecResponse, stmt::ExprContext};

/// An [`stmt::Input`] that resolves column references into a record produced
/// by `item_to_record`. After lowering, filter/condition expressions reference
/// columns via `ExprReference::Column { column: i }` where `i` is the column's
/// position in `table.columns`. `item_to_record` builds the record in that same
/// order, so indexing by `col.column` gives the right field.
struct RecordInput<'a>(&'a stmt::ValueRecord);

impl stmt::Input for RecordInput<'_> {
    fn resolve_ref(
        &mut self,
        expr_reference: &stmt::ExprReference,
        projection: &stmt::Projection,
    ) -> Option<stmt::Expr> {
        match expr_reference {
            stmt::ExprReference::Column(col) => {
                Some(self.0.fields[col.column].entry(projection).to_expr())
            }
            _ => None,
        }
    }
}

/// Returns `true` when the DynamoDB `ConditionalCheckFailedException` was
/// caused by the *filter* expression failing (→ return count 0), or `false`
/// when it was caused by the *condition* expression failing (→ return an
/// error).
///
/// Strategy: DynamoDB returns the item's pre-update state when
/// `ReturnValuesOnConditionCheckFailure::AllOld` is set.  We evaluate the
/// filter in-memory against that snapshot:
///
/// - No old item → the record didn't exist; the filter trivially didn't
///   match → count 0.
/// - Old item exists, filter evaluates to `false` → count 0.
/// - Old item exists, filter evaluates to `true` (or there is no filter) →
///   the condition must have been the failing part → error.
fn filter_failed(
    old_item: Option<&HashMap<String, AttributeValue>>,
    table: &db::Table,
    filter: Option<&stmt::Expr>,
) -> bool {
    let Some(filter) = filter else {
        return false;
    };

    let Some(item) = old_item else {
        return true;
    };

    let record = item_to_record(item, table.columns.iter()).unwrap();
    !filter.eval_bool(RecordInput(&record)).unwrap_or(false)
}

/// Interprets a `ConditionalCheckFailedException` from `update_item`: if the
/// filter was the failing predicate return an empty response; otherwise surface
/// a condition error.
fn on_update_item_condition_failed(
    item: Option<&HashMap<String, AttributeValue>>,
    message: Option<&str>,
    table: &db::Table,
    filter: Option<&stmt::Expr>,
    returning: bool,
) -> Result<ExecResponse> {
    if filter_failed(item, table, filter) {
        if returning {
            Ok(ExecResponse::empty_value_stream())
        } else {
            Ok(ExecResponse::count(0))
        }
    } else {
        Err(toasty_core::Error::condition_failed(
            message
                .unwrap_or("DynamoDB conditional check failed")
                .to_string(),
        ))
    }
}

/// Interprets a `TransactionCanceledException` from `transact_write_items`:
/// if every `ConditionalCheckFailed` reason was caused by the filter return an
/// empty response; if any was caused by the condition expression surface a
/// condition error.
fn on_transaction_cancelled(
    reasons: &[CancellationReason],
    message: Option<&str>,
    table: &db::Table,
    filter: Option<&stmt::Expr>,
    returning: bool,
) -> Result<ExecResponse> {
    let any_condition_failed = reasons
        .iter()
        .filter(|r| r.code() == Some("ConditionalCheckFailed"))
        .any(|r| !filter_failed(r.item(), table, filter));

    if any_condition_failed {
        Err(toasty_core::Error::condition_failed(
            message
                .unwrap_or("DynamoDB conditional check failed")
                .to_string(),
        ))
    } else if returning {
        Ok(ExecResponse::empty_value_stream())
    } else {
        Ok(ExecResponse::count(0))
    }
}

impl Connection {
    pub(crate) async fn exec_update_by_key(
        &mut self,
        schema: &db::Schema,
        op: operation::UpdateByKey,
    ) -> Result<ExecResponse> {
        let table = schema.table(op.table);
        let cx = ExprContext::new_with_target(schema, table);

        let mut expr_attrs = ExprAttrs::default();

        let unique_indices = table
            .indices
            .iter()
            .filter(|index| {
                if !index.primary_key && index.unique {
                    index.columns.iter().any(|index_column| {
                        let column = index_column.table_column(schema);
                        op.assignments
                            .keys()
                            .any(|projection| *projection == column.id.index)
                    })
                } else {
                    false
                }
            })
            .collect::<Vec<_>>();

        let filter_expression = match (&op.filter, &op.condition) {
            (Some(filter), None) => Some(ddb_expression(&cx, &mut expr_attrs, false, filter)),
            (None, Some(condition)) => Some(ddb_expression(&cx, &mut expr_attrs, false, condition)),
            (Some(filter), Some(condition)) => {
                let f = ddb_expression(&cx, &mut expr_attrs, false, filter);
                let c = ddb_expression(&cx, &mut expr_attrs, false, condition);
                Some(format!("({f}) AND ({c})"))
            }
            _ => None,
        };

        let mut update_expression_set = String::new();
        let mut update_expression_remove = String::new();
        let mut ret = vec![];

        for (projection, assignment) in op.assignments.iter() {
            let stmt::Assignment::Set(expr) = assignment else {
                todo!("only SET supported in DynamoDB; got {assignment:#?}");
            };
            let value = match expr {
                stmt::Expr::Value(value) => value,
                _ => todo!("op = {:#?}", op),
            };

            ret.push(value.clone());

            let column = expr_attrs.column(table.resolve(projection)).to_string();

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
                        .table_name(&table.name)
                        .set_key(Some(ddb_key(table, key)))
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
                        if let UpdateItemError::ConditionalCheckFailedException(cce) = e.err() {
                            return on_update_item_condition_failed(
                                cce.item(),
                                cce.message.as_deref(),
                                table,
                                op.filter.as_ref(),
                                op.returning,
                            );
                        }
                        return Err(toasty_core::Error::driver_operation_failed(
                            SdkError::ServiceError(e),
                        ));
                    }
                } else {
                    let mut transact_items = vec![];

                    for key in &op.keys {
                        transact_items.push(
                            TransactWriteItem::builder()
                                .update(
                                    Update::builder()
                                        .table_name(&table.name)
                                        .set_key(Some(ddb_key(table, key)))
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
                        if let TransactWriteItemsError::TransactionCanceledException(tce) = e.err()
                        {
                            return on_transaction_cancelled(
                                tce.cancellation_reasons(),
                                tce.message(),
                                table,
                                op.filter.as_ref(),
                                op.returning,
                            );
                        }
                        return Err(toasty_core::Error::driver_operation_failed(
                            SdkError::ServiceError(e),
                        ));
                    }
                }
            }
            [index] => {
                // Updating a unique-indexed column requires synchronizing the separate
                // DynamoDB index table. Because DynamoDB has no native unique-constraint
                // support, Toasty maintains a dedicated table per unique index and keeps
                // it in sync manually.
                //
                // The sequence is:
                //   1. Read the current unique column value(s) with get_item.
                //   2. Compare against the incoming assignment to decide whether the
                //      unique value actually changed.
                //   3a. Unchanged → plain update_item; no index surgery needed.
                //   3b. Changed (or first-time set) → transact_write_items that
                //       atomically: updates the base table, deletes the old index entry
                //       (if any), and inserts the new index entry with an
                //       attribute_not_exists guard to enforce uniqueness.
                //
                // Concurrency contract:
                //   The get_item in step 1 is NOT atomic with the write in step 3. A
                //   concurrent writer could mutate the unique column between those two
                //   operations.
                //
                //   Changed branch (3b): the base-table Update inside the transaction
                //   carries `<unique_col> = :old_value` as its condition, so a
                //   concurrent mutation is detected atomically at commit time and the
                //   transaction is cancelled.
                //
                //   Unchanged branch (3a): the only concurrent-safety guarantee comes
                //   from op.condition (e.g. a #[version] check). If the model has no
                //   version field, a narrow ABA race is possible where a concurrent
                //   writer changes the unique column away and back between the read and
                //   the update_item, leaving the index in an inconsistent state. Models
                //   with mutable unique fields should use #[version] to close this gap.
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
                // Records that have had their unique attribute updated from a
                // previous value.
                let mut updated_unique_attrs = HashMap::new();

                // Read the current unique column value(s) to determine whether index
                // surgery is needed. Version is not fetched here; it is verified
                // atomically at write time via op.condition.
                let res = self
                    .client
                    .get_item()
                    .table_name(&table.name)
                    .set_key(Some(ddb_key(table, key)))
                    .set_attributes_to_get(Some(attributes_to_get))
                    .send()
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;

                let Some(mut curr_unique_values) = res.item else {
                    return Err(toasty_core::Error::record_not_found(format!(
                        "table={} key={:?}",
                        table.name, key
                    )));
                };

                for index_column in &index.columns {
                    let column = index_column.table_column(schema);

                    for (projection, assignment) in op.assignments.iter() {
                        if *projection == column.id.index {
                            if let Some(prev) = curr_unique_values.remove(&column.name) {
                                let stmt::Assignment::Set(expr) = assignment else {
                                    unreachable!(
                                        "unique index assignments are always Set; got {assignment:#?}"
                                    );
                                };
                                let stmt::Expr::Value(value) = expr else {
                                    unreachable!(
                                        "unique index assignment expression is always a Value; got {expr:#?}"
                                    );
                                };

                                if Value::from_ddb(&column.ty, &prev).into_inner() != *value {
                                    updated_unique_attrs.insert(column.id, prev);
                                }
                            } else {
                                set_unique_attrs.insert(column.id, ());
                            }
                        }
                    }
                }

                if updated_unique_attrs.is_empty() && set_unique_attrs.is_empty() {
                    // The unique column appears in the assignment but its value is
                    // unchanged — no index table surgery needed; just update the
                    // main table directly.
                    let res = self
                        .client
                        .update_item()
                        .table_name(&table.name)
                        .set_key(Some(ddb_key(table, key)))
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
                        if let UpdateItemError::ConditionalCheckFailedException(cce) = e.err() {
                            return on_update_item_condition_failed(
                                cce.item(),
                                cce.message.as_deref(),
                                table,
                                op.filter.as_ref(),
                                op.returning,
                            );
                        }
                        return Err(toasty_core::Error::driver_operation_failed(
                            SdkError::ServiceError(e),
                        ));
                    }
                } else {
                    assert!(
                        updated_unique_attrs.len() + set_unique_attrs.len() == 1,
                        "TODO: support more than one unique attr"
                    );

                    let mut condition_expression = String::new();

                    for column_id in set_unique_attrs.keys() {
                        let column = expr_attrs.column(schema.column(*column_id)).to_string();
                        condition_expression = format!("attribute_not_exists({column})");
                    }

                    for (column_id, prev) in &updated_unique_attrs {
                        let column = expr_attrs.column(schema.column(*column_id)).to_string();
                        let value = expr_attrs.ddb_value(prev.clone());

                        condition_expression = format!("{column} = {value}");
                    }

                    if let Some(filter_expression) = filter_expression {
                        condition_expression.push_str(" AND ");
                        condition_expression.push_str(&filter_expression);
                    }

                    transact_items.push(
                        TransactWriteItem::builder()
                            .update(
                                Update::builder()
                                    .table_name(&table.name)
                                    .set_key(Some(ddb_key(table, key)))
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
                        let name = &schema.column(*column_id).name;
                        transact_items.push(
                            TransactWriteItem::builder()
                                .delete(
                                    Delete::builder()
                                        .table_name(&index.name)
                                        .key(name.clone(), prev.clone())
                                        .build()
                                        .unwrap(),
                                )
                                .build(),
                        );
                    }

                    for column_id in updated_unique_attrs.keys().chain(set_unique_attrs.keys()) {
                        let name = &schema.column(*column_id).name;

                        let mut index_insert_items = HashMap::new();

                        for index_column in &index.columns {
                            let column = index_column.table_column(schema);
                            let (_, assignment) = op
                                .assignments
                                .iter()
                                .find(|(projection, _)| **projection == column_id.index)
                                .unwrap();

                            let stmt::Assignment::Set(expr) = assignment else {
                                unreachable!(
                                    "unique index assignments are always Set; got {assignment:#?}"
                                );
                            };
                            let stmt::Expr::Value(value) = expr else {
                                unreachable!(
                                    "unique index assignment expression is always a Value; got {expr:#?}"
                                );
                            };

                            if !value.is_null() {
                                index_insert_items.insert(
                                    column.name.clone(),
                                    Value::from(value.clone()).to_ddb(),
                                );
                            }
                        }

                        // This will be empty if **unsetting** a unique attribute.
                        if index_insert_items.is_empty() {
                            continue;
                        }

                        for (index, column) in table.primary_key_columns().enumerate() {
                            let key_field = match key {
                                stmt::Value::Record(record) => &record[index],
                                value => value,
                            };
                            index_insert_items.insert(
                                column.name.clone(),
                                Value::from(key_field.clone()).to_ddb(),
                            );
                        }

                        let mut expression_names = HashMap::new();
                        let expr_name = format!("#{name}");
                        let condition_expression = format!("attribute_not_exists({expr_name})");
                        expression_names.insert(expr_name, name.clone());

                        transact_items.push(
                            TransactWriteItem::builder()
                                .put(
                                    Put::builder()
                                        .table_name(&index.name)
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
                        if let TransactWriteItemsError::TransactionCanceledException(tce) = e.err()
                        {
                            return on_transaction_cancelled(
                                tce.cancellation_reasons(),
                                tce.message(),
                                table,
                                op.filter.as_ref(),
                                op.returning,
                            );
                        }
                        return Err(toasty_core::Error::driver_operation_failed(
                            SdkError::ServiceError(e),
                        ));
                    }
                }
            }
            _ => todo!(),
        }

        // If we get here, then returning should be false
        Ok(if op.returning {
            let values = stmt::ValueStream::from_value(stmt::Value::record_from_vec(ret));
            ExecResponse::value_stream(values)
        } else {
            ExecResponse::count(op.keys.len() as _)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::filter_failed;
    use crate::db;
    use aws_sdk_dynamodb::types::AttributeValue;
    use std::collections::HashMap;
    use toasty_core::{
        schema::db::{Column, ColumnId, IndexId, PrimaryKey, TableId, Type},
        stmt::{self, BinaryOp, Expr, ExprBinaryOp, ExprColumn, ExprReference},
    };

    fn make_table() -> db::Table {
        db::Table {
            id: TableId(0),
            name: "t".to_string(),
            columns: vec![Column {
                id: ColumnId {
                    table: TableId(0),
                    index: 0,
                },
                name: "status".to_string(),
                ty: stmt::Type::String,
                storage_ty: Type::Text,
                nullable: false,
                primary_key: false,
                auto_increment: false,
                versionable: false,
            }],
            primary_key: PrimaryKey {
                columns: vec![],
                index: IndexId {
                    table: TableId(0),
                    index: 0,
                },
            },
            indices: vec![],
        }
    }

    /// Build `status = "active"` as a column-reference filter expression.
    fn status_eq_active() -> Expr {
        Expr::BinaryOp(ExprBinaryOp {
            lhs: Box::new(Expr::Reference(ExprReference::Column(ExprColumn {
                nesting: 0,
                table: 0,
                column: 0, // column 0 in the table → "status"
            }))),
            op: BinaryOp::Eq,
            rhs: Box::new(Expr::Value(stmt::Value::String("active".to_string()))),
        })
    }

    fn item_with_status(status: &str) -> HashMap<String, AttributeValue> {
        HashMap::from([("status".to_string(), AttributeValue::S(status.to_string()))])
    }

    // No filter at all: the condition expression failed → caller should surface an error,
    // not return count 0.  filter_failed must return false.
    #[test]
    fn no_filter_returns_false() {
        let table = make_table();
        assert!(!filter_failed(None, &table, None));
    }

    // Filter present but item is missing (record was deleted between read and check):
    // treat as "filter didn't match" → count 0.
    #[test]
    fn missing_item_with_filter_returns_true() {
        let table = make_table();
        let filter = status_eq_active();
        assert!(filter_failed(None, &table, Some(&filter)));
    }

    // Item present and filter matches: the filter was NOT the failing part, so the
    // condition expression must have failed → return false (surface an error).
    #[test]
    fn matching_item_returns_false() {
        let table = make_table();
        let filter = status_eq_active();
        let item = item_with_status("active");
        assert!(!filter_failed(Some(&item), &table, Some(&filter)));
    }

    // Item present but filter does not match: the filter failed → count 0.
    #[test]
    fn non_matching_item_returns_true() {
        let table = make_table();
        let filter = status_eq_active();
        let item = item_with_status("inactive");
        assert!(filter_failed(Some(&item), &table, Some(&filter)));
    }
}
