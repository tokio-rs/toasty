use super::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum Offset {
    /// Keyset-based offset for forwards pagination
    After(Expr),

    /// Keyset-based offset for backwards pagination
    Before(Expr),

    /// Count-based offset
    Count(Expr),
}
