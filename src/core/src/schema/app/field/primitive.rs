use super::*;

#[derive(Debug, PartialEq)]
pub struct FieldPrimitive {
    /// The field's primitive type
    pub ty: stmt::Type,
}
