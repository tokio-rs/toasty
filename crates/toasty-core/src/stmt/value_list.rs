//! List-related methods and trait implementations for [`Value`].

use crate::stmt::Value;

impl Value {
    /// Creates a [`Value::List`] from a vector of values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty_core::stmt::Value;
    /// let list = Value::list_from_vec(vec![Value::from(1_i64), Value::from(2_i64)]);
    /// assert!(list.is_list());
    /// ```
    pub fn list_from_vec(items: Vec<Self>) -> Self {
        Self::List(items)
    }

    /// Returns `true` if this value is a [`Value::List`].
    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    /// Consumes this value and returns the inner `Vec<Value>`, panicking
    /// if this is not a [`Value::List`].
    ///
    /// # Panics
    ///
    /// Panics if the value is not a `List` variant.
    #[track_caller]
    pub fn into_list_unwrap(self) -> Vec<Value> {
        match self {
            Value::List(list) => list,
            _ => panic!("expected Value::List; actual={self:#?}"),
        }
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Value::List(value)
    }
}

impl<T, const N: usize> PartialEq<[T; N]> for Value
where
    T: PartialEq<Value>,
{
    fn eq(&self, other: &[T; N]) -> bool {
        match self {
            Value::List(items) => items.iter().enumerate().all(|(i, item)| other[i].eq(item)),
            _ => false,
        }
    }
}

impl<T, const N: usize> PartialEq<Value> for [T; N]
where
    T: PartialEq<Value>,
{
    fn eq(&self, other: &Value) -> bool {
        other.eq(self)
    }
}
