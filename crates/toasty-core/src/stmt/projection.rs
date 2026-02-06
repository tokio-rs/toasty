use crate::{
    schema::{
        app::{self, Field, FieldId, Model},
        db::ColumnId,
    },
    stmt::{Expr, Value},
};

use std::{fmt, ops};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Projection {
    steps: Steps,
}

pub trait Project {
    fn project(self, projection: &Projection) -> Option<Expr>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Steps {
    /// References the projection base
    Identity,

    /// One field step
    Single([usize; 1]),

    /// Multi field steps
    Multi(Vec<usize>),
}

pub struct Iter<'a>(std::slice::Iter<'a, usize>);

impl Projection {
    pub const fn identity() -> Self {
        Self {
            steps: Steps::Identity,
        }
    }

    /// The path references the root (i.e. the projection base)
    pub const fn is_identity(&self) -> bool {
        matches!(self.steps, Steps::Identity)
    }

    pub fn single(step: usize) -> Self {
        Self {
            steps: Steps::Single([step]),
        }
    }

    /// Mostly here for `const`
    pub const fn from_index(index: usize) -> Self {
        Self {
            steps: Steps::Single([index]),
        }
    }

    pub fn as_slice(&self) -> &[usize] {
        self.steps.as_slice()
    }

    pub fn push(&mut self, step: usize) {
        match &mut self.steps {
            Steps::Identity => {
                self.steps = Steps::Single([step]);
            }
            Steps::Single([first]) => {
                self.steps = Steps::Multi(vec![*first, step]);
            }
            Steps::Multi(steps) => {
                steps.push(step);
            }
        }
    }

    pub fn resolve_field<'a>(&self, schema: &'a app::Schema, expr_self: &'a Model) -> &'a Field {
        self.steps.resolve_field(schema, expr_self)
    }

    pub fn resolves_to(&self, other: impl Into<Self>) -> bool {
        let other = other.into();
        *self == other
    }
}

impl ops::Deref for Projection {
    type Target = [usize];

    fn deref(&self) -> &Self::Target {
        self.steps.as_slice()
    }
}

impl ops::DerefMut for Projection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.steps {
            Steps::Identity => &mut [],
            Steps::Single(step) => &mut step[..],
            Steps::Multi(steps) => &mut steps[..],
        }
    }
}

impl<'a> IntoIterator for &'a Projection {
    type Item = usize;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self[..].iter())
    }
}

impl From<&Field> for Projection {
    fn from(value: &Field) -> Self {
        Self::single(value.id.index)
    }
}

impl From<FieldId> for Projection {
    fn from(value: FieldId) -> Self {
        Self::single(value.index)
    }
}

impl From<ColumnId> for Projection {
    fn from(value: ColumnId) -> Self {
        Self::single(value.index)
    }
}

impl From<usize> for Projection {
    fn from(value: usize) -> Self {
        Self::single(value)
    }
}

impl From<&[usize]> for Projection {
    fn from(value: &[usize]) -> Self {
        match value {
            [] => Self::identity(),
            [value] => Self::single(*value),
            value => Self {
                steps: Steps::Multi(value.into()),
            },
        }
    }
}

impl<const N: usize> From<[usize; N]> for Projection {
    fn from(value: [usize; N]) -> Self {
        Self::from(&value[..])
    }
}

impl fmt::Debug for Projection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("Projection");

        if self.is_identity() {
            f.field(&"identity");
        } else {
            for field in &self[..] {
                f.field(&field);
            }
        }

        f.finish()
    }
}

impl Steps {
    fn as_slice(&self) -> &[usize] {
        match self {
            Self::Identity => &[],
            Self::Single(step) => &step[..],
            Self::Multi(steps) => &steps[..],
        }
    }

    fn resolve_field<'a>(&self, schema: &'a app::Schema, expr_self: &'a Model) -> &'a Field {
        use crate::schema::app::FieldTy::*;

        let [first, rest @ ..] = self.as_slice() else {
            panic!("need at most one path step")
        };
        let mut projected = &expr_self.fields[*first];

        for step in rest {
            let target = match &projected.ty {
                Primitive(..) => panic!("failed to resolve path"),
                Embedded(_) => {
                    // TODO: Handle path projection through embedded fields
                    todo!("embedded field path projection")
                }
                BelongsTo(belongs_to) => belongs_to.target(schema),
                HasMany(has_many) => has_many.target(schema),
                HasOne(_) => todo!(),
            };

            projected = &target.fields[*step];
        }

        projected
    }
}

impl Iterator for Iter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        self.0.next().copied()
    }
}

impl Project for Expr {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection)?.to_expr())
    }
}

impl Project for &Expr {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection)?.to_expr())
    }
}

impl Project for &&Expr {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection)?.to_expr())
    }
}

impl Project for Value {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection).to_expr())
    }
}

impl Project for &Value {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection).to_expr())
    }
}

impl Project for &&Value {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection).to_expr())
    }
}
