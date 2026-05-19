use crate::{
    schema::app::{BelongsTo, FieldTy, HasKind, Model, ModelId, Schema},
    stmt,
};

/// The inverse side of a one-to-one relationship.
///
/// A `HasOne` field on model A means "A has exactly one B". A direct `HasOne`
/// pairs with a [`BelongsTo`] field on model B that holds the foreign key; a
/// multi-step (`via`) `HasOne` reaches B by following a path of existing
/// relations. Which one it is is recorded in [`kind`](HasOne::kind).
///
/// # Examples
///
/// ```ignore
/// // Given a `User` model that has one `Profile`:
/// let has_one: &HasOne = user_field.ty.as_has_one_unwrap();
/// let profile_model = has_one.target(&schema);
/// let inverse = has_one.pair(&schema); // the BelongsTo on Profile
/// ```
#[derive(Debug, Clone)]
pub struct HasOne {
    /// The [`ModelId`] of the associated (target) model.
    pub target: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// How this relation reaches its target — a paired `BelongsTo`
    /// ([`HasKind::Direct`]) or a [`Via`](super::Via) path
    /// ([`HasKind::Via`]).
    pub kind: HasKind,
}

impl HasOne {
    /// Resolves the target [`Model`] from the given schema.
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }

    /// Resolves the paired [`BelongsTo`] relation on the target model.
    ///
    /// # Panics
    ///
    /// Panics if this is a multi-step (`via`) relation — it has no pair — or
    /// if the paired field is not a `BelongsTo` variant.
    pub fn pair<'a>(&self, schema: &'a Schema) -> &'a BelongsTo {
        let pair = self
            .kind
            .pair_id()
            .expect("`via` relation has no paired `BelongsTo`");
        schema.field(pair).ty.as_belongs_to_unwrap()
    }
}

impl From<HasOne> for FieldTy {
    fn from(value: HasOne) -> Self {
        Self::HasOne(value)
    }
}
