use crate::{
    schema::app::{Model, ModelId, Schema},
    stmt,
};

#[derive(Debug, Clone)]
pub struct Embedded {
    /// The embedded model being referenced
    pub target: ModelId,

    /// The embedded field's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,
}

impl Embedded {
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }
}
