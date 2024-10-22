use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct Arg {
    /// Argument name
    pub name: String,

    /// Argument type
    pub ty: stmt::Type,
}
