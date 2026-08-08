use super::{Node, Path, Query, Visit, VisitMut};

/// An association preload entry on a `Returning::Model` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct Include {
    /// The relation path to preload.
    pub path: Path,

    /// Query applied to the related rows, if the include is constrained.
    pub query: Option<Query>,
}

impl Include {
    /// Creates an unfiltered `Include`.
    pub fn new(path: Path) -> Self {
        Self { path, query: None }
    }

    /// Creates an `Include` with a relation query.
    pub fn with_query(path: Path, query: Query) -> Self {
        Self {
            path,
            query: Some(query),
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
