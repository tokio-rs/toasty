use super::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum Limit {
    /// Traditional LIMIT/OFFSET - no pagination metadata needed
    Offset { limit: Expr, offset: Option<Expr> },
    /// Forward cursor-based pagination - engine should return next_cursor
    PaginateForward { limit: Expr, after: Option<Expr> },
}
