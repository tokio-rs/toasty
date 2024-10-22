use super::*;

#[derive(Debug, PartialEq)]
pub struct FieldPrimitive {
    /// Which table column the field is mapped to
    pub column: ColumnId,

    /// Which index in the lowering map lowers this field.
    pub lowering: usize,

    /// The field's primitive type
    pub ty: stmt::Type,
}
