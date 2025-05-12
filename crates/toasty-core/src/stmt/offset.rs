use super::Expr;

#[derive(Debug, Clone)]
pub enum Offset {
    /// Keyset-based offset
    After(Expr),

    /// Count-based offset
    Count(Expr),
}
