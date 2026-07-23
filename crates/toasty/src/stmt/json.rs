use super::{Expr, IntoExpr, List, Path, Value as StmtValue};
use crate::schema::{Field, Load};
use serde_json::Value as JsonValue;
use toasty_core::schema::app::{FieldPrimitive, FieldTy, SerializeFormat};
use toasty_core::{schema::db, stmt};

use std::fmt;

fn load_json<T>(value: stmt::Value) -> crate::Result<T>
where
    T: for<'de> serde_core::Deserialize<'de>,
{
    let json = <String as Load>::load(value)?;
    serde_json::from_str(&json).map_err(|e| {
        toasty_core::Error::from_args(format_args!("failed to deserialize JSON field: {e}"))
    })
}

fn json_field_ty(storage_ty: Option<db::Type>, missing_type_message: &'static str) -> FieldTy {
    FieldTy::Primitive(FieldPrimitive {
        ty: stmt::Type::String,
        storage_ty: Some(storage_ty.expect(missing_type_message)),
        serialize: Some(SerializeFormat::Json),
    })
}

fn json_expr<T>(value: &(impl serde_core::Serialize + ?Sized)) -> Expr<T> {
    let json = serde_json::to_string(value).expect("failed to serialize JSON field");
    Expr::<String>::from_value(StmtValue::from(json)).cast()
}

/// A field wrapper that serializes `T` as JSON in a database column.
///
/// Use `Json<T>` as a model field type when the column has no native database
/// representation for `T` — for example, a serde-derived struct, a `HashMap`,
/// or any other type that round-trips through
/// [`serde::Serialize`](serde::Serialize) and
/// [`serde::Deserialize`](serde::Deserialize). The value is JSON-encoded on
/// insert and update, and decoded on read.
///
/// Every `Json<T>` field must select its database column type with
/// `#[column(type = ...)]`. Use `text` or `varchar(...)` for text-backed JSON,
/// `json` for PostgreSQL or MySQL native JSON, and `jsonb` for PostgreSQL JSONB.
/// A field whose Rust type is already [`serde_json::Value`] does not need the
/// `Json<T>` wrapper and supports the same column types.
///
/// # Two nullable variants
///
/// `Json<T>` composes with `Option` in two distinct, both-useful ways:
///
/// | Field type           | `None` is stored as      |
/// |----------------------|--------------------------|
/// | `Option<Json<T>>`    | SQL `NULL`               |
/// | `Json<Option<T>>`    | JSON literal `"null"`    |
///
/// # Setter ergonomics
///
/// The crate provides an `IntoExpr<Json<T>>` impl for any
/// `T: serde::Serialize`, so create / update setters and the `create!`
/// macro accept the bare inner value without an explicit `Json(...)`
/// wrapper. Both forms produce the same expression:
///
/// ```ignore
/// // for a `payload: Json<Payload>` (or `Deferred<Json<Payload>>`) field
/// Repository::create().payload(my_payload.clone()).exec(&mut db).await?;
/// Repository::create().payload(Json(my_payload.clone())).exec(&mut db).await?;
/// ```
///
/// The `Json(...)` form is still useful when type inference needs a
/// nudge (e.g., comparison expressions like `.eq(Json("hello"))`).
///
/// # Composition with `Deferred`
///
/// `Json<T>` is the only wrapper allowed inside [`Deferred`](crate::Deferred)
/// for a serde-typed field, since a deferred column needs both lazy
/// loading and a serializer the macro can drive through trait dispatch:
///
/// ```ignore
/// #[derive(Debug, toasty::Model)]
/// struct Repository {
///     #[key] #[auto]
///     id: uuid::Uuid,
///     #[column(type = text)]
///     schema: toasty::Deferred<toasty::Json<MySchema>>,
/// }
/// ```
///
/// # Examples
///
/// ```ignore
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Tags(Vec<String>);
///
/// #[derive(Debug, toasty::Model)]
/// struct Item {
///     #[key] #[auto]
///     id: i64,
///     #[column(type = text)]
///     tags: toasty::Json<Tags>,
/// }
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Json<T>(pub T);

impl<T> From<T> for Json<T> {
    fn from(value: T) -> Self {
        Json(value)
    }
}

impl<T> std::ops::Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> AsRef<T> for Json<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for Json<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Load for Json<T>
where
    T: for<'de> serde_core::Deserialize<'de>,
{
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::String
    }

    fn load(value: stmt::Value) -> crate::Result<Self> {
        // SQL `NULL` never reaches here: a nullable JSON column is typed
        // `Option<Json<T>>`, and `Option<T>: Load` intercepts `Value::Null`
        // before delegating to the inner type. `Json<Option<T>>` stores the
        // JSON literal `"null"` as a non-null string, so its `None` case
        // comes through as `Value::String("null")` and `serde_json` decodes
        // it. A bare `Value::Null` at this point is a driver bug.
        load_json(value).map(Json)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T> Field for Json<T>
where
    T: serde_core::Serialize + for<'de> serde_core::Deserialize<'de>,
{
    const REQUIRES_EXPLICIT_COLUMN_TYPE: bool = true;

    type ExprTarget = Self;
    type Path<Origin> = Path<Origin, Self>;
    type ListPath<Origin> = Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: Path<Origin, List<Self::ExprTarget>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    /// Tag the schema entry as JSON-serialized with explicit storage.
    ///
    /// The macro never inspects the wrapper at the AST level; the
    /// `serialize: Some(Json)` metadata flows through trait dispatch
    /// alongside the storage type, so external schema consumers can still
    /// see that the column is JSON-typed even though the encoding itself is
    /// invisible to the macro.
    fn field_ty(storage_ty: Option<db::Type>) -> FieldTy {
        json_field_ty(
            storage_ty,
            "`toasty::Json<T>` fields require `#[column(type = ...)]`; use \
             `#[column(type = text)]` for text-backed JSON storage",
        )
    }

    fn key_constraint<Origin>(&self, _target: Path<Origin, Self::Inner>) -> Expr<bool> {
        // JSON columns are not valid foreign-key targets — the comparison
        // would happen at the serialized-string level, which is rarely what
        // the user wants. The trait method exists for every `Field`; we
        // satisfy it with a panic rather than admitting nonsense semantics.
        unreachable!("Json<T> fields cannot be used as foreign-key targets")
    }
}

impl<T> IntoExpr<Json<T>> for Json<T>
where
    T: serde_core::Serialize,
{
    fn into_expr(self) -> Expr<Json<T>> {
        json_expr(&self.0)
    }

    fn by_ref(&self) -> Expr<Json<T>> {
        json_expr(&self.0)
    }
}

// `IntoExpr<Json<T>>` for `&Json<T>` comes from the blanket
// `impl<T: IntoExpr<T>> IntoExpr<T> for &T` in `stmt::into_expr`.

/// Accept a bare `T` wherever the API expects `IntoExpr<Json<T>>`, so
/// callers don't have to spell `Json(value)` at setter sites:
///
/// ```ignore
/// // both forms work for a `payload: Json<Payload>` field
/// Repository::create().payload(Json(payload.clone())).exec(&mut db).await?;
/// Repository::create().payload(payload.clone()).exec(&mut db).await?;
/// ```
///
/// The blanket only fires when `T: serde::Serialize`; it doesn't overlap
/// the explicit `IntoExpr<Json<T>> for Json<T>` impl because `Json<U>`
/// itself is not `Serialize` (no derive on the wrapper).
impl<T> IntoExpr<Json<T>> for T
where
    T: serde_core::Serialize,
{
    fn into_expr(self) -> Expr<Json<T>> {
        json_expr(&self)
    }

    fn by_ref(&self) -> Expr<Json<T>> {
        json_expr(self)
    }
}

impl<T> super::assignment::Assign<Json<T>> for Json<T>
where
    T: serde_core::Serialize,
{
    fn into_assignment(self) -> super::assignment::Assignment<Json<T>> {
        super::set(<Self as IntoExpr<Json<T>>>::into_expr(self))
    }
}

/// Mirrors the `IntoExpr<Json<T>> for T` blanket on the assignment side so
/// update builders accept a bare value too:
///
/// ```ignore
/// repo.update().payload(payload.clone()).exec(&mut db).await?;
/// ```
impl<T> super::assignment::Assign<Json<T>> for T
where
    T: serde_core::Serialize,
{
    fn into_assignment(self) -> super::assignment::Assignment<Json<T>> {
        super::set(<Self as IntoExpr<Json<T>>>::into_expr(self))
    }
}

impl Load for JsonValue {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::String
    }

    fn load(value: stmt::Value) -> crate::Result<Self> {
        load_json(value)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl Field for JsonValue {
    const REQUIRES_EXPLICIT_COLUMN_TYPE: bool = true;

    type ExprTarget = Self;
    type Path<Origin> = Path<Origin, Self>;
    type ListPath<Origin> = Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: Path<Origin, List<Self::ExprTarget>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn field_ty(storage_ty: Option<db::Type>) -> FieldTy {
        json_field_ty(
            storage_ty,
            "`serde_json::Value` fields require `#[column(type = ...)]`; use \
             `#[column(type = text)]` for text-backed JSON storage",
        )
    }

    fn key_constraint<Origin>(&self, _target: Path<Origin, Self::Inner>) -> Expr<bool> {
        unreachable!("serde_json::Value fields cannot be used as foreign-key targets")
    }
}

impl IntoExpr<JsonValue> for JsonValue {
    fn into_expr(self) -> Expr<JsonValue> {
        json_expr(&self)
    }

    fn by_ref(&self) -> Expr<JsonValue> {
        json_expr(self)
    }
}

impl super::assignment::Assign<JsonValue> for JsonValue {
    fn into_assignment(self) -> super::assignment::Assignment<JsonValue> {
        super::set(<Self as IntoExpr<JsonValue>>::into_expr(self))
    }
}

impl<T: fmt::Display> fmt::Display for Json<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> serde_core::Serialize for Json<T>
where
    T: serde_core::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T> serde_core::Deserialize<'de> for Json<T>
where
    T: serde_core::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde_core::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Json)
    }
}
