use super::{
    Assignments, InsertTarget, Node, Projection, Query, Returning, Statement, Visit, VisitMut,
};
use crate::schema::db::ColumnId;
use crate::stmt;

/// An `INSERT` statement that creates new records.
///
/// Combines an [`InsertTarget`] (where to insert), a [`Query`] source
/// (the values to insert), optional [`Upsert`] conflict handling, and an
/// optional [`Returning`] clause.
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
///     upsert: None,
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

    /// Optional conflict handling that turns this insert into an upsert.
    pub upsert: Option<Box<Upsert>>,

    /// Optional `RETURNING` clause to return data from the insertion.
    pub returning: Option<Returning>,
}

/// Conflict handling attached to an [`Insert`].
///
/// The target selects one primary-key or unique-constraint conflict. `Update`
/// applies the normalized [`shared`](Self::shared) assignments to the matching
/// row, while `Ignore` leaves it unchanged.
///
/// Before normalization, [`shared`](Self::shared),
/// [`defaults`](Self::defaults), [`update_defaults`](Self::update_defaults),
/// [`create`](Self::create), and [`update`](Self::update) contain the
/// declarative assignments. The engine first routes `update_defaults` to any
/// branch without an explicit assignment. Normalization then writes the create
/// branch into the insert source, overlays the update branch onto `shared`, and
/// clears `create` and `update`. Defaults remain available to non-SQL drivers
/// and are cleared before SQL serialization. The engine also stores model-field
/// targets before lowering and database-column targets afterward. SQL drivers
/// receive the normalized, lowered form inside
/// [`Operation::QuerySql`](crate::driver::Operation::QuerySql); non-SQL drivers
/// receive it inside [`Operation::Upsert`](crate::driver::Operation::Upsert).
#[derive(Debug, Clone, PartialEq)]
pub struct Upsert {
    /// The unique constraint that selects the conflicting row.
    pub target: UpsertTarget,

    /// Assignments applied to both the create and update branches.
    ///
    /// Normalization derives create values from these assignments and retains
    /// the assignments for conflict updates.
    pub shared: Assignments,

    /// Values declared with `#[default]` on model fields.
    ///
    /// These supply omitted create fields and initialize shared mutations.
    /// Explicit create assignments override them.
    pub defaults: Assignments,

    /// Values declared with `#[update]` on model fields.
    ///
    /// Before verification, the engine routes each value to the create branch,
    /// update branch, or both according to which branches already have an
    /// explicit assignment.
    pub update_defaults: Assignments,

    /// Assignments applied only when the insert creates a record.
    ///
    /// Explicit `on_create` assignments replace defaults and shared
    /// assignments for the same field.
    pub create: Assignments,

    /// Assignments applied only when the target matches an existing record.
    ///
    /// These override shared assignments for the same field and may reference
    /// stored columns or fields projected from
    /// [`ExprIncoming`](super::ExprIncoming), the row proposed by the insert source.
    pub update: Assignments,

    /// Whether to update or ignore a conflicting row.
    pub action: UpsertAction,
}

/// The fields or columns that identify the selected upsert conflict.
#[derive(Debug, Clone, PartialEq)]
pub enum UpsertTarget {
    /// Model-field projections used before engine lowering.
    Fields(Vec<Projection>),

    /// Database columns sent to the driver after engine lowering.
    Columns(Vec<ColumnId>),
}

/// Action to take when an upsert finds an existing row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertAction {
    /// Update the conflicting row.
    Update,

    /// Leave the conflicting row unchanged and return no row.
    Ignore,
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

    /// Returns `true` if this statement is an [`Insert`] with an upsert action.
    pub fn is_upsert(&self) -> bool {
        matches!(self, Statement::Insert(insert) if insert.upsert.is_some())
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
