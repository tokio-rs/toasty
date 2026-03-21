use super::{Expr, InsertTable, Query};
use crate::schema::app::ModelId;

/// The target of an [`Insert`](super::Insert) statement.
///
/// Specifies where new records should be inserted. At the model level this is
/// typically a model ID or a scoped query. After lowering, it becomes a table
/// with column mappings.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::InsertTarget;
/// use toasty_core::schema::app::ModelId;
///
/// let target = InsertTarget::Model(ModelId(0));
/// assert!(target.is_model());
/// assert!(!target.is_table());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum InsertTarget {
    /// Insert into a scoped query. The inserted value should satisfy the
    /// query's filter, which may set default field values or validate
    /// existing ones.
    Scope(Box<Query>),

    /// Insert a model by its ID.
    Model(ModelId),

    /// Insert into a database table (lowered form).
    Table(InsertTable),
}

impl InsertTarget {
    /// Returns `true` if this target is a `Model` variant.
    pub fn is_model(&self) -> bool {
        matches!(self, InsertTarget::Model(..))
    }

    /// Returns the model ID for this target.
    ///
    /// For `Scope` targets, extracts the model ID from the inner query's
    /// select source.
    ///
    /// # Panics
    ///
    /// Panics if this is a `Table` variant.
    #[track_caller]
    pub fn model_id_unwrap(&self) -> ModelId {
        match self {
            Self::Scope(query) => query.body.as_select_unwrap().source.model_id_unwrap(),
            Self::Model(model_id) => *model_id,
            _ => panic!("expected InsertTarget::Model; actual={self:#?}"),
        }
    }

    /// Returns `true` if this target is a `Table` variant.
    pub fn is_table(&self) -> bool {
        matches!(self, InsertTarget::Table(..))
    }

    /// Returns a reference to the inner [`InsertTable`] if this is a `Table`
    /// variant.
    pub fn as_table(&self) -> Option<&InsertTable> {
        match self {
            Self::Table(table) => Some(table),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`InsertTable`].
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Table` variant.
    #[track_caller]
    pub fn as_table_unwrap(&self) -> &InsertTable {
        self.as_table()
            .unwrap_or_else(|| panic!("expected InsertTarget::Table; actual={self:#?}"))
    }

    /// Adds a constraint expression to this target.
    ///
    /// For `Scope` targets, the expression is added as a filter. For `Model`
    /// targets, the target is upgraded to a `Scope` wrapping a select query
    /// with the constraint as a filter.
    pub fn add_constraint(&mut self, expr: impl Into<Expr>) {
        let expr = expr.into();
        match self {
            Self::Scope(query) => query.add_filter(expr),
            Self::Model(model_id) => {
                *self = Self::Scope(Box::new(Query::new_select(*model_id, expr)));
            }
            _ => todo!("{self:#?}"),
        }
    }
}

impl From<Query> for InsertTarget {
    fn from(value: Query) -> Self {
        Self::Scope(Box::new(value))
    }
}
