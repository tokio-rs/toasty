use super::{Node, Path, Query, Visit, VisitMut};

/// An association preload entry on a `Returning::Model` clause.
///
/// The filter is stored as a bare `SELECT` over the relation target rather
/// than an `Expr` so the predicate's scope travels with it (the same
/// mechanism `Path::any` relies on); lowering extracts the `WHERE` clause
/// and ignores everything else.
#[derive(Debug, Clone, PartialEq)]
pub struct Include {
    /// The relation path to preload.
    pub path: Path,

    /// Optional filter restricting which related rows load.
    pub filter: Option<Query>,
}

impl Include {
    /// Creates an `Include` with no filter.
    pub fn new(path: Path) -> Self {
        Self { path, filter: None }
    }

    /// Creates an `Include` with the given filter query.
    pub fn with_filter(path: Path, filter: Query) -> Self {
        Self {
            path,
            filter: Some(filter),
        }
    }
}

impl From<Path> for Include {
    fn from(path: Path) -> Self {
        Self::new(path)
    }
}

impl Node for Include {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_include(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_include_mut(self);
    }
}
