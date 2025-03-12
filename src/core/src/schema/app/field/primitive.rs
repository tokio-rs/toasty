use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct FieldPrimitive {
    /// The field's primitive type
    pub ty: stmt::Type,
}
