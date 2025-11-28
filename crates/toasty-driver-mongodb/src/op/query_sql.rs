use crate::MongoDb;
use toasty_core::{
    driver::{operation::QuerySql, Response},
    schema::db::Schema,
    stmt,
    Result,
};
use std::sync::Arc;

pub async fn execute(driver: &MongoDb, schema: &Arc<Schema>, op: QuerySql) -> Result<Response> {
    // QuerySql is used for operations that were originally SQL statements
    // For MongoDB, we need to handle specific statement types
    match op.stmt {
        stmt::Statement::Insert(insert) => {
            // Delegate to insert operation
            let insert_op = toasty_core::driver::operation::Insert {
                stmt: stmt::Statement::Insert(insert),
                ret: op.ret,
            };
            super::insert::execute(driver, schema, insert_op).await
        }
        _ => {
            todo!("QuerySql for non-insert statements: {:?}", op.stmt)
        }
    }
}
