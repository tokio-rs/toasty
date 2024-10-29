use super::*;

use crate::stmt::Statement as DataStatement;

#[derive(Debug, Clone)]
pub enum Statement<'stmt> {
    CreateIndex(CreateIndex<'stmt>),
    CreateTable(CreateTable<'stmt>),
    Delete(Delete<'stmt>),
    Insert(Insert<'stmt>),
    Query(Query<'stmt>),
    Update(Update<'stmt>),
}

impl<'stmt> Statement<'stmt> {
    pub fn serialize(&self, schema: &Schema, params: &mut impl Params<'stmt>) -> String {
        Serializer::new(schema).serialize_sql_stmt(self, params)
    }
}

impl<'stmt> From<DataStatement<'stmt>> for Statement<'stmt> {
    fn from(value: DataStatement<'stmt>) -> Self {
        match value {
            DataStatement::Delete(stmt) => Statement::Delete(stmt),
            DataStatement::Insert(stmt) => Statement::Insert(stmt),
            DataStatement::Query(stmt) => Statement::Query(stmt),
            DataStatement::Update(stmt) => Statement::Update(stmt),
            _ => todo!("stmt={value:#?}"),
        }
    }
}
