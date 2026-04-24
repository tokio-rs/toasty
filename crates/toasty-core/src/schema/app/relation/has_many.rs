use crate::{
    schema::app::{BelongsTo, FieldId, Model, ModelId, Name, Schema},
    stmt,
};

/// The inverse side of a one-to-many relationship.
///
/// A `HasMany` field on model A means "A has many Bs". The actual foreign key
/// lives on model B as a [`BelongsTo`] field pointing back at A. The two
/// sides are linked via the [`pair`](HasMany::pair) field.
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

    /// The [`BelongsTo`] field on the target model that pairs with this
    /// relation. If a `#[has_many(pair = <field>)]` was supplied, the macro
    /// resolves this at schema-construction time via `field_name_to_id` on
    /// the target. Otherwise the linker fills it in by searching the target
    /// model for a unique `BelongsTo` back to the source.
    pub pair: FieldId,
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
    /// Panics if the paired field is not a `BelongsTo` variant.
    pub fn pair<'a>(&self, schema: &'a Schema) -> &'a BelongsTo {
        schema.field(self.pair).ty.as_belongs_to_unwrap()
    }
}
