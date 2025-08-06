use crate::stmt;

#[derive(Debug, Clone)]
pub struct Arg {
    /// Argument name
    pub name: String,

    /// Argument type
    pub ty: stmt::Type,
}
