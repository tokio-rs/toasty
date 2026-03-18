use super::Expr;

/// A conditional expression: if cond₁ then expr₁ else if cond₂ then expr₂ … else default.
#[derive(Clone, PartialEq)]
pub struct ExprIf {
    pub branches: Vec<IfBranch>,
    pub r#else: Box<Expr>,
}

/// A single branch of a conditional expression.
#[derive(Clone, PartialEq)]
pub struct IfBranch {
    pub cond: Box<Expr>,
    pub then: Box<Expr>,
}

impl Expr {
    pub fn r#if(cond: impl Into<Expr>, then: impl Into<Expr>, r#else: impl Into<Expr>) -> Expr {
        Expr::If(ExprIf {
            branches: vec![IfBranch {
                cond: Box::new(cond.into()),
                then: Box::new(then.into()),
            }],
            r#else: Box::new(r#else.into()),
        })
    }
}

impl From<ExprIf> for Expr {
    fn from(value: ExprIf) -> Self {
        Self::If(value)
    }
}

impl std::fmt::Debug for ExprIf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExprIf")
            .field("branches", &self.branches)
            .field("else", &self.r#else)
            .finish()
    }
}

impl std::fmt::Debug for IfBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IfBranch")
            .field("cond", &self.cond)
            .field("then", &self.then)
            .finish()
    }
}
