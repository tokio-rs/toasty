use crate::stmt;

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
