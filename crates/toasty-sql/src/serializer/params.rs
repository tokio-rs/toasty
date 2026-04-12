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
    /// Appends a value (with optional type and storage-type hints) and returns its [`Placeholder`].
    fn push(
        &mut self,
        param: &stmt::Value,
        type_hint: Option<&stmt::Type>,
        storage_ty: Option<&db::Type>,
    ) -> Placeholder;
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

/// A parameter value paired with optional type hints.
///
/// Type hints let drivers pick the right wire format when the value alone
/// is ambiguous (e.g. distinguishing `INTEGER` from `BIGINT`). The storage
/// type lets drivers use the correct database-specific type (e.g. a native
/// enum OID instead of `TEXT`).
///
/// # Example
///
/// ```
/// use toasty_sql::TypedValue;
///
/// let tv = TypedValue {
///     value: toasty_core::stmt::Value::Null,
///     type_hint: None,
///     storage_ty: None,
/// };
/// assert!(tv.type_hint.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct TypedValue {
    /// The parameter value.
    pub value: stmt::Value,
    /// An optional type hint for the value.
    pub type_hint: Option<stmt::Type>,
    /// The database storage type of the target column, if known.
    pub storage_ty: Option<db::Type>,
}

impl TypedValue {
    /// Infers the type of this value, using the type hint if available
    pub fn infer_ty(&self) -> stmt::Type {
        self.type_hint
            .clone()
            .unwrap_or_else(|| self.value.infer_ty())
    }
}

impl Params for Vec<stmt::Value> {
    fn push(
        &mut self,
        value: &stmt::Value,
        _type_hint: Option<&stmt::Type>,
        _storage_ty: Option<&db::Type>,
    ) -> Placeholder {
        self.push(value.clone());
        Placeholder(self.len())
    }
}

impl Params for Vec<TypedValue> {
    fn push(
        &mut self,
        value: &stmt::Value,
        type_hint: Option<&stmt::Type>,
        storage_ty: Option<&db::Type>,
    ) -> Placeholder {
        self.push(TypedValue {
            value: value.clone(),
            type_hint: type_hint.cloned(),
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
