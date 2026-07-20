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
///
/// The terminal segment of the path is usually another relation (the via
/// reaches a model). It may also be a **scalar field**, in which case the via
/// projects that field through the relation path — e.g.
/// `#[has_many(via = todos.tags.name)] tag_names: Vec<String>` collects the
/// `name` of every tag reachable through todos. For a scalar terminal,
/// [`terminal`](Self::terminal) holds the terminal field's index on
/// [`target`](Self::target) (the model the relation chain reaches), and
/// [`path`](Self::path) still includes that terminal step as its last element.
///
/// A common model-terminal use is many-to-many traversal through a join model.
/// If `User` has many `Membership` records and each membership belongs to a
/// `Group`, `#[has_many(via = memberships.group)]` exposes the distinct groups
/// reachable from a user. The join model owns the foreign keys and any fields
/// that describe the connection.
#[derive(Debug, Clone)]
pub struct Via {
    /// The [`ModelId`] of the model the relation chain reaches. For a relation
    /// terminal this is the associated (target) model; for a scalar terminal
    /// it is the model that owns the projected terminal field.
    pub target: ModelId,

    /// The expression type this field evaluates to from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// Whether this relation is one-to-many or one-to-one.
    pub cardinality: Cardinality,

    /// The resolved field path, rooted at the model that declares the via
    /// relation. When [`terminal`](Self::terminal) is `Some`, the last element
    /// of the path is the scalar terminal field (on [`target`](Self::target))
    /// and the preceding elements form the relation chain.
    pub path: stmt::Path,

    /// For a scalar-terminal via, the index of the projected terminal field on
    /// [`target`](Self::target). `None` when the via reaches a model (the
    /// terminal segment is itself a relation).
    pub terminal: Option<usize>,
}

impl Via {
    /// Create a `Via` from its fully resolved field path.
    pub fn new(
        target: ModelId,
        expr_ty: stmt::Type,
        cardinality: Cardinality,
        path: stmt::Path,
        terminal: Option<usize>,
    ) -> Self {
        Self {
            target,
            expr_ty,
            cardinality,
            path,
            terminal,
        }
    }

    /// Returns `true` when the via projects a scalar terminal field rather than
    /// reaching a model.
    pub fn is_scalar(&self) -> bool {
        self.terminal.is_some()
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
