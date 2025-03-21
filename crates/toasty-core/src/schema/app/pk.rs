use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct PrimaryKey {
    /// Fields composing the primary key
    pub fields: Vec<FieldId>,

    /// Query by primary key
    pub query: QueryId,

    /// Primary key index
    pub index: IndexId,
}
