use super::*;

#[derive(Debug, PartialEq)]
pub struct HasMany {
    /// Associated model
    pub target: ModelId,

    /// The association's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,

    /// Singular item name
    pub singular: Name,

    /// The `BelongsTo` association that pairs with this
    pub pair: FieldId,

    /// Queries on the association. These are queries scoped by the owner of the
    /// association.
    pub queries: Vec<ScopedQuery>,
}

impl HasMany {
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }

    pub fn pair<'a>(&self, schema: &'a Schema) -> &'a BelongsTo {
        schema.field(self.pair).ty.expect_belongs_to()
    }
}
