use super::{Path, Query};

/// A reference to an association traversal from a source query.
///
/// Used in [`SourceModel::via`](super::SourceModel) to indicate that a model
/// is reached by following a relation path from another query's results.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Association, Query, Path};
///
/// let assoc = Association {
///     source: Box::new(Query::unit()),
///     path: Path::from(vec![0]),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Association {
    /// The source query whose results are the starting point.
    pub source: Box<Query>,

    /// The field path from the source model to the target model.
    pub path: Path,
}
