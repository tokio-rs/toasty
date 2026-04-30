use super::{Expr, Node, Path, Visit, VisitMut};

/// An association preload entry on a `Returning::Model` clause.
///
/// Pairs the path to a relation with an optional predicate that restricts
/// which related rows load. A bare `Path` converts in via `From` with no
/// filter, so existing call sites that pass a path keep working unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct Include {
    /// The relation path to preload.
    pub path: Path,

    /// Optional predicate, evaluated in the relation target's scope.
    /// `None` means "load all matching rows".
    pub filter: Option<Expr>,
}

impl Include {
    /// Creates an `Include` with no filter.
    pub fn new(path: Path) -> Self {
        Self { path, filter: None }
    }

    /// Creates an `Include` with the given filter.
    pub fn with_filter(path: Path, filter: Expr) -> Self {
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
