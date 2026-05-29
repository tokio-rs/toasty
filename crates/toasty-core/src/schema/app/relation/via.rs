use crate::{
    schema::app::{Cardinality, Model, ModelId, Name, Schema},
    stmt,
};

/// A multi-step relation path.
///
/// A `Has` relation declared with `#[has_many(via = a.b)]` or
/// `#[has_one(via = a.b)]` reaches its
/// target by following a path of existing relations rather than pairing with a
/// single `BelongsTo`. The path is resolved at macro-expansion time — the
/// derive emits a chained call on the model's `Fields` struct
/// (e.g. `User::fields().comments().article()`) and converts it into a
/// [`stmt::Path`], so a misspelled or otherwise unresolvable segment is a
/// Rust compile error rather than a runtime schema validation failure.
#[derive(Debug, Clone)]
pub struct Via {
    /// The [`ModelId`] of the associated (target) model.
    pub target: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// Whether this relation is one-to-many or one-to-one.
    pub cardinality: Cardinality,

    /// The resolved field path, rooted at the model that declares the via
    /// relation.
    pub path: stmt::Path,
}

impl Via {
    /// Create a `Via` from its fully resolved field path.
    pub fn new(
        target: ModelId,
        expr_ty: stmt::Type,
        cardinality: Cardinality,
        path: stmt::Path,
    ) -> Self {
        Self {
            target,
            expr_ty,
            cardinality,
            path,
        }
    }

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
}
