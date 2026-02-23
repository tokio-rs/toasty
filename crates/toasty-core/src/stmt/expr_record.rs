use super::{Expr, Node, Visit, VisitMut};
use crate::stmt::{self, Value};

use std::{fmt, ops};

/// A record of expressions.
///
/// Represents a fixed-size, heterogeneous collection of expressions accessed by
/// position. Like Rust tuples, each field can have a different type.
///
/// # Examples
///
/// ```text
/// record(a, b, c)  // a record with three fields
/// record[0]        // access the first field
/// ```
#[derive(Clone, Default, PartialEq)]
pub struct ExprRecord {
    /// The field expressions in positional order.
    pub fields: Vec<Expr>,
}

impl Expr {
    pub fn record<T>(items: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<Self>,
    {
        Self::Record(ExprRecord::from_iter(items))
    }

    pub fn record_from_vec(fields: Vec<Self>) -> Self {
        Self::Record(ExprRecord::from_vec(fields))
    }

    pub fn is_record(&self) -> bool {
        matches!(self, Self::Record(_))
    }

    pub fn as_record(&self) -> Option<&ExprRecord> {
        match self {
            Self::Record(expr_record) => Some(expr_record),
            _ => None,
        }
    }

    pub fn as_record_unwrap(&self) -> &ExprRecord {
        match self {
            Self::Record(expr_record) => expr_record,
            _ => panic!("self={self:#?}"),
        }
    }

    pub fn as_record_mut(&mut self) -> &mut ExprRecord {
        match self {
            Self::Record(expr_record) => expr_record,
            _ => panic!(),
        }
    }

    pub fn into_record(self) -> ExprRecord {
        match self {
            Self::Record(expr_record) => expr_record,
            _ => panic!(),
        }
    }

    pub fn record_len(&self) -> Option<usize> {
        match self {
            Expr::Record(expr_record) => Some(expr_record.len()),
            Expr::Value(Value::Record(value_record)) => Some(value_record.len()),
            _ => None,
        }
    }

    pub fn into_record_items(self) -> Option<impl Iterator<Item = Expr>> {
        let ret: Option<Box<dyn Iterator<Item = Expr>>> = match self {
            Expr::Record(expr_record) => Some(Box::new(expr_record.into_iter())),
            Expr::Value(Value::Record(value_record)) => {
                Some(Box::new(value_record.into_iter().map(Expr::Value)))
            }
            _ => None,
        };

        ret
    }
}

impl ExprRecord {
    pub fn from_vec(fields: Vec<Expr>) -> Self {
        Self { fields }
    }

    pub fn push(&mut self, expr: Expr) {
        self.fields.push(expr)
    }

    pub fn resize(&mut self, new_len: usize, value: impl Into<stmt::Expr>) {
        self.fields.resize(new_len, value.into());
    }
}

impl<A> FromIterator<A> for ExprRecord
where
    A: Into<Expr>,
{
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        Self::from_vec(iter.into_iter().map(Into::into).collect())
    }
}

impl ops::Deref for ExprRecord {
    type Target = [Expr];

    fn deref(&self) -> &Self::Target {
        &self.fields[..]
    }
}

impl ops::DerefMut for ExprRecord {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields[..]
    }
}

impl ops::Index<usize> for ExprRecord {
    type Output = Expr;

    fn index(&self, index: usize) -> &Self::Output {
        &self.fields[index]
    }
}

impl ops::IndexMut<usize> for ExprRecord {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.fields[index]
    }
}

impl IntoIterator for ExprRecord {
    type Item = Expr;
    type IntoIter = std::vec::IntoIter<Expr>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<'a> IntoIterator for &'a ExprRecord {
    type Item = &'a Expr;
    type IntoIter = std::slice::Iter<'a, Expr>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.iter()
    }
}

impl<'a> IntoIterator for &'a mut ExprRecord {
    type Item = &'a mut Expr;
    type IntoIter = std::slice::IterMut<'a, Expr>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.iter_mut()
    }
}

impl AsRef<[Expr]> for ExprRecord {
    fn as_ref(&self) -> &[Expr] {
        self.fields.as_ref()
    }
}

impl From<ExprRecord> for Expr {
    fn from(value: ExprRecord) -> Self {
        Self::Record(value)
    }
}

macro_rules! impl_for_tuple {
    ( $len:literal; $(($T:ident, $idx:tt)),+ ) => {
        impl<$($T),+> From<($($T,)+)> for ExprRecord
        where
            $($T: Into<Expr>,)+
        {
            fn from(src: ($($T,)+)) -> Self {
                Self {
                    fields: vec![$(src.$idx.into()),+],
                }
            }
        }

        impl<$($T),+> PartialEq<($($T,)+)> for Expr
        where
            $(Expr: PartialEq<$T>,)+
            $(Value: PartialEq<$T>,)+
        {
            fn eq(&self, other: &($($T,)+)) -> bool {
                match self {
                    Expr::Record(r) => {
                        r.fields.len() == $len
                            $(&& r.fields[$idx].eq(&other.$idx))+
                    }
                    Expr::Value(Value::Record(r)) => {
                        r.fields.len() == $len
                            $(&& r.fields[$idx].eq(&other.$idx))+
                    }
                    _ => false,
                }
            }
        }

        impl<$($T),+> PartialEq<Expr> for ($($T,)+)
        where
            Expr: PartialEq<($($T,)+)>,
        {
            fn eq(&self, other: &Expr) -> bool {
                other.eq(self)
            }
        }
    };
}

impl_for_tuple!(1; (T0, 0));
impl_for_tuple!(2; (T0, 0), (T1, 1));
impl_for_tuple!(3; (T0, 0), (T1, 1), (T2, 2));
impl_for_tuple!(4; (T0, 0), (T1, 1), (T2, 2), (T3, 3));
impl_for_tuple!(5; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4));
impl_for_tuple!(6; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5));
impl_for_tuple!(7; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6));
impl_for_tuple!(8; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7));
impl_for_tuple!(9; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7), (T8, 8));
impl_for_tuple!(10; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7), (T8, 8), (T9, 9));
impl_for_tuple!(11; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7), (T8, 8), (T9, 9), (T10, 10));
impl_for_tuple!(12; (T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5), (T6, 6), (T7, 7), (T8, 8), (T9, 9), (T10, 10), (T11, 11));

impl Node for ExprRecord {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_expr_record(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_expr_record_mut(self);
    }
}

impl fmt::Debug for ExprRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fields.as_slice().fmt(f)
    }
}
