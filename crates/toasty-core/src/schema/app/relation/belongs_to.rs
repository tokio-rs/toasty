use crate::{
    schema::app::{FieldId, FieldTy, ForeignKey, Model, ModelId, Schema},
    stmt,
};

/// The owning side of a relationship. Stores the foreign key that references
/// another model's primary key.
///
/// A `BelongsTo` field does not store data in its own column; instead, its
/// [`ForeignKey`] contains one or more primitive fields on the same model
/// whose values match the target model's primary key.
///
/// # Examples
///
/// ```ignore
/// // Given a `Comment` model that belongs to a `Post`:
/// let belongs_to: &BelongsTo = comment_field.ty.as_belongs_to_unwrap();
/// let post_model = belongs_to.target(&schema);
/// ```
#[derive(Debug, Clone)]
pub struct BelongsTo {
    /// The [`ModelId`] of the referenced (target) model.
    pub target: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// The inverse [`HasMany`](super::HasMany) or [`HasOne`](super::HasOne)
    /// field on the target model, if one exists.
    pub pair: Option<FieldId>,

    /// The foreign key mapping source fields to the target's primary key
    /// fields.
    pub foreign_key: ForeignKey,
}

impl BelongsTo {
    /// Resolves the target [`Model`] from the given schema.
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }
}

impl From<BelongsTo> for FieldTy {
    fn from(value: BelongsTo) -> Self {
        Self::BelongsTo(value)
    }
}
