use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprModel {
    pub id: ModelId,
}

impl Expr {
    pub fn model(id: impl Into<ModelId>) -> Expr {
        ExprModel { id: id.into() }.into()
    }

    pub fn is_model(&self) -> bool {
        matches!(self, Expr::Model(_))
    }
}

impl From<ExprModel> for Expr {
    fn from(value: ExprModel) -> Self {
        Expr::Model(value)
    }
}

impl From<ModelId> for Expr {
    fn from(value: ModelId) -> Self {
        Expr::model(value)
    }
}
