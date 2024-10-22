use super::*;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Values<'stmt> {
    pub rows: Vec<Vec<Expr<'stmt>>>,
}

impl<'stmt> Values<'stmt> {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input<'stmt>) {
        for row in &mut self.rows {
            for item in row {
                item.substitute_ref(input);
            }
        }
    }
}
