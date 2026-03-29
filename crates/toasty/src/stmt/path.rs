use super::{Expr, IntoExpr, IntoStatement, List};
use crate::schema::Register;
use std::{fmt, marker::PhantomData};
use toasty_core::{
    schema::app::VariantId,
    stmt::{self, Direction, OrderByExpr},
};

/// A typed path from a root model `T` to a field of type `U`.
///
/// `Path` represents a traversal through a model's fields and relations. The
/// type parameter `T` is the root model the path starts from, and `U` is the
/// type of the value at the end of the path.
///
/// Paths are the primary way to reference model fields in queries. Generated
/// code provides accessor methods (e.g., `User::fields().name()`) that return
/// paths, which you then use with comparison methods to build filter
/// expressions:
///
/// ```
/// # #[derive(Debug, toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// # }
/// // Path<User, String> — the "name" field on User
/// let path = User::fields().name();
///
/// // Expr<bool> — a filter expression
/// let filter = path.eq("Alice");
/// ```
///
/// Paths can also be used to construct order-by clauses via
/// [`asc`](Path::asc) and [`desc`](Path::desc), and can be chained to
/// navigate through relations with [`chain`](Path::chain).
pub struct Path<T, U> {
    pub(super) untyped: stmt::Path,
    _p: PhantomData<(T, U)>,
}

impl<T: Register> Path<T, T> {
    /// Create a path that points to the root model itself.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::Path;
    ///
    /// let root = Path::<User, User>::root();
    /// ```
    pub fn root() -> Self {
        Self {
            untyped: stmt::Path::model(T::id()),
            _p: PhantomData,
        }
    }
}

impl<M: Register> Path<List<M>, List<M>> {
    /// Create an identity path for a list of model `M`.
    ///
    /// This is the list counterpart of [`Path::root`] — it produces a
    /// `Path<List<M>, List<M>>` rooted at the model's identity.
    pub fn from_model_list() -> Self {
        Self {
            untyped: stmt::Path::model(M::id()),
            _p: PhantomData,
        }
    }
}

impl<T, U> Path<T, U> {
    /// Create a path to the field at `index` on model `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::Path;
    ///
    /// // Path to the second field (name, index 1)
    /// let path = Path::<User, String>::from_field_index(1);
    /// ```
    pub fn from_field_index(index: usize) -> Self
    where
        T: Register,
    {
        Self {
            untyped: stmt::Path::from_index(T::id(), index),
            _p: PhantomData,
        }
    }

    /// Converts this path into a variant-rooted path for use in `.matches()`
    /// closures on embedded enum fields.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// # use toasty::stmt::Path;
    /// # use toasty_core::schema::app::{ModelId, VariantId};
    /// let path = Path::<User, String>::from_field_index(1);
    /// let variant_id = VariantId { model: ModelId(0), index: 0 };
    /// let _variant_path = path.into_variant(variant_id);
    /// ```
    pub fn into_variant(self, variant_id: VariantId) -> Self {
        Self {
            untyped: stmt::Path::from_variant(self.untyped, variant_id),
            _p: PhantomData,
        }
    }

    /// Append `other` to this path, producing a new path from `T` to `V`.
    ///
    /// The origin of `other` is left unconstrained because list field structs
    /// store `Path<Origin, List<M>>` while chaining segments rooted at `M`
    /// (not `List<M>`). The untyped path concatenation is always correct; the
    /// generic parameters serve only as compile-time markers.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::Path;
    ///
    /// let user_path = Path::<User, User>::root();
    /// let name_path = Path::<User, String>::from_field_index(1);
    /// let _chained: Path<User, String> = user_path.chain(name_path);
    /// ```
    pub fn chain<X, V>(mut self, other: impl Into<Path<X, V>>) -> Path<T, V> {
        let other = other.into();
        self.untyped.chain(&other.untyped);

        Path {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }

    /// Test whether this field equals `rhs`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let filter = User::fields().name().eq("Alice");
    /// ```
    pub fn eq(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::eq(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    /// Test whether this field does not equal `rhs`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let filter = User::fields().name().ne("Alice");
    /// ```
    pub fn ne(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::ne(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    /// Test whether this field is greater than `rhs`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let filter = User::fields().id().gt(10);
    /// ```
    pub fn gt(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::gt(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    /// Test whether this field is greater than or equal to `rhs`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let filter = User::fields().id().ge(1);
    /// ```
    pub fn ge(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::ge(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    /// Test whether this field is less than `rhs`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let filter = User::fields().id().lt(100);
    /// ```
    pub fn lt(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::lt(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    /// Test whether this field is less than or equal to `rhs`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let filter = User::fields().id().le(100);
    /// ```
    pub fn le(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::le(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    /// Test whether this field's value is in `rhs`.
    ///
    /// `rhs` can be any collection that implements `IntoExpr<List<U>>`, such
    /// as a `Vec`, array, or slice.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let filter = User::fields().id().in_list([1_i64, 2, 3]);
    /// ```
    pub fn in_list(self, rhs: impl IntoExpr<List<U>>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::in_list(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    /// Test whether this field's value appears in the result set of a
    /// subquery.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Path, Query};
    ///
    /// // A path targeting User values
    /// let path = Path::<User, User>::root();
    /// // A subquery returning List<User>
    /// let subquery = Query::<List<User>>::filter(User::fields().name().eq("Alice"));
    /// let filter = path.in_query(subquery);
    /// ```
    pub fn in_query<Q>(self, rhs: Q) -> Expr<bool>
    where
        Q: IntoStatement<Returning = List<U>>,
    {
        let query = rhs.into_statement().into_untyped_query();
        Expr {
            untyped: stmt::Expr::in_subquery(self.untyped.into_stmt(), query),
            _p: PhantomData,
        }
    }

    /// Produce an ascending [`OrderByExpr`] for this path.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let mut q = User::all();
    /// q.order_by(User::fields().name().asc());
    /// ```
    pub fn asc(self) -> OrderByExpr {
        OrderByExpr {
            expr: self.untyped.into_stmt(),
            order: Some(Direction::Asc),
        }
    }

    /// Produce a descending [`OrderByExpr`] for this path.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let mut q = User::all();
    /// q.order_by(User::fields().name().desc());
    /// ```
    pub fn desc(self) -> OrderByExpr {
        OrderByExpr {
            expr: self.untyped.into_stmt(),
            order: Some(Direction::Desc),
        }
    }
}

impl<T, U> Path<T, List<U>> {
    /// Build an `IN subquery` expression that tests whether **any** associated
    /// record satisfies `filter`.
    ///
    /// The path must point to a `HasMany` (or similar collection) field on the
    /// parent model. The returned expression can be used as a filter on the
    /// parent query.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// # #[derive(Debug, toasty::Model)]
    /// # struct Todo {
    /// #     #[key]
    /// #     id: i64,
    /// #     title: String,
    /// # }
    /// use toasty::stmt::{Path, List};
    ///
    /// // Find users that have at least one todo with "urgent" in the title
    /// let todos_path = Path::<User, List<Todo>>::from_field_index(2);
    /// let filter = todos_path.any(Todo::fields().title().eq("urgent"));
    /// ```
    pub fn any(self, filter: Expr<bool>) -> Expr<bool>
    where
        U: crate::schema::Model,
    {
        // Build a query on the child model filtered by `filter`
        let child_query = super::Query::<List<U>>::filter(filter);

        Expr {
            untyped: stmt::Expr::in_subquery(self.untyped.into_stmt(), child_query.untyped),
            _p: PhantomData,
        }
    }
}

impl<T, U> Path<T, Option<U>> {
    /// Test whether this optional field is `NULL`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// #     bio: Option<String>,
    /// # }
    /// let filter = User::fields().bio().is_none();
    /// ```
    pub fn is_none(self) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::is_null(self.untyped.into_stmt()),
            _p: PhantomData,
        }
    }

    /// Test whether this optional field is not `NULL`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// #     bio: Option<String>,
    /// # }
    /// let filter = User::fields().bio().is_some();
    /// ```
    pub fn is_some(self) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::is_not_null(self.untyped.into_stmt()),
            _p: PhantomData,
        }
    }
}

impl<T, U> Clone for Path<T, U> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<T, U> IntoExpr<U> for Path<T, U> {
    fn into_expr(self) -> Expr<U> {
        Expr {
            untyped: self.untyped.into_stmt(),
            _p: PhantomData,
        }
    }

    fn by_ref(&self) -> Expr<U> {
        Self::into_expr(self.clone())
    }
}

impl<T, U> From<Path<T, U>> for stmt::Path {
    fn from(value: Path<T, U>) -> Self {
        value.untyped
    }
}

impl<T, U> fmt::Debug for Path<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.untyped)
    }
}
