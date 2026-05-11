use super::{Expr, IntoExpr, IntoStatement, List};
use crate::schema::{Field, Register};
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

    /// Build a filter `Expr<bool>` from this path, automatically wrapping
    /// the body with an `is_variant(parent, variant_id)` AND-gate when the
    /// path is variant-rooted.
    ///
    /// All boolean-producing methods on `Path` (`eq`, `ne`, `gt`, `is_none`,
    /// `starts_with`, `any`, …) funnel through this so that filter-context
    /// uses of a variant-rooted path implicitly require the variant to
    /// match. Path-yielding contexts (`include`, `order_by`, `chain`)
    /// bypass this helper and keep the bare path.
    fn build_filter<F>(self, build_body: F) -> Expr<bool>
    where
        F: FnOnce(stmt::Expr) -> stmt::Expr,
    {
        let gate = match &self.untyped.root {
            stmt::PathRoot::Variant { parent, variant_id } => {
                let parent_stmt = parent.as_ref().clone().into_stmt();
                Some(stmt::Expr::is_variant(parent_stmt, *variant_id))
            }
            _ => None,
        };
        let body = build_body(self.untyped.into_stmt());
        let untyped = match gate {
            Some(g) => stmt::Expr::and(g, body),
            None => body,
        };
        Expr {
            untyped,
            _p: PhantomData,
        }
    }

    /// Test whether this field equals `rhs`.
    ///
    /// For a variant-rooted path (e.g. `contact().email().address()`), the
    /// resulting filter implicitly requires the variant to match — it
    /// expands to `is_email(contact) AND address == rhs`.
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
        let rhs = rhs.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::eq(path, rhs))
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
        let rhs = rhs.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::ne(path, rhs))
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
        let rhs = rhs.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::gt(path, rhs))
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
        let rhs = rhs.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::ge(path, rhs))
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
        let rhs = rhs.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::lt(path, rhs))
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
        let rhs = rhs.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::le(path, rhs))
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
        let rhs = rhs.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::in_list(path, rhs))
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
        self.build_filter(move |path| stmt::Expr::in_subquery(path, query))
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
        self.build_filter(move |path| stmt::Expr::in_subquery(path, child_query.untyped))
    }

    /// Build a `NOT IN subquery` expression that tests whether **all** associated
    /// records satisfy `filter`.
    ///
    /// The path must point to a `HasMany` (or similar collection) field on the
    /// parent model. Returns `true` when every associated record matches
    /// `filter`, including the vacuous case where the parent has no associated
    /// records (matching Rust's `[].iter().all()` semantics).
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
    /// #     complete: bool,
    /// # }
    /// use toasty::stmt::{Path, List};
    ///
    /// // Find users whose todos are all complete
    /// let todos_path = Path::<User, List<Todo>>::from_field_index(2);
    /// let filter = todos_path.all(Todo::fields().complete().eq(true));
    /// ```
    pub fn all(self, filter: Expr<bool>) -> Expr<bool>
    where
        U: crate::schema::Model,
    {
        // parent NOT IN (SELECT child_fk FROM child WHERE NOT filter)
        let child_query = super::Query::<List<U>>::filter(filter.not());
        self.build_filter(move |path| {
            stmt::Expr::not(stmt::Expr::in_subquery(path, child_query.untyped))
        })
    }
}

/// Container-style predicates on a `Vec<scalar>` model field. The path target
/// is the [`List<U>`] marker (matching `Field::Path<Origin>` for `Vec<U>`),
/// and the element type `U` is constrained to a path-target scalar so the
/// same `IntoExpr` infrastructure that powers `eq`/`in_list` covers the
/// right-hand side.
impl<T, U> Path<T, List<U>>
where
    U: crate::schema::Scalar,
{
    /// Test whether the array contains `value`.
    ///
    /// Mirrors [`Vec::contains`]. Lowers to `value = ANY(col)` on PostgreSQL.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let filter = User::fields().tags().contains("admin");
    /// ```
    pub fn contains(self, value: impl IntoExpr<U>) -> Expr<bool> {
        let value = value.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::any_op(value, stmt::BinaryOp::Eq, path))
    }

    /// Test whether the array contains every element of `values`.
    ///
    /// Mirrors [`HashSet::is_superset`](std::collections::HashSet::is_superset).
    /// Lowers to `col @> values` (PostgreSQL `@>` operator).
    pub fn is_superset(self, values: impl IntoExpr<List<U>>) -> Expr<bool> {
        let values = values.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::array_is_superset(path, values))
    }

    /// Test whether the array shares at least one element with `values`.
    ///
    /// Negation of [`HashSet::is_disjoint`](std::collections::HashSet::is_disjoint).
    /// Lowers to `col && values` (PostgreSQL `&&` operator).
    pub fn intersects(self, values: impl IntoExpr<List<U>>) -> Expr<bool> {
        let values = values.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::array_intersects(path, values))
    }

    /// Returns the array length.
    ///
    /// Mirrors [`Vec::len`]. Lowers to `cardinality(col)` on PostgreSQL.
    pub fn len(self) -> Expr<i64> {
        Expr::from_untyped(stmt::Expr::array_length(self.untyped.into_stmt()))
    }

    /// Returns `true` if the array is empty.
    ///
    /// Equivalent to `.len().eq(0)`. Mirrors [`Vec::is_empty`].
    pub fn is_empty(self) -> Expr<bool> {
        let untyped = stmt::Expr::eq(
            stmt::Expr::array_length(self.untyped.into_stmt()),
            stmt::Expr::Value(stmt::Value::I64(0)),
        );
        Expr::from_untyped(untyped)
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
        self.build_filter(stmt::Expr::is_null)
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
        self.build_filter(stmt::Expr::is_not_null)
    }
}

impl<T, U> Path<T, U>
where
    U: Field<Inner = String>,
{
    /// Test whether this string field starts with `prefix`.
    ///
    /// Available on any string-valued field, including `String`,
    /// `Option<String>`, and other wrappers whose `Field::Inner` is `String`.
    /// For DynamoDB, this maps to `begins_with` in a `KeyConditionExpression`
    /// (sort key) or `FilterExpression` (non-key attribute).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// #     nickname: Option<String>,
    /// # }
    /// let filter = User::fields().name().starts_with("Al".to_string());
    /// let filter = User::fields().nickname().starts_with("Al".to_string());
    /// ```
    pub fn starts_with(self, prefix: impl IntoExpr<String>) -> Expr<bool> {
        let prefix = prefix.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::starts_with(path, prefix))
    }

    /// Test whether this string field matches a SQL `LIKE` pattern.
    ///
    /// Available on any string-valued field, including `String`,
    /// `Option<String>`, and other wrappers whose `Field::Inner` is `String`.
    /// The caller is responsible for including any `%` or `_` wildcard
    /// characters in `pattern`. Not supported by the DynamoDB driver — use
    /// [`starts_with`](Self::starts_with) instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// #     nickname: Option<String>,
    /// # }
    /// let filter = User::fields().name().like("Al%".to_string());
    /// let filter = User::fields().nickname().like("Al%".to_string());
    /// ```
    pub fn like(self, pattern: impl IntoExpr<String>) -> Expr<bool> {
        let pattern = pattern.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::like(path, pattern))
    }

    /// Case-insensitive variant of [`like`](Self::like).
    ///
    /// On PostgreSQL this serializes to `ILIKE`. On SQLite and MySQL it
    /// serializes to plain `LIKE`, since both engines are already
    /// case-insensitive for ASCII by default — note that Unicode case-folding
    /// behavior depends on locale (PostgreSQL) or column collation (MySQL),
    /// and SQLite's `LIKE` is ASCII-only without the ICU extension. Not
    /// supported by the DynamoDB driver.
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
    /// // Matches "Alice", "ALICIA", and "alfred".
    /// let filter = User::fields().name().ilike("al%".to_string());
    /// ```
    pub fn ilike(self, pattern: impl IntoExpr<String>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::ilike(self.untyped.into_stmt(), pattern.into_expr().untyped),
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
