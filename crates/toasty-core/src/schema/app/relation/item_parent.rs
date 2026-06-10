use crate::{
    schema::app::{FieldTy, Model, ModelId, Schema},
    stmt,
};

/// An item-collection parent reference.
///
/// `ItemParent` marks the field a child uses to address its
/// item-collection parent. Unlike [`BelongsTo`](super::BelongsTo) it carries
/// **no** foreign-key columns: a child's partition and sort keys already
/// encode the parent in the symmetric-key layout, so navigation lowers to a
/// partition-scoped query with a sort-key prefix filter rather than a
/// value-equality join. See design R2.9 in the symmetric-key item-collection
/// design document.
///
/// The variant is type-system scaffolding only — no code constructs it yet.
/// The macro layer continues to emit [`BelongsTo`](super::BelongsTo) for
/// `#[item_parent]` until B4.7 swaps the emission, and lowering rules land
/// in B4.8 / B4.9.
#[derive(Debug, Clone)]
pub struct ItemParent {
    /// The target (parent) model.
    pub target: ModelId,

    /// The expression type the field surfaces as — `Deferred<Parent>` from
    /// the macro layer, kept for symmetry with
    /// [`BelongsTo::expr_ty`](super::BelongsTo::expr_ty).
    pub expr_ty: stmt::Type,
}

impl ItemParent {
    /// Resolves the target [`Model`] from the given schema.
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }
}

impl From<ItemParent> for FieldTy {
    fn from(value: ItemParent) -> Self {
        Self::ItemParent(value)
    }
}
