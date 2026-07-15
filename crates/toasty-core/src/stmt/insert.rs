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
    pub upsert: Option<Upsert>,

    /// Optional `RETURNING` clause to return data from the insertion.
    pub returning: Option<Returning>,
}

/// Conflict handling attached to an [`Insert`].
///
/// The target selects one primary-key or unique-constraint conflict. `Update`
/// applies [`assignments`](Self::assignments) to the matching row, while
/// `Ignore` leaves it unchanged. The insert source contains the values for the
/// create branch in both cases.
///
/// The engine stores model-field targets before lowering and database-column
/// targets afterward. Drivers receive only the lowered column form inside
/// [`Operation::Upsert`](crate::driver::Operation::Upsert).
#[derive(Debug, Clone, PartialEq)]
pub struct Upsert {
    /// The unique constraint that selects the conflicting row.
    pub target: UpsertTarget,

    /// Assignments applied when the target matches an existing row.
    ///
    /// These may reference stored columns and [`FuncIncoming`](super::FuncIncoming)
    /// values proposed by the insert source.
    pub assignments: Assignments,

    /// Create-only default assignments retained for key-value lowering.
    ///
    /// DynamoDB uses these to initialize required values with
    /// `if_not_exists` without overwriting an existing item's field.
    pub create_defaults: Assignments,

    /// Whether to update or ignore a conflicting row.
    pub action: UpsertAction,

    /// Whether the caller explicitly configured the create branch with
    /// `on_create`.
    ///
    /// The verifier checks this flag against the driver's branch-assignment
    /// capability.
    pub explicit_create: bool,

    /// Whether the caller explicitly configured the update branch with
    /// `on_update`.
    ///
    /// The verifier checks this flag against the driver's branch-assignment
    /// capability.
    pub explicit_update: bool,

    /// Shared assignments that cannot initialize the corresponding create
    /// field.
    ///
    /// Verification rejects these assignments and directs the caller to use
    /// separate create and update branches.
    pub invalid_shared_assignments: Vec<Projection>,
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
