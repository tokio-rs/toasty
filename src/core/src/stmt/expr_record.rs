use super::*;

use std::{fmt, ops};

#[derive(Clone, Default, PartialEq)]
pub struct ExprRecord<'stmt> {
    pub fields: Vec<Expr<'stmt>>,
}

impl<'stmt> Expr<'stmt> {
    pub fn record<T>(items: impl IntoIterator<Item = T>) -> Expr<'stmt>
    where
        T: Into<Expr<'stmt>>,
    {
        Expr::Record(ExprRecord::from_iter(items.into_iter()))
    }

    pub fn is_record(&self) -> bool {
        matches!(self, Expr::Record(_))
    }

    pub fn as_record(&self) -> &ExprRecord<'stmt> {
        match self {
            Expr::Record(expr_record) => expr_record,
            _ => panic!(),
        }
    }

    pub fn as_record_mut(&mut self) -> &mut ExprRecord<'stmt> {
        match self {
            Expr::Record(expr_record) => expr_record,
            _ => panic!(),
        }
    }
}

impl<'stmt> ExprRecord<'stmt> {
    pub fn from_iter<T>(iter: impl Iterator<Item = T>) -> ExprRecord<'stmt>
    where
        T: Into<Expr<'stmt>>,
    {
        ExprRecord::from_vec(iter.map(Into::into).collect())
    }

    pub fn from_vec(fields: Vec<Expr<'stmt>>) -> ExprRecord<'stmt> {
        ExprRecord { fields }
    }

    // TODO: delete this
    pub fn is_identity(&self) -> bool {
        (0..self.fields.len()).all(|i| {
            let Expr::Project(expr_project) = &self.fields[i] else {
                return false;
            };

            let [step] = &expr_project.projection[..] else {
                return false;
            };

            step.into_usize() == i
        })
    }

    pub fn push(&mut self, expr: Expr<'stmt>) {
        self.fields.push(expr)
    }

    pub fn resize(&mut self, new_len: usize, value: impl Into<stmt::Expr<'stmt>>) {
        self.fields.resize(new_len, value.into());
    }

    pub(crate) fn simplify(&mut self) -> Option<Expr<'stmt>> {
        let mut all_values = true;

        for expr in &mut self.fields {
            expr.simplify();

            all_values &= expr.is_value();
        }

        if all_values {
            let mut values = vec![];

            for expr in self.fields.drain(..) {
                let Expr::Value(value) = expr else { panic!() };
                values.push(value);
            }

            Some(Expr::Value(Value::Record(Record::from_vec(values).into())))
        } else {
            None
        }
    }
}

impl<'stmt> ops::Deref for ExprRecord<'stmt> {
    type Target = [Expr<'stmt>];

    fn deref(&self) -> &Self::Target {
        &self.fields[..]
    }
}

impl<'stmt> ops::DerefMut for ExprRecord<'stmt> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields[..]
    }
}

impl<'stmt> ops::Index<usize> for ExprRecord<'stmt> {
    type Output = Expr<'stmt>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.fields[index]
    }
}

impl<'stmt> ops::IndexMut<usize> for ExprRecord<'stmt> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.fields[index]
    }
}

impl<'stmt> ops::Index<PathStep> for ExprRecord<'stmt> {
    type Output = Expr<'stmt>;

    fn index(&self, index: PathStep) -> &Self::Output {
        &self.fields[index.into_usize()]
    }
}

impl<'stmt> ops::IndexMut<PathStep> for ExprRecord<'stmt> {
    fn index_mut(&mut self, index: PathStep) -> &mut Self::Output {
        &mut self.fields[index.into_usize()]
    }
}

impl<'stmt> IntoIterator for ExprRecord<'stmt> {
    type Item = Expr<'stmt>;
    type IntoIter = std::vec::IntoIter<Expr<'stmt>>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a ExprRecord<'stmt> {
    type Item = &'a Expr<'stmt>;
    type IntoIter = std::slice::Iter<'a, Expr<'stmt>>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a mut ExprRecord<'stmt> {
    type Item = &'a mut Expr<'stmt>;
    type IntoIter = std::slice::IterMut<'a, Expr<'stmt>>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.iter_mut()
    }
}

impl<'stmt> AsRef<[Expr<'stmt>]> for ExprRecord<'stmt> {
    fn as_ref(&self) -> &[Expr<'stmt>] {
        self.fields.as_ref()
    }
}

impl<'stmt> From<Record<'stmt>> for ExprRecord<'stmt> {
    fn from(src: Record<'stmt>) -> ExprRecord<'stmt> {
        ExprRecord::from_vec(src.into_iter().map(Into::into).collect())
    }
}

impl<'stmt> From<ExprRecord<'stmt>> for Expr<'stmt> {
    fn from(value: ExprRecord<'stmt>) -> Expr<'stmt> {
        Expr::Record(value)
    }
}

impl<'stmt, E1, E2> From<(E1, E2)> for ExprRecord<'stmt>
where
    E1: Into<Expr<'stmt>>,
    E2: Into<Expr<'stmt>>,
{
    fn from(src: (E1, E2)) -> ExprRecord<'stmt> {
        ExprRecord {
            fields: vec![src.0.into(), src.1.into()],
        }
    }
}

impl<'stmt> Node<'stmt> for ExprRecord<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_expr_record(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_expr_record(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_expr_record_mut(self);
    }
}

impl<'stmt> fmt::Debug for ExprRecord<'stmt> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fields.as_slice().fmt(f)
    }
}
