use crate::stmt::ExprArg;

use super::{Expr, Projection};

/// Projects a field or element from a base expression.
///
/// A [projection] extracts a nested value from a record, tuple, or other
/// composite type using a path of field indices.
///
/// # Examples
///
/// ```text
/// project(record, [0])     // extracts the first field
/// project(record, [1, 2])  // extracts field 1, then field 2
/// ```
///
/// [projection]: https://en.wikipedia.org/wiki/Projection_(relational_algebra)
#[derive(Debug, Clone, PartialEq)]
pub struct ExprProject {
    /// The expression to project from.
    pub base: Box<Expr>,

    /// The path specifying which field(s) to extract.
    pub projection: Projection,
}

impl Expr {
    pub fn project(base: impl Into<Self>, projection: impl Into<Projection>) -> Self {
        ExprProject {
            base: Box::new(base.into()),
            projection: projection.into(),
        }
        .into()
    }

    pub fn arg_project(expr_arg: impl Into<ExprArg>, projection: impl Into<Projection>) -> Self {
        Self::project(Self::arg(expr_arg), projection)
    }

    pub fn is_project(&self) -> bool {
        matches!(self, Self::Project(..))
    }

    pub fn as_project(&self) -> &ExprProject {
        match self {
            Self::Project(expr_project) => expr_project,
            _ => panic!(),
        }
    }
}

impl ExprProject {}

impl From<ExprProject> for Expr {
    fn from(value: ExprProject) -> Self {
        Self::Project(value)
    }
}
