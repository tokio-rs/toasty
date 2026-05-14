use crate::{
    schema::app::{BelongsTo, HasKind, Model, ModelId, Name, Schema},
    stmt,
};

/// The inverse side of a one-to-many relationship.
///
/// A `HasMany` field on model A means "A has many Bs". A direct `HasMany`
/// pairs with a [`BelongsTo`] field on model B that holds the foreign key; a
/// multi-step (`via`) `HasMany` reaches B by following a path of existing
/// relations. Which one it is is recorded in [`kind`](HasMany::kind).
///
/// # Examples
///
/// ```ignore
/// // Given a `User` model that has many `Post`s:
/// let has_many: &HasMany = user_field.ty.as_has_many_unwrap();
/// let post_model = has_many.target(&schema);
/// let inverse = has_many.pair(&schema); // the BelongsTo on Post
/// ```
#[derive(Debug, Clone)]
pub struct HasMany {
    /// The [`ModelId`] of the associated (target) model.
    pub target: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// The singular name for one associated item (used in generated method
    /// names).
    pub singular: Name,

    /// How this relation reaches its target — a paired `BelongsTo`
    /// ([`HasKind::Direct`]) or a [`Via`](super::Via) path
    /// ([`HasKind::Via`]).
    pub kind: HasKind,
}

impl HasMany {
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
