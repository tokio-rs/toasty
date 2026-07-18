use crate::schema::{Field, Load};
use toasty_core::{schema::db, stmt};

use super::{Expr, IntoExpr, List, Path};

/// A schema-less JSON value stored in a database-native document column.
///
/// Unlike [`Json<T>`](super::Json), `JsonValue` is not encoded as an opaque
/// string. PostgreSQL stores it as `jsonb`, MySQL as `JSON`, SQLite and Turso
/// as JSON text, and DynamoDB as native document attributes.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct JsonValue(pub serde_json::Value);

impl JsonValue {
    /// Returns a JSON null value.
    pub const fn null() -> Self {
        Self(serde_json::Value::Null)
    }
}

impl From<serde_json::Value> for JsonValue {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

impl From<JsonValue> for serde_json::Value {
    fn from(value: JsonValue) -> Self {
        value.0
    }
}

impl std::ops::Deref for JsonValue {
    type Target = serde_json::Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for JsonValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<serde_json::Value> for JsonValue {
    fn as_ref(&self) -> &serde_json::Value {
        &self.0
    }
}

impl AsMut<serde_json::Value> for JsonValue {
    fn as_mut(&mut self) -> &mut serde_json::Value {
        &mut self.0
    }
}

impl serde_core::Serialize for JsonValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> serde_core::Deserialize<'de> for JsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde_core::Deserializer<'de>,
    {
        serde_json::Value::deserialize(deserializer).map(Self)
    }
}

impl Load for JsonValue {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Json
    }

    fn load(value: stmt::Value) -> crate::Result<Self> {
        let stmt::Value::Json(value) = value else {
            return Err(toasty_core::Error::type_conversion(value, "JsonValue"));
        };

        Ok(Self(value_from_stmt(*value)?))
    }

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl Field for JsonValue {
    type ExprTarget = Self;
    type Path<Origin> = Path<Origin, Self>;
    type ListPath<Origin> = Path<Origin, List<Self>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut stmt::Assignments,
        _projection: stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn field_ty(storage_ty: Option<db::Type>) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: stmt::Type::Json,
            storage_ty,
            serialize: None,
        })
    }

    fn key_constraint<Origin>(&self, _target: Path<Origin, Self>) -> Expr<bool> {
        unreachable!("JsonValue fields cannot be used as foreign-key targets")
    }
}

impl IntoExpr<JsonValue> for JsonValue {
    fn into_expr(self) -> Expr<JsonValue> {
        Expr::from_value(stmt::Value::Json(Box::new(value_into_stmt(self.0))))
    }

    fn by_ref(&self) -> Expr<JsonValue> {
        Expr::from_value(stmt::Value::Json(Box::new(value_into_stmt(self.0.clone()))))
    }
}

impl IntoExpr<JsonValue> for serde_json::Value {
    fn into_expr(self) -> Expr<JsonValue> {
        JsonValue(self).into_expr()
    }

    fn by_ref(&self) -> Expr<JsonValue> {
        JsonValue(self.clone()).into_expr()
    }
}

impl super::assignment::Assign<JsonValue> for JsonValue {
    fn into_assignment(self) -> super::assignment::Assignment<JsonValue> {
        super::set(self.into_expr())
    }
}

impl super::assignment::Assign<JsonValue> for serde_json::Value {
    fn into_assignment(self) -> super::assignment::Assignment<JsonValue> {
        super::set(self.into_expr())
    }
}

fn value_into_stmt(value: serde_json::Value) -> stmt::Value {
    match value {
        serde_json::Value::Null => stmt::Value::Null,
        serde_json::Value::Bool(value) => stmt::Value::Bool(value),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                stmt::Value::I64(value)
            } else if let Some(value) = value.as_u64() {
                stmt::Value::U64(value)
            } else {
                stmt::Value::F64(value.as_f64().expect("JSON number is representable as f64"))
            }
        }
        serde_json::Value::String(value) => stmt::Value::String(value),
        serde_json::Value::Array(values) => {
            stmt::Value::List(values.into_iter().map(value_into_stmt).collect())
        }
        serde_json::Value::Object(values) => stmt::Value::Object(stmt::ValueObject::from_vec(
            values
                .into_iter()
                .map(|(key, value)| (key, value_into_stmt(value)))
                .collect(),
        )),
    }
}

fn value_from_stmt(value: stmt::Value) -> crate::Result<serde_json::Value> {
    Ok(match value {
        stmt::Value::Null => serde_json::Value::Null,
        stmt::Value::Bool(value) => serde_json::Value::Bool(value),
        stmt::Value::I64(value) => serde_json::Value::Number(value.into()),
        stmt::Value::U64(value) => serde_json::Value::Number(value.into()),
        stmt::Value::F64(value) => serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| {
                toasty_core::Error::from_args(format_args!(
                    "cannot load non-finite number as JsonValue"
                ))
            })?,
        stmt::Value::String(value) => serde_json::Value::String(value),
        stmt::Value::List(values) => serde_json::Value::Array(
            values
                .into_iter()
                .map(value_from_stmt)
                .collect::<crate::Result<_>>()?,
        ),
        stmt::Value::Object(value) => serde_json::Value::Object(
            value
                .entries
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_stmt(value)?)))
                .collect::<crate::Result<_>>()?,
        ),
        value => return Err(toasty_core::Error::type_conversion(value, "JsonValue")),
    })
}
