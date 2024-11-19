use super::*;

use std::{fmt, ops};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Projection {
    steps: Steps,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Steps {
    /// References the projection base
    Identity,

    /// One field step
    Single([PathStep; 1]),

    /// Multi field steps
    Multi(Vec<PathStep>),
}

pub struct Iter<'a>(std::slice::Iter<'a, PathStep>);

impl Projection {
    pub const fn identity() -> Projection {
        Projection {
            steps: Steps::Identity,
        }
    }

    /// The path references the root (i.e. the projection base)
    pub const fn is_identity(&self) -> bool {
        matches!(self.steps, Steps::Identity)
    }

    pub fn single(step: impl Into<PathStep>) -> Projection {
        Projection {
            steps: Steps::Single([step.into()]),
        }
    }

    /// Mostly here for `const`
    pub const fn from_index(index: usize) -> Projection {
        Projection {
            steps: Steps::Single([PathStep::from_usize(index)]),
        }
    }

    pub fn as_slice(&self) -> &[PathStep] {
        self.steps.as_slice()
    }

    pub fn push(&mut self, step: impl Into<PathStep>) {
        let step = step.into();

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

    pub fn resolve_field<'a>(&self, schema: &'a Schema, expr_self: &'a Model) -> &'a Field {
        self.steps.resolve_field(schema, expr_self)
    }

    pub fn resolve_value<'a, 'stmt>(&self, expr_self: &'a Value) -> &'a Value {
        let mut ret = expr_self;

        for step in self.as_slice() {
            match ret {
                Value::Record(record) => ret = &record[step.into_usize()],
                Value::Enum(value_enum) => {
                    assert_eq!(value_enum.variant, step.into_usize());

                    ret = match &value_enum.fields[..] {
                        [] => todo!("expr_self={:#?}; projection={:#?}", expr_self, self),
                        [field] => field,
                        [..] => todo!(
                            "in theory the path should also reference a field... but it does not"
                        ),
                    };
                }
                _ => todo!("ret={:#?}", ret),
            }
        }

        ret
    }

    pub fn resolve_expr<'a, 'stmt>(&self, base: &'a Expr) -> &'a Expr {
        let mut ret = base;

        for step in self.as_slice() {
            match ret {
                Expr::Record(expr) => ret = &expr[step.into_usize()],
                _ => todo!("ret={ret:#?}; base={base:#?}"),
            }
        }

        ret
    }

    pub fn resolves_to(&self, field: impl Into<PathStep>) -> bool {
        let field = field.into();
        let [step] = &self[..] else { return false };
        *step == field
    }
}

impl ops::Deref for Projection {
    type Target = [PathStep];

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
    type Item = PathStep;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self[..].iter())
    }
}

impl From<&Field> for Projection {
    fn from(value: &Field) -> Self {
        Projection::single(value)
    }
}

impl<T, I> From<T> for Projection
where
    T: IntoIterator<Item = I>,
    I: Into<PathStep>,
{
    fn from(value: T) -> Projection {
        let mut projection = Projection::identity();

        for step in value {
            projection.push(step);
        }

        projection
    }
}

impl fmt::Debug for Projection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("Projection");

        if self.is_identity() {
            f.field(&"identity");
        } else {
            for field in &self[..] {
                f.field(&field.into_usize());
            }
        }

        f.finish()
    }
}

impl Steps {
    fn as_slice(&self) -> &[PathStep] {
        match self {
            Steps::Identity => &[],
            Steps::Single(step) => &step[..],
            Steps::Multi(steps) => &steps[..],
        }
    }

    fn resolve_field<'a>(&self, schema: &'a Schema, expr_self: &'a Model) -> &'a Field {
        use crate::schema::FieldTy::*;

        let [first, rest @ ..] = self.as_slice() else {
            panic!("need at most one path step")
        };
        let mut projected = &expr_self.fields[first.into_usize()];

        for step in rest {
            let target = match &projected.ty {
                Primitive(..) => panic!("failed to resolve path"),
                BelongsTo(belongs_to) => belongs_to.target(schema),
                HasMany(has_many) => has_many.target(schema),
                HasOne(_) => todo!(),
            };

            projected = &target.fields[step.into_usize()];
        }

        projected
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = PathStep;

    fn next(&mut self) -> Option<PathStep> {
        self.0.next().copied()
    }
}
