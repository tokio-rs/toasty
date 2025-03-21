use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprField {
    pub field: FieldId,
}

impl Expr {
    pub fn field(field: impl Into<FieldId>) -> Expr {
        ExprField {
            field: field.into(),
        }
        .into()
    }

    pub fn is_field(&self) -> bool {
        matches!(self, Expr::Field(_))
    }

    pub fn as_field(&self) -> &ExprField {
        match self {
            Expr::Field(field) => field,
            _ => todo!(),
        }
    }
}

impl From<ExprField> for Expr {
    fn from(value: ExprField) -> Self {
        Expr::Field(value)
    }
}

impl From<&Field> for ExprField {
    fn from(value: &Field) -> Self {
        ExprField { field: value.id }
    }
}

impl From<&Field> for Expr {
    fn from(value: &Field) -> Self {
        Expr::field(value.id())
    }
}

impl From<FieldId> for ExprField {
    fn from(value: FieldId) -> Self {
        ExprField { field: value }
    }
}

impl From<FieldId> for Expr {
    fn from(value: FieldId) -> Self {
        Expr::field(value)
    }
}
