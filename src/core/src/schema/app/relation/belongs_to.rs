use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct BelongsTo {
    /// Model that owns the relation
    pub target: ModelId,

    /// The association's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,

    /// The `HasMany` or `HasOne` association that pairs with this
    pub pair: Option<FieldId>,

    /// The foreign key is a set of primitive fields that match the target's
    /// primary key.
    pub foreign_key: ForeignKey,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ForeignKey {
    pub fields: Vec<ForeignKeyField>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ForeignKeyField {
    /// The field on the source model that is acting as the foreign key
    pub source: FieldId,

    /// The field on the target model that this FK field maps to.
    pub target: FieldId,
}

impl BelongsTo {
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }
}

impl From<BelongsTo> for FieldTy {
    fn from(value: BelongsTo) -> Self {
        FieldTy::BelongsTo(value)
    }
}

impl ForeignKey {
    pub(crate) fn placeholder() -> ForeignKey {
        ForeignKey { fields: vec![] }
    }

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
