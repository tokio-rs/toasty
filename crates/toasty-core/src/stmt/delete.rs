use super::{Node, Query, Returning, Source, Statement, Visit, VisitMut};
use crate::stmt::{self, Filter};

/// A `DELETE` statement that removes existing records.
///
/// Specifies a source to delete from, a filter selecting which records to
/// delete, and an optional returning clause.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Delete, Source, Filter};
/// use toasty_core::schema::app::ModelId;
///
/// let delete = Delete {
///     from: Source::from(ModelId(0)),
///     filter: Filter::default(),
///     returning: None,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Delete {
    /// The source to delete from (`FROM` clause).
    pub from: Source,

    /// Filter selecting which records to delete (`WHERE` clause).
    pub filter: Filter,

    /// Optional `RETURNING` clause.
    pub returning: Option<Returning>,
}

impl Delete {
    /// Returns a [`Query`] that selects the records this delete would remove.
    pub fn selection(&self) -> Query {
        stmt::Query::new_select(self.from.model_id_unwrap(), self.filter.clone())
    }
}

impl Statement {
    /// Returns `true` if this statement is a [`Delete`].
    pub fn is_delete(&self) -> bool {
        matches!(self, Statement::Delete(..))
    }

    /// Attempts to return a reference to an inner [`Delete`].
    ///
    /// * If `self` is a [`Statement::Delete`], a reference to the inner [`Delete`] is
    ///   returned wrapped in [`Some`].
    /// * Else, [`None`] is returned.
    pub fn as_delete(&self) -> Option<&Delete> {
        match self {
            Self::Delete(delete) => Some(delete),
            _ => None,
        }
    }

    /// Consumes `self` and attempts to return the inner [`Delete`].
    ///
    /// * If `self` is a [`Statement::Delete`], inner [`Delete`] is returned wrapped in
    ///   [`Some`].
    /// * Else, [`None`] is returned.
    pub fn into_delete(self) -> Option<Delete> {
        match self {
            Self::Delete(delete) => Some(delete),
            _ => None,
        }
    }

    /// Consumes `self` and returns the inner [`Delete`].
    ///
    /// # Panics
    ///
    /// If `self` is not a [`Statement::Delete`].
    pub fn into_delete_unwrap(self) -> Delete {
        match self {
            Self::Delete(delete) => delete,
            v => panic!("expected `Delete`, found {v:#?}"),
        }
    }
}

impl From<Delete> for Statement {
    fn from(src: Delete) -> Self {
        Self::Delete(src)
    }
}

impl Node for Delete {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_delete(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_delete_mut(self);
    }
}
