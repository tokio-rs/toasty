use super::*;

#[derive(Debug, Clone)]
pub struct ExprKey {
    pub model: ModelId,
}

impl Expr {
    pub fn key(model: impl Into<ModelId>) -> Self {
        ExprKey {
            model: model.into(),
        }
        .into()
    }
}

impl From<ExprKey> for Expr {
    fn from(value: ExprKey) -> Self {
        Self::Key(value)
    }
}

impl From<ModelId> for ExprKey {
    fn from(value: ModelId) -> Self {
        Self { model: value }
    }
}
