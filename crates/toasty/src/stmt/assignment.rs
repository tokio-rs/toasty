use super::{IntoExpr, List, Path};
use std::marker::PhantomData;
use toasty_core::stmt;

/// Apply a field mutation to an update statement's [`Assignments`] map.
///
/// This trait unifies all update mutations behind a single bound. Every
/// update builder setter accepts `impl Assign<T>`.
///
/// All types that implement [`IntoExpr<T>`] also implement `Assign<T>`,
/// defaulting to set semantics. This means plain values work everywhere.
/// For collection fields, a single item (`impl IntoExpr<Todo>`) won't
/// satisfy `Assign<List<Todo>>` — callers must use an explicit combinator
/// ([`insert`], [`remove`]) to specify intent.
///
/// Arrays and [`Vec`]s of [`Assignment<T>`] implement `Assign<T>`,
/// allowing multiple operations in a single setter call:
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
///
/// [`Assignments`]: toasty_core::stmt::Assignments
pub trait Assign<T> {
    /// Record one or more assignments into the given [`Assignments`] map at
    /// the specified projection.
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection);
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

type PatchFn = Box<dyn FnOnce(&mut stmt::Assignments, stmt::Projection)>;

enum AssignmentKind {
    Set(stmt::Expr),
    Insert(stmt::Expr),
    Remove(stmt::Expr),
    Patch(PatchFn),
    Apply(PatchFn),
}

// Assignment<T> implements Assign<T>
impl<T> Assign<T> for Assignment<T> {
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection) {
        match self.kind {
            AssignmentKind::Set(expr) => assignments.set(projection, expr),
            AssignmentKind::Insert(expr) => assignments.insert(projection, expr),
            AssignmentKind::Remove(expr) => assignments.remove(projection, expr),
            AssignmentKind::Patch(f) => f(assignments, projection),
            AssignmentKind::Apply(f) => f(assignments, projection),
        }
    }
}

// Arrays of Assignment<T>
impl<T, const N: usize> Assign<T> for [Assignment<T>; N] {
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection) {
        for item in self {
            item.assign(assignments, projection.clone());
        }
    }
}

// Vec of Assignment<T>
impl<T> Assign<T> for Vec<Assignment<T>> {
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection) {
        for item in self {
            item.assign(assignments, projection.clone());
        }
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
        }
    };
    // Generic: impl<generics> Assign<Target> for Source where bounds
    // Uses { } instead of [ ] to avoid parsing ambiguity with array types.
    ({ $($gen:tt)* } $source:ty => $target:ty) => {
        impl<$($gen)*> super::assignment::Assign<$target> for $source {
            fn assign(self, assignments: &mut toasty_core::stmt::Assignments, projection: toasty_core::stmt::Projection) {
                assignments.set(projection, super::IntoExpr::<$target>::into_expr(self).untyped);
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
pub fn patch<T, U>(path: Path<T, U>, value: impl Assign<U> + 'static) -> Assignment<T> {
    let path_projection = path.untyped.projection;

    Assignment {
        kind: AssignmentKind::Patch(Box::new(move |assignments, mut projection| {
            // Append the path's field steps to the parent projection,
            // so the inner assignment lands at the correct nested key.
            for &step in path_projection.as_slice() {
                projection.push(step);
            }
            value.assign(assignments, projection);
        })),
        _p: PhantomData,
    }
}

/// Apply multiple operations to a single field.
///
/// Wraps an `impl Assign<T>` (typically an array or `Vec` of
/// [`Assignment<T>`]) into a single [`Assignment<T>`]. Each inner operation
/// is applied in order when the assignment is executed.
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
pub fn apply<T>(ops: impl Assign<T> + 'static) -> Assignment<T> {
    Assignment {
        kind: AssignmentKind::Apply(Box::new(move |assignments, projection| {
            ops.assign(assignments, projection);
        })),
        _p: PhantomData,
    }
}
