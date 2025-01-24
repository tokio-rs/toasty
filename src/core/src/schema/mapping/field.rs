use super::*;

#[derive(Debug, Clone)]
pub struct Field {
    pub column: ColumnId,

    /// Which entry in the lowering map lowers this field.
    pub lowering: usize,
}
