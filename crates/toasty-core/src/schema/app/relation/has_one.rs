use super::*;

#[derive(Debug, Clone)]
pub struct HasOne {
    /// Associated model
    pub target: ModelId,

    /// The association's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,

    /// The `BelongsTo` association that pairs with this
    pub pair: FieldId,
}

impl HasOne {
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }

    pub fn pair<'a>(&self, schema: &'a Schema) -> &'a BelongsTo {
        schema.field(self.pair).ty.expect_belongs_to()
    }
}

impl From<HasOne> for FieldTy {
    fn from(value: HasOne) -> Self {
        Self::HasOne(value)
    }
}
