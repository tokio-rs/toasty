use crate::serializer::ExprContext;

use super::{Flavor, Formatter, ToSql};

use toasty_core::{schema::db, stmt};

/// Collects query parameter values during SQL serialization.
///
/// Implement this trait to control how bound parameters are stored. The
/// serializer calls [`push`](Params::push) each time it encounters a value
/// that should be sent as a bind parameter rather than inlined into the SQL
/// string.
pub trait Params {
    /// Appends a value (with an optional storage type) and returns its [`Placeholder`].
    fn push(&mut self, param: &stmt::Value, storage_ty: Option<&db::Type>) -> Placeholder;
}

/// A positional bind-parameter placeholder.
///
/// The inner `usize` is the 1-based parameter index. The serializer renders
/// it in the target dialect's format (`$1`, `?1`, or `?`).
///
/// # Example
///
/// ```
/// use toasty_sql::serializer::Placeholder;
///
/// let p = Placeholder(3);
/// assert_eq!(p.0, 3);
/// ```
pub struct Placeholder(pub usize);

/// A parameter value paired with an optional database storage type.
///
/// The storage type lets drivers pick the right wire format. For example,
/// `db::Type::Integer(8)` maps to PostgreSQL `INT8`, `db::Type::Enum(..)`
/// maps to the cached enum OID, etc. When `None`, the driver infers the
/// type from the value itself.
///
/// # Example
///
/// ```
/// use toasty_sql::TypedValue;
///
/// let tv = TypedValue {
///     value: toasty_core::stmt::Value::Null,
///     storage_ty: None,
/// };
/// assert!(tv.storage_ty.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct TypedValue {
    /// The parameter value.
    pub value: stmt::Value,
    /// The database storage type of the target column, if known.
    pub storage_ty: Option<db::Type>,
}

impl Params for Vec<stmt::Value> {
    fn push(&mut self, value: &stmt::Value, _storage_ty: Option<&db::Type>) -> Placeholder {
        self.push(value.clone());
        Placeholder(self.len())
    }
}

impl Params for Vec<TypedValue> {
    fn push(&mut self, value: &stmt::Value, storage_ty: Option<&db::Type>) -> Placeholder {
        self.push(TypedValue {
            value: value.clone(),
            storage_ty: storage_ty.cloned(),
        });
        Placeholder(self.len())
    }
}

impl ToSql for Placeholder {
    fn to_sql<P: Params>(self, _cx: &ExprContext<'_>, f: &mut Formatter<'_, P>) {
        use std::fmt::Write;

        match f.serializer.flavor {
            Flavor::Mysql => write!(&mut f.dst, "?").unwrap(),
            Flavor::Postgresql => write!(&mut f.dst, "${}", self.0).unwrap(),
            Flavor::Sqlite => write!(&mut f.dst, "?{}", self.0).unwrap(),
        }
    }
}
