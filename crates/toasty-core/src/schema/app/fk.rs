use super::{Field, FieldId, Schema};

#[derive(Debug, Clone)]
pub struct ForeignKey {
    pub fields: Vec<ForeignKeyField>,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyField {
    /// The field on the source model that is acting as the foreign key
    pub source: FieldId,

    /// The field on the target model that this FK field maps to.
    pub target: FieldId,
}

impl ForeignKey {
    pub(crate) fn is_placeholder(&self) -> bool {
        self.fields.is_empty()
    }
}

impl ForeignKeyField {
    pub fn source<'a>(&self, schema: &'a Schema) -> &'a Field {
        schema.field(self.source)
    }

    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Field {
        schema.field(self.target)
    }
}
