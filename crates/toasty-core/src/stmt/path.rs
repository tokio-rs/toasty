use super::{Expr, Projection, Step};
use crate::schema::app::{FieldId, ModelId};

/// Describes a traversal through fields.
///
/// The root is not specified as part of the struct.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    /// Model the path originates from
    pub root: ModelId,

    /// Traversal through the fields
    pub projection: Projection,
}

impl Path {
    pub fn model(root: impl Into<ModelId>) -> Self {
        Self {
            root: root.into(),
            projection: Projection::identity(),
        }
    }

    pub fn field(root: impl Into<ModelId>, field: usize) -> Self {
        Self {
            root: root.into(),
            projection: Projection::field(field),
        }
    }

    pub const fn from_index(root: ModelId, index: usize) -> Self {
        Self {
            root,
            projection: Projection::field(index),
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
        match self.projection.as_slice() {
            [] => Expr::ref_ancestor_model(0),
            [Step::Field(field), project @ ..] => {
                let mut ret = Expr::ref_self_field(FieldId {
                    model: self.root,
                    index: *field,
                });

                if !project.is_empty() {
                    ret = Expr::project(ret, project);
                }

                ret
            }
            _ => panic!("invalid path"),
        }
    }
}
