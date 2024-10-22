use super::*;

#[derive(Debug, PartialEq)]
pub struct ModelIndex {
    /// Uniquely identifies the model index within the schema
    pub id: ModelIndexId,

    /// Fields included in the index.
    pub fields: Vec<ModelIndexField>,

    /// When `true`, indexed entries are unique
    pub unique: bool,

    /// When true, the index is the primary key
    pub primary_key: bool,

    pub lowering: IndexLowering,
}

#[derive(Debug, PartialEq)]
pub struct ModelIndexId {
    pub model: ModelId,
    pub index: usize,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct ModelIndexField {
    /// The field being indexed
    pub field: FieldId,

    /// The comparison operation used to index the field
    pub op: IndexOp,

    /// Scope of the index
    pub scope: IndexScope,
}

impl ModelIndex {
    pub fn partition_fields(&self) -> &[ModelIndexField] {
        let i = self.index_of_first_local_field();
        &self.fields[0..i]
    }

    pub fn local_fields(&self) -> &[ModelIndexField] {
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

impl Into<FieldId> for &ModelIndexField {
    fn into(self) -> FieldId {
        self.field
    }
}
