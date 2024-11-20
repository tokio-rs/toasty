use super::*;

use std::ops;

#[derive(Clone, PartialEq)]
pub struct Assignments {
    pub fields: PathFieldSet,

    pub exprs: Vec<Option<Expr>>,
}

impl Assignments {
    pub fn with_capacity(capacity: usize) -> Assignments {
        Assignments {
            fields: PathFieldSet::new(),
            exprs: Vec::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn contains(&self, field: impl Into<PathStep>) -> bool {
        self.fields.contains(field)
    }

    pub fn get(&self, field: impl Into<PathStep>) -> Option<&Expr> {
        let index = field.into().into_usize();

        if index >= self.exprs.len() {
            None
        } else {
            self.exprs[index].as_ref()
        }
    }

    pub fn set(&mut self, field: impl Into<PathStep>, expr: impl Into<Expr>) {
        *self.slot(field.into().into_usize()) = expr.into();
    }

    pub fn unset(&mut self, field: impl Into<PathStep>) {
        let field = field.into();
        self.fields.unset(field);

        self.exprs[field.into_usize()] = None;
    }

    pub fn push(&mut self, field: impl Into<PathStep>, expr: impl Into<Expr>) {
        self.slot(field.into().into_usize()).push(expr.into());
    }

    pub fn take(&mut self, field: impl Into<PathStep>) -> stmt::Expr {
        let field = field.into();
        self.fields.unset(field);

        self.exprs[field.into_usize()].take().unwrap()
    }

    // TODO: probably should create an `assignment::Entry` type
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a Expr)> + '_ {
        self.fields.iter().map(|path_step| {
            let index = path_step.into_usize();
            (index, self.exprs[index].as_ref().unwrap())
        })
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = (usize, &'a mut Expr)> + '_ {
        self.exprs
            .iter_mut()
            .enumerate()
            .filter_map(|(i, entry)| entry.as_mut().map(|e| (i, e)))
    }

    fn slot(&mut self, index: usize) -> &mut Expr {
        self.fields.insert(index);

        if self.exprs.len() <= index {
            self.exprs.resize(index + 1, None);
        }

        if self.exprs[index].is_none() {
            self.exprs[index] = Some(Expr::default());
        }

        self.exprs[index].as_mut().unwrap()
    }
}

impl Default for Assignments {
    fn default() -> Self {
        Assignments {
            fields: PathFieldSet::new(),
            exprs: vec![],
        }
    }
}

impl<I: Into<PathStep>> ops::Index<I> for Assignments {
    type Output = Expr;

    fn index(&self, index: I) -> &Self::Output {
        let index = index.into().into_usize();
        self.exprs[index].as_ref().unwrap()
    }
}

impl<I: Into<PathStep>> ops::IndexMut<I> for Assignments {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        let index = index.into().into_usize();
        self.exprs[index].as_mut().unwrap()
    }
}

impl fmt::Debug for Assignments {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut fmt = f.debug_struct("Assignments");

        for (i, expr) in self.exprs.iter().enumerate() {
            if let Some(expr) = expr {
                fmt.field(&format!("{i}"), expr);
            }
        }

        fmt.finish()
    }
}
