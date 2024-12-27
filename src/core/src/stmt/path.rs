use super::*;

/// Describes a traversal through fields.
///
/// The root is not specified as part of the struct.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path {
    /// Model the path originates from
    pub root: ModelId,

    /// Traversal through the fields
    pub projection: Projection,
}

impl Path {
    pub fn model(root: impl Into<ModelId>) -> Path {
        Path {
            root: root.into(),
            projection: Projection::identity(),
        }
    }

    pub fn field(root: impl Into<ModelId>, field: usize) -> Path {
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
        for field in &other.projection[..] {
            self.projection.push(*field);
        }
    }

    pub fn into_stmt(self) -> Expr {
        let [field, project @ ..] = self.projection.as_slice() else {
            todo!("path={self:#?}")
        };

        let mut ret = stmt::Expr::field(FieldId {
            model: self.root,
            index: *field,
        });

        if !project.is_empty() {
            ret = stmt::Expr::project(ret, project);
        }

        ret
    }
}
