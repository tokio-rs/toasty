use crate::{
    schema::app::{Cardinality, FieldId, FieldTy, ItemParent, Model, ModelId, Name, Schema},
    stmt,
};

/// The parent's view of an item-collection relationship.
///
/// `HasItems` is the parent-side counterpart to [`ItemParent`]: a
/// `#[has_many]` (or `#[has_one]`) field on the parent that targets an
/// item-collection child. Where [`Has`](super::Has) pairs with
/// [`BelongsTo`](super::BelongsTo) and lowers to a foreign-key value-equality
/// query, `HasItems` pairs with [`ItemParent`] and lowers to a
/// partition-scoped query with a sort-key prefix filter — see design R2.9
/// in the symmetric-key item-collection design document.
///
/// The schema linker promotes a [`Has`](super::Has) to `HasItems` after
/// `link_relations` resolves `pair_id`, when the resolved pair turns out to
/// be an [`ItemParent`]. The macro layer never emits `HasItems` directly;
/// it always emits `Has`, and promotion runs before any verifier observes
/// the field.
#[derive(Debug, Clone)]
pub struct HasItems {
    /// The [`ModelId`] of the associated (child) model.
    pub target: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective. Mirrors [`Has::expr_ty`](super::Has::expr_ty) so the
    /// promotion is structurally a rewrap.
    pub expr_ty: stmt::Type,

    /// Whether this relation is one-to-many or one-to-one. Item collections
    /// are typically `Many`, but `HasOne` paired with `ItemParent` is also
    /// permitted (e.g. the canonical "exactly one Settings row per Tenant"
    /// pattern).
    pub cardinality: Cardinality,

    /// The paired [`ItemParent`] field on the target (child) model.
    pub pair_id: FieldId,
}

impl HasItems {
    /// Returns `true` when this is a one-to-many relation.
    pub fn is_many(&self) -> bool {
        self.cardinality.is_many()
    }

    /// Returns `true` when this is a one-to-one relation.
    pub fn is_one(&self) -> bool {
        self.cardinality.is_one()
    }

    /// Returns the singular item name for a one-to-many relation.
    pub fn singular(&self) -> Option<&Name> {
        self.cardinality.singular()
    }

    /// Resolves the target [`Model`] from the given schema.
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }

    /// Resolves the paired [`ItemParent`] relation on the target model.
    ///
    /// # Panics
    ///
    /// Panics if the paired field is not an `ItemParent` variant.
    pub fn pair<'a>(&self, schema: &'a Schema) -> &'a ItemParent {
        schema.field(self.pair_id).ty.as_item_parent_unwrap()
    }
}

impl From<HasItems> for FieldTy {
    fn from(value: HasItems) -> Self {
        Self::HasItems(value)
    }
}
