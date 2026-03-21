use super::{Assignments, Node, Query, Returning, Statement, Visit, VisitMut};
use crate::{
    schema::{app::ModelId, db::TableId},
    stmt::{self, Condition, Filter},
};

/// An `UPDATE` statement that modifies existing records.
///
/// Combines a target (what to update), assignments (how to change fields),
/// a filter (which records to update), an optional condition (a guard that
/// must hold for the update to apply), and an optional returning clause.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Update, UpdateTarget, Assignments, Filter, Condition};
/// use toasty_core::schema::app::ModelId;
///
/// let update = Update {
///     target: UpdateTarget::Model(ModelId(0)),
///     assignments: Assignments::default(),
///     filter: Filter::default(),
///     condition: Condition::default(),
///     returning: None,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// The target to update (model, table, or query).
    pub target: UpdateTarget,

    /// The field assignments to apply.
    pub assignments: Assignments,

    /// Filter selecting which records to update (`WHERE` clause).
    pub filter: Filter,

    /// An optional condition that must be satisfied for the update to apply.
    /// Unlike `filter`, a condition failing does not produce an error but
    /// silently skips the update.
    pub condition: Condition,

    /// Optional `RETURNING` clause.
    pub returning: Option<Returning>,
}

impl Statement {
    pub fn is_update(&self) -> bool {
        matches!(self, Self::Update(_))
    }
}

/// The target of an [`Update`] statement.
///
/// Specifies what entity is being updated. At the model level this is a model
/// ID or a scoped query. After lowering, it becomes a table ID.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::UpdateTarget;
/// use toasty_core::schema::app::ModelId;
///
/// let target = UpdateTarget::Model(ModelId(0));
/// assert_eq!(target.model_id(), Some(ModelId(0)));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateTarget {
    /// Update records returned by a query. The query must select a model.
    Query(Box<Query>),

    /// Update a model by its ID.
    Model(ModelId),

    /// Update a database table (lowered form).
    Table(TableId),
}

impl Update {
    /// Returns a [`Query`] that selects the records this update would modify.
    pub fn selection(&self) -> Query {
        stmt::Query::new_select(self.target.model_id_unwrap(), self.filter.clone())
    }
}

impl UpdateTarget {
    /// Returns the model ID for this target, if applicable.
    pub fn model_id(&self) -> Option<ModelId> {
        match self {
            Self::Model(model_id) => Some(*model_id),
            Self::Query(query) => query
                .body
                .as_select()
                .and_then(|select| select.source.model_id()),
            _ => None,
        }
    }

    /// Returns the model ID for this target.
    ///
    /// # Panics
    ///
    /// Panics if this is a `Table` variant.
    #[track_caller]
    pub fn model_id_unwrap(&self) -> ModelId {
        match self {
            Self::Model(model_id) => *model_id,
            Self::Query(query) => query.body.as_select_unwrap().source.model_id_unwrap(),
            _ => todo!("not a model"),
        }
    }

    /// Returns `true` if this target is a `Table` variant.
    pub fn is_table(&self) -> bool {
        matches!(self, UpdateTarget::Table(..))
    }

    /// Creates a `Table` target from a table ID.
    pub fn table(table: impl Into<TableId>) -> Self {
        Self::Table(table.into())
    }

    /// Returns the table ID if this is a `Table` variant.
    pub fn as_table(&self) -> Option<TableId> {
        match self {
            Self::Table(table) => Some(*table),
            _ => None,
        }
    }

    /// Returns the table ID, panicking if this is not a `Table` variant.
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Table` variant.
    #[track_caller]
    pub fn as_table_unwrap(&self) -> TableId {
        self.as_table()
            .unwrap_or_else(|| panic!("expected UpdateTarget::Table; actual={self:#?}"))
    }
}

impl From<Update> for Statement {
    fn from(src: Update) -> Self {
        Self::Update(src)
    }
}

impl Node for Update {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_update(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_update_mut(self);
    }
}
