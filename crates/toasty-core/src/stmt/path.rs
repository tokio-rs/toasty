use super::{Expr, Projection};
use crate::schema::app::{FieldId, ModelId, VariantId};

/// The root of a path traversal.
///
/// A path can originate from a top-level model or from a specific variant of
/// an embedded enum field.
#[derive(Debug, Clone, PartialEq)]
pub enum PathRoot {
    /// The path originates from a top-level model.
    Model(ModelId),

    /// The path originates from a specific variant of an embedded enum.
    ///
    /// `parent` navigates to the enum field; subsequent projection steps index
    /// into that variant's fields using 0-based local indices.
    Variant {
        parent: Box<Path>,
        variant_id: VariantId,
    },
}

impl PathRoot {
    /// Returns the `ModelId`, panicking if this root is a `Variant` root.
    pub fn expect_model(&self) -> ModelId {
        match self {
            PathRoot::Model(id) => *id,
            PathRoot::Variant { .. } => panic!("expected Model root, got Variant root"),
        }
    }

    /// Returns the `ModelId` if this is a `Model` root, or `None` for a
    /// `Variant` root.
    pub fn as_model(&self) -> Option<ModelId> {
        match self {
            PathRoot::Model(id) => Some(*id),
            PathRoot::Variant { .. } => None,
        }
    }
}

/// Describes a traversal through fields.
///
/// The root is not specified as part of the struct.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    /// Where the path originates from
    pub root: PathRoot,

    /// Traversal through the fields
    pub projection: Projection,
}

impl Path {
    pub fn model(root: impl Into<ModelId>) -> Self {
        Self {
            root: PathRoot::Model(root.into()),
            projection: Projection::identity(),
        }
    }

    pub fn field(root: impl Into<ModelId>, field: usize) -> Self {
        Self {
            root: PathRoot::Model(root.into()),
            projection: Projection::single(field),
        }
    }

    pub const fn from_index(root: ModelId, index: usize) -> Self {
        Self {
            root: PathRoot::Model(root),
            projection: Projection::from_index(index),
        }
    }

    /// Creates a path rooted at a specific enum variant.
    ///
    /// `parent` is the path that navigates to the enum field. Subsequent
    /// projection steps (appended via [`chain`][Path::chain]) index into the
    /// variant's fields using 0-based local indices.
    pub fn from_variant(parent: Path, variant_id: VariantId) -> Self {
        Self {
            root: PathRoot::Variant {
                parent: Box::new(parent),
                variant_id,
            },
            projection: Projection::identity(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.projection.is_empty()
    }

    pub fn len(&self) -> usize {
        self.projection.len()
    }

    pub fn chain(&mut self, other: &Self) {
        for field in &other.projection[..] {
            self.projection.push(*field);
        }
    }

    pub fn into_stmt(self) -> Expr {
        match self.root {
            PathRoot::Model(model_id) => match self.projection.as_slice() {
                [] => Expr::ref_ancestor_model(0),
                [field, project @ ..] => {
                    let mut ret = Expr::ref_self_field(FieldId {
                        model: model_id,
                        index: *field,
                    });

                    if !project.is_empty() {
                        ret = Expr::project(ret, project);
                    }

                    ret
                }
            },
            PathRoot::Variant { parent, .. } => {
                let parent_expr = parent.into_stmt();
                match self.projection.as_slice() {
                    [] => parent_expr,
                    [local_idx, rest @ ..] => {
                        // Record position 0 is the discriminant; variant fields
                        // start at position 1, so add 1 to the local field index.
                        let mut ret =
                            Expr::project(parent_expr, Projection::single(local_idx + 1));

                        if !rest.is_empty() {
                            ret = Expr::project(ret, rest);
                        }

                        ret
                    }
                }
            }
        }
    }
}
