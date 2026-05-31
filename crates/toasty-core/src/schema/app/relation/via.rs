use crate::{
    schema::app::{Cardinality, ModelId, Name},
    stmt,
};

/// A multi-step relation path.
///
/// A `Has` relation declared with `#[has_many(via = a.b)]` or
/// `#[has_one(via = a.b)]` reaches its final model by following a path of
/// existing relations rather than pairing with a single `BelongsTo`. The path
/// is resolved at macro-expansion time — the derive emits a chained call on the
/// model's `Fields` struct
/// (e.g. `User::fields().comments().article()`) and converts it into a
/// [`stmt::Path`], so a misspelled or otherwise unresolvable segment is a
/// Rust compile error rather than a runtime schema validation failure.
#[derive(Debug, Clone)]
pub struct Via {
    /// The final [`ModelId`] reached by following the relation-only path.
    ///
    /// For model-terminal vias, this is also the value returned by the field.
    /// For projected vias, the field returns
    /// [`terminal_projection`](Self::terminal_projection) evaluated against
    /// this model.
    pub final_model: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// Whether this relation is one-to-many or one-to-one.
    pub cardinality: Cardinality,

    /// The full path declared in the model attribute, rooted at the model that
    /// declares the via relation.
    pub(crate) declared_path: stmt::Path,

    /// The relation-only prefix of the declared path, populated by schema
    /// linking.
    pub path: stmt::Path,

    /// Projection from [`final_model`](Self::final_model) to the terminal
    /// value. Identity means the terminal is the final model itself.
    pub terminal_projection: stmt::Projection,
}

impl Via {
    /// Create a `Via` from its fully resolved field path.
    pub fn new(
        final_model: ModelId,
        expr_ty: stmt::Type,
        cardinality: Cardinality,
        path: stmt::Path,
    ) -> Self {
        Self {
            final_model,
            expr_ty,
            cardinality,
            declared_path: path.clone(),
            path,
            terminal_projection: stmt::Projection::identity(),
        }
    }

    /// Create a `Via` whose final model is resolved during schema linking.
    #[doc(hidden)]
    pub fn unresolved(
        expr_ty: stmt::Type,
        cardinality: Cardinality,
        declared_path: stmt::Path,
    ) -> Self {
        Self {
            final_model: ModelId::placeholder(),
            expr_ty,
            cardinality,
            path: declared_path.clone(),
            declared_path,
            terminal_projection: stmt::Projection::identity(),
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
}
