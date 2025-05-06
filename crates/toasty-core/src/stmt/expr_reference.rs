#[derive(Debug, Clone)]
pub enum ExprReference {
    /// Reference a column from a CTE table
    Cte {
        /// What level of nesting the reference is compared to the CTE being
        /// referenced.
        nesting: usize,

        /// Column index in the CTEs
        index: usize,
    },
}
