use super::{IntoExpr, List};
use std::marker::PhantomData;
use toasty_core::stmt;

/// Convert a value into a field assignment for an update statement.
///
/// This trait is used for field types where the mutation semantics are
/// ambiguous from a plain value alone — primarily **has-many** collection
/// fields. For these fields, callers must use an explicit combinator
/// ([`insert`], [`remove`], or [`set`]) to specify the intent.
///
/// Scalar fields continue to accept `impl IntoExpr<T>` directly (a plain
/// value means "set"). Only collection setters use `IntoAssignment`.
///
/// Arrays and [`Vec`]s of assignments implement `IntoAssignment<T>` when their
/// elements do, allowing multiple operations in a single setter call:
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
pub trait IntoAssignment<T> {
    /// Record one or more assignments into the given [`Assignments`] map at
    /// the specified projection.
    fn into_assignment(self, assignments: &mut stmt::Assignments, projection: stmt::Projection);
}

/// A typed assignment produced by the [`insert`], [`remove`], and [`set`]
/// combinators.
///
/// `Assignment<T>` implements `IntoAssignment<T>`, so it can be passed directly
/// to any update builder setter that accepts `impl IntoAssignment<T>`.
pub struct Assignment<T> {
    kind: AssignmentKind,
    _p: PhantomData<T>,
}

enum AssignmentKind {
    Set(stmt::Expr),
    Insert(stmt::Expr),
    Remove(stmt::Expr),
}

impl<T> Assignment<T> {
    fn new_set(expr: stmt::Expr) -> Self {
        Self {
            kind: AssignmentKind::Set(expr),
            _p: PhantomData,
        }
    }

    fn new_insert(expr: stmt::Expr) -> Self {
        Self {
            kind: AssignmentKind::Insert(expr),
            _p: PhantomData,
        }
    }

    fn new_remove(expr: stmt::Expr) -> Self {
        Self {
            kind: AssignmentKind::Remove(expr),
            _p: PhantomData,
        }
    }
}

// Assignment<T> implements IntoAssignment<T>
impl<T> IntoAssignment<T> for Assignment<T> {
    fn into_assignment(self, assignments: &mut stmt::Assignments, projection: stmt::Projection) {
        match self.kind {
            AssignmentKind::Set(expr) => assignments.set(projection, expr),
            AssignmentKind::Insert(expr) => assignments.insert(projection, expr),
            AssignmentKind::Remove(expr) => assignments.remove(projection, expr),
        }
    }
}

// Arrays of assignments: [Q; N] implements IntoAssignment<T> when Q does.
impl<T, Q: IntoAssignment<T>, const N: usize> IntoAssignment<T> for [Q; N] {
    fn into_assignment(self, assignments: &mut stmt::Assignments, projection: stmt::Projection) {
        for item in self {
            item.into_assignment(assignments, projection.clone());
        }
    }
}

// Vec of assignments
impl<T, Q: IntoAssignment<T>> IntoAssignment<T> for Vec<Q> {
    fn into_assignment(self, assignments: &mut stmt::Assignments, projection: stmt::Projection) {
        for item in self {
            item.into_assignment(assignments, projection.clone());
        }
    }
}

/// Insert a value into a collection field.
///
/// Takes an expression of `T` (a single item) and produces an assignment for
/// `List<T>` (the collection). The returned [`Assignment`] can be passed to any
/// update builder setter that accepts `impl IntoAssignment<List<T>>`.
///
/// # Examples
///
/// ```ignore
/// user.update()
///     .todos(stmt::insert(Todo::create().title("Buy groceries")))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn insert<T>(expr: impl IntoExpr<T>) -> Assignment<List<T>> {
    Assignment::new_insert(expr.into_expr().untyped)
}

/// Remove a value from a collection field.
///
/// Takes an expression of `T` (the item to remove) and produces an assignment
/// for `List<T>` (the collection).
///
/// What "remove" means depends on the belongs-to side of the relationship:
/// - **Optional foreign key**: The foreign key is set to `NULL`.
/// - **Required foreign key**: The related record is deleted.
///
/// # Examples
///
/// ```ignore
/// user.update()
///     .todos(stmt::remove(&todo_a))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn remove<T>(expr: impl IntoExpr<T>) -> Assignment<List<T>> {
    Assignment::new_remove(expr.into_expr().untyped)
}

/// Replace a field's value entirely.
///
/// For collection fields, `set` replaces the entire collection: all current
/// members are disassociated (following the same optional/required foreign key
/// rules as [`remove`]), then the new set is associated.
///
/// Pass an empty slice to clear the collection:
///
/// ```ignore
/// user.update()
///     .todos(stmt::set::<List<Todo>>([]))
///     .exec(&mut db)
///     .await?;
/// ```
///
/// For scalar fields, `set` is equivalent to passing a plain value (the
/// setter already defaults to set semantics).
///
/// # Examples
///
/// ```ignore
/// // Replace all todos
/// user.update()
///     .todos(stmt::set([
///         Todo::create().title("Only todo"),
///     ]))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn set<T>(expr: impl IntoExpr<T>) -> Assignment<T> {
    Assignment::new_set(expr.into_expr().untyped)
}
