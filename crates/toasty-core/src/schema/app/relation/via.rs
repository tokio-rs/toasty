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

    /// Mutable access to the [`Via`] path, or `None` for a direct relation.
    pub fn via_mut(&mut self) -> Option<&mut Via> {
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
/// single `BelongsTo`. The path is written as field-name segments in the
/// attribute and resolved to a [`stmt::Path`] by the schema linker.
#[derive(Debug, Clone)]
pub struct Via {
    /// Field-name segments from the `via = ...` attribute, e.g.
    /// `["comments", "article"]` for `via = comments.article`.
    pub segments: Vec<String>,

    /// The resolved field path, rooted at the model that declares the via
    /// relation. Populated by the schema linker; `None` until linking runs.
    pub path: Option<stmt::Path>,
}

impl Via {
    /// Create an unresolved `Via` from the attribute's field-name segments.
    pub fn new(segments: Vec<String>) -> Self {
        Self {
            segments,
            path: None,
        }
    }

    /// The resolved field path.
    ///
    /// # Panics
    ///
    /// Panics if the schema linker has not resolved the path yet.
    pub fn path(&self) -> &stmt::Path {
        self.path
            .as_ref()
            .expect("via path has not been resolved by the schema linker")
    }
}
