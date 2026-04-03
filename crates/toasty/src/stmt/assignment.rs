use super::{IntoExpr, List, Path};
use std::marker::PhantomData;
use toasty_core::stmt;

/// Apply a field mutation to an update statement's [`Assignments`] map.
///
/// Every update builder setter accepts `impl Assign<T>`. All types that
/// implement [`IntoExpr<T>`] also implement `Assign<T>` with set semantics,
/// so update setters accept the same types as create setters.
///
/// For has-many fields, arrays and [`Vec`]s implement
/// `Assign<List<T>>` with set (replace) semantics. Use [`insert`],
/// [`remove`], or [`apply`] for incremental mutations.
///
/// [`Assignments`]: toasty_core::stmt::Assignments
pub trait Assign<T> {
    /// Record one or more assignments into the given [`Assignments`] map at
    /// the specified projection.
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection);

    /// Convert this value into an [`Assignment<T>`].
    fn to_assignment(self) -> Assignment<T>;
}

/// A typed assignment produced by the [`insert`], [`remove`], [`set`], and
/// [`patch`] combinators.
///
/// `Assignment<T>` implements `Assign<T>`, so it can be passed directly
/// to any update builder setter that accepts `impl Assign<T>`.
pub struct Assignment<T> {
    kind: AssignmentKind,
    _p: PhantomData<T>,
}

impl<T> Assignment<T> {
    /// Create an `Assignment` with set semantics from a raw expression.
    ///
    /// This is used internally by `impl_assign_via_expr!` where the source and
    /// target types may differ (e.g. `T` → `Option<T>`).
    pub(super) fn new_set(expr: stmt::Expr) -> Self {
        Assignment {
            kind: AssignmentKind::Set(expr),
            _p: PhantomData,
        }
    }
}

enum AssignmentKind {
    Set(stmt::Expr),
    Insert(stmt::Expr),
    Remove(stmt::Expr),
    Patch {
        path: stmt::Projection,
        inner: Box<AssignmentKind>,
    },
    Apply(Vec<AssignmentKind>),
}

fn apply_kind(
    kind: AssignmentKind,
    assignments: &mut stmt::Assignments,
    projection: stmt::Projection,
) {
    match kind {
        AssignmentKind::Set(expr) => assignments.set(projection, expr),
        AssignmentKind::Insert(expr) => assignments.insert(projection, expr),
        AssignmentKind::Remove(expr) => assignments.remove(projection, expr),
        AssignmentKind::Patch { path, inner } => {
            let mut full_projection = projection;
            for &step in path.as_slice() {
                full_projection.push(step);
            }
            apply_kind(*inner, assignments, full_projection);
        }
        AssignmentKind::Apply(ops) => {
            for kind in ops {
                apply_kind(kind, assignments, projection.clone());
            }
        }
    }
}

// Assignment<T> implements Assign<T>
impl<T> Assign<T> for Assignment<T> {
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection) {
        apply_kind(self.kind, assignments, projection);
    }

    fn to_assignment(self) -> Assignment<T> {
        self
    }
}

/// Helper macro: generates `impl Assign<$target> for $source` with set
/// semantics by delegating to `IntoExpr`. Used alongside every `IntoExpr`
/// impl to keep the two traits in sync.
macro_rules! impl_assign_via_expr {
    // Simple: impl Assign<T> for S
    ($source:ty => $target:ty) => {
        impl super::assignment::Assign<$target> for $source {
            fn assign(self, assignments: &mut toasty_core::stmt::Assignments, projection: toasty_core::stmt::Projection) {
                assignments.set(projection, super::IntoExpr::<$target>::into_expr(self).untyped);
            }

            fn to_assignment(self) -> super::assignment::Assignment<$target> {
                super::assignment::Assignment::new_set(super::IntoExpr::<$target>::into_expr(self).untyped)
            }
        }
    };
    // Generic: impl<generics> Assign<Target> for Source where bounds
    // Uses { } instead of [ ] to avoid parsing ambiguity with array types.
    ({ $($gen:tt)* } $source:ty => $target:ty) => {
        impl<$($gen)*> super::assignment::Assign<$target> for $source {
            fn assign(self, assignments: &mut toasty_core::stmt::Assignments, projection: toasty_core::stmt::Projection) {
                assignments.set(projection, super::IntoExpr::<$target>::into_expr(self).untyped);
            }

            fn to_assignment(self) -> super::assignment::Assignment<$target> {
                super::assignment::Assignment::new_set(super::IntoExpr::<$target>::into_expr(self).untyped)
            }
        }
    };
}

// Make the macro available to into_expr.rs (sibling module)
pub(super) use impl_assign_via_expr;

/// Insert a value into a collection field.
///
/// Takes an expression of `T` (a single item) and produces an assignment for
/// `List<T>` (the collection). The returned [`Assignment`] can be passed to any
/// update builder setter that accepts `impl Assign<List<T>>`.
///
/// [`Assignments`]: toasty_core::stmt::Assignments
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
    Assignment {
        kind: AssignmentKind::Insert(expr.into_expr().untyped),
        _p: PhantomData,
    }
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
    Assignment {
        kind: AssignmentKind::Remove(expr.into_expr().untyped),
        _p: PhantomData,
    }
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
    Assignment {
        kind: AssignmentKind::Set(expr.into_expr().untyped),
        _p: PhantomData,
    }
}

/// Partially update a sub-field of an embedded type.
///
/// Takes a [`Path<T, U>`] (identifying which sub-field to update) and a
/// value (`impl Assign<U>` — either a plain value or a nested
/// [`Assignment<U>`] for deeper patching). Returns an [`Assignment<T>`]
/// that can be passed to the parent field's setter.
///
/// # Examples
///
/// ```ignore
/// // Update a single sub-field
/// user.update()
///     .critter(stmt::patch(Creature::fields().human().profession(), "doctor"))
///     .exec(&mut db)
///     .await?;
///
/// // Nested patching
/// user.update()
///     .kind(stmt::patch(
///         Kind::variants().admin().perm(),
///         stmt::patch(Permission::fields().everything(), true),
///     ))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn patch<T, U>(path: Path<T, U>, value: impl Assign<U>) -> Assignment<T> {
    let inner = value.to_assignment();
    Assignment {
        kind: AssignmentKind::Patch {
            path: path.untyped.projection,
            inner: Box::new(inner.kind),
        },
        _p: PhantomData,
    }
}

/// Apply multiple operations to a single field.
///
/// Takes an array or [`Vec`] of [`Assignment<T>`] and applies each in order.
///
/// # Examples
///
/// ```ignore
/// user.update()
///     .todos(stmt::apply([
///         stmt::insert(Todo::create().title("Buy groceries")),
///         stmt::insert(Todo::create().title("Walk the dog")),
///         stmt::remove(&old_todo),
///     ]))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn apply<T>(ops: impl IntoIterator<Item = Assignment<T>>) -> Assignment<T> {
    let ops: Vec<AssignmentKind> = ops.into_iter().map(|a| a.kind).collect();
    Assignment {
        kind: AssignmentKind::Apply(ops),
        _p: PhantomData,
    }
}
