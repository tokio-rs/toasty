use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Assignments<'stmt> {
    pub fields: PathFieldSet,

    pub exprs: Vec<Option<Expr<'stmt>>>,
}

impl<'stmt> Assignments<'stmt> {
    pub fn with_capacity(capacity: usize) -> Assignments<'stmt> {
        Assignments {
            fields: PathFieldSet::new(),
            exprs: Vec::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn set(&mut self, field: impl Into<PathStep>, expr: impl Into<Expr<'stmt>>) {
        let index = field.into().into_usize();
        self.fields.insert(index);

        if self.exprs.len() <= index {
            self.exprs.resize(index + 1, None);
        }

        self.exprs[index] = Some(expr.into());
    }
}

impl<'stmt> Default for Assignments<'stmt> {
    fn default() -> Self {
        Assignments {
            fields: PathFieldSet::new(),
            exprs: vec![],
        }
    }
}
