use crate::{schema::app::FieldId, stmt};

/// How a `HasMany` or `HasOne` relation reaches its target.
///
/// Both relation kinds share this: the question — "is the target reached
/// through a paired `BelongsTo`, or by following a path?" — is the same for
/// has-many and has-one.
#[derive(Debug, Clone)]
pub enum HasKind {
    /// The target is reached through a `BelongsTo` field on the target model.
    /// Carries that paired `BelongsTo` field's id.
    ///
    /// If a `#[has_many(pair = <field>)]` / `#[has_one(pair = <field>)]` was
    /// supplied, the macro resolves the id at schema-construction time.
    /// Otherwise the linker fills it in by searching the target model for a
    /// unique `BelongsTo` back to the source.
    Direct(FieldId),

    /// The target is reached by following a [`Via`] path of existing
    /// relations rather than pairing with a single `BelongsTo`.
    Via(Via),
}

impl HasKind {
    /// The paired `BelongsTo` field id, or `None` for a `via` relation.
    pub fn pair_id(&self) -> Option<FieldId> {
        match self {
            HasKind::Direct(pair) => Some(*pair),
            HasKind::Via(_) => None,
        }
    }

    /// The [`Via`] path, or `None` for a direct relation.
    pub fn via(&self) -> Option<&Via> {
        match self {
            HasKind::Via(via) => Some(via),
            HasKind::Direct(_) => None,
        }
    }
}

/// A multi-step relation path.
///
/// A `HasMany` or `HasOne` declared with `#[has_many(via = a.b)]` reaches its
/// target by following a path of existing relations rather than pairing with a
/// single `BelongsTo`. The path is resolved at macro-expansion time — the
/// derive emits a chained call on the model's `Fields` struct
/// (e.g. `User::fields().comments().article()`) and converts it into a
/// [`stmt::Path`], so a misspelled or otherwise unresolvable segment is a
/// Rust compile error rather than a runtime schema validation failure.
#[derive(Debug, Clone)]
pub struct Via {
    /// The resolved field path, rooted at the model that declares the via
    /// relation.
    pub path: stmt::Path,
}

impl Via {
    /// Create a `Via` from its fully resolved field path.
    pub fn new(path: stmt::Path) -> Self {
        Self { path }
    }
}
