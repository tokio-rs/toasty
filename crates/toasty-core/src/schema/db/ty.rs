use crate::stmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Boolean,
    Integer,
    Text,
    VarChar(usize),
}

impl Type {
    pub fn from_app(ty: &stmt::Type) -> Type {
        match ty {
            stmt::Type::Bool => Type::Boolean,
            stmt::Type::I64 => Type::Integer,
            stmt::Type::String => Type::Text,
            _ => todo!("ty={:#?}", ty),
        }
    }
}
