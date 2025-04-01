#[derive(Debug, Clone, PartialEq)]
pub enum ExprReference {
    /// Reference a column from a CTE table
    Cte(usize),
}
