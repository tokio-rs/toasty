use super::Type;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeEnum {
    // Will be populated in Phase 2 of embedded enum support
}

impl From<TypeEnum> for Type {
    fn from(value: TypeEnum) -> Self {
        Self::Enum(value)
    }
}
