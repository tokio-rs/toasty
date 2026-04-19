use super::{Direction, OrderBy, OrderByExpr};

use super::Path;

/// A field path used to order query results from newest to oldest.
///
/// `LatestBy` wraps a [`Path`] and converts into an [`OrderBy`] clause with
/// [`Direction::Desc`] automatically applied.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{LatestBy, Path};
///
/// let latest = LatestBy {
///     path: Path::from(vec![0]),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LatestBy {
    /// The field path to order by descending.
    pub path: Path,
}

impl From<LatestBy> for OrderBy {
    fn from(value: LatestBy) -> Self {
        OrderBy {
            exprs: vec![OrderByExpr {
                expr: value.path.into_stmt(),
                order: Some(Direction::Desc),
            }],
        }
    }
}
