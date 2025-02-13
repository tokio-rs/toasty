use super::*;

use crate::schema::db::Schema;
use crate::stmt::sql::serialize::SerializeResult;
use crate::stmt::Statement as DataStatement;

#[derive(Debug, Clone)]
pub enum Statement {
    CreateIndex(CreateIndex),
    CreateTable(CreateTable),
    DropTable(DropTable),
    Delete(Delete),
    Insert(Insert),
    Query(Query),
    Update(Update),
}

impl Statement {
    pub fn serialize(&self, schema: &Schema, params: &mut impl Params) -> SerializeResult {
        Serializer::new(schema).serialize_sql_stmt(self, params)
    }
}

impl From<DataStatement> for Statement {
    fn from(value: DataStatement) -> Self {
        match value {
            DataStatement::Delete(stmt) => Statement::Delete(stmt),
            DataStatement::Insert(stmt) => Statement::Insert(stmt),
            DataStatement::Query(stmt) => Statement::Query(stmt),
            DataStatement::Update(stmt) => Statement::Update(stmt),
        }
    }
}
