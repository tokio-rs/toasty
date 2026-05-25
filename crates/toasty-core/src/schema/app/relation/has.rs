use crate::{
    schema::app::{BelongsTo, FieldId, Model, ModelId, Name, Schema},
    stmt,
};

/// The inverse side of a relationship.
///
/// A `Has` field on model A reaches an associated model B through a paired
/// [`BelongsTo`] field on B that holds the foreign key. Its cardinality records
/// whether the field represents "A has many Bs" or "A has one B".
#[derive(Debug, Clone)]
pub struct Has {
    /// The [`ModelId`] of the associated (target) model.
    pub target: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// Whether this relation is one-to-many or one-to-one.
    pub cardinality: Cardinality,

    /// The paired `BelongsTo` field on the target model.
    pub pair_id: FieldId,
}

/// Cardinality for a relation field that reaches another model.
#[derive(Debug, Clone)]
pub enum Cardinality {
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

    /// Resolves the paired [`BelongsTo`] relation on the target model.
    ///
    /// # Panics
    ///
    /// Panics if the paired field is not a `BelongsTo` variant.
    pub fn pair<'a>(&self, schema: &'a Schema) -> &'a BelongsTo {
        schema.field(self.pair_id).ty.as_belongs_to_unwrap()
    }
}

impl Cardinality {
    /// Returns `true` when this relation yields zero or more items.
    pub fn is_many(&self) -> bool {
        matches!(self, Cardinality::Many { .. })
    }

    /// Returns `true` when this relation yields at most one item.
    pub fn is_one(&self) -> bool {
        matches!(self, Cardinality::One)
    }

    /// Returns the singular item name for a one-to-many relation.
    pub fn singular(&self) -> Option<&Name> {
        match self {
            Cardinality::Many { singular } => Some(singular),
            Cardinality::One => None,
        }
    }
}
