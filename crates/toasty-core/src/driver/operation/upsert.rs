use super::{Operation, TypedValue};
use crate::stmt;

/// Executes a lowered single-row upsert on a non-SQL database driver.
///
/// The query engine emits this operation only after verifying the requested
/// target and branch behavior against [`Capability`](crate::driver::Capability).
/// [`stmt`](Self::stmt) contains one values row and an
/// [`stmt::Upsert`](crate::stmt::Upsert) clause whose target has been lowered
/// from model fields to database columns.
///
/// A driver must perform the conflict check and the create, update, or ignore
/// action atomically. It must not implement this operation as a read followed
/// by a separate insert or update. An update action returns the stored row. An
/// ignore action returns one row after an insert and zero rows after the
/// selected target conflicts.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::operation::{Operation, Upsert};
///
/// let op = Upsert {
///     stmt: lowered_insert,
///     params: typed_params,
///     ret: Some(return_types),
/// };
/// let operation: Operation = op.into();
/// ```
#[derive(Debug, Clone)]
pub struct Upsert {
    /// The lowered insert statement carrying the conflict target and action.
    ///
    /// Literal bind values are replaced with `Expr::Arg(n)`, where `n` indexes
    /// [`params`](Self::params). The statement's `upsert` field is always
    /// `Some`, and its target is [`UpsertTarget::Columns`](crate::stmt::UpsertTarget::Columns).
    pub stmt: stmt::Insert,

    /// Typed bind parameters extracted from [`stmt`](Self::stmt).
    pub params: Vec<TypedValue>,

    /// Types of the columns returned by the operation, in projection order.
    ///
    /// `Some(types)` requires the driver to return the stored row projected to
    /// these types. `None` requires no returned row.
    pub ret: Option<Vec<stmt::Type>>,
}

impl From<Upsert> for Operation {
    fn from(value: Upsert) -> Self {
        Self::Upsert(value)
    }
}
