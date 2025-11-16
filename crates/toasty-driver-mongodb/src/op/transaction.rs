use crate::MongoDb;
use toasty_core::{
    driver::{operation::Transaction, Response, Rows},
    schema::db::Schema,
    Result,
};
use std::sync::Arc;

pub async fn execute(
    _driver: &MongoDb,
    _schema: &Arc<Schema>,
    op: Transaction,
) -> Result<Response> {
    match op {
        Transaction::Start => {
            // TODO: Start a MongoDB session and transaction
            // MongoDB transactions require replica sets or sharded clusters
            // For now, return empty response
            // In full implementation:
            // 1. Create ClientSession from driver.client
            // 2. Start transaction on session
            // 3. Store session in driver state for future operations
            todo!("Transaction::Start - requires session management")
        }
        Transaction::Commit => {
            // TODO: Commit the current transaction
            // Get session from driver state and commit
            todo!("Transaction::Commit - requires session management")
        }
        Transaction::Rollback => {
            // TODO: Rollback the current transaction
            // Get session from driver state and abort
            todo!("Transaction::Rollback - requires session management")
        }
    }
}
