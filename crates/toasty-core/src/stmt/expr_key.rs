use super::Expr;
use crate::schema::app::ModelId;

/// References the primary key of a model.
///
/// Used in queries to refer to a model's primary key field(s) without
/// explicitly naming them.
///
/// # Examples
///
/// ```text
/// key(User)  // refers to the primary key of the `User` model
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprKey {
    /// The model whose primary key is referenced.
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
