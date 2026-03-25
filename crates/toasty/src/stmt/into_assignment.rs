use super::{IntoExpr, List};
use std::marker::PhantomData;
use toasty_core::stmt::{self, AssignmentOp, Assignments, Projection};

/// Convert a value into one or more field assignments on an update builder.
///
/// This trait unifies all update mutations for collection (has-many) fields.
/// Update builder setters for has-many fields accept `impl IntoAssignment<List<T>>`
/// so the caller must be explicit about the operation:
///
/// ```ignore
/// user.update()
///     .todos(stmt::insert(Todo::create().title("Buy groceries")))
///     .exec(&mut db)
///     .await?;
/// ```
///
/// Arrays of assignments implement `IntoAssignment<T>` to combine multiple
/// operations in a single call:
///
/// ```ignore
/// user.update()
///     .todos([
///         stmt::insert(Todo::create().title("New task")),
///         stmt::remove(&old_todo),
///     ])
///     .exec(&mut db)
///     .await?;
/// ```
pub trait IntoAssignment<T> {
    /// Apply this assignment to the given [`Assignments`] map at `projection`.
    fn into_assignment(self, assignments: &mut Assignments, projection: Projection);
}

/// A typed assignment operation returned by the [`insert`], [`remove`], and
/// [`set`] combinator functions.
///
/// Implements [`IntoAssignment<T>`] so it can be passed directly to any update
/// builder setter that accepts assignments.
pub struct Assignment<T> {
    op: AssignmentOp,
    expr: stmt::Expr,
    _p: PhantomData<T>,
}

impl<T> IntoAssignment<T> for Assignment<T> {
    fn into_assignment(self, assignments: &mut Assignments, projection: Projection) {
        match self.op {
            AssignmentOp::Set => assignments.set(projection, self.expr),
            AssignmentOp::Insert => assignments.insert(projection, self.expr),
            AssignmentOp::Remove => assignments.remove(projection, self.expr),
        }
    }
}

/// Arrays of assignments implement `IntoAssignment<T>` by applying each element
/// in order. This enables combining multiple operations on a single field:
///
/// ```ignore
/// user.update()
///     .todos([
///         stmt::insert(Todo::create().title("Buy groceries")),
///         stmt::insert(Todo::create().title("Walk the dog")),
///         stmt::remove(&old_todo),
///     ])
///     .exec(&mut db)
///     .await?;
/// ```
impl<T, const N: usize> IntoAssignment<T> for [Assignment<T>; N] {
    fn into_assignment(self, assignments: &mut Assignments, projection: Projection) {
        for assignment in self {
            assignment.into_assignment(assignments, projection.clone());
        }
    }
}

impl<T> IntoAssignment<T> for Vec<Assignment<T>> {
    fn into_assignment(self, assignments: &mut Assignments, projection: Projection) {
        for assignment in self {
            assignment.into_assignment(assignments, projection.clone());
        }
    }
}

/// Insert a value into a collection field.
///
/// Takes an expression of `T` (a single item) and produces an assignment for
/// `List<T>` (a collection). Use this with has-many relation setters on update
/// builders:
///
/// ```ignore
/// user.update()
///     .todos(stmt::insert(Todo::create().title("Buy groceries")))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn insert<T>(expr: impl IntoExpr<T>) -> Assignment<List<T>> {
    Assignment {
        op: AssignmentOp::Insert,
        expr: expr.into_expr().untyped,
        _p: PhantomData,
    }
}

/// Remove a value from a collection field.
///
/// Takes an expression of `T` (the item to remove) and produces an assignment
/// for `List<T>` (a mutation on the collection).
///
/// What "remove" means depends on the belongs-to side of the relationship:
///
/// - **Optional foreign key** (`user_id: Option<Id>`): The foreign key is set
///   to `NULL`. The record continues to exist.
/// - **Required foreign key** (`user_id: Id`): The record is deleted.
///
/// ```ignore
/// user.update()
///     .todos(stmt::remove(&todo_a))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn remove<T>(expr: impl IntoExpr<T>) -> Assignment<List<T>> {
    Assignment {
        op: AssignmentOp::Remove,
        expr: expr.into_expr().untyped,
        _p: PhantomData,
    }
}

/// Replace a field's value entirely.
///
/// For collection fields, this disassociates all current items and associates
/// the new set. Pass an empty collection to clear the field:
///
/// ```ignore
/// user.update()
///     .todos(stmt::set([]))
///     .exec(&mut db)
///     .await?;
/// ```
///
/// For scalar fields, `stmt::set` is equivalent to passing the value directly
/// (the default behavior is already "set").
pub fn set<T>(expr: impl IntoExpr<T>) -> Assignment<T> {
    Assignment {
        op: AssignmentOp::Set,
        expr: expr.into_expr().untyped,
        _p: PhantomData,
    }
}
