use super::Value;

use std::{hash::Hash, ops};

#[derive(Debug, Default, Clone, Eq)]
pub struct ValueRecord {
    pub fields: Vec<Value>,
}

impl ValueRecord {
    pub fn from_vec(fields: Vec<Value>) -> Self {
        Self { fields }
    }

    pub fn as_slice(&self) -> &[Value] {
        &self[..]
    }
}

impl ops::Deref for ValueRecord {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.fields[..]
    }
}

impl ops::DerefMut for ValueRecord {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields[..]
    }
}

impl IntoIterator for ValueRecord {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<'a> IntoIterator for &'a ValueRecord {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut ValueRecord {
    type Item = &'a mut Value;
    type IntoIter = std::slice::IterMut<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

// This implementation delegates the PartialEq implementation to the [Value]
// (slice) implementation of PartialEq
impl PartialEq for ValueRecord {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

// had to impl hash for value record because conflicting implementations of hash trait
impl Hash for ValueRecord {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

macro_rules! impl_value_eq_tuple {
    ( $len:literal; $(($T:ident, $idx:tt)),+ ) => {
        impl<$($T),+> PartialEq<($($T,)+)> for Value
        where
            $(Value: PartialEq<$T>,)+
        {
            fn eq(&self, other: &($($T,)+)) -> bool {
                match self {
                    Value::Record(v) => {
                        v.fields.len() == $len
                            $(&& v.fields[$idx].eq(&other.$idx))+
                    }
                    _ => false,
                }
            }
        }

        impl<$($T),+> PartialEq<Value> for ($($T,)+)
        where
            $($T: PartialEq<Value>,)+
        {
            fn eq(&self, other: &Value) -> bool {
                match other {
                    Value::Record(v) => {
                        v.fields.len() == $len
                            $(&& self.$idx.eq(&v.fields[$idx]))+
                    }
                    _ => false,
                }
            }
        }
    };
}

impl_value_eq_tuple!(1; (T0, 0));
impl_value_eq_tuple!(2; (T0, 0), (T1, 1));
impl_value_eq_tuple!(3; (T0, 0), (T1, 1), (T2, 2));
impl_value_eq_tuple!(4; (T0, 0), (T1, 1), (T2, 2), (T3, 3));
impl_value_eq_tuple!(5; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4));
impl_value_eq_tuple!(6; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5));
impl_value_eq_tuple!(7; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6));
impl_value_eq_tuple!(8; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7));
impl_value_eq_tuple!(9; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7), (T8, 8));
impl_value_eq_tuple!(10; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7), (T8, 8), (T9, 9));
impl_value_eq_tuple!(11; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7), (T8, 8), (T9, 9), (T10, 10));
impl_value_eq_tuple!(12; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7), (T8, 8), (T9, 9), (T10, 10), (T11, 11));
