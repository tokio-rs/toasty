use super::*;

#[derive(Debug, Default, Clone)]
pub struct Values<'stmt> {
    pub rows: Vec<Vec<Expr<'stmt>>>,
}

impl<'stmt> Values<'stmt> {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
