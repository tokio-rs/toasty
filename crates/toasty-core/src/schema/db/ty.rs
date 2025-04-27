#[derive(Debug, Clone)]
pub enum Type {
    Boolean,
    Integer,
    Text,
    VarChar(usize),
}
