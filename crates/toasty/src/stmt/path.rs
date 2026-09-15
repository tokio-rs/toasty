use super::{Expr, IntoExpr, IntoStatement, List};
use crate::schema::{Field, Model};
use std::{fmt, marker::PhantomData};
use toasty_core::{
    schema::app::{self, ModelId, VariantId},
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

impl<T, U> Path<T, U> {
    /// Create a path rooted at the model (or embedded type) identified by
    /// `model`.
    ///
    /// This is the low-level constructor behind the `path_root` and
    /// `path_model_list` helpers on the [`Model`](crate::schema::Model) and
    /// [`Embed`](crate::schema::Embed) traits, which supply the [`ModelId`]
    /// from their own `id()`.
    pub(crate) fn from_model_id(model: ModelId) -> Self {
        Self {
            untyped: stmt::Path::model(model),
            _p: PhantomData,
        }
    }

    /// Create a path to the field at `index` on the model (or embedded type)
    /// identified by `model`.
    ///
    /// Low-level constructor behind the `path_field` helpers on the
    /// [`Model`](crate::schema::Model) and [`Embed`](crate::schema::Embed)
    /// traits.
    pub(crate) fn field_at(model: ModelId, index: usize) -> Self {
        Self {
            untyped: stmt::Path::from_index(model, index),
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
    /// # use toasty::schema::Model;
    /// # use toasty_core::schema::app::{ModelId, VariantId};
    /// let path = User::path_field::<String>(1);
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
    /// use toasty::schema::Model;
    ///
    /// let user_path = User::path_root();
    /// let name_path = User::path_field::<String>(1);
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

    /// Build a filter `Expr<bool>` from this path's expression.
    ///
    /// The filter is the bare predicate. For a variant-rooted path, the
    /// engine's statement normalization adds the `is_variant` checks the
    /// predicate requires.
    fn build_filter<F>(self, build_body: F) -> Expr<bool>
    where
        F: FnOnce(stmt::Expr) -> stmt::Expr,
    {
        Expr::from_untyped(build_body(self.untyped.into_stmt()))
    }

    /// Test whether this field equals `rhs`.
    ///
    /// For a variant-rooted path (e.g. `contact().email().address()`), the
    /// resulting filter implicitly requires the variant to match — the
    /// engine expands it to `is_email(contact) AND address == rhs`.
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

    /// Test whether this field's value is in the inclusive range `[low, high]`.
    ///
    /// Generates a `BETWEEN low AND high` condition in SQL and a native
    /// `BETWEEN` condition expression in DynamoDB.
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
    /// let filter = User::fields().id().between(18_i64, 65_i64);
    /// ```
    pub fn between(self, low: impl IntoExpr<U>, high: impl IntoExpr<U>) -> Expr<bool> {
        let low = low.into_expr().untyped;
        let high = high.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::between(path, low, high))
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
    /// use toasty::stmt::{List, Query};
    /// use toasty::schema::Model;
    ///
    /// // A path targeting User values
    /// let path = User::path_root();
    /// // A subquery returning List<User>
    /// let subquery = Query::<List<User>>::all().filter(User::fields().name().eq("Alice"));
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
    /// let q = User::all()
    ///     .order_by(User::fields().name().asc());
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
    /// let q = User::all()
    ///     .order_by(User::fields().name().desc());
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
    /// This also works on model-terminal `#[has_many(via = ...)]` fields. In a
    /// join-model many-to-many relation, call `any` on the derived field to
    /// filter by the opposite endpoint, or on the direct join-model field to
    /// filter by data stored on the connection.
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
    /// use toasty::stmt::List;
    /// use toasty::schema::Model;
    ///
    /// // Find users that have at least one todo with "urgent" in the title
    /// let todos_path = User::path_field::<List<Todo>>(2);
    /// let filter = todos_path.any(Todo::fields().title().eq("urgent"));
    /// ```
    pub fn any(self, filter: Expr<bool>) -> Expr<bool>
    where
        U: crate::schema::Model,
    {
        // Build a query on the child model filtered by `filter`
        let child_query = super::Query::<List<U>>::all().filter(filter);
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
    /// use toasty::stmt::List;
    /// use toasty::schema::Model;
    ///
    /// // Find users whose todos are all complete
    /// let todos_path = User::path_field::<List<Todo>>(2);
    /// let filter = todos_path.all(Todo::fields().complete().eq(true));
    /// ```
    pub fn all(self, filter: Expr<bool>) -> Expr<bool>
    where
        U: crate::schema::Model,
    {
        // parent NOT IN (SELECT child_fk FROM child WHERE NOT filter)
        let child_query = super::Query::<List<U>>::all().filter(filter.not());
        self.build_filter(move |path| stmt::Expr::not_in_subquery(path, child_query.untyped))
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
        self.build_filter(|path| {
            stmt::Expr::eq(
                stmt::Expr::array_length(path),
                stmt::Expr::Value(stmt::Value::I64(0)),
            )
        })
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
    /// `.like()` is a pass-through to the database's own `LIKE`, whose case
    /// sensitivity differs by backend: case-sensitive on PostgreSQL,
    /// case-insensitive for ASCII on SQLite, and collation-dependent on MySQL.
    /// For a case-insensitive match on PostgreSQL, use [`ilike`](Self::ilike).
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

    /// Case-insensitive variant of [`like`](Self::like), mapping to
    /// PostgreSQL's `ILIKE` operator.
    ///
    /// PostgreSQL is the only supported backend with a native `ILIKE`, so
    /// `.ilike()` works only there. Toasty does not emulate it: on MySQL,
    /// SQLite, and DynamoDB the query is rejected with an unsupported-feature
    /// error. PostgreSQL's `LIKE` is case-sensitive and so provides `ILIKE` as
    /// its case-insensitive companion; the other backends fold ASCII case in
    /// `LIKE` (SQLite) or set it through the column collation (MySQL), so there
    /// is no operator with matching semantics for `.ilike()` to pass through to.
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
        let pattern = pattern.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::ilike(path, pattern))
    }

    /// Test whether this string field matches a SQL `LIKE` pattern using an
    /// explicit escape character.
    ///
    /// Available on any string-valued field, including `String`,
    /// `Option<String>`, and other wrappers whose `Field::Inner` is `String`.
    ///
    /// The caller is responsible for constructing the `LIKE` pattern. Toasty does
    /// not escape `pattern` automatically. Any `%` or `_` characters that are not
    /// escaped by `escape` are treated as SQL `LIKE` wildcards:
    ///
    /// - `%` matches any sequence of characters.
    /// - `_` matches any single character.
    /// - `escape` followed by `%`, `_`, or `escape` matches that character
    ///   literally.
    ///
    /// This is useful when the pattern needs to match literal `%` or `_`
    /// characters. For example, with `escape` set to `'\\'`, the pattern
    /// `"Alice\\%%"` matches strings beginning with the literal text `"Alice%"`.
    /// The first `%` is escaped and matched literally; the second `%` remains a
    /// wildcard.
    ///
    /// Not supported by the DynamoDB driver. Use [`starts_with`](Self::starts_with)
    /// instead when targeting DynamoDB.
    ///
    /// `.like_with_escape()` is a pass-through to the database's own `LIKE`
    /// operator with an `ESCAPE` clause. Case sensitivity differs by backend:
    /// case-sensitive on PostgreSQL, case-insensitive for ASCII on SQLite, and
    /// collation-dependent on MySQL. For a case-insensitive match on PostgreSQL,
    /// use [`ilike`](Self::ilike).
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
    /// // Match names starting with "Al".
    /// let filter = User::fields().name().like_with_escape("Al%".to_string(), '\\');
    ///
    /// // Match names starting with the literal text "Alice%".
    /// let filter = User::fields().name().like_with_escape("Alice\\%%".to_string(), '\\');
    ///
    /// // Also works on nullable string fields.
    /// let filter = User::fields().nickname().like_with_escape("Al%".to_string(), '\\');
    /// ```
    pub fn like_with_escape(self, pattern: impl IntoExpr<String>, escape: char) -> Expr<bool> {
        let pattern = pattern.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::like_with_escape(path, pattern, escape))
    }

    /// Test whether this string field matches a case-insensitive SQL `LIKE`
    /// pattern using an explicit escape character.
    ///
    /// This is the escaped variant of [`ilike`](Self::ilike). It maps to
    /// PostgreSQL's `ILIKE ... ESCAPE ...` syntax.
    ///
    /// PostgreSQL is the only supported backend with a native `ILIKE`, so
    /// `.ilike_with_escape()` works only there. Toasty does not emulate it: on
    /// MySQL, SQLite, and DynamoDB the query is rejected with an
    /// unsupported-feature error.
    ///
    /// The caller is responsible for constructing the `ILIKE` pattern. Toasty does
    /// not escape `pattern` automatically. Any `%` or `_` characters that are not
    /// escaped by `escape` are treated as SQL pattern wildcards:
    ///
    /// - `%` matches any sequence of characters.
    /// - `_` matches any single character.
    /// - `escape` followed by `%`, `_`, or `escape` matches that character
    ///   literally.
    ///
    /// This is useful when the pattern needs to match literal `%` or `_`
    /// characters while still matching case-insensitively. For example, with
    /// `escape` set to `'\\'`, the pattern `"Alice\\%%"` matches strings beginning
    /// with the literal text `"Alice%"`, ignoring case. It can match `"Alice%1"`,
    /// `"ALICE%1"`, or `"alice%foo"`, but not `"AliceA1"`.
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
    /// // Match names starting with "al", ignoring case.
    /// let filter = User::fields().name().ilike_with_escape("al%".to_string(), '\\');
    ///
    /// // Match names starting with the literal text "alice%", ignoring case.
    /// let filter = User::fields().name().ilike_with_escape("alice\\%%".to_string(), '\\');
    /// ```
    pub fn ilike_with_escape(self, pattern: impl IntoExpr<String>, escape: char) -> Expr<bool> {
        let pattern = pattern.into_expr().untyped;
        self.build_filter(move |path| stmt::Expr::ilike_with_escape(path, pattern, escape))
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

impl<T, U> Path<T, U>
where
    T: Model,
{
    /// App-level (Rust) name of the leaf field this path ends at (embed
    /// prefixes discarded), or `None` when the leaf has no app-level name.
    ///
    /// The `inner` field of a tuple-newtype embed is transparent — it takes
    /// the parent field's column and has no app-level name — so it returns
    /// `None`.
    ///
    /// # Panics
    ///
    /// Panics if the path does not end at a field, or if the projection
    /// crosses a relation: only embedded struct, embedded enum, and document
    /// steps are supported.
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
    /// let name = User::fields().name().field_name();
    /// assert_eq!(name.as_deref(), Some("name"));
    /// ```
    pub fn field_name(&self) -> Option<String> {
        let models = Self::reachable_models();
        let (leaf, _) = Self::resolve_path_or_panic(&models, &self.untyped);
        leaf.name.app.clone()
    }

    /// Whether this field is the target of a single-field unique index.
    ///
    /// Index membership only, not a global-uniqueness guarantee: `NULL`s do
    /// not conflict (SQL treats them as distinct; DynamoDB skips the index
    /// entry), so `true` implies globally unique values only for a
    /// non-nullable column (non-optional leaf, no nullable parent embed or
    /// enum variant crossed). True for `#[unique]` fields, enum-level
    /// `#[unique(variant::field)]` and `#[unique(shared)]` references (every
    /// `#[shared(shared)]` member), and single-field primary keys.
    /// Components of composite indices are not unique on their own.
    /// Fields inside a `#[document]` embed report `false`: their app-level
    /// index has no database backing (`collect_indices` only recurses into
    /// column-expanded embeds).
    ///
    /// # Panics
    ///
    /// Panics if the path does not end at a field, or if the projection
    /// crosses a relation: only embedded struct, embedded enum, and document
    /// steps are supported.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     #[unique]
    /// #     email: String,
    /// # }
    /// assert!(User::fields().email().is_unique());
    /// assert!(User::fields().id().is_unique());
    /// ```
    pub fn is_unique(&self) -> bool {
        let models = Self::reachable_models();
        let (field, document) = Self::resolve_path_or_panic(&models, &self.untyped);
        if document.is_some() {
            return false;
        }
        let owner = Self::model_by_id(&models, field.id.model);
        let indices = match owner {
            app::Model::Root(root) => &root.indices,
            app::Model::EmbeddedStruct(embedded) => &embedded.indices,
            app::Model::EmbeddedEnum(embedded) => &embedded.indices,
        };
        indices.iter().any(|idx| {
            if !idx.unique || idx.fields.len() != 1 {
                return false;
            }
            let indexed = idx.fields[0].field;
            if indexed == field.id {
                return true;
            }
            // Enum-level `#[unique(name)]` stores the first `#[shared(name)]`
            // member only, but constrains the shared column for every member.
            let Some(shared) = &field.shared else {
                return false;
            };
            Self::model_by_id(&models, indexed.model)
                .fields()
                .get(indexed.index)
                .and_then(|f| f.shared.as_ref())
                == Some(shared)
        })
    }

    /// Collects the models reachable from `T` so field lookups can walk into
    /// embedded models. Built per call: there is no global registry, and
    /// `T::register` rebuilds the schema of every reachable model, so the
    /// metadata accessors built on this are for one-off use, not per-row
    /// loops.
    fn reachable_models() -> app::ModelSet {
        let mut models = app::ModelSet::new();
        T::register(&mut models);
        models
    }

    fn model_by_id(models: &app::ModelSet, id: ModelId) -> &app::Model {
        models
            .get(id)
            .unwrap_or_else(|| panic!("model {id:?} is not registered"))
    }

    /// Resolve `path` through the shared app-schema walk, mapping its errors
    /// onto the panic messages these accessors document.
    fn resolve_path_or_panic<'a>(
        models: &'a app::ModelSet,
        path: &stmt::Path,
    ) -> (&'a app::Field, Option<&'a app::Field>) {
        match models.resolve_path(path) {
            Ok(resolved) => resolved,
            Err(app::ResolveError::Empty | app::ResolveError::OutOfBounds { .. }) => {
                panic!("path does not end at a field")
            }
            Err(app::ResolveError::NonEmbedded { .. }) => {
                panic!("cannot project through non-embedded field")
            }
            Err(app::ResolveError::NotEmbeddedEnum { .. }) => {
                panic!("variant path parent is not the embedded enum the variant id names")
            }
            Err(app::ResolveError::UnknownModel(id)) => {
                panic!("model {id:?} is not registered")
            }
        }
    }
}

/// Nullability of a path's leaf field, read off the leaf's Rust type.
///
/// The app schema's `nullable` flag is generated from the field type's
/// [`Field::NULLABLE`], so the typed path already carries the answer and no
/// schema walk is needed.
///
/// Only storable leaf types expose this: relation terminals and model roots
/// have no method, and projecting through a relation is a compile error
/// rather than the runtime panic `field_name`/`is_unique` still produce. An
/// embed root reports `false` but has no leaf field.
impl<T, U> Path<T, U>
where
    T: Model,
    U: Field,
{
    /// Whether the leaf field is `Option`-marked.
    ///
    /// Reports the leaf field's own nullability only; a `false` result does
    /// not rule out storage `NULL`s from a nullable parent embed or an
    /// inactive enum variant (see [`is_unique`](Self::is_unique)).
    ///
    /// Only sound for generated accessors; hand-built `path_field::<U>` /
    /// `chain` paths must supply the field's `ExprTarget` as `U`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     bio: Option<String>,
    /// # }
    /// assert!(User::fields().bio().is_nullable());
    /// assert!(!User::fields().id().is_nullable());
    /// ```
    pub fn is_nullable(&self) -> bool {
        U::NULLABLE
    }
}

/// List-targeted paths (`Vec<T>` fields) are never `Option`-wrapped;
/// `Option<Vec<T>>` keeps its wrapper as the path target and is covered by
/// the `U: Field` impl above.
impl<T, U> Path<T, List<U>>
where
    T: Model,
{
    /// Always `false` for a list-targeted path.
    pub fn is_nullable(&self) -> bool {
        false
    }
}

impl<T, U> fmt::Debug for Path<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.untyped)
    }
}
