use super::*;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypeEnum {
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    /// Enum discriminant
    pub discriminant: usize,

    /// Enum fields
    pub fields: Vec<Type>,
}

impl TypeEnum {
    pub fn insert_variant(&mut self) -> &mut EnumVariant {
        let discriminant = self.variants.len();
        self.variants.push(EnumVariant {
            discriminant,
            fields: vec![],
        });

        &mut self.variants[discriminant]
    }
}

impl From<TypeEnum> for Type {
    fn from(value: TypeEnum) -> Self {
        Self::Enum(value)
    }
}
