use super::{Visit, VisitMut};
use std::fmt;

pub trait Node: fmt::Debug {
    fn visit<V: Visit>(&self, visit: V)
    where
        Self: Sized;

    fn visit_mut<V: VisitMut>(&mut self, visit: V);
}

impl<T: Node> Node for Option<T> {
    fn visit<V: Visit>(&self, visit: V)
    where
        Self: Sized,
    {
        if let Some(node) = self {
            node.visit(visit);
        }
    }

    fn visit_mut<V: VisitMut>(&mut self, visit: V) {
        if let Some(node) = self {
            node.visit_mut(visit);
        }
    }
}

impl<T: Node> Node for &mut T {
    fn visit<V: Visit>(&self, visit: V)
    where
        Self: Sized,
    {
        (**self).visit(visit)
    }

    fn visit_mut<V: VisitMut>(&mut self, visit: V) {
        (**self).visit_mut(visit)
    }
}
