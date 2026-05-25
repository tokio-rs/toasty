use crate::{
    schema::app::{BelongsTo, HasKind, Model, ModelId, Name, Schema},
    stmt,
};

/// The inverse side of a relationship.
///
/// A `Has` field on model A reaches an associated model B either through a
/// paired [`BelongsTo`] field on B that holds the foreign key, or by following a
/// multi-step (`via`) path of existing relations. Its cardinality records
/// whether the field represents "A has many Bs" or "A has one B".
#[derive(Debug, Clone)]
pub struct Has {
    /// The [`ModelId`] of the associated (target) model.
    pub target: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// Whether this relation is one-to-many or one-to-one.
    pub cardinality: HasCardinality,

    /// How this relation reaches its target — a paired `BelongsTo`
    /// ([`HasKind::Direct`]) or a [`Via`](super::Via) path
    /// ([`HasKind::Via`]).
    pub kind: HasKind,
}

/// Cardinality for a [`Has`] relation.
#[derive(Debug, Clone)]
pub enum HasCardinality {
    /// The relation yields zero or more associated items.
    Many {
        /// The singular name for one associated item (used in generated method
        /// names).
        singular: Name,
    },

    /// The relation yields at most one associated item.
    One,
}

impl Has {
    /// Returns `true` when this is a one-to-many relation.
    pub fn is_many(&self) -> bool {
        matches!(self.cardinality, HasCardinality::Many { .. })
    }

    /// Returns `true` when this is a one-to-one relation.
    pub fn is_one(&self) -> bool {
        matches!(self.cardinality, HasCardinality::One)
    }

    /// Returns the singular item name for a one-to-many relation.
    pub fn singular(&self) -> Option<&Name> {
        match &self.cardinality {
            HasCardinality::Many { singular } => Some(singular),
            HasCardinality::One => None,
        }
    }

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
