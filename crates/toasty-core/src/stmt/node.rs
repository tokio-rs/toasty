use super::*;

use std::fmt;

pub trait Node: fmt::Debug {
    fn visit<V: Visit>(&self, visit: V)
    where
        Self: Sized;

    fn visit_mut<V: VisitMut>(&mut self, visit: V);
}
