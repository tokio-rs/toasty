use crate::schema::app;

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
    pub fn model(root: impl Into<ModelId>) -> Self {
        Self {
            root: root.into(),
            projection: Projection::identity(),
        }
    }

    pub fn field(root: impl Into<ModelId>, field: usize) -> Self {
        Self {
            root: root.into(),
            projection: Projection::single(field),
        }
    }

    pub const fn from_index(root: ModelId, index: usize) -> Self {
        Self {
            root,
            projection: Projection::from_index(index),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.projection.is_empty()
    }

    pub fn len(&self) -> usize {
        self.projection.len()
    }

    pub fn resolve_field<'a>(&self, schema: &'a app::Schema) -> &'a Field {
        let expr_self = schema.model(self.root);
        self.projection.resolve_field(schema, expr_self)
    }

    pub fn chain(&mut self, other: &Self) {
        for field in &other.projection[..] {
            self.projection.push(*field);
        }
    }

    pub fn into_stmt(self) -> Expr {
        match self.projection.as_slice() {
            [] => Expr::key(self.root),
            [field, project @ ..] => {
                let mut ret = Expr::field(FieldId {
                    model: self.root,
                    index: *field,
                });

                if !project.is_empty() {
                    ret = Expr::project(ret, project);
                }

                ret
            }
        }
    }
}
