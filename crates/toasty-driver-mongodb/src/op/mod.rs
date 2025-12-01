mod insert;
mod get_by_key;
mod update_by_key;
mod delete_by_key;
mod query_pk;
mod find_pk_by_index;
mod query_sql;
mod transaction;

use crate::MongoDb;
use toasty_core::{
    driver::{operation::Operation, Response},
    schema::db::Schema,
    Result,
};
use std::sync::Arc;

pub async fn execute_operation(
    driver: &MongoDb,
    schema: &Arc<Schema>,
    op: Operation,
) -> Result<Response> {
    match op {
        Operation::Insert(op) => insert::execute(driver, schema, op).await,
        Operation::GetByKey(op) => get_by_key::execute(driver, schema, op).await,
        Operation::UpdateByKey(op) => update_by_key::execute(driver, schema, op).await,
        Operation::DeleteByKey(op) => delete_by_key::execute(driver, schema, op).await,
        Operation::QueryPk(op) => query_pk::execute(driver, schema, op).await,
        Operation::FindPkByIndex(op) => find_pk_by_index::execute(driver, schema, op).await,
        Operation::QuerySql(op) => query_sql::execute(driver, schema, op).await,
        Operation::Transaction(op) => transaction::execute(driver, schema, op).await,
    }
}
