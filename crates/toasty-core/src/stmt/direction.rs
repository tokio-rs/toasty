/// Sort direction for an [`OrderByExpr`](super::OrderByExpr).
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::Direction;
///
/// let dir = Direction::Asc;
/// assert_eq!(dir, Direction::Asc);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
    /// Ascending order (smallest first).
    Asc,
    /// Descending order (largest first).
    Desc,
}
