use super::*;

#[derive(Debug, Clone)]
pub struct PrimaryKey {
    /// Fields composing the primary key
    pub fields: Vec<FieldId>,

    /// Primary key index
    pub index: IndexId,
}
