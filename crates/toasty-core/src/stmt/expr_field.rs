use super::*;

#[derive(Debug, Clone)]
pub struct ExprField {
    pub field: FieldId,
}

impl Expr {
    pub fn field(field: impl Into<FieldId>) -> Self {
        ExprField {
            field: field.into(),
        }
        .into()
    }

    pub fn is_field(&self) -> bool {
        matches!(self, Self::Field(_))
    }

    pub fn as_field(&self) -> &ExprField {
        match self {
            Self::Field(field) => field,
            _ => todo!(),
        }
    }
}

impl From<ExprField> for Expr {
    fn from(value: ExprField) -> Self {
        Self::Field(value)
    }
}

impl From<&Field> for ExprField {
    fn from(value: &Field) -> Self {
        Self { field: value.id }
    }
}

impl From<&Field> for Expr {
    fn from(value: &Field) -> Self {
        Self::field(value.id())
    }
}

impl From<FieldId> for ExprField {
    fn from(value: FieldId) -> Self {
        Self { field: value }
    }
}

impl From<FieldId> for Expr {
    fn from(value: FieldId) -> Self {
        Self::field(value)
    }
}
