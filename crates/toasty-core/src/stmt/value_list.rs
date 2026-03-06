use crate::stmt::Value;

impl Value {
    pub fn list_from_vec(items: Vec<Self>) -> Self {
        Self::List(items)
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    #[track_caller]
    pub fn unwrap_list(self) -> Vec<Value> {
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
