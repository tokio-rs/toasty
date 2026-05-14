use crate::{
    schema::app::{BelongsTo, FieldId, FieldTy, Model, ModelId, Schema, Via},
    stmt,
};

/// The inverse side of a one-to-one relationship.
///
/// A `HasOne` field on model A means "A has exactly one B". The actual foreign
/// key lives on model B as a [`BelongsTo`] field. The two sides are linked via
/// the [`pair`](HasOne::pair) field.
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

    /// The [`BelongsTo`] field on the target model that pairs with this
    /// relation. If a `#[has_one(pair = <field>)]` was supplied, the macro
    /// resolves this at schema-construction time via `field_name_to_id` on
    /// the target. Otherwise the linker fills it in by searching the target
    /// model for a unique `BelongsTo` back to the source.
    ///
    /// Unused (and left as a placeholder) when [`via`](HasOne::via) is set —
    /// a `via` relation has no foreign key of its own.
    pub pair: FieldId,

    /// When set, this is a multi-step relation: the target is reached by
    /// following a path of existing relations rather than pairing with a
    /// single `BelongsTo`. See [`Via`].
    pub via: Option<Via>,
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
    /// Panics if the paired field is not a `BelongsTo` variant.
    pub fn pair<'a>(&self, schema: &'a Schema) -> &'a BelongsTo {
        schema.field(self.pair).ty.as_belongs_to_unwrap()
    }
}

impl From<HasOne> for FieldTy {
    fn from(value: HasOne) -> Self {
        Self::HasOne(value)
    }
}
