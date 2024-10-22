use super::*;

#[derive(Debug, Clone)]
pub struct Select<'stmt> {
    /// What columns to include
    pub project: Vec<Expr<'stmt>>,

    /// `FROM` part, includes joints
    pub from: Vec<TableWithJoins>,

    /// WHERE
    pub selection: Option<Expr<'stmt>>,
}
