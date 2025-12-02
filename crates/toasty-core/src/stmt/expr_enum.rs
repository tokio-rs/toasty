use super::{Expr, ExprRecord};

/// An enum variant expression with associated fields.
///
/// Represents a specific variant of an enum type along with its field values.
///
/// # Examples
///
/// ```text
/// enum(0, {name, age})  // variant 0 with fields `name` and `age`
/// enum(1, {})           // variant 1 with no fields
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprEnum {
    /// The index of the enum variant.
    pub variant: usize,

    /// The field values for this variant.
    pub fields: ExprRecord,
}

impl From<ExprEnum> for Expr {
    fn from(value: ExprEnum) -> Self {
        Self::Enum(value)
    }
}
