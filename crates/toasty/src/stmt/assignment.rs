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
    /// Convert into an [`Assignment<T>`] value.
    fn into_assignment(self) -> Assignment<T>;

    /// Record one or more assignments into the given [`Assignments`] map at
    /// the specified projection.
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection)
    where
        Self: Sized,
    {
        self.into_assignment().kind.apply(assignments, projection);
    }
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

enum AssignmentKind {
    Set(stmt::Expr),
    Insert(stmt::Expr),
    Remove(stmt::Expr),
    Append(stmt::Expr),
    Patch {
        path_projection: stmt::Projection,
        inner: Box<AssignmentKind>,
    },
    Apply(Vec<AssignmentKind>),
}

impl AssignmentKind {
    fn apply(self, assignments: &mut stmt::Assignments, projection: stmt::Projection) {
        match self {
            AssignmentKind::Set(expr) => assignments.set(projection, expr),
            AssignmentKind::Insert(expr) => assignments.insert(projection, expr),
            AssignmentKind::Remove(expr) => assignments.remove(projection, expr),
            AssignmentKind::Append(expr) => assignments.append(projection, expr),
            AssignmentKind::Patch {
                path_projection,
                inner,
            } => {
                let mut projection = projection;
                for &step in path_projection.as_slice() {
                    projection.push(step);
                }
                inner.apply(assignments, projection);
            }
            AssignmentKind::Apply(ops) => {
                for op in ops {
                    op.apply(assignments, projection.clone());
                }
            }
        }
    }
}

// Assignment<T> implements Assign<T>
impl<T> Assign<T> for Assignment<T> {
    fn into_assignment(self) -> Assignment<T> {
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
            fn into_assignment(self) -> super::assignment::Assignment<$target> {
                $crate::stmt::set(
                    super::IntoExpr::<$target>::into_expr(self),
                )
            }
        }
    };
    // Generic: impl<generics> Assign<Target> for Source where bounds
    // Uses { } instead of [ ] to avoid parsing ambiguity with array types.
    ({ $($gen:tt)* } $source:ty => $target:ty) => {
        impl<$($gen)*> super::assignment::Assign<$target> for $source {
            fn into_assignment(self) -> super::assignment::Assignment<$target> {
                $crate::stmt::set(
                    super::IntoExpr::<$target>::into_expr(self),
                )
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

/// Append one element to an ordered collection field (e.g. `Vec<scalar>`).
///
/// Takes an expression of `T` (the element to append) and produces an
/// assignment for `List<T>` (the collection). The append is atomic
/// against the existing column value on every supported backend.
///
/// After `.exec()`, the instance's field reflects the post-update value
/// (old contents followed by the appended element).
///
/// # Examples
///
/// ```ignore
/// user.update()
///     .tags(stmt::push("admin"))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn push<T>(expr: impl IntoExpr<T>) -> Assignment<List<T>> {
    let element = expr.into_expr().untyped;
    Assignment {
        kind: AssignmentKind::Append(stmt::Expr::list([element])),
        _p: PhantomData,
    }
}

/// Append every element of a list to an ordered collection field.
///
/// Takes a list-shaped expression (anything that converts to `List<T>`
/// — `Vec<T>`, `[T; N]`, `&[T]`, …) and produces an assignment for
/// `List<T>`. Elements are appended in order and the operation is
/// atomic against the existing column value, same as [`push`].
///
/// After `.exec()`, the instance's field reflects the post-update value.
/// `stmt::extend(iter)` of an empty iterator is a no-op.
///
/// # Examples
///
/// ```ignore
/// user.update()
///     .tags(stmt::extend(["admin", "verified"]))
///     .exec(&mut db)
///     .await?;
/// ```
pub fn extend<T>(items: impl IntoExpr<List<T>>) -> Assignment<List<T>> {
    Assignment {
        kind: AssignmentKind::Append(items.into_expr().untyped),
        _p: PhantomData,
    }
}

/// Remove every element from an ordered collection field.
///
/// Produces an assignment for `List<T>` that replaces the column with an
/// empty list. Equivalent to passing an empty Vec to the field setter,
/// just more explicit at the call site.
///
/// # Examples
///
/// ```ignore
/// user.update()
///     .tags(stmt::clear())
///     .exec(&mut db)
///     .await?;
/// ```
pub fn clear<T>() -> Assignment<List<T>> {
    Assignment {
        kind: AssignmentKind::Set(stmt::Expr::list(Vec::<stmt::Expr>::new())),
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
    let inner = value.into_assignment();

    Assignment {
        kind: AssignmentKind::Patch {
            path_projection: path.untyped.projection,
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
