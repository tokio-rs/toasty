use super::{Type, Value};

/// A query parameter: a value paired with an optional type hint.
///
/// Type hints let drivers pick the correct wire format when the value alone
/// is ambiguous (e.g., distinguishing `INTEGER` from `BIGINT`). When the
/// hint is `None`, the type is inferred from the value at bind time.
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::{Param, Type, Value};
///
/// let p = Param::new(Value::from(42_i64), Some(Type::I64));
/// assert_eq!(p.ty(), Type::I64);
///
/// let untyped = Param::from(Value::from("hello"));
/// assert_eq!(untyped.ty(), Type::String);
/// ```
#[derive(Debug, Clone)]
pub struct Param {
    /// The parameter value.
    pub value: Value,

    /// An optional type hint for the parameter.
    pub type_hint: Option<Type>,
}

impl Param {
    /// Creates a new parameter with an explicit type hint.
    pub fn new(value: Value, type_hint: Option<Type>) -> Self {
        Self { value, type_hint }
    }

    /// Returns the type of this parameter, preferring the hint over inference.
    pub fn ty(&self) -> Type {
        self.type_hint
            .clone()
            .unwrap_or_else(|| self.value.infer_ty())
    }
}

impl From<Value> for Param {
    fn from(value: Value) -> Self {
        Self {
            value,
            type_hint: None,
        }
    }
}
