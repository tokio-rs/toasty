use crate::schema::app::ModelId;
use crate::stmt::Expr;

/// Tests whether the row in the current scope is an instance of `model`.
///
/// Emitted during lowering for reads against item-collection child models;
/// the storage driver enforces it natively (column equality for SQL,
/// sort-key prefix for DynamoDB).
#[derive(Debug, Clone, PartialEq)]
pub struct ExprIsModel {
    /// The model id to check
    pub model: ModelId,
}

impl Expr {
    /// Creates a variant check expression to test the model id
    pub fn is_model(model: ModelId) -> Self {
        ExprIsModel { model }.into()
    }
}

impl From<ExprIsModel> for Expr {
    fn from(value: ExprIsModel) -> Self {
        Self::IsModel(value)
    }
}
