use super::*;

use std::fmt;

pub trait Node<'stmt>: fmt::Debug {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self
    where
        Self: Sized;

    fn visit<V: Visit<'stmt>>(&self, visit: V)
    where
        Self: Sized;

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, visit: V)
    where
        Self: Sized;
}
