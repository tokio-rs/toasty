#[cfg(feature = "serde")]
use crate::schema::{Field, Load};
#[cfg(feature = "serde")]
use crate::stmt::{self, Expr, List};
#[cfg(feature = "serde")]
use std::marker::PhantomData;

/// A sized marker type representing "JSON-encoded `T`".
///
/// Used as a type parameter to [`Statement`](crate::Statement) (and other
/// `Load`-bound types) to encode that the column stores `T` as a JSON
/// string. Decoding pulls the column as a `String` and runs
/// `serde_json::from_str::<T>`, so callers get `T` back — the wrapper is
/// invisible at the call site.
///
/// Generated code uses `Statement<Json<T>>` as the return type for the
/// lazy-load accessor on a field annotated with both `#[serialize(json)]`
/// and `#[deferred]`. `T` itself only needs to implement
/// `serde::Deserialize`, not Toasty's [`Load`].
///
/// A `Null` column value is mapped to the JSON literal `"null"` before
/// deserializing so `Json<Option<T>>` decodes a `NULL` cell as `None`.
#[cfg(feature = "serde")]
pub struct Json<T>(PhantomData<T>);

#[cfg(feature = "serde")]
impl<T> Load for Json<T>
where
    T: for<'de> serde_core::Deserialize<'de>,
{
    type Output = T;

    fn ty() -> toasty_core::stmt::Type {
        toasty_core::stmt::Type::String
    }

    fn load(value: toasty_core::stmt::Value) -> crate::Result<T> {
        let json = match value {
            toasty_core::stmt::Value::Null => std::borrow::Cow::Borrowed("null"),
            v => std::borrow::Cow::Owned(<String as Load>::load(v)?),
        };
        serde_json::from_str(&json).map_err(|e| {
            toasty_core::Error::from_args(format_args!("failed to deserialize JSON field: {e}"))
        })
    }
}

/// `Field` impl that lets `Json<T>` appear in a typed [`Path`](stmt::Path).
///
/// Generated code returns `<Json<T> as Field>::Path<__Origin>` —
/// i.e. `Path<__Origin, Json<T>>` — from the `.include()`-only accessor
/// on a `#[serialize] + #[deferred]` field, so the path's type tag
/// reflects what the column actually stores (JSON-encoded `T`) rather
/// than the raw `String`. The associated types stay scalar-shaped
/// because JSON columns have no usable predicate / update surface; the
/// FK helper is unreachable.
#[cfg(feature = "serde")]
impl<T> Field for Json<T>
where
    T: for<'de> serde_core::Deserialize<'de>,
{
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: stmt::Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn key_constraint<Origin>(&self, _target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        unreachable!("JSON-serialized fields cannot be used as foreign keys")
    }
}
