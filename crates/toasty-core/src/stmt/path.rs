use crate::schema::app;

use super::*;

/// Describes a traversal through fields.
///
/// The root is not specified as part of the struct.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path {
    /// Model the path originates from
    pub root: ModelRef,

    /// Traversal through the fields
    pub projection: Projection,
}

impl Path {
    pub fn model(root: impl Into<ModelRef>) -> Self {
        Self {
            root: root.into(),
            projection: Projection::identity(),
        }
    }

    pub fn field(root: impl Into<ModelRef>, field: usize) -> Self {
        Self {
            root: root.into(),
            projection: Projection::single(field),
        }
    }

    pub fn from_index(root: impl Into<ModelRef>, index: usize) -> Self {
        Self {
            root: root.into(),
            projection: Projection::from_index(index),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.projection.is_empty()
    }

    pub fn len(&self) -> usize {
        self.projection.len()
    }

    /// Resolve the ModelRef to ModelId using the provided schema
    pub fn resolve(&mut self, schema: &app::Schema) -> Result<()> {
        self.root.resolve(schema)
    }

    pub fn resolve_field<'a>(&self, schema: &'a app::Schema) -> &'a Field {
        let model_id = self.root.model_id(); // Will panic if not resolved
        let expr_self = schema.model(model_id);
        self.projection.resolve_field(schema, expr_self)
    }

    pub fn chain(&mut self, other: &Self) {
        for field in &other.projection[..] {
            self.projection.push(*field);
        }
    }

    pub fn into_stmt(self) -> Expr {
        let model_id = self.root.model_id(); // Will panic if not resolved
        match self.projection.as_slice() {
            [] => Expr::key(model_id),
            [field, project @ ..] => {
                let mut ret = Expr::field(FieldId {
                    model: model_id,
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
