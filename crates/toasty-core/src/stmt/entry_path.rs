use super::{projection, Projection};

/// A path that can be used to navigate into a composite [`Value`](super::Value)
/// or [`Expr`](super::Expr).
///
/// Implemented for `usize` (single-step navigation) and `&Projection`
/// (multi-step navigation).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Value, ValueRecord};
///
/// let record = Value::record_from_vec(vec![Value::from(1_i64), Value::from(2_i64)]);
/// // Navigate with a single usize step
/// let entry = record.entry(0_usize);
/// ```
pub trait EntryPath {
    /// The iterator type yielding each step index.
    type Iter: Iterator<Item = usize>;

    /// Returns an iterator over the step indices.
    fn step_iter(self) -> Self::Iter;
}

impl EntryPath for usize {
    type Iter = std::option::IntoIter<Self>;

    fn step_iter(self) -> Self::Iter {
        Some(self).into_iter()
    }
}

impl<'a> EntryPath for &'a Projection {
    type Iter = projection::Iter<'a>;

    fn step_iter(self) -> Self::Iter {
        self.into_iter()
    }
}
