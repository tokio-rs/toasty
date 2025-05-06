use super::*;

#[derive(Debug, Clone)]
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

impl BelongsTo {
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }
}

impl From<BelongsTo> for FieldTy {
    fn from(value: BelongsTo) -> Self {
        Self::BelongsTo(value)
    }
}
