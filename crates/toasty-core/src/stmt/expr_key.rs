use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprKey {
    pub model: ModelId,
}

impl Expr {
    pub fn key(model: impl Into<ModelId>) -> Expr {
        ExprKey {
            model: model.into(),
        }
        .into()
    }
}

impl From<ExprKey> for Expr {
    fn from(value: ExprKey) -> Self {
        Expr::Key(value)
    }
}

impl From<ModelId> for ExprKey {
    fn from(value: ModelId) -> Self {
        ExprKey { model: value }
    }
}
