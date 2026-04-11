use crate::serializer::ExprContext;

use super::{Flavor, Formatter, ToSql};

use toasty_core::stmt;

/// Collects query parameter values during SQL serialization.
///
/// Implement this trait to control how bound parameters are stored. The
/// serializer calls [`push`](Params::push) each time it encounters a value
/// that should be sent as a bind parameter rather than inlined into the SQL
/// string.
pub trait Params {
    /// Appends a value (with an optional type hint) and returns its [`Placeholder`].
    fn push(&mut self, param: &stmt::Value, type_hint: Option<&stmt::Type>) -> Placeholder;
}

/// A positional bind-parameter placeholder.
///
/// The `index` is the 1-based parameter index. The serializer renders
/// it in the target dialect's format (`$1`, `?1`, or `?`).
///
/// When `cast` is set, PostgreSQL placeholders are rendered with a type
/// cast (e.g. `$1::my_enum`). This is needed for native enum types where
/// PostgreSQL rejects a bare TEXT parameter.
///
/// # Example
///
/// ```
/// use toasty_sql::serializer::Placeholder;
///
/// let p = Placeholder { index: 3, cast: None };
/// assert_eq!(p.index, 3);
/// ```
pub struct Placeholder {
    /// 1-based parameter index.
    pub index: usize,
    /// Optional SQL type cast (e.g. enum type name for PostgreSQL).
    pub cast: Option<String>,
}

/// A parameter value paired with an optional type hint.
///
/// Type hints let drivers pick the right wire format when the value alone
/// is ambiguous (e.g. distinguishing `INTEGER` from `BIGINT`).
///
/// # Example
///
/// ```
/// use toasty_sql::TypedValue;
///
/// let tv = TypedValue {
///     value: toasty_core::stmt::Value::Null,
///     type_hint: None,
/// };
/// assert!(tv.type_hint.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct TypedValue {
    /// The parameter value.
    pub value: stmt::Value,
    /// An optional type hint for the value.
    pub type_hint: Option<stmt::Type>,
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
    fn push(&mut self, value: &stmt::Value, _type_hint: Option<&stmt::Type>) -> Placeholder {
        self.push(value.clone());
        Placeholder {
            index: self.len(),
            cast: None,
        }
    }
}

impl Params for Vec<TypedValue> {
    fn push(&mut self, value: &stmt::Value, type_hint: Option<&stmt::Type>) -> Placeholder {
        self.push(TypedValue {
            value: value.clone(),
            type_hint: type_hint.cloned(),
        });
        Placeholder {
            index: self.len(),
            cast: None,
        }
    }
}

impl ToSql for Placeholder {
    fn to_sql<P: Params>(self, _cx: &ExprContext<'_>, f: &mut Formatter<'_, P>) {
        use std::fmt::Write;

        match f.serializer.flavor {
            Flavor::Mysql => write!(&mut f.dst, "?").unwrap(),
            Flavor::Postgresql => {
                write!(&mut f.dst, "${}", self.index).unwrap();
                if let Some(ref cast) = self.cast {
                    write!(&mut f.dst, "::{}", cast).unwrap();
                }
            }
            Flavor::Sqlite => write!(&mut f.dst, "?{}", self.index).unwrap(),
        }
    }
}
