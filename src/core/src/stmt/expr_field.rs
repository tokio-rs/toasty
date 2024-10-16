use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprField {
    pub field: FieldId,
}

impl<'stmt> Expr<'stmt> {
    pub fn field(field: impl Into<FieldId>) -> Expr<'stmt> {
        ExprField {
            field: field.into(),
        }
        .into()
    }

    pub fn is_field(&self) -> bool {
        matches!(self, Expr::Field(_))
    }
}

impl<'stmt> From<ExprField> for Expr<'stmt> {
    fn from(value: ExprField) -> Self {
        Expr::Field(value)
    }
}

impl<'stmt> From<&Field> for ExprField {
    fn from(value: &Field) -> Self {
        ExprField { field: value.id }
    }
}

impl<'stmt> From<&Field> for Expr<'stmt> {
    fn from(value: &Field) -> Self {
        Expr::field(value)
    }
}

impl<'stmt> From<FieldId> for ExprField {
    fn from(value: FieldId) -> Self {
        ExprField { field: value }
    }
}

impl<'stmt> From<FieldId> for Expr<'stmt> {
    fn from(value: FieldId) -> Self {
        Expr::field(value)
    }
}
