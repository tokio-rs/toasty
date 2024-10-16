use super::*;

use std::{fmt, ops};

/// Describes a traversal through fields.
///
/// The root is not specified as part of the struct.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Path {
    /// Model the path originates from
    pub root: ModelId,

    /// Traversal through the fields
    pub projection: Projection,
}

impl Path {
    pub fn field(root: impl Into<ModelId>, field: PathStep) -> Path {
        Path {
            root: root.into(),
            projection: Projection::single(field),
        }
    }

    pub const fn from_index(root: ModelId, index: usize) -> Path {
        Path {
            root,
            projection: Projection::from_index(index),
        }
    }

    pub fn len(&self) -> usize {
        self.projection.len()
    }

    pub fn chain(&mut self, other: &Path) {
        for field in &other[..] {
            self.projection.push(*field);
        }
    }

    pub fn into_self_project_expr<'stmt>(self) -> Expr<'stmt> {
        todo!("delete this")
    }
}

impl ops::Deref for Path {
    type Target = [PathStep];

    fn deref(&self) -> &Self::Target {
        self.projection.deref()
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("Path");

        f.finish()
    }
}
