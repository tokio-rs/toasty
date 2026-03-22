use super::{Visit, VisitMut};
use std::fmt;

/// A node in the statement AST that can be traversed by a visitor.
///
/// Every AST type (statements, expressions, values, etc.) implements `Node`
/// to participate in the visitor pattern defined by [`Visit`] and
/// [`VisitMut`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Node, Expr, Value, visit};
///
/// let expr = Expr::from(Value::from(42_i64));
/// visit::for_each_expr(&expr, |e| {
///     println!("{:?}", e);
/// });
/// ```
pub trait Node: fmt::Debug {
    /// Traverses this node with an immutable visitor.
    fn visit<V: Visit>(&self, visit: V)
    where
        Self: Sized;

    /// Traverses this node with a mutable visitor.
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
