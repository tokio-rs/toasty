use super::{InsertTarget, Node, Query, Returning, Statement, Visit, VisitMut};
use crate::stmt;

/// An `INSERT` statement that creates new records.
///
/// Combines an [`InsertTarget`] (where to insert), a [`Query`] source
/// (the values to insert), and an optional [`Returning`] clause.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Insert, InsertTarget, Query, Values, Expr};
/// use toasty_core::schema::app::ModelId;
///
/// let insert = Insert {
///     target: InsertTarget::Model(ModelId(0)),
///     source: Query::values(Values::new(vec![Expr::null()])),
///     returning: None,
/// };
/// assert!(insert.target.is_model());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Insert {
    /// The target to insert into (model, table, or scoped query).
    pub target: InsertTarget,

    /// The source query providing values to insert.
    pub source: Query,

    /// Optional `RETURNING` clause to return data from the insertion.
    pub returning: Option<Returning>,
}

impl Insert {
    /// Merges another `Insert` into this one by appending its value rows.
    ///
    /// Both inserts must target the same model, and both sources must be
    /// `VALUES` expressions.
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
    /// Returns `true` if this statement is an [`Insert`].
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
    pub fn into_insert_unwrap(self) -> Insert {
        match self {
            Self::Insert(insert) => insert,
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
