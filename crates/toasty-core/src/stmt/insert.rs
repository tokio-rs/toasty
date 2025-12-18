use super::{InsertTarget, Node, Query, Returning, Statement, Visit, VisitMut};
use crate::stmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Insert {
    /// Where to insert the values
    pub target: InsertTarget,

    /// Source of values to insert
    pub source: Query,

    /// Optionally return data from the insertion
    pub returning: Option<Returning>,
}

impl Insert {
    pub fn merge(&mut self, other: Self) {
        match (&self.target, &other.target) {
            (InsertTarget::Model(a), InsertTarget::Model(b)) if a == b => {}
            _ => todo!("handle this case"),
        }

        match (&mut self.source.body, other.source.body) {
            (stmt::ExprSet::Values(self_values), stmt::ExprSet::Values(other_values)) => {
                for expr in other_values.rows {
                    self_values.rows.push(expr);
                }
            }
            (self_source, other) => todo!("self={:#?}; other={:#?}", self_source, other),
        }
    }
}

impl Statement {
    pub fn is_insert(&self) -> bool {
        matches!(self, Statement::Insert(..))
    }

    /// Attempts to return a reference to an inner [`Insert`].
    ///
    /// * If `self` is a [`Statement::Insert`], a reference to the inner [`Insert`] is
    ///   returned wrapped in [`Some`].
    /// * Else, [`None`] is returned.
    pub fn as_insert(&self) -> Option<&Insert> {
        match self {
            Self::Insert(insert) => Some(insert),
            _ => None,
        }
    }

    /// Consumes `self` and attempts to return the inner [`Insert`].
    ///
    /// * If `self` is a [`Statement::Insert`], inner [`Insert`] is returned wrapped in
    ///   [`Some`].
    /// * Else, [`None`] is returned.
    pub fn into_insert(self) -> Option<Insert> {
        match self {
            Self::Insert(insert) => Some(insert),
            _ => None,
        }
    }

    /// Consumes `self` and returns the inner [`Insert`].
    ///
    /// # Panics
    ///
    /// If `self` is not a [`Statement::Insert`].
    pub fn unwrap_insert(self) -> Insert {
        match self {
            Self::Insert(insert) => insert,
            v => panic!("expected `Insert`, found {v:#?}"),
        }
    }

    /// Attempts to return a reference to the insert statement's source query.
    ///
    /// Returns `None` if the statement is not an [`Statement::Insert`].
    pub fn insert_source(&self) -> Option<&Query> {
        match self {
            Statement::Insert(insert) => Some(&insert.source),
            _ => None,
        }
    }

    /// Returns a reference to the insert statement's source query.
    ///
    /// # Panics
    ///
    /// Panics if the statement is not a [`Statement::Insert`].
    #[track_caller]
    pub fn insert_source_unwrap(&self) -> &Query {
        match self {
            Statement::Insert(insert) => &insert.source,
            v => panic!("expected `Insert`, found {v:#?}"),
        }
    }
}

impl From<Insert> for Statement {
    fn from(src: Insert) -> Self {
        Self::Insert(src)
    }
}

impl Node for Insert {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_insert(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_insert_mut(self);
    }
}
