mod create_table;
mod delete_by_key;
mod find_pk_by_index;
mod get_by_key;
mod insert;
mod query_pk;
mod scan;
mod update_by_key;
mod upsert;

use super::{
    AttributeDefinition, BillingMode, Connection, Delete, ExprAttrs, GlobalSecondaryIndex,
    KeysAndAttributes, Projection, ProjectionType, Put, PutRequest,
    ReturnValuesOnConditionCheckFailure, SdkError, TransactWriteItem, TransactWriteItemsError,
    TypeExt, Update, UpdateItemError, Value, WriteRequest, ddb_expression, ddb_key, ddb_key_schema,
    deserialize_ddb_cursor, item_to_record, serialize_ddb_cursor,
};
use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;
use toasty_core::{
    Result, Schema,
    driver::operation,
    schema::db::{self, Table},
    stmt,
};

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

/// Returns `true` when a DynamoDB conditional check failed because the *filter*
/// expression no longer matches (→ the mutation is a no-op), or `false` when it
/// failed because the *condition* expression did (→ surface an error).
///
/// Strategy: DynamoDB returns the item's pre-write state when
/// `ReturnValuesOnConditionCheckFailure::AllOld` is set. We evaluate the
/// filter in-memory against that snapshot:
///
/// - No old item → the record didn't exist; the filter trivially didn't
///   match → no-op.
/// - Old item exists, filter evaluates to `false` → no-op.
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
