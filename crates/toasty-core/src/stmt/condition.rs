use crate::stmt::{Expr, Node, Statement, Visit, VisitMut};

/// A guard condition on an [`Update`](super::Update) statement.
///
/// Unlike a [`Filter`](super::Filter), a condition does not select which rows
/// to operate on. Instead, it is evaluated after the filter and determines
/// whether the update should actually be applied. If the condition is not met,
/// the update is silently skipped.
///
/// When `expr` is `None`, no condition is applied (the update always proceeds).
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::Condition;
///
/// let cond = Condition::default();
/// assert!(cond.is_none());
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Condition {
    /// The condition expression, or `None` for unconditional updates.
    pub expr: Option<Expr>,
}

impl Condition {
    /// Creates a condition from an expression.
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
