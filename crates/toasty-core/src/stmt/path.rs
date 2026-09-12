use super::{Expr, PathStep, Projection};
use crate::schema::app::{FieldId, ModelId, VariantId};

/// A rooted traversal through application fields and enum variants.
///
/// Field indices following a variant selection are local to that variant.
/// Schema-aware lowering resolves these steps to record projections.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    /// The model where traversal starts.
    pub root: ModelId,
    steps: Steps,
}

// Keep single-field paths const-compatible for generated field accessors.
#[derive(Debug, Clone, PartialEq)]
enum Steps {
    Identity,
    Single([PathStep; 1]),
    Multi(Vec<PathStep>),
}

impl Path {
    /// Create a path to the model itself.
    pub fn model(root: impl Into<ModelId>) -> Self {
        Self {
            root: root.into(),
            steps: Steps::Identity,
        }
    }

    /// Create a path to one model field.
    pub fn field(root: impl Into<ModelId>, field: usize) -> Self {
        Self::from_index(root.into(), field)
    }

    /// Create a single-field path in a const context.
    pub const fn from_index(root: ModelId, index: usize) -> Self {
        Self {
            root,
            steps: Steps::Single([PathStep::Field(index)]),
        }
    }

    /// Select a variant of the enum reached by `parent`.
    pub fn from_variant(mut parent: Path, variant: VariantId) -> Self {
        parent.push(PathStep::Variant(variant));
        parent
    }

    /// Create a path from an ordered sequence of selections.
    pub fn from_steps(root: ModelId, steps: impl IntoIterator<Item = PathStep>) -> Self {
        let mut path = Self::model(root);
        for step in steps {
            path.push(step);
        }
        path
    }

    /// Return all selections in traversal order.
    pub fn steps(&self) -> &[PathStep] {
        match &self.steps {
            Steps::Identity => &[],
            Steps::Single(step) => step,
            Steps::Multi(steps) => steps,
        }
    }

    /// Return mutable selections in traversal order.
    pub fn steps_mut(&mut self) -> &mut [PathStep] {
        match &mut self.steps {
            Steps::Identity => &mut [],
            Steps::Single(step) => step,
            Steps::Multi(steps) => steps,
        }
    }

    /// Append a field or variant selection.
    pub fn push(&mut self, step: PathStep) {
        match &mut self.steps {
            Steps::Identity => self.steps = Steps::Single([step]),
            Steps::Single([first]) => self.steps = Steps::Multi(vec![*first, step]),
            Steps::Multi(steps) => steps.push(step),
        }
    }

    /// Return a projection when this path contains only field selections.
    pub fn field_projection(&self) -> Option<Projection> {
        let mut projection = Projection::identity();
        for step in self.steps() {
            let PathStep::Field(index) = step else {
                return None;
            };
            projection.push(*index);
        }
        Some(projection)
    }

    /// Append `other`'s selections to this path.
    pub fn chain(&mut self, other: &Self) {
        for step in other.steps() {
            self.push(*step);
        }
    }

    /// Build an expression, preserving variant selections for lowering.
    pub fn into_stmt(self) -> Expr {
        match self.steps() {
            [] => Expr::ref_ancestor_model(0),
            [PathStep::Field(index), rest @ ..] => {
                let base = Expr::ref_self_field(FieldId {
                    model: self.root,
                    index: *index,
                });
                Expr::path(base, rest.to_vec())
            }
            steps => Expr::path(Expr::ref_ancestor_model(0), steps.to_vec()),
        }
    }
}
