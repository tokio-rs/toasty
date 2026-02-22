use super::{FieldId, ModelId};
use crate::schema::db::{IndexOp, IndexScope};

#[derive(Debug, Clone)]
pub struct Index {
    /// Uniquely identifies the model index within the schema
    pub id: IndexId,

    /// Fields included in the index.
    pub fields: Vec<IndexField>,

    /// When `true`, indexed entries are unique
    pub unique: bool,

    /// When true, the index is the primary key
    pub primary_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexId {
    pub model: ModelId,
    pub index: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct IndexField {
    /// The field being indexed
    pub field: FieldId,

    /// The comparison operation used to index the field
    pub op: IndexOp,

    /// Scope of the index
    pub scope: IndexScope,
}

impl Index {
    pub fn partition_fields(&self) -> &[IndexField] {
        let i = self.index_of_first_local_field();
        &self.fields[0..i]
    }

    pub fn local_fields(&self) -> &[IndexField] {
        let i = self.index_of_first_local_field();
        &self.fields[i..]
    }

    fn index_of_first_local_field(&self) -> usize {
        self.fields
            .iter()
            .position(|field| field.scope.is_local())
            .unwrap_or(self.fields.len())
    }
}
