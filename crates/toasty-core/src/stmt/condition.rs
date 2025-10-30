use crate::stmt::{Expr, Node, Statement, Visit, VisitMut};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Condition {
    pub expr: Option<Expr>,
}

impl Condition {
    pub fn new(expr: impl Into<Expr>) -> Condition {
        Condition {
            expr: Some(expr.into()),
        }
    }

    pub fn is_some(&self) -> bool {
        self.expr.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.expr.is_none()
    }
}

impl Statement {
    pub fn condition(&self) -> Option<&Condition> {
        match self {
            Statement::Update(update) if update.condition.is_some() => Some(&update.condition),
            _ => None,
        }
    }

    /// Returns a mutable reference to the statement's condition.
    ///
    /// Returns `None` for statements that do not support conditions.
    pub fn condition_mut(&mut self) -> Option<&mut Condition> {
        match self {
            Statement::Update(update) => Some(&mut update.condition),
            _ => None,
        }
    }

    /// Returns a mutable reference to the statement's condition.
    ///
    /// # Panics
    ///
    /// Panics if the statement does not support conditions.
    #[track_caller]
    pub fn condition_mut_unwrap(&mut self) -> &mut Condition {
        match self {
            Statement::Update(update) => &mut update.condition,
            _ => panic!("expected Statement with condition"),
        }
    }
}

impl Node for Condition {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_condition(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_condition_mut(self);
    }
}

impl<T> From<T> for Condition
where
    Expr: From<T>,
{
    fn from(value: T) -> Self {
        Condition {
            expr: Some(value.into()),
        }
    }
}
